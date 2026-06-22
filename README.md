# rust-py-cache

> **An ultra-fast local cache for Python, powered by Rust.**

Cache local, em memória, thread-safe, com TTL, expiração lazy e métricas. O core
é escrito em Rust (PyO3 + maturin) com um `DashMap` concorrente; a API Python é
mínima. É um "mini Redis local" que vive **dentro do processo** Python.

[![status](https://img.shields.io/badge/status-v0.1%20MVP-orange)](./ROADMAP.md)

## Instalação

```bash
pip install rust-py-cache   # (após publicação no PyPI — ver ROADMAP)
```

Para desenvolver localmente (precisa de Rust + maturin):

```bash
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
maturin develop          # compila o Rust e instala no venv
pytest                   # roda os testes
```

## Uso

```python
from rust_py_cache import Cache

cache = Cache()

cache.set("user:1", {"name": "Roberto"}, ttl=60)   # ttl em segundos
user = cache.get("user:1")                          # {"name": "Roberto"}
cache.get("missing", default=0)                     # 0

cache.exists("user:1")        # True (considera TTL)
cache.delete("user:1")        # True se removeu, False se não existia
cache.len()                   # tamanho aproximado
cache.keys()                  # lista de chaves
cache.cleanup_expired()       # remove expirados; retorna nº removidos
cache.clear()                 # remove tudo (mantém contadores)
cache.stats()                 # {'hits', 'misses', 'sets', 'deletes', 'expired', 'size'}
```

### Decorator de memoização

```python
@cache.cached(ttl=60)
def soma(a, b):
    return a + b

soma(2, 3)   # executa e cacheia
soma(2, 3)   # vem do cache

# chave customizada (string fixa ou callable):
@cache.cached(ttl=300, key=lambda user_id: f"user:{user_id}")
def load_user(user_id):
    ...
```

Veja exemplos completos em [`examples/`](./examples) (FastAPI e Django).

## API

| Método | Descrição |
|---|---|
| `set(key, value, ttl=None)` | Grava. `ttl` em segundos (`int`/`float`); `None` = sem expiração; `ttl <= 0` → `ValueError`. Sobrescreve. |
| `get(key, default=None)` | Valor, ou `default` se ausente/expirado (remove se expirado). |
| `delete(key)` | `True` se removeu, `False` se não existia. |
| `exists(key)` | `True`/`False`, considerando TTL. |
| `keys()` | Lista de chaves (pode conter expiradas não coletadas). |
| `len()` / `len(cache)` | Tamanho aproximado. |
| `clear()` | Remove tudo (não zera contadores). |
| `cleanup_expired()` | Remove expirados; retorna quantos. |
| `stats()` | `dict` com `hits, misses, sets, deletes, expired, size`. |
| `@cache.cached(ttl=None, key=None)` | Decorator de memoização. |

## Como funciona

- **Serialização:** no MVP o valor é serializado com `pickle` (do lado do Python,
  via PyO3) e guardado como bytes opacos (`Vec<u8>`) no core Rust.
- **Concorrência:** `DashMap` (HashMap com lock por shard) + contadores `AtomicU64`,
  sem lock global no caminho quente. Thread-safe sem busy loop.
- **TTL:** expiração **lazy** — uma chave expirada é removida ao ser acessada
  (`get`/`exists`) ou em `cleanup_expired()`. Não há thread de background no MVP.

## Limitações

- Cache é **local ao processo**: múltiplos workers = múltiplos caches independentes.
- **Não** substitui Redis para cache distribuído.
- Dados são **perdidos** ao reiniciar o processo.
- `pickle` **não** deve ser usado para desserializar dados não confiáveis.
- TTL lazy: itens expirados podem permanecer até serem acessados ou até `cleanup_expired()`.

## Desenvolvimento

```bash
cargo test          # testes do core Rust
maturin develop     # recompila e instala
pytest              # testes Python
```

> Se `maturin develop` reclamar de `VIRTUAL_ENV` e `CONDA_PREFIX` setados ao mesmo
> tempo, rode `conda deactivate` antes, ou use `env -u CONDA_PREFIX maturin develop`.

## Roadmap

Etapas e próximos passos (eviction LRU/LFU, expiração em background, serializer
configurável, namespaces, etc.) em [ROADMAP.md](./ROADMAP.md).

## Licença

MIT
