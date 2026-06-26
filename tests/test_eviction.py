"""Eviction (LRU) + expiração em background.

A correção da LRU "qual chave sai" é verificada de forma determinística nos
testes Rust (`src/cache.rs`), onde o relógio é injetado. Aqui cobrimos a API
Python: seleção de política, o getter `eviction_policy`, validação, retorno de
`set` e a thread de expiração em background.
"""

import time

import pytest

from rust_py_cache import Cache


def test_eviction_policy_defaults_to_reject():
    cache = Cache(max_size=2)
    assert cache.eviction_policy == "reject"


def test_eviction_policy_getter_reports_lru():
    cache = Cache(max_size=2, eviction_policy="lru")
    assert cache.eviction_policy == "lru"


def test_invalid_eviction_policy_raises():
    with pytest.raises(ValueError):
        Cache(max_size=2, eviction_policy="lfu")


def test_reject_blocks_new_keys_when_full():
    cache = Cache(max_size=2, eviction_policy="reject")
    assert cache.set("a", 1) is True
    assert cache.set("b", 2) is True
    assert cache.set("c", 3) is False  # cheio
    assert len(cache) == 2
    assert cache.stats()["evicted"] == 0
    # sobrescrever chave existente continua permitido
    assert cache.set("a", 10) is True


def test_lru_evicts_to_make_room():
    cache = Cache(max_size=2, eviction_policy="lru")
    cache.set("a", 1)
    cache.set("b", 2)
    cache.set("c", 3)  # cheio + chave nova => remove a LRU
    assert len(cache) == 2
    assert cache.stats()["evicted"] == 1


def test_lru_keeps_recently_used():
    cache = Cache(max_size=2, eviction_policy="lru")
    cache.set("a", 1)
    cache.set("b", 2)
    time.sleep(0.01)
    assert cache.get("a") == 1  # "b" passa a ser a menos recentemente usada
    cache.set("c", 3)
    assert cache.get("b") is None  # evicted
    assert cache.get("a") == 1
    assert cache.get("c") == 3


def test_no_max_size_never_evicts():
    cache = Cache()
    for i in range(100):
        cache.set(f"k{i}", i)
    assert len(cache) == 100
    assert cache.stats()["evicted"] == 0


def test_background_expiration_sweeps_automatically():
    cache = Cache(cleanup_interval=0.5)
    cache.set("a", 1, ttl=0.5)
    assert len(cache) == 1
    # Sem chamar cleanup_expired(): a thread de fundo deve coletar depois que o
    # TTL (0.5s) vence e o sweeper (0.5s) roda.
    time.sleep(1.6)
    assert len(cache) == 0
    assert cache.stats()["expired"] >= 1
