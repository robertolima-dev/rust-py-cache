# ROADMAP — rust-py-cache

> **An ultra-fast local cache for Python, powered by Rust.**

Cache local, em memória, thread-safe, com TTL, expiração automática e métricas.
Core em Rust (PyO3 + maturin), API simples em Python. Um "mini Redis local" dentro
do processo Python.

```python
from rust_py_cache import Cache

cache = Cache()
cache.set("user:1", {"name": "Roberto"}, ttl=60)
user = cache.get("user:1")
```

---

## Princípios da mentoria

- Avançar **etapa por etapa**; não entregar tudo de uma vez.
- Sempre dizer **em qual arquivo** cada código entra e dar **comandos de terminal**.
- Sempre explicar conceitos PyO3 e Rust (ownership, borrowing, `Arc`, `Mutex`,
  `DashMap`, `AtomicU64`, GIL) quando aparecerem.
- Sempre **criar testes antes de avançar**.
- API Python simples; sem overengineering no MVP; preservar compatibilidade.

---

## Decisões de arquitetura

### Serialização (decidido)
- **MVP: `pickle` como serializer padrão.** Prioriza ergonomia Python (suporta
  quase qualquer objeto). O core Rust trata o valor como `Vec<u8>` opaco.
- **Segurança:** documentar que `pickle` **não** deve desserializar dados não
  confiáveis.
- **Futuro (v0.2):** `Cache(serializer="json")` e `Cache(serializer="pickle")`.
  Trocar o serializer não altera o core (continua `Vec<u8>`).

### Layout do projeto (mixed Rust + Python via maturin)
```txt
rust-py-cache/
├── Cargo.toml
├── pyproject.toml
├── README.md
├── LICENSE
├── ROADMAP.md
├── src/                      # core Rust
│   ├── lib.rs                # #[pymodule] _rust_py_cache + #[pyclass] Cache
│   ├── cache.rs              # RustCache (DashMap + stats)
│   ├── entry.rs              # CacheEntry
│   ├── ttl.rs                # helpers de tempo/TTL
│   ├── stats.rs              # CacheStats (AtomicU64)
│   ├── errors.rs             # erros -> exceções Python
│   ├── serializer.rs         # pickle/json
│   └── eviction.rs           # (futuro) LRU/LFU
├── python/
│   └── rust_py_cache/
│       ├── __init__.py       # reexporta Cache do módulo nativo
│       ├── decorators.py     # @cache.cached
│       ├── fastapi.py        # helpers FastAPI
│       └── django.py         # backend Django
├── tests/                    # pytest
└── examples/
```

O módulo nativo é compilado como `rust_py_cache._rust_py_cache`; o `__init__.py`
reexporta `Cache` para que `from rust_py_cache import Cache` funcione.

### Modelo interno (Rust)
```rust
pub struct CacheEntry {
    pub value: Vec<u8>,            // bytes serializados (pickle)
    pub expires_at: Option<u64>,  // epoch em ms; None = sem expiração
    pub created_at: u64,
    pub last_accessed_at: u64,
    pub hits: u64,
}

pub struct RustCache {
    store: DashMap<String, CacheEntry>,
    stats: CacheStats,
}

pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub sets: AtomicU64,
    pub deletes: AtomicU64,
    pub expired: AtomicU64,
}
```

---

## API Python alvo

```python
cache.set(key, value, ttl=None)   # ttl em segundos (int ou float); None = sem expiração
cache.get(key, default=None)      # None/default se ausente ou expirado (remove se expirado)
cache.delete(key)                 # True se removeu, False se não existia
cache.exists(key)                 # considera TTL
cache.clear()                     # remove tudo
cache.len()                       # tamanho (pode incluir expirados não coletados)
cache.stats()                     # dict: hits, misses, sets, deletes, expired, size
cache.keys()                      # lista de chaves
cache.cleanup_expired()           # remove expirados; retorna nº removidos
```

