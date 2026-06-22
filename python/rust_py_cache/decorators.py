"""Decorator de memoização — Etapa 17.

Fornece `Cache.cached`, um decorator que guarda o retorno de uma função no cache.
Fica em Python (não no core Rust) porque mexe com `*args/**kwargs`, geração de
chave e `functools.wraps` — tudo muito mais natural aqui.
"""

import functools

# Sentinela para distinguir "ausente no cache" de "valor None legítimo".
# `cache.get(key, _MISSING)` devolve `_MISSING` só quando a chave não existe.
_MISSING = object()


def _default_key(func, args, kwargs):
    """Chave determinística a partir da função e dos argumentos.

    Usa `repr` dos argumentos — simples e legível. Pressupõe argumentos
    repr-áveis e estáveis (o caso comum: números, strings, tuplas). Para casos
    exóticos, o usuário pode passar `key=...` explícito.
    """
    parts = [func.__module__ or "", func.__qualname__, repr(args)]
    if kwargs:
        parts.append(repr(sorted(kwargs.items())))
    return "|".join(parts)


def cached(self, ttl=None, key=None):
    """Decorator: memoiza o retorno da função neste cache.

    Parâmetros:
        ttl: validade em segundos (igual a `set`); `None` = sem expiração.
        key: opcional. String fixa, ou callable `key(*args, **kwargs) -> str`.
             Se omitido, a chave é derivada de função + argumentos.

    Uso:
        @cache.cached(ttl=60)
        def soma(a, b):
            return a + b
    """

    def decorator(func):
        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            if key is None:
                cache_key = _default_key(func, args, kwargs)
            elif callable(key):
                cache_key = key(*args, **kwargs)
            else:
                cache_key = key

            hit = self.get(cache_key, _MISSING)
            if hit is not _MISSING:
                return hit

            result = func(*args, **kwargs)
            self.set(cache_key, result, ttl=ttl)
            return result

        # Expõe a chave-base para inspeção/invalidação manual em testes.
        wrapper.__wrapped__ = func
        return wrapper

    return decorator
