# Phenotype-org standard justfile

default:
    @just --list

build:
    cargo build --workspace

test:
    cargo test --workspace

lint:
    cargo clippy --workspace -- -D warnings
    cargo fmt --check

fmt:
    cargo fmt

audit:
    cargo deny check
    cargo audit

unused:
    cargo machete

ci: lint test audit unused

docs:
    cargo doc --no-deps --workspace
