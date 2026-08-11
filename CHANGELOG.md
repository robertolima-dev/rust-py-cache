# Changelog

Todas as mudanças relevantes deste projeto são registradas aqui.

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
e o versionamento segue [SemVer](https://semver.org/lang/pt-BR/).

## [0.2.1] - 2026-08-11

### Corrigido

- `@cache.cached` sobre função `async def` guardava no cache o objeto coroutine
  em vez do resultado do `await`, quebrando o decorator já na primeira chamada
  com `TypeError: cannot pickle 'coroutine' object`. O decorator agora detecta
  `inspect.iscoroutinefunction` e gera um wrapper `async` que faz o `await`
  antes do `set`. (BACKLOG2 P0-2)

## [0.2.0]

### Adicionado

- Expiração em background via `Sweeper` (thread dedicada com `Weak<T>`,
  encerramento limpo no `Drop`).
- Evicção por `max_size` com política LRU.
- Exemplos para FastAPI e Django.

## [0.1.x]

### Adicionado

- `Cache` com `set` / `get(default=...)` / `delete` / `exists` / `keys` /
  `len` / `clear` / `cleanup_expired` / `stats`.
- TTL por entrada com expiração preguiçosa.
- `stats()` cumulativo baseado em `AtomicU64`.
- Decorator de memoização `@cache.cached(ttl=..., key=...)`.
- Core em Rust sobre `DashMap`, valores serializados com `pickle`.
