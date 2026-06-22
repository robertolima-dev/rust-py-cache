//! `RustCache`: o núcleo de armazenamento, independente do PyO3.
//!
//! Mantemos a lógica de cache aqui, *sem* tipos do Python, para que seja fácil
//! de testar em Rust puro e para separar responsabilidades: `lib.rs` cuida da
//! ponte com o Python; `cache.rs` cuida do armazenamento.

use dashmap::DashMap;

use crate::entry::CacheEntry;
use crate::stats::{CacheStats, StatsSnapshot};

/// Núcleo concorrente do cache.
///
/// `DashMap` é um `HashMap` thread-safe com *sharding*: ele divide o mapa em
/// vários segmentos, cada um com seu próprio lock. Threads que mexem em chaves
/// de shards diferentes não competem pelo mesmo lock — ao contrário de um
/// `Mutex<HashMap>` global, que serializaria todo acesso. Repare que os métodos
/// abaixo recebem `&self` (referência compartilhada, *imutável*): o `DashMap`
/// faz *interior mutability* — ele cuida da sincronização internamente, então
/// não precisamos de `&mut self` nem de um `Mutex` por fora. As `stats` seguem
/// a mesma ideia com `AtomicU64`.
#[derive(Debug, Default)]
pub struct RustCache {
    store: DashMap<String, CacheEntry>,
    stats: CacheStats,
}

