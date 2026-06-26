//! Expiração em background: uma thread que varre as entradas expiradas em
//! intervalos regulares, em vez de depender só da expiração preguiçosa (lazy)
//! ou de chamadas manuais a `cleanup_expired()`.
//!
//! ## Ciclo de vida
//!
//! A thread recebe um [`Weak<RustCache>`], não um `Arc` forte: assim ela **não**
//! mantém o cache vivo. Se o objeto `Cache` do Python for coletado, o `Arc` some,
//! o `upgrade()` falha e a thread encerra sozinha. Além disso o `Cache` guarda
//! este [`Sweeper`]; quando ele é destruído (`Drop`), sinalizamos a parada e
//! fazemos `join` — sem threads órfãs.
//!
//! O sono é em fatias curtas (não um único `sleep(interval)`) para o encerramento
//! responder rápido ao sinal de parada, sem esperar o intervalo inteiro.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::cache::RustCache;
use crate::ttl::now_ms;

/// Handle da thread de varredura. Pára e faz `join` automaticamente no `Drop`.
pub struct Sweeper {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Sweeper {
    /// Inicia a thread que varre `cache` a cada `interval`.
    pub fn start(cache: &Arc<RustCache>, interval: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let weak = Arc::downgrade(cache);
        let handle = thread::spawn(move || run(weak, interval, stop_thread));
        Sweeper {
            stop,
            handle: Some(handle),
        }
    }
}

/// Laço da thread: dorme `interval` (em fatias) e então varre os expirados,
/// repetindo até receber o sinal de parada ou o cache deixar de existir.
fn run(cache: Weak<RustCache>, interval: Duration, stop: Arc<AtomicBool>) {
    let tick = Duration::from_millis(200);
    loop {
        let mut waited = Duration::ZERO;
        while waited < interval {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let chunk = tick.min(interval - waited);
            thread::sleep(chunk);
            waited += chunk;
        }
        if stop.load(Ordering::Relaxed) {
            return;
        }
        match cache.upgrade() {
            Some(c) => {
                c.cleanup_expired(now_ms());
            }
            None => return, // o cache foi coletado; nada mais a varrer
        }
    }
}

impl Drop for Sweeper {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
