"""Testes de set/get — Etapas 5 e 6 (com expiração lazy, parte da Etapa 11)."""

import time

import pytest

from rust_py_cache import Cache


def test_set_get_roundtrip():
    cache = Cache()
    cache.set("user:1", {"name": "Roberto"})
    assert cache.get("user:1") == {"name": "Roberto"}
    assert cache.len() == 1


def test_get_missing_returns_none():
    cache = Cache()
    assert cache.get("nope") is None


def test_get_missing_with_default():
    cache = Cache()
    assert cache.get("nope", default=42) == 42
    # default não deve criar a chave
    assert cache.len() == 0


def test_overwrite_existing_key():
    cache = Cache()
    cache.set("k", "v1")
    cache.set("k", "v2")
    assert cache.get("k") == "v2"
    assert cache.len() == 1


def test_complex_python_objects():
    cache = Cache()
    value = {"nums": [1, 2, 3], "nested": {"t": (4, 5)}, "s": {6, 7}}
    cache.set("c", value)
    assert cache.get("c") == value


def test_ttl_none_never_expires():
    cache = Cache()
    cache.set("k", "v", ttl=None)
    time.sleep(0.05)
    assert cache.get("k") == "v"


def test_ttl_expires_lazily():
    cache = Cache()
    cache.set("k", "v", ttl=0.05)  # 50 ms
    assert cache.get("k") == "v"
    time.sleep(0.08)
    assert cache.get("k") is None
    # a leitura expirada removeu a entrada
    assert cache.len() == 0


@pytest.mark.parametrize("bad_ttl", [0, -1, -0.5])
def test_ttl_non_positive_raises(bad_ttl):
    cache = Cache()
    with pytest.raises(ValueError):
        cache.set("k", "v", ttl=bad_ttl)
