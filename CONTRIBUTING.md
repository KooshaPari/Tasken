# Contributing to Tasken

We welcome contributions! This document outlines the development workflow,
prerequisites, and quality gates for contributing.

## Prerequisites

- **Rust toolchain**: Install via [rustup](https://rustup.rs/) (MSRV: 1.75)
- **`just`** (recommended): `cargo install just` — provides dev recipes
- **`cargo-deny`** (optional): `cargo install cargo-deny` — license/advisory checks
- **`cargo-llvm-cov`** (optional): `cargo install cargo-llvm-cov` — coverage

## Bootstrap

```bash
# 1. Clone the repository
git clone git@github.com:KooshaPari/Tasken.git
cd Tasken

# 2. Verify the build works
cargo build --workspace

# 3. Run the full test suite
cargo test --workspace

# 4. (Optional) Run the full CI sweep via `just`
just ci
```

## Development Workflow

1. **Create a feature branch** from `main`:
   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make changes** with clear, focused commits. Follow the existing code
   style (see below).

3. **Run quality checks** before pushing:
   ```bash
   # Build
   cargo build --workspace --all-targets

   # Test
   cargo test --workspace

   # Lint (Clippy + fmt)
   cargo clippy --workspace --all-targets -- -D warnings
   cargo fmt --check

   # Or use the `just` shortcut:
   just ci
   ```

4. **Submit a Pull Request** with a clear description of changes, testing
   done, and related issues.

## Code Style

- Formatting: 2 spaces, 100-char width — enforced by `cargo fmt`
- Follow Rust 2021 idioms
- All public items must have doc comments
- Errors use `thiserror`; use `anyhow` for application-level context
- Tests should be co-located (`#[cfg(test)]`) or in `tests/` for
  integration tests

## Testing Requirements

- All existing tests **must pass** before a PR is merged
- New functionality should include unit tests (inline `#[cfg(test)]`)
- Cross-module behaviour needs integration tests in `tests/`
- Coverage gate: 85% line coverage (`cargo llvm-cov --workspace --fail-under-lines 85`)
- Property-based tests and fuzz targets are welcome for complex logic

## Pull Request Process

1. Ensure your branch is up-to-date with `main`
2. Run `cargo test --workspace` and `cargo clippy -D warnings` — they **must** pass
3. Update documentation if public APIs change
4. The PR template includes a checklist — fill it out
5. A maintainer will review your PR

## Governance

- Maintainers review all PRs before merge
- Release tags follow SemVer (`vMAJOR.MINOR.PATCH`)
- Changelog entries are generated from conventional commit messages
- Security issues: see [SECURITY.md](SECURITY.md) for responsible disclosure
