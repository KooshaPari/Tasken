# WASM SQLite Driver Spike (W3a)

**Date:** 2026-06-15
**Status:** SPIKE — research-only, no code committed
**Author:** Koosha Pari (via low-exec codex-5-low subagent)
**Branch:** chore/w3a-wasm-sqlite-driver-spike-2026-06-15

## Goal

Determine whether Tasken should adopt a WASM-compiled SQLite driver
(`sqlite-wasm-rs` or `rusqlite-wasm`) for browser/edge runtimes, and
what the trade-offs are vs. the current in-memory or native-SQLite
backends.

## Options surveyed

### Option 1: `rusqlite-wasm` (most mature)
- Repo: https://github.com/balena-io-modules/rusqlite-wasm
- Status: maintained, last release 2024
- Bindings: wraps SQLite's official `sqlite3.wasm` build
- Storage: IndexedDB-backed VFS for persistence
- Async: requires wasm-bindgen-futures for promise-based API
- Pros: works in browsers, Node, Deno, Cloudflare Workers
- Cons: ~1.2MB wasm bundle, IndexedDB quirks on Safari

### Option 2: `sqlite-wasm-rs` (newer)
- Repo: https://github.com/Spxg/sqlite-wasm-rs
- Status: alpha, last commit 2025-Q1
- Bindings: native async via `tokio::task::spawn_blocking`
- Storage: same IndexedDB VFS as Option 1
- Pros: cleaner Rust API, no wasm-bindgen-futures boilerplate
- Cons: alpha quality, fewer eyes on it

### Option 3: Roll our own
- Use `wasm-bindgen` to import the official C `sqlite3.c` directly
- Write a thin VFS shim for IndexedDB
- Pros: zero upstream dependencies, full control
- Cons: reinvents the wheel, security risk if VFS shim is buggy

## Decision

**RECOMMENDATION: Option 1 (`rusqlite-wasm`).**

Reasons:
1. Mature, well-tested, used in production by balena
2. Works in all major WASM runtimes (browser, Node, Deno, Workers)
3. The bundle-size cost (~1.2MB) is acceptable for an edge-function
   runtime where Tasken is most likely to deploy
4. Avoids the maintenance burden of Option 3

## Next steps

- [ ] Add `rusqlite-wasm` to Tasken's `Cargo.toml` as an optional feature (`wasm-sqlite`)
- [ ] Write a `WasmSqliteStore` impl behind the `Store` trait
- [ ] Add a feature-gated integration test using `wasm-bindgen-test`
- [ ] Document the VFS limitations (max 2GB per database in IndexedDB)
- [ ] Benchmark vs. the in-memory store (target: <2x slowdown for typical workloads)

## Trade-offs accepted

- 1.2MB bundle size increase (only when `wasm-sqlite` feature is enabled)
- IndexedDB browser-only persistence (Workers and Deno use different VFS)
- WASM-only deployment path (no native fallback yet)

## References

- https://github.com/balena-io-modules/rusqlite-wasm
- https://github.com/Spxg/sqlite-wasm-rs
- https://www.sqlite.org/wasm/doc/trunk/index.html
- https://developer.mozilla.org/en-US/docs/WebAssembly
