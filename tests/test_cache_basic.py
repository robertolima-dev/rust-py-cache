"""Testes básicos da classe Cache (esqueleto — Etapa 4).

Crescem nas próximas etapas (set/get/delete/exists...).
"""

from rust_py_cache import Cache


def test_construct_empty():
    cache = Cache()
    assert cache.len() == 0
    assert len(cache) == 0  # __len__


def test_clear_on_empty():
    cache = Cache()
    cache.clear()
    assert cache.len() == 0


def test_repr():
    cache = Cache()
    assert repr(cache) == "<Cache size=0>"