impl RustCache {
    /// Cria um cache vazio.
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
            stats: CacheStats::new(),
        }
    }

    /// Grava `value` (bytes já serializados) sob `key`, sobrescrevendo se existir.
    ///
    /// O core não sabe o que há nos bytes — quem serializa (pickle) é a camada
    /// `lib.rs`, do lado do Python. Aqui só montamos a `CacheEntry` e inserimos.
    /// `insert` no `DashMap` pega o lock só do shard daquela chave.
    pub fn set(&self, key: String, value: Vec<u8>, expires_at: Option<u64>, now: u64) {
        self.store
            .insert(key, CacheEntry::new(value, expires_at, now));
        self.stats.record_set();
    }

    /// Lê os bytes de `key`, aplicando **expiração lazy**.
    ///
    /// - Chave ausente → `None` (conta como *miss*).
    /// - Chave expirada em relação a `now` → remove a entrada, conta *miss* +
    ///   *expired*, devolve `None`.
    /// - Caso contrário → `Some(bytes)` (cópia, p/ soltar o lock logo) + *hit*.
    ///
    /// Detalhe de concorrência: `store.get` devolve um *guard* que segura o lock
    /// de leitura do shard. Se chamássemos `store.remove` com esse guard ainda
    /// vivo, travaríamos o mesmo shard contra nós mesmos (deadlock). Por isso
    /// `drop(entry)` **antes** do `remove`.
    pub fn get(&self, key: &str, now: u64) -> Option<Vec<u8>> {
        let Some(entry) = self.store.get(key) else {
            self.stats.record_miss();
            return None;
        };
        if entry.is_expired(now) {
            drop(entry);
            self.store.remove(key);
            self.stats.record_expired(1);
            self.stats.record_miss();
            return None;
        }
        self.stats.record_hit();
        Some(entry.value.clone())
    }

    /// `true` se `key` existe e **não** está expirada. Coleta a chave se expirada.
    ///
    /// Diferente de `get`, não mexe nos contadores de hit/miss (não é uma leitura
    /// de valor), mas conta a coleta de uma entrada expirada como *expired*.
    pub fn exists(&self, key: &str, now: u64) -> bool {
        let Some(entry) = self.store.get(key) else {
            return false;
        };
        if entry.is_expired(now) {
            drop(entry);
            self.store.remove(key);
            self.stats.record_expired(1);
            return false;
        }
        true
    }

    /// Remove `key` do mapa. Devolve `true` se havia uma entrada, `false` se não.
    ///
    /// É uma remoção *física*: `DashMap::remove` devolve `Some((k, v))` se a chave
    /// existia. No MVP não distinguimos "expirada mas ainda não coletada" de
    /// "viva" — se o registro estava no mapa, contamos como removido. (Consistente
    /// com `len`, que também conta expirados não coletados.)
    pub fn delete(&self, key: &str) -> bool {
        if self.store.remove(key).is_some() {
            self.stats.record_delete();
            true
        } else {
            false
        }
    }

    /// Lista todas as chaves atualmente no mapa.
    ///
    /// Pode incluir chaves expiradas ainda não coletadas (mesmo critério de
    /// `len`). Coletamos as chaves num `Vec` para soltar os locks dos shards
    /// antes de devolver — não seguramos guards do `DashMap` fora daqui.
    pub fn keys(&self) -> Vec<String> {
        self.store.iter().map(|e| e.key().clone()).collect()
    }

    /// Remove todas as entradas expiradas em relação a `now`. Devolve quantas
    /// foram removidas e contabiliza-as em *expired*.
    ///
    /// Fazemos em duas fases para não remover durante a iteração (que segura
    /// locks de shard): (1) coletamos as chaves expiradas; (2) removemos com
    /// `remove_if`, que **recheca** a expiração — assim não apagamos uma chave
    /// que outra thread acabou de regravar com novo TTL entre as duas fases.
    pub fn cleanup_expired(&self, now: u64) -> usize {
        let expired: Vec<String> = self
            .store
            .iter()
            .filter(|e| e.value().is_expired(now))
            .map(|e| e.key().clone())
            .collect();

        let mut removed = 0u64;
        for key in expired {
            if self.store.remove_if(&key, |_, v| v.is_expired(now)).is_some() {
                removed += 1;
            }
        }
        self.stats.record_expired(removed);
        removed as usize
    }

    /// Snapshot dos contadores (para `stats()` do Python).
    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// Quantidade de entradas no mapa.
    ///
    /// É um valor *aproximado*: pode incluir entradas já expiradas que ainda não
    /// foram coletadas (a expiração no MVP é lazy). Ver `cleanup_expired()`.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Remove todas as entradas. Não zera os contadores (são histórico de uso).
    pub fn clear(&self) {
        self.store.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `now` fixo facilita testar expiração sem depender do relógio real.
    const NOW: u64 = 1_000_000;

    #[test]
    fn set_then_get_returns_value() {
        let c = RustCache::new();
        c.set("a".into(), b"v".to_vec(), None, NOW);
        assert_eq!(c.get("a", NOW), Some(b"v".to_vec()));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn get_missing_is_none_and_counts_miss() {
        let c = RustCache::new();
        assert_eq!(c.get("x", NOW), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn expired_get_collects_and_counts() {
        let c = RustCache::new();
        c.set("a".into(), b"v".to_vec(), Some(NOW + 10), NOW);
        // ainda válido
        assert!(c.get("a", NOW + 5).is_some());
        // depois de expirar: removido e contado
        assert_eq!(c.get("a", NOW + 20), None);
        assert_eq!(c.len(), 0);
        let s = c.stats();
        assert_eq!(s.expired, 1);
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
    }

    #[test]
    fn exists_respects_ttl() {
        let c = RustCache::new();
        c.set("a".into(), b"v".to_vec(), Some(NOW + 10), NOW);
        assert!(c.exists("a", NOW + 5));
        assert!(!c.exists("a", NOW + 20));
        assert_eq!(c.len(), 0); // coletada
    }

    #[test]
    fn delete_counts_only_real_removals() {
        let c = RustCache::new();
        c.set("a".into(), b"v".to_vec(), None, NOW);
        assert!(c.delete("a"));
        assert!(!c.delete("a"));
        assert_eq!(c.stats().deletes, 1);
    }

    #[test]
    fn cleanup_removes_only_expired() {
        let c = RustCache::new();
        c.set("keep".into(), b"v".to_vec(), None, NOW);
        c.set("gone".into(), b"v".to_vec(), Some(NOW + 10), NOW);
        let removed = c.cleanup_expired(NOW + 20);
        assert_eq!(removed, 1);
        assert!(c.exists("keep", NOW + 20));
        assert!(!c.exists("gone", NOW + 20));
        assert_eq!(c.stats().expired, 1);
    }
}
