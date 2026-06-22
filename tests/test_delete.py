"""Testes de delete — Etapa 7."""

from rust_py_cache import Cache


def test_delete_existing_returns_true():
    cache = Cache()
    cache.set("k", "v")
    assert cache.delete("k") is True
    assert cache.len() == 0
    assert cache.get("k") is None


def test_delete_missing_returns_false():
    cache = Cache()
    assert cache.delete("nope") is False


def test_delete_is_idempotent():
    cache = Cache()
    cache.set("k", "v")
    assert cache.delete("k") is True
    assert cache.delete("k") is False


def test_delete_only_target_key():
    cache = Cache()
    cache.set("a", 1)
    cache.set("b", 2)
    assert cache.delete("a") is True
    assert cache.get("a") is None
    assert cache.get("b") == 2
    assert cache.len() == 1
