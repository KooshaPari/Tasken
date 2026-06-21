# phenotype.just — shared recipe library for Phenotype-org repositories
#
# Usage (in a consumer repo's justfile):
#     import "phenotype.just"
#
# All recipes here are workspace-agnostic; they auto-detect the build system
# (cargo / npm / pnpm / yarn / bun / uv / poetry / go / mix) and run the
# appropriate command. Recipes that are stack-specific (e.g. `register-startmenu`
# for Electrobun) live in the consumer's justfile, NOT here.
#
# Versioning: SemVer. Breaking a recipe signature is a major bump.

# === variables ===

# Detect the build system by walking up from the justfile dir.
# Returns one of: cargo | npm | pnpm | yarn | bun | uv | poetry | go | mix | none
build_system := `\
  if [ -f Cargo.toml ]; then echo cargo; \
  elif [ -f package.json ] && [ -f pnpm-lock.yaml ]; then echo pnpm; \
  elif [ -f package.json ] && [ -f yarn.lock ]; then echo yarn; \
  elif [ -f package.json ] && [ -f bun.lockb ]; then echo bun; \
  elif [ -f package.json ] && [ -f bun.lock ]; then echo bun; \
  elif [ -f package.json ]; then echo npm; \
  elif [ -f pyproject.toml ] && [ -f uv.lock ]; then echo uv; \
  elif [ -f pyproject.toml ] && [ -f poetry.lock ]; then echo poetry; \
  elif [ -f pyproject.toml ]; then echo uv; \
  elif [ -f go.mod ]; then echo go; \
  elif [ -f mix.exs ]; then echo mix; \
  else echo none; fi`

# Recipe to invoke for "build".
build_cmd := if build_system == "cargo" { "cargo build --workspace" } \
             else if build_system == "pnpm" { "pnpm -r build" } \
             else if build_system == "yarn" { "yarn workspaces run build" } \
             else if build_system == "bun" { "bun run build" } \
             else if build_system == "npm" { "npm run build --workspaces --if-present" } \
             else if build_system == "uv" { "uv sync --all-extras" } \
             else if build_system == "poetry" { "poetry install --all-extras" } \
             else if build_system == "go" { "go build ./..." } \
             else if build_system == "mix" { "mix compile" } \
             else { "echo 'no build system detected' && exit 1" }

# Recipe to invoke for "test".
test_cmd := if build_system == "cargo" { "cargo test --workspace" } \
            else if build_system == "pnpm" { "pnpm -r test" } \
            else if build_system == "yarn" { "yarn workspaces run test" } \
            else if build_system == "bun" { "bun test" } \
            else if build_system == "npm" { "npm test --workspaces --if-present" } \
            else if build_system == "uv" { "uv run pytest" } \
            else if build_system == "poetry" { "poetry run pytest" } \
            else if build_system == "go" { "go test ./..." } \
            else if build_system == "mix" { "mix test" } \
            else { "echo 'no test runner detected' && exit 1" }

# Recipe to invoke for "lint".
lint_cmd := if build_system == "cargo" { "cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check" } \
            else if build_system == "pnpm" { "pnpm -r lint && pnpm -r format:check" } \
            else if build_system == "yarn" { "yarn workspaces run lint" } \
            else if build_system == "bun" { "bun run lint" } \
            else if build_system == "npm" { "npm run lint --workspaces --if-present" } \
            else if build_system == "uv" { "uv run ruff check . && uv run ruff format --check ." } \
            else if build_system == "poetry" { "poetry run ruff check . && poetry run ruff format --check ." } \
            else if build_system == "go" { "go vet ./... && gofmt -l ." } \
            else if build_system == "mix" { "mix format --check-formatted && mix credo --strict" } \
            else { "echo 'no linter detected' && exit 1" }

# === core recipes ===

# Default: list recipes.
default:
    @just --list -u

# Build the workspace.
build:
    {{build_cmd}}

# Run the test suite.
test:
    {{test_cmd}}

# Lint (linter + formatter check).
lint:
    {{lint_cmd}}

# Auto-format all files in place.
fmt:
    @just _fmt

# Stack-specific formatters.
_fmt:
    @if [ "{{build_system}}" = "cargo" ]; then cargo fmt; \
    elif [ "{{build_system}}" = "pnpm" ]; then pnpm -r format; \
    elif [ "{{build_system}}" = "yarn" ]; then yarn workspaces run format; \
    elif [ "{{build_system}}" = "bun" ]; then bun run fmt; \
    elif [ "{{build_system}}" = "npm" ]; then npm run format --workspaces --if-present; \
    elif [ "{{build_system}}" = "uv" ]; then uv run ruff format .; \
    elif [ "{{build_system}}" = "poetry" ]; then poetry run ruff format .; \
    elif [ "{{build_system}}" = "go" ]; then gofmt -w .; \
    elif [ "{{build_system}}" = "mix" ]; then mix format; \
    else echo "no formatter detected for {{build_system}}"; fi

# Security audits. Stack-specific.
audit:
    @just _audit

