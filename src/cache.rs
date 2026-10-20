//! `RustCache`: o núcleo de armazenamento, independente do PyO3.
//!
//! Mantemos a lógica de cache aqui, *sem* tipos do Python, para que seja fácil
//! de testar em Rust puro e para separar responsabilidades: `lib.rs` cuida da
//! ponte com o Python; `cache.rs` cuida do armazenamento.

use std::sync::Mutex;

use dashmap::DashMap;

use crate::entry::CacheEntry;
use crate::stats::{CacheStats, StatsSnapshot};

/// O que fazer quando `max_size` é atingido e chega uma chave **nova**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvictionPolicy {
    /// Rejeita a escrita (`set` devolve `false`). Comportamento padrão.
    #[default]
    Reject,
    /// Remove as entradas menos recentemente usadas (LRU) para abrir espaço.
    Lru,
}

/// Fração de `max_size` removida de uma vez quando a LRU precisa abrir espaço
/// (1/32 ≈ 3%, no mínimo 1 entrada). Ver [`RustCache::evict_lru_batch`].
const EVICTION_BATCH_DIVISOR: usize = 32;

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
#[derive(Debug)]
pub struct RustCache {
    store: DashMap<String, CacheEntry>,
    stats: CacheStats,
    /// Limite opcional de número de chaves.
    max_size: Option<usize>,
    /// Política aplicada quando o cache enche e chega uma chave nova.
    eviction_policy: EvictionPolicy,
    /// Serializa só o caminho de evicção: quando várias threads encontram o
    /// cache cheio ao mesmo tempo, uma varre e as outras, ao entrar, veem que
    /// já há espaço e seguem — sem N varreduras simultâneas. Não é tocado no
    /// caminho comum de `get`/`set`.
    eviction_lock: Mutex<()>,
}

impl RustCache {
    /// Cria um cache vazio. `max_size = None` => sem limite de chaves.
    pub fn new(max_size: Option<usize>, eviction_policy: EvictionPolicy) -> Self {
        Self {
            store: DashMap::new(),
            stats: CacheStats::new(),
            max_size,
            eviction_policy,
            eviction_lock: Mutex::new(()),
        }
    }

    /// A política de evicção configurada.
    pub fn eviction_policy(&self) -> EvictionPolicy {
        self.eviction_policy
    }

    /// Abre espaço removendo, de uma vez, as `max / 32` (no mínimo 1) entradas
    /// menos recentemente usadas. Conta cada uma em `evicted`.
    ///
    /// ## Por que em lote
    ///
    /// Achar a entrada mais antiga num `DashMap` exige varrer o mapa inteiro —
    /// não há ordem nem acesso aleatório. Até a 0.2 isso acontecia a **cada**
    /// `set` de chave nova com o cache cheio, que é o regime normal de um cache
    /// cheio sob escrita: O(n) por inserção. Removendo ~3% por varredura, a
    /// próxima varredura só vem depois de outras ~n/32 inserções — custo
    /// amortizado O(1) (≈ 64 visitas por inserção), sem lock global no caminho
    /// comum e sem `unsafe` para amostrar shards.
    ///
    /// Uma lista LRU exata exigiria um lock global em todo `get`, que é o que o
    /// `DashMap` existe para evitar. A amostragem estilo Redis exigiria acesso
    /// aleatório às entradas, que o `DashMap` só oferece pela `raw-api`
    /// (tabela interna do hashbrown, iteração `unsafe`).
    ///
    /// ## Como
    ///
    /// 1. Coleta só os `last_accessed_at` (um `Vec<u64>`, sem clonar chaves) e
    ///    acha o corte do lote com `select_nth_unstable` — O(n), sem ordenar.
    /// 2. Coleta as chaves com acesso `<= corte` (até o tamanho do lote).
    /// 3. Remove com `remove_if`, que **recheca** o carimbo: uma chave lida por
    ///    outra thread entre as fases deixou de ser LRU e fica.
    fn evict_lru_batch(&self, max: usize) {
        let _guard = self
            .eviction_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Outra thread pode ter aberto espaço enquanto esperávamos o lock.
        if self.store.len() < max {
            return;
        }

        let mut stamps: Vec<u64> = self
            .store
            .iter()
            .map(|e| e.value().last_accessed_at)
            .collect();
        if stamps.is_empty() {
            return;
        }
        let batch = (max / EVICTION_BATCH_DIVISOR).clamp(1, stamps.len());
        let (_, &mut cutoff, _) = stamps.select_nth_unstable(batch - 1);
        drop(stamps);

        let victims: Vec<String> = self
            .store
            .iter()
            .filter(|e| e.value().last_accessed_at <= cutoff)
            .take(batch)
            .map(|e| e.key().clone())
            .collect();

        for key in victims {
            if self
                .store
                .remove_if(&key, |_, v| v.last_accessed_at <= cutoff)
                .is_some()
            {
                self.stats.record_evicted();
            }
        }
    }

