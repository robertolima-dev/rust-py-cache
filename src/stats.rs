//! `CacheStats`: contadores de uso do cache.
//!
//! Usamos `AtomicU64` em vez de `Mutex<u64>` porque incrementar contador é a
//! operação mais quente do cache e não queremos um lock global no caminho. Cada
//! `fetch_add` é uma instrução atômica de hardware — várias threads incrementam
//! sem se bloquear.
//!
//! `Ordering::Relaxed`: só garantimos atomicidade de *cada* contador, sem impor
//! ordem entre contadores diferentes. Para métricas isso basta — não tomamos
//! decisões de correção com base na ordem relativa de `hits` vs `misses`.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub sets: AtomicU64,
    pub deletes: AtomicU64,
    pub expired: AtomicU64,
    pub evicted: AtomicU64,
}

/// Cópia "congelada" dos contadores num instante — fácil de mandar pro Python.
#[derive(Debug, Clone, Copy)]
pub struct StatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub sets: u64,
    pub deletes: u64,
    pub expired: u64,
    pub evicted: u64,
}

impl CacheStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_set(&self) {
        self.sets.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_delete(&self) {
        self.deletes.fetch_add(1, Ordering::Relaxed);
    }

    /// Soma `n` ao contador de expirados (usado por `cleanup_expired`, que remove
    /// vários de uma vez).
    pub fn record_expired(&self, n: u64) {
        self.expired.fetch_add(n, Ordering::Relaxed);
    }

    /// Conta uma entrada removida por evicção (política LRU ao atingir `max_size`).
    pub fn record_evicted(&self) {
        self.evicted.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            sets: self.sets.load(Ordering::Relaxed),
            deletes: self.deletes.load(Ordering::Relaxed),
            expired: self.expired.load(Ordering::Relaxed),
            evicted: self.evicted.load(Ordering::Relaxed),
        }
    }
}
