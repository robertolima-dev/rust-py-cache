"""rust-py-cache — An ultra-fast local cache for Python, powered by Rust."""

# O módulo nativo (_rust_py_cache) é compilado pelo Rust/maturin e fica DENTRO
# deste pacote. O core (set/get/delete/exists/keys/stats/cleanup_expired) vem do
# Rust; aqui só adicionamos açúcar Python — o decorator `cached` — via subclasse.
from ._rust_py_cache import Cache as _RustCache, hello
from .decorators import cached as _cached


class Cache(_RustCache):
    """Cache local em memória (core em Rust) + decorator `cached` em Python.

    Herda todos os métodos do core nativo e adiciona `@cache.cached(...)`. A
    herança só é possível porque o `#[pyclass(subclass)]` libera o tipo nativo
    para ser estendido no Python.
    """

    cached = _cached


__all__ = ["Cache", "hello"]
__version__ = "0.1.0"