### Regras de comportamento
- `get()` → `None`/`default` se a chave não existir.
- Chave expirada: `get()` remove a chave e retorna `default`/`None` (expiração **lazy**).
- `exists()` considera TTL.
- `len()` documentado como tamanho aproximado (pode conter expirados ainda não coletados).
- `delete()` → `True`/`False`.
- `set()` sobrescreve chave existente.
- `ttl=None` = sem expiração; **`ttl <= 0` → `ValueError`** (decisão explícita).
- Thread-safe, sem busy loop; no MVP a expiração é lazy (sem thread em background).

---

## Etapas — MVP (v0.1)

| # | Etapa | Status |
|---|-------|--------|
| 1 | Projeto maturin/PyO3 + `hello()` + build local | ✅ feito |
| 2 | Configurar `Cargo.toml` (pyo3, dashmap, thiserror; serde/serde_json/time adiados p/ v0.2) | ✅ feito |
| 3 | `import rust_py_cache; rust_py_cache.hello()` | ✅ (parte da Etapa 1) |
| 4 | `#[pyclass] Cache` (+ `len`/`__len__`/`clear`/`__repr__`) | ✅ feito |
| 5 | `set(key, value, ttl=None)` | ✅ feito |
| 6 | `get(key, default=None)` | ✅ feito |
| 7 | `delete(key)` | ✅ feito |
| 8 | `exists(key)` | ✅ feito |
| 9 | `clear()` | ✅ (Etapa 4) |
| 10 | `len()` | ✅ (Etapa 4) |
| 11 | TTL lazy | ✅ feito (lazy expiry em `get`/`exists` + `cleanup_expired`) |
| 12 | Stats com `AtomicU64` | ✅ feito (`src/stats.rs`) |
| 13 | `stats()` | ✅ feito |
| 14 | `keys()` | ✅ feito |
| 15 | `cleanup_expired()` | ✅ feito |
| 16 | Testes pytest | ✅ feito (25 pytest + 6 cargo test) |
| 17 | Decorator `@cache.cached(ttl=60)` | ✅ feito (`decorators.py`) |
| 18 | Exemplo FastAPI | ✅ feito (`examples/fastapi_example.py`) |
| 19 | Exemplo Django | ✅ feito (`examples/django_example.py`) |
| 20 | README | ✅ feito |
| 21 | GitHub Actions (cargo test, pytest, maturin build) | ✅ feito (`.github/workflows/`) |
| 22 | Publicar no TestPyPI | 📋 pronto, requer ação do usuário ([PUBLISHING.md](./PUBLISHING.md)) |
| 23 | Publicar no PyPI | 📋 pronto, requer ação do usuário ([PUBLISHING.md](./PUBLISHING.md)) |

### Testes obrigatórios (pytest)
set/get; get inexistente; get com default; delete existente/inexistente; exists;
clear; len; TTL expira; TTL sem expiração; sobrescrever; stats hits/misses/sets/
deletes/expired; cleanup_expired; objetos Python complexos; concorrência com
threads; decorator básico.

---

## v0.2.0 e além (futuro)

- Expiração automática em background (sem busy loop)
- LRU / LFU eviction; `max_size`; `max_memory`
- Namespaces
- Async decorator; FastAPI dependency; Django backend compatibility
- Persistência opcional em arquivo / snapshot
- Métricas Prometheus; compressão; serializer customizável (`json`/`pickle`)
- Batch: `mget`/`mset`; atomic `incr`/`decr`
- Invalidação distribuída

---

## Limitações (documentar no README)

- Cache é **local ao processo**; múltiplos workers = múltiplos caches independentes.
- Não substitui Redis para cache distribuído.
- Dados são perdidos ao reiniciar o processo.
- `pickle` não deve ser usado com dados não confiáveis.
- TTL lazy: itens expirados podem permanecer até serem acessados ou até `cleanup_expired()`.

---

## Stack

Rust • PyO3 • maturin • Python 3.10+ • serde / serde_json • dashmap •
thiserror • time • pytest • GitHub Actions • TestPyPI • PyPI
