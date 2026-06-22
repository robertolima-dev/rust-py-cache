# rust-py-cache

> **An ultra-fast local cache for Python, powered by Rust.**

A local, in-memory, thread-safe cache with TTL, lazy expiration, and metrics. The
core is written in Rust (PyO3 + maturin) on top of a concurrent `DashMap`; the
Python API is minimal. Think of it as a "mini Redis" living **inside** your Python
process.

[![PyPI](https://img.shields.io/pypi/v/rust-py-cache)](https://pypi.org/project/rust-py-cache/)
[![Python](https://img.shields.io/pypi/pyversions/rust-py-cache)](https://pypi.org/project/rust-py-cache/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow)](./LICENSE)

## Installation

```bash
pip install rust-py-cache
```

To work on it locally (requires Rust + maturin):

```bash
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
maturin develop          # compiles the Rust core and installs into the venv
pytest                   # runs the tests
```

## Usage

```python
from rust_py_cache import Cache

cache = Cache()

cache.set("user:1", {"name": "Roberto"}, ttl=60)   # ttl in seconds
user = cache.get("user:1")                          # {"name": "Roberto"}
cache.get("missing", default=0)                     # 0

cache.exists("user:1")        # True (honors TTL)
cache.delete("user:1")        # True if removed, False if absent
cache.len()                   # approximate size
cache.keys()                  # list of keys
cache.cleanup_expired()       # remove expired entries; returns the count
cache.clear()                 # remove everything (keeps counters)
cache.stats()                 # {'hits', 'misses', 'sets', 'deletes', 'expired', 'size'}
```

### Memoization decorator

```python
@cache.cached(ttl=60)
def add(a, b):
    return a + b

add(2, 3)   # runs and caches
add(2, 3)   # served from cache

# custom key (fixed string or callable):
@cache.cached(ttl=300, key=lambda user_id: f"user:{user_id}")
def load_user(user_id):
    ...
```

See full examples under [`examples/`](./examples) (FastAPI and Django).

## API

| Method | Description |
|---|---|
| `set(key, value, ttl=None)` | Store a value. `ttl` in seconds (`int`/`float`); `None` = no expiration; `ttl <= 0` → `ValueError`. Overwrites. |
| `get(key, default=None)` | The value, or `default` if missing/expired (expired entries are removed). |
| `delete(key)` | `True` if removed, `False` if it didn't exist. |
| `exists(key)` | `True`/`False`, honoring TTL. |
| `keys()` | List of keys (may include expired-but-not-yet-collected ones). |
| `len()` / `len(cache)` | Approximate size. |
| `clear()` | Remove everything (does not reset counters). |
| `cleanup_expired()` | Remove expired entries; returns how many. |
| `stats()` | `dict` with `hits, misses, sets, deletes, expired, size`. |
| `@cache.cached(ttl=None, key=None)` | Memoization decorator. |

## How it works

- **Serialization:** in the MVP, values are serialized with `pickle` (on the Python
  side, via PyO3) and stored as opaque bytes (`Vec<u8>`) in the Rust core.
- **Concurrency:** `DashMap` (a HashMap with per-shard locks) plus `AtomicU64`
  counters, with no global lock on the hot path. Thread-safe, no busy loop.
- **TTL:** expiration is **lazy** — an expired key is removed when accessed
  (`get`/`exists`) or via `cleanup_expired()`. There is no background thread in the MVP.

## Limitations

- The cache is **process-local**: multiple workers = multiple independent caches.
- It does **not** replace Redis for distributed caching.
- Data is **lost** when the process restarts.
- `pickle` must **not** be used to deserialize untrusted data.
- Lazy TTL: expired items may linger until accessed or until `cleanup_expired()`.

## Development

```bash
cargo test          # Rust core tests
maturin develop     # rebuild and install
pytest              # Python tests
```

> If `maturin develop` complains about both `VIRTUAL_ENV` and `CONDA_PREFIX` being
> set, run `conda deactivate` first, or use `env -u CONDA_PREFIX maturin develop`.

## Roadmap

Stages and next steps (LRU/LFU eviction, background expiration, configurable
serializer, namespaces, etc.) are in [ROADMAP.md](./ROADMAP.md).

## License

MIT
