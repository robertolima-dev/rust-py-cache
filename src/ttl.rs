//! Helpers de tempo para o cache.
//!
//! Trabalhamos sempre com *epoch em milissegundos* (`u64`): é barato de comparar
//! e de armazenar em `CacheEntry::expires_at`. No MVP o TTL é o único uso de tempo.

use std::time::{SystemTime, UNIX_EPOCH};

/// Instante atual em milissegundos desde a época Unix (1970-01-01).
///
/// `duration_since(UNIX_EPOCH)` pode falhar se o relógio do sistema estiver antes
/// de 1970 — algo que não acontece na prática. Por isso o `expect`: se acontecer,
/// é um ambiente quebrado e queremos saber na hora, não silenciar.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("relógio do sistema antes de 1970-01-01")
        .as_millis() as u64
}

/// Converte um TTL em segundos (vindo do Python como `int`/`float`) num instante
/// de expiração absoluto (epoch-ms), somado a `now`.
///
/// Retorna:
/// - `Ok(None)`     se `ttl` é `None` (a entrada nunca expira);
/// - `Ok(Some(ts))` se `ttl > 0`;
/// - `Err(())`      se `ttl <= 0` — o chamador transforma isso em `ValueError`.
///
/// Decisão do roadmap: `ttl <= 0` é erro explícito, não "expira imediatamente".
pub fn expires_at_from_ttl(ttl: Option<f64>, now: u64) -> Result<Option<u64>, ()> {
    match ttl {
        None => Ok(None),
        Some(secs) if secs > 0.0 => Ok(Some(now + (secs * 1000.0) as u64)),
        Some(_) => Err(()),
    }
}
