# Tasken Justfile
#
# After 2026-06-11, this justfile is a thin shell that re-exports the shared
# `phenotype.just` library (defined in just/phenotype.just). The 9 most
# common recipes (default, build, test, lint, fmt, audit, unused, ci, docs)
# are now defined once in the library and parameterized over the build
# system.
#
# Stack-specific recipes (e.g. `clean`, `dev`) stay in this file.
#
# To upgrade: pull the latest phenotype.just from the central repo, or
# vendor it as a git submodule.

import "just/phenotype.just"

# Explicit recipe aliases for the required targets
# These are thin wrappers around the shared phenotype.just recipes.

check:
    @just typecheck

clippy:
    @if [ "{{build_system}}" = "cargo" ]; then cargo clippy --workspace --all-targets -- -D warnings; \
    else echo "no clippy for {{build_system}}"; fi

deny:
    @if [ "{{build_system}}" = "cargo" ]; then \
        (command -v cargo-deny >/dev/null && cargo deny check || echo "cargo-deny not installed; skip"); \
    else echo "no deny for {{build_system}}"; fi

doc:
    @just docs