    /// Grava `value` (bytes já serializados) sob `key`, sobrescrevendo se existir.
    ///
    /// O core não sabe o que há nos bytes — quem serializa (pickle) é a camada
    /// `lib.rs`, do lado do Python. Aqui só montamos a `CacheEntry` e inserimos.
    /// `insert` no `DashMap` pega o lock só do shard daquela chave.
    ///
    /// Sobrescrever uma chave existente sempre é permitido. Para uma chave
    /// **nova** com o cache cheio (`max_size`): com `Reject` devolve `false` sem
    /// inserir; com `Lru` remove um lote das menos recentemente usadas (ver
    /// [`Self::evict_lru_batch`]) e prossegue.
    /// Devolve `true` quando a entrada foi (ou seria) gravada.
    pub fn set(&self, key: String, value: Vec<u8>, expires_at: Option<u64>, now: u64) -> bool {
        if let Some(max) = self.max_size {
            if self.store.len() >= max && !self.store.contains_key(&key) {
                match self.eviction_policy {
                    EvictionPolicy::Reject => return false,
                    EvictionPolicy::Lru => self.evict_lru_batch(max),
                }
            }
        }

        self.store
            .insert(key, CacheEntry::new(value, expires_at, now));
        self.stats.record_set();
        true
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
        // `get_mut` segura o lock de ESCRITA do shard: além de ler o valor,
        // atualizamos `last_accessed_at`/`hits` (base do LRU) no mesmo acesso.
        let Some(mut entry) = self.store.get_mut(key) else {
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
        entry.last_accessed_at = now;
        entry.hits += 1;
        let value = entry.value.clone();
        self.stats.record_hit();
        Some(value)
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
            if self
                .store
                .remove_if(&key, |_, v| v.is_expired(now))
                .is_some()
            {
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
        let c = RustCache::new(None, EvictionPolicy::Reject);
        c.set("a".into(), b"v".to_vec(), None, NOW);
        assert_eq!(c.get("a", NOW), Some(b"v".to_vec()));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn get_missing_is_none_and_counts_miss() {
        let c = RustCache::new(None, EvictionPolicy::Reject);
        assert_eq!(c.get("x", NOW), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn expired_get_collects_and_counts() {
        let c = RustCache::new(None, EvictionPolicy::Reject);
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
        let c = RustCache::new(None, EvictionPolicy::Reject);
        c.set("a".into(), b"v".to_vec(), Some(NOW + 10), NOW);
        assert!(c.exists("a", NOW + 5));
        assert!(!c.exists("a", NOW + 20));
        assert_eq!(c.len(), 0); // coletada
    }

    #[test]
    fn delete_counts_only_real_removals() {
        let c = RustCache::new(None, EvictionPolicy::Reject);
        c.set("a".into(), b"v".to_vec(), None, NOW);
        assert!(c.delete("a"));
        assert!(!c.delete("a"));
        assert_eq!(c.stats().deletes, 1);
    }

    #[test]
    fn cleanup_removes_only_expired() {
        let c = RustCache::new(None, EvictionPolicy::Reject);
        c.set("keep".into(), b"v".to_vec(), None, NOW);
        c.set("gone".into(), b"v".to_vec(), Some(NOW + 10), NOW);
        let removed = c.cleanup_expired(NOW + 20);
        assert_eq!(removed, 1);
        assert!(c.exists("keep", NOW + 20));
        assert!(!c.exists("gone", NOW + 20));
        assert_eq!(c.stats().expired, 1);
    }

    #[test]
    fn reject_policy_blocks_new_keys_when_full() {
        let c = RustCache::new(Some(2), EvictionPolicy::Reject);
        assert!(c.set("a".into(), b"1".to_vec(), None, NOW));
        assert!(c.set("b".into(), b"2".to_vec(), None, NOW));
        // cheio + chave nova => rejeita, sem evicção
        assert!(!c.set("c".into(), b"3".to_vec(), None, NOW));
        assert_eq!(c.len(), 2);
        assert_eq!(c.stats().evicted, 0);
        // sobrescrever chave existente é sempre permitido
        assert!(c.set("a".into(), b"10".to_vec(), None, NOW));
    }

    #[test]
    fn lru_policy_evicts_least_recently_used() {
        let c = RustCache::new(Some(2), EvictionPolicy::Lru);
        c.set("a".into(), b"1".to_vec(), None, NOW);
        c.set("b".into(), b"2".to_vec(), None, NOW);
        // Acessa "a" mais tarde: "b" vira a menos recentemente usada.
        assert!(c.get("a", NOW + 5).is_some());
        // Insere "c": remove "b" (LRU), mantém "a" e "c".
        assert!(c.set("c".into(), b"3".to_vec(), None, NOW + 10));
        assert_eq!(c.len(), 2);
        assert_eq!(c.stats().evicted, 1);
        assert!(c.get("b", NOW + 11).is_none());
        assert!(c.get("a", NOW + 11).is_some());
        assert!(c.get("c", NOW + 11).is_some());
    }

    #[test]
    fn lru_evicts_a_batch_of_the_oldest() {
        // max 64 => lote de 64/32 = 2.
        let c = RustCache::new(Some(64), EvictionPolicy::Lru);
        for i in 0..64u64 {
            c.set(format!("k{i}"), b"v".to_vec(), None, NOW + i);
        }
        // k0 e k1 são as mais antigas; tocar em k0 a torna a mais recente.
        assert!(c.get("k0", NOW + 100).is_some());
        assert!(c.set("new".into(), b"v".to_vec(), None, NOW + 101));
        assert_eq!(c.stats().evicted, 2);
        assert_eq!(c.len(), 63);
        assert!(c.exists("k0", NOW + 102));
        assert!(!c.exists("k1", NOW + 102));
        assert!(!c.exists("k2", NOW + 102));
        assert!(c.exists("k3", NOW + 102));
        assert!(c.exists("new", NOW + 102));
    }

    #[test]
    fn lru_amortizes_the_scan() {
        // Escrita sustentada com o cache cheio: o número de varreduras é
        // ~inserções / lote, não uma por inserção.
        let max = 1_024;
        let c = RustCache::new(Some(max), EvictionPolicy::Lru);
        for i in 0..(max as u64 * 4) {
            c.set(format!("k{i}"), b"v".to_vec(), None, NOW + i);
            assert!(c.len() <= max);
        }
        let evicted = c.stats().evicted;
        let batch = (max / EVICTION_BATCH_DIVISOR) as u64;
        assert_eq!(evicted % batch, 0, "sempre um lote cheio por varredura");
        // 3 * max inserções além da capacidade => ~3 * 32 varreduras.
        assert!(evicted / batch <= 3 * EVICTION_BATCH_DIVISOR as u64 + 1);
        // As sobreviventes são as mais recentes.
        assert!(c.exists(&format!("k{}", max as u64 * 4 - 1), NOW));
        assert!(!c.exists("k0", NOW));
    }

    #[test]
    fn no_max_size_never_evicts() {
        let c = RustCache::new(None, EvictionPolicy::Lru);
        for i in 0..50 {
            c.set(format!("k{i}"), b"v".to_vec(), None, NOW);
        }
        assert_eq!(c.len(), 50);
        assert_eq!(c.stats().evicted, 0);
    }
}
