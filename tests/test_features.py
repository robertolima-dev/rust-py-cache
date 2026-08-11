"""Testes de exists/keys/stats/cleanup_expired/clear e decorator — Etapas 8-17."""

import asyncio
import time

from rust_py_cache import Cache


def test_exists_considers_ttl():
    cache = Cache()
    cache.set("a", 1)
    cache.set("b", 2, ttl=0.05)
    assert cache.exists("a") is True
    assert cache.exists("b") is True
    assert cache.exists("nope") is False
    time.sleep(0.08)
    assert cache.exists("b") is False  # expirou
    assert cache.len() == 1  # "b" foi coletada


def test_keys_lists_current_keys():
    cache = Cache()
    cache.set("a", 1)
    cache.set("b", 2)
    assert sorted(cache.keys()) == ["a", "b"]


def test_clear_keeps_counters():
    cache = Cache()
    cache.set("a", 1)
    cache.get("a")
    cache.clear()
    assert cache.len() == 0
    assert cache.stats()["sets"] == 1  # histórico preservado


def test_cleanup_expired_returns_count():
    cache = Cache()
    cache.set("keep", 1)
    cache.set("g1", 1, ttl=0.05)
    cache.set("g2", 2, ttl=0.05)
    time.sleep(0.08)
    removed = cache.cleanup_expired()
    assert removed == 2
    assert cache.keys() == ["keep"]


def test_stats_tracks_operations():
    cache = Cache()
    cache.set("a", 1)        # sets=1
    cache.get("a")           # hits=1
    cache.get("missing")     # misses=1
    cache.delete("a")        # deletes=1
    s = cache.stats()
    assert s["sets"] == 1
    assert s["hits"] == 1
    assert s["misses"] == 1
    assert s["deletes"] == 1
    assert s["size"] == 0
    assert set(s) == {
        "hits",
        "misses",
        "sets",
        "deletes",
        "expired",
        "evicted",
        "size",
    }


def test_decorator_caches_calls():
    cache = Cache()
    calls = {"n": 0}

    @cache.cached(ttl=60)
    def soma(a, b):
        calls["n"] += 1
        return a + b

    assert soma(2, 3) == 5
    assert soma(2, 3) == 5  # vem do cache
    assert calls["n"] == 1  # função só rodou uma vez
    assert soma(4, 5) == 9  # args diferentes -> nova execução
    assert calls["n"] == 2


def test_decorator_respects_ttl():
    cache = Cache()
    calls = {"n": 0}

    @cache.cached(ttl=0.05)
    def f():
        calls["n"] += 1
        return calls["n"]

    assert f() == 1
    assert f() == 1
    time.sleep(0.08)
    assert f() == 2  # cache expirou, recomputa


def test_decorator_caches_none_result():
    cache = Cache()
    calls = {"n": 0}

    @cache.cached()
    def returns_none():
        calls["n"] += 1
        return None

    assert returns_none() is None
    assert returns_none() is None
    assert calls["n"] == 1  # None foi cacheado, não recomputou


def test_decorator_caches_async_function():
    """P0-2: `@cached` sobre `async def` deve cachear o RESULTADO do await.

    Antes da correção, o wrapper síncrono guardava o objeto coroutine: a
    primeira chamada devolvia o valor certo e a segunda levantava
    `RuntimeError: cannot reuse already awaited coroutine`.
    """
    cache = Cache()
    calls = {"n": 0}

    @cache.cached(ttl=60)
    async def busca(x):
        calls["n"] += 1
        return x * 2

    async def cenario():
        # Duas chamadas no mesmo event loop — é aqui que o bug aparecia.
        primeira = await busca(21)
        segunda = await busca(21)
        terceira = await busca(10)  # args diferentes -> recomputa
        return primeira, segunda, terceira

    primeira, segunda, terceira = asyncio.run(cenario())

    assert primeira == 42
    assert segunda == 42
    assert terceira == 20
    assert calls["n"] == 2  # busca(21) rodou uma vez só


def test_decorator_async_keeps_coroutine_function():
    """O wrapper de uma `async def` continua sendo uma coroutine function."""
    cache = Cache()

    @cache.cached()
    async def f():
        return 1

    assert asyncio.iscoroutinefunction(f)
    assert f.__name__ == "f"


def test_decorator_async_caches_none_result():
    cache = Cache()
    calls = {"n": 0}

    @cache.cached()
    async def returns_none():
        calls["n"] += 1
        return None

    async def cenario():
        return await returns_none(), await returns_none()

    a, b = asyncio.run(cenario())
    assert a is None
    assert b is None
    assert calls["n"] == 1
