# Roadmap — rust-py-cache

Direction for `rust-py-cache`: an ultra-fast, process-local cache for Python with
a Rust core (PyO3 + maturin). The MVP is intentionally small; this document tracks
the stages already shipped and what comes next.

> Status legend: ✅ shipped · 🔜 planned (next) · 💡 idea (no version yet) · ⚠️ note

---

## Shipped — v0.1.x (current: 0.1.2)

- ✅ `Cache` with `set` / `get(default=...)` / `delete` / `exists` / `keys` /
  `len` / `clear` / `cleanup_expired` / `stats`.
- ✅ TTL per entry with **lazy expiration** (expired keys are dropped on access or
  via `cleanup_expired()`).
- ✅ Cumulative `stats()` (`hits, misses, sets, deletes, expired, size`) backed by
  `AtomicU64` — no global lock on the hot path.
- ✅ Memoization decorator `@cache.cached(ttl=..., key=...)` (fixed string or
  callable key).
- ✅ Rust core on `DashMap` (per-shard locks); values serialized with `pickle` and
  stored as opaque bytes (`Vec<u8>`).
- ✅ Examples for FastAPI and Django.

---

## Planned

### v0.2 — Background expiration

- 🔜 Optional background sweeper thread to reclaim expired entries proactively,
  configurable via the constructor (e.g. `Cache(cleanup_interval=...)`), instead
  of relying only on lazy expiration / manual `cleanup_expired()`.
- 🔜 Keep the sweeper opt-in and lock-friendly (sweep shard by shard) so it never
  stalls the hot path.

### v0.3 — LRU eviction

- 🔜 `Cache(max_size=...)` with **least-recently-used** eviction once the cap is hit.
- 🔜 Track recency cheaply (avoid a global lock); surface evictions in `stats()`
  (e.g. an `evicted` counter).

### v0.4 — LFU eviction

- 🔜 **Least-frequently-used** eviction policy as an alternative to LRU, selectable
  at construction time (e.g. `Cache(max_size=..., policy="lfu")`).

### v0.5 — Configurable serializer

- 🔜 Pluggable serialization instead of hard-coded `pickle`: e.g.
  `Cache(serializer="json" | "pickle" | "msgpack")` or a custom
  encode/decode pair.
- ⚠️ Security: `pickle` must not deserialize untrusted data — a JSON/MessagePack
  option makes the cache safe to share across trust boundaries.

### v0.6 — Namespaces

- 🔜 Logical key namespaces (e.g. `cache.namespace("users")`) so independent
  features can share one `Cache` instance without key collisions, with per-namespace
  `clear()` and stats.

---

## Ideas / future (no version assigned)

- 💡 Prometheus metrics exporter (align with `rust-py-monitor`).
- 💡 Redis synchronization / a distributed tier (today the cache is process-local).
- 💡 ImmutableLog integration for eviction events
  (`{"event_type": "cache_evicted", "key": ..., "reason": ...}`).
- 💡 Binary value formats end-to-end (MessagePack / CBOR / Bincode) for smaller,
  faster payloads.
- 💡 Benchmarks suite (throughput vs. `dict`, `cachetools`, in-process Redis).

---

## Known limitations (by design, for now)

- Process-local: multiple workers = multiple independent caches.
- Not a replacement for Redis in distributed setups.
- Data is lost on process restart.
- Lazy TTL: expired items may linger until accessed or `cleanup_expired()` runs
  (addressed by v0.2).

---

## Contributing to the roadmap

Versions and ordering are indicative and may shift. Bump the version in **both**
`Cargo.toml` and `pyproject.toml` (kept in sync) plus `__version__`, ship tests
(`cargo test` + `pytest`), then tag `vX.Y.Z` to trigger the release workflow.
