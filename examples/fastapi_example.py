"""Exemplo FastAPI — Etapa 18.

Cache local por processo na frente de uma função "cara". Rode com:

    pip install fastapi uvicorn
    uvicorn examples.fastapi_example:app --reload

Endpoints:
    GET /users/{user_id}   -> dado cacheado por 30s
    GET /cache/stats       -> métricas do cache
    DELETE /cache/{key}    -> invalida uma chave
"""

import time

from fastapi import FastAPI

from rust_py_cache import Cache

app = FastAPI(title="rust-py-cache demo")
cache = Cache()


def _load_user_from_db(user_id: int) -> dict:
    """Simula uma consulta lenta ao banco."""
    time.sleep(0.2)
    return {"id": user_id, "name": f"User {user_id}", "loaded_at": time.time()}


@app.get("/users/{user_id}")
def get_user(user_id: int):
    key = f"user:{user_id}"
    cached = cache.get(key)
    if cached is not None:
        return {"source": "cache", "data": cached}

    user = _load_user_from_db(user_id)
    cache.set(key, user, ttl=30)
    return {"source": "db", "data": user}


# Alternativa com o decorator: memoiza automaticamente por argumento.
@cache.cached(ttl=30)
def fibonacci(n: int) -> int:
    if n < 2:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)


@app.get("/fib/{n}")
def get_fib(n: int):
    return {"n": n, "value": fibonacci(n)}


@app.get("/cache/stats")
def cache_stats():
    return cache.stats()


@app.delete("/cache/{key}")
def invalidate(key: str):
    return {"deleted": cache.delete(key)}
