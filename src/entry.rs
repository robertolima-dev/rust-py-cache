//! `CacheEntry`: uma entrada armazenada no cache.
//!
//! A partir das Etapas 5/6, `value`/`expires_at`/`created_at`/`is_expired` já são
//! usados por set/get. Restam `last_accessed_at` e `hits`, reservados para stats
//! (Etapa 12) e eviction LRU/LFU (v0.2). Por isso o `allow(dead_code)` continua —
//! sai quando esses dois campos passarem a ser lidos.
#![allow(dead_code)]

/// Valor guardado para uma chave, com metadados de tempo e acesso.
///
/// `value` são bytes opacos para o Rust: no MVP é o resultado de `pickle.dumps`
/// feito do lado do Python. O core não interpreta esses bytes.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Bytes serializados do valor Python.
    pub value: Vec<u8>,
    /// Instante de expiração em epoch-ms. `None` = nunca expira.
    pub expires_at: Option<u64>,
    /// Quando a entrada foi criada (epoch-ms).
    pub created_at: u64,
    /// Último acesso por `get` bem-sucedido (epoch-ms) — base para LRU futuro.
    pub last_accessed_at: u64,
    /// Quantas vezes esta entrada foi lida — base para LFU futuro.
    pub hits: u64,
}

impl CacheEntry {
    /// Cria uma entrada nova, com `created_at` e `last_accessed_at` = `now`.
    pub fn new(value: Vec<u8>, expires_at: Option<u64>, now: u64) -> Self {
        Self {
            value,
            expires_at,
            created_at: now,
            last_accessed_at: now,
            hits: 0,
        }
    }

    /// `true` se a entrada já expirou em relação a `now` (epoch-ms).
    /// Entradas sem `expires_at` nunca expiram.
    pub fn is_expired(&self, now: u64) -> bool {
        match self.expires_at {
            Some(exp) => now >= exp,
            None => false,
        }
    }
}
