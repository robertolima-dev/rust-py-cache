# Changelog

Todas as mudanças relevantes deste projeto são registradas aqui.

O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
e o versionamento segue [SemVer](https://semver.org/lang/pt-BR/).

## [0.2.3] — não publicada

Versão de empacotamento e qualidade: nada muda no comportamento da biblioteca.

### Adicionado

- **`LICENSE`** (MIT). O projeto declarava MIT no `pyproject.toml` desde sempre,
  mas o texto da licença não existia no repositório nem na wheel (**P1-4**).
- **`clippy.toml` e `Makefile`** compartilhados com os outros quatro `rust_py_*`,
  e um job `lint` (`cargo fmt --check` + `cargo clippy -D warnings`) no `ci.yml`
  e no `release.yml` — este barrando a publicação, como o `test` já barrava
  (**P1-6**). O `cargo clippy` já estava limpo aqui; foi o único dos cinco.

### Corrigido

- **Metadado de licença ausente no PyPI.** O `pyproject.toml` usava
  `license = { text = "MIT" }`, forma legada que o Metadata 2.4 depreca. Passou
  para a do PEP 639 (`license = "MIT"` + `license-files = ["LICENSE"]`), e o
  classifier `License :: OSI Approved` saiu — ele é mutuamente exclusivo com
  `License-Expression`. A 0.2.2 no PyPI mostra `license: MIT` no campo legado e
  `license_expression: null`; a partir daqui é o contrário, que é o correto.

- **Piso da maturin subiu para 1.14** no `[build-system]`. Era `>=1.7`, e
  versões anteriores não emitem `License-Expression` — foi essa, e não a forma
  no `pyproject`, a causa raiz do metadado faltando.

### Declarado como Cargo

- `Cargo.toml` não tinha campo `license` nenhum. Agora tem `license = "MIT"`,
  como os outros quatro.

## [0.2.2] - 2026-08-11

Release de infraestrutura: nada muda no comportamento da biblioteca, mas muda
quem consegue instalá-la sem compilar.

### Corrigido

- **A publicação não roda mais sem teste.** O `ci.yml` roda em push/PR, mas é
  um workflow **separado**: um push de tag ia direto para build e publicação
  sem depender dele. O `release.yml` agora tem o seu próprio job `test` e todos
  os demais declaram `needs: test`. O job `publish` também ganhou
  `if: startsWith(github.ref, 'refs/tags/')` — sem essa guarda, um
  `workflow_dispatch` a partir de um branch publicaria no PyPI.
  (BACKLOG2 P1-1)
- **Cobertura de plataformas.** A 0.2.1 publicou 3 wheels — uma por runner
  (ubuntu, macos, windows) — e a de Linux herdava a glibc da máquina de build
  em vez de passar pelo container manylinux. Caíam no sdist, ou seja,
  precisavam de toolchain Rust: Linux aarch64, Alpine/musl e Mac Intel. Agora
  são 7 wheels (2 manylinux, 2 musllinux, 1 Windows, 2 macOS) mais o sdist.
- **O upload aguenta o 500 do PyPI.** O upload é arquivo por arquivo e sem
  transação — uma falha no meio deixa a versão publicada pela metade, e o PyPI
  nunca permite reusar um nome de arquivo. Agora há `skip-existing` (que é o
  que torna a segunda tentativa segura), uma segunda tentativa explícita,
  conferência do conjunto de artefatos **antes** de publicar e validação contra
  a API do PyPI **depois**. (BACKLOG2 P1-9)

### Alterado

- `crate-type` ganhou `rlib` além de `cdylib`, e `extension-module` virou
  feature opcional ligada por padrão. Assim
  `cargo test --no-default-features --lib` produz um binário de teste que linka
  no Linux, em vez de depender da tolerância do linker do macOS. É o mesmo
  arranjo dos outros quatro projetos. (BACKLOG2 P1-8)
- O `ci.yml` deixou de criar um virtualenv à mão para rodar `maturin develop`;
  agora usa `pip install ".[dev]"`, que aciona o mesmo backend maturin sem
  exigir virtualenv.

### Adicionado

- Extra `[project.optional-dependencies] dev`. Este era o único dos cinco
  projetos sem um. (BACKLOG2 P2-9)

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
