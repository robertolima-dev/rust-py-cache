//! rust-py-cache — core Rust da biblioteca.
//!
//! `lib.rs` é a camada de ponte com o Python (PyO3). A lógica de armazenamento
//! mora em `cache.rs`; o modelo de dados em `entry.rs`.

mod cache;
mod entry;
mod stats;
mod ttl;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::cache::RustCache;
use crate::ttl::{expires_at_from_ttl, now_ms};

/// Serializa um objeto Python em bytes via `pickle.dumps`.
///
/// PyO3: `py.import("pickle")` nos dá o módulo `pickle` do interpretador que está
/// rodando — ou seja, "serialização do lado do Python", só que disparada do Rust.
/// `call_method1` chama `pickle.dumps(value)` (o `1` = um argumento posicional).
/// O retorno é um `bytes` do Python; `downcast::<PyBytes>` confirma o tipo e
/// `as_bytes().to_vec()` copia para um `Vec<u8>` que o core guarda como opaco.
fn dumps(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let pickle = py.import("pickle")?;
    let data = pickle.call_method1("dumps", (value,))?;
    Ok(data.downcast::<PyBytes>()?.as_bytes().to_vec())
}

/// Reconstrói um objeto Python a partir dos bytes via `pickle.loads`.
///
/// `unbind()` transforma o `Bound<'py, PyAny>` (preso ao GIL atual) num
/// `Py<PyAny>` independente, que é o que devolvemos ao Python.
fn loads(py: Python<'_>, data: &[u8]) -> PyResult<Py<PyAny>> {
    let pickle = py.import("pickle")?;
    let obj = pickle.call_method1("loads", (PyBytes::new(py, data),))?;
    Ok(obj.unbind())
}

/// Função de teste exposta ao Python (mantida da Etapa 1).
#[pyfunction]
fn hello() -> PyResult<String> {
    Ok("Hello from rust-py-cache 🦀".to_string())
}

/// Cache local, em memória, thread-safe.
///
/// PyO3:
/// - `#[pyclass]` transforma este struct Rust numa classe Python (`Cache`).
/// - Para PyO3 expor a classe com segurança entre threads, o conteúdo precisa ser
///   `Send + Sync`. `RustCache` é, porque `DashMap` é thread-safe.
/// - Os métodos recebem `&self` (empréstimo compartilhado): não precisamos de
///   `&mut self` porque o `DashMap` lá dentro faz interior mutability. Isso é o
///   que permite várias threads Python chamarem o mesmo objeto `Cache`.
/// - `subclass`: permite herdar de `Cache` no Python — usado pelo wrapper que
///   adiciona o decorator `@cache.cached` (ver `python/.../decorators.py`).
#[pyclass(subclass)]
pub struct Cache {
    inner: RustCache,
}

#[pymethods]
impl Cache {
    /// `Cache()` — cria um cache vazio.
    ///
    /// `#[new]` marca o construtor. Devolver `Self` basta: o PyO3 embrulha o
    /// valor Rust no objeto Python.
    #[new]
    fn new() -> Self {
        Cache {
            inner: RustCache::new(),
        }
    }

    /// `set(key, value, ttl=None)` — grava `value` sob `key`.
    ///
    /// - `ttl` em segundos (`int` ou `float`); `None` = sem expiração.
    /// - `ttl <= 0` levanta `ValueError` (decisão explícita do roadmap).
    /// - Sobrescreve a chave se já existir.
    ///
    /// PyO3:
    /// - `py: Python<'_>` é o *token do GIL*: prova, em tempo de compilação, que
    ///   seguramos o GIL — necessário para chamar `pickle`.
    /// - `value: &Bound<'_, PyAny>` é qualquer objeto Python, emprestado.
    /// - `#[pyo3(signature = ...)]` define o default `ttl=None` visível no Python.
    #[pyo3(signature = (key, value, ttl=None))]
    fn set(
        &self,
        py: Python<'_>,
        key: String,
        value: &Bound<'_, PyAny>,
        ttl: Option<f64>,
    ) -> PyResult<()> {
        let now = now_ms();
        let expires_at = expires_at_from_ttl(ttl, now)
            .map_err(|_| PyValueError::new_err("ttl deve ser > 0 (ou None para não expirar)"))?;
        let bytes = dumps(py, value)?;
        self.inner.set(key, bytes, expires_at, now);
        Ok(())
    }

    /// `get(key, default=None)` — devolve o valor de `key`.
    ///
    /// - Chave ausente → `default` (que por padrão é `None`).
    /// - Chave expirada → remove a entrada (expiração lazy) e devolve `default`.
    /// - Caso contrário → o objeto Python original, desserializado por `pickle`.
    #[pyo3(signature = (key, default=None))]
    fn get(
        &self,
        py: Python<'_>,
        key: &str,
        default: Option<Py<PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        match self.inner.get(key, now_ms()) {
            Some(bytes) => loads(py, &bytes),
            None => Ok(default.unwrap_or_else(|| py.None())),
        }
    }

    /// `exists(key)` — `True` se a chave existe e não expirou (considera TTL).
    fn exists(&self, key: &str) -> bool {
        self.inner.exists(key, now_ms())
    }

    /// `delete(key)` — remove `key`. `True` se existia, `False` se não existia.
    fn delete(&self, key: &str) -> bool {
        self.inner.delete(key)
    }

    /// `keys()` — lista de chaves atuais (pode incluir expiradas não coletadas).
    fn keys(&self) -> Vec<String> {
        self.inner.keys()
    }

    /// `cleanup_expired()` — remove expirados agora; devolve quantos saíram.
    fn cleanup_expired(&self) -> usize {
        self.inner.cleanup_expired(now_ms())
    }

    /// `stats()` — dict com hits, misses, sets, deletes, expired e size.
    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let s = self.inner.stats();
        let d = PyDict::new(py);
        d.set_item("hits", s.hits)?;
        d.set_item("misses", s.misses)?;
        d.set_item("sets", s.sets)?;
        d.set_item("deletes", s.deletes)?;
        d.set_item("expired", s.expired)?;
        d.set_item("size", self.inner.len())?;
        Ok(d)
    }

    /// Número de entradas (aproximado — pode incluir expirados não coletados).
    fn len(&self) -> usize {
        self.inner.len()
    }

    /// Suporte a `len(cache)` no Python.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Remove todas as entradas.
    fn clear(&self) {
        self.inner.clear();
    }

    /// Representação amigável: `<Cache size=N>`.
    fn __repr__(&self) -> String {
        format!("<Cache size={}>", self.inner.len())
    }
}

/// Ponto de entrada do módulo nativo `_rust_py_cache`.
#[pymodule]
fn _rust_py_cache(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello, m)?)?;
    m.add_class::<Cache>()?;
    Ok(())
}