_audit:
    @if [ "{{build_system}}" = "cargo" ]; then \
        (command -v cargo-deny >/dev/null && cargo deny check || echo "cargo-deny not installed; skip") && \
        (command -v cargo-audit >/dev/null && cargo audit || echo "cargo-audit not installed; skip"); \
    elif [ "{{build_system}}" = "uv" ] || [ "{{build_system}}" = "poetry" ]; then \
        uv run pip-audit 2>/dev/null || poetry run pip-audit 2>/dev/null || echo "pip-audit not installed; skip"; \
    elif [ "{{build_system}}" = "pnpm" ]; then pnpm audit --prod; \
    elif [ "{{build_system}}" = "bun" ]; then bun audit; \
    elif [ "{{build_system}}" = "npm" ]; then npm audit --omit=dev; \
    elif [ "{{build_system}}" = "go" ]; then govulncheck ./... 2>/dev/null || echo "govulncheck not installed; skip"; \
    else echo "no audit tool for {{build_system}}"; fi

# Find unused dependencies. Stack-specific.
unused:
    @just _unused

_unused:
    @if [ "{{build_system}}" = "cargo" ]; then \
        (command -v cargo-machete >/dev/null && cargo machete || echo "cargo-machete not installed; skip"); \
    elif [ "{{build_system}}" = "uv" ] || [ "{{build_system}}" = "poetry" ]; then \
        uv run deptry . 2>/dev/null || poetry run deptry . 2>/dev/null || echo "deptry not installed; skip"; \
    elif [ "{{build_system}}" = "pnpm" ]; then pnpm dlx depcheck; \
    else echo "no unused-dep tool for {{build_system}}"; fi

# Type-check. Stack-specific.
typecheck:
    @just _typecheck

_typecheck:
    @if [ "{{build_system}}" = "cargo" ]; then cargo check --workspace --all-targets; \
    elif [ "{{build_system}}" = "uv" ] || [ "{{build_system}}" = "poetry" ]; then \
        (uv run mypy . 2>/dev/null || poetry run mypy . 2>/dev/null || echo "mypy not installed; skip"); \
    elif [ "{{build_system}}" = "pnpm" ]; then pnpm -r typecheck; \
    elif [ "{{build_system}}" = "bun" ]; then bun run typecheck 2>/dev/null || echo "no typecheck script"; \
    elif [ "{{build_system}}" = "npm" ]; then npm run typecheck --workspaces --if-present; \
    elif [ "{{build_system}}" = "go" ]; then go build ./... && go vet ./...; \
    else echo "no typechecker for {{build_system}}"; fi

# Full local CI sweep: lint + test + audit + unused + typecheck.
ci: lint typecheck test audit unused
    @echo "✓ all CI checks passed"

# Generate docs.
docs:
    @just _docs

_docs:
    @if [ "{{build_system}}" = "cargo" ]; then cargo doc --no-deps --workspace; \
    elif [ "{{build_system}}" = "uv" ] || [ "{{build_system}}" = "poetry" ]; then \
        uv run sphinx-build -b html docs/ docs/_build/ 2>/dev/null || \
        poetry run sphinx-build -b html docs/ docs/_build/ 2>/dev/null || \
        echo "no sphinx config; skip"; \
    else echo "no doc generator for {{build_system}}"; fi

# === developer-experience recipes ===

# Watch mode: rebuild on change. Stack-specific.
dev:
    @just _dev

_dev:
    @if [ "{{build_system}}" = "cargo" ]; then \
        (command -v cargo-watch >/dev/null && cargo watch -x build -x test || \
         echo "cargo-watch not installed; install with: cargo install cargo-watch"); \
    elif [ "{{build_system}}" = "bun" ]; then bun run dev; \
    elif [ "{{build_system}}" = "pnpm" ]; then pnpm -r dev; \
    elif [ "{{build_system}}" = "npm" ]; then npm run dev --workspaces --if-present; \
    else echo "no dev script for {{build_system}}"; fi

# Clean build artifacts.
clean:
    @just _clean

_clean:
    @if [ "{{build_system}}" = "cargo" ]; then cargo clean; \
    elif [ "{{build_system}}" = "uv" ] || [ "{{build_system}}" = "poetry" ]; then \
        rm -rf .venv dist build *.egg-info .pytest_cache .mypy_cache .ruff_cache; \
    elif [ "{{build_system}}" = "pnpm" ]; then pnpm clean 2>/dev/null || rm -rf node_modules; \
    elif [ "{{build_system}}" = "bun" ]; then rm -rf node_modules dist build; \
    elif [ "{{build_system}}" = "npm" ]; then rm -rf node_modules dist build; \
    elif [ "{{build_system}}" = "go" ]; then go clean -cache -testcache; \
    elif [ "{{build_system}}" = "mix" ]; then mix clean; \
    else rm -rf build dist; fi

# Print detected build system (debug).
info:
    @echo "build_system={{build_system}}"
    @echo "build_cmd={{build_cmd}}"
    @echo "test_cmd={{test_cmd}}"
    @echo "lint_cmd={{lint_cmd}}"
