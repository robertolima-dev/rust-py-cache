# Publicação — TestPyPI e PyPI (Etapas 22 e 23)

> Estas etapas exigem **suas credenciais** e publicam artefatos externos. Por isso
> ficam documentadas aqui e **não** são executadas automaticamente. Rode você,
> conscientemente, quando a v0.1 estiver pronta.

## Pré-requisitos

```bash
pip install maturin twine
```

Tenha contas em https://test.pypi.org e https://pypi.org e gere um **API token**
em cada uma (Account settings → API tokens).

## 1. Build dos artefatos

```bash
# wheel(s) abi3 + sdist na pasta dist/
maturin build --release --out dist
maturin sdist --out dist
```

## 2. Publicar no TestPyPI (Etapa 22 — ensaio)

```bash
twine upload --repository testpypi dist/*
# usuário: __token__   |   senha: o token do TestPyPI (começa com "pypi-")
```

Teste a instalação a partir do TestPyPI num venv limpo:

```bash
pip install --index-url https://test.pypi.org/simple/ \
            --extra-index-url https://pypi.org/simple/ \
            rust-py-cache
python -c "from rust_py_cache import Cache; c=Cache(); c.set('k',1); print(c.get('k'))"
```

## 3. Publicar no PyPI (Etapa 23 — produção)

```bash
twine upload dist/*
# usuário: __token__   |   senha: o token do PyPI
```

## Alternativa recomendada: release automático por tag

O workflow [`.github/workflows/release.yml`](.github/workflows/release.yml) já faz
build multi-plataforma e publica no PyPI via **Trusted Publishing (OIDC)** — sem
token no repositório. Para usar:

1. Em https://pypi.org → projeto → *Publishing*, registre o publisher confiável
   (owner/repo, workflow `release.yml`, environment `pypi`).
2. Bump da versão em `Cargo.toml` **e** `pyproject.toml` (manter iguais).
3. Crie e publique a tag:

   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

O workflow constrói as wheels (Linux/macOS/Windows) + sdist e publica.

## Checklist de versão

- [ ] `version` igual em `Cargo.toml` e `pyproject.toml`
- [ ] `__version__` em `python/rust_py_cache/__init__.py`
- [ ] `cargo test` e `pytest` verdes
- [ ] README atualizado
- [ ] Tag `vX.Y.Z` criada
