# Observability Remediation

This repository now has a minimal additive observability baseline:

- Structured logs are initialized from `src/main.rs` through `src/infrastructure/observability.rs`.
- Each CLI invocation gets a `request_id` correlation field via the root span `taskkit.process`.
- The CLI exposes `health` and `ready` commands for local probes.
- A lightweight in-process metrics hook tracks command and healthcheck counts.

## Applied diffs

- `Cargo.toml`
  - Added `tracing` and `tracing-subscriber` runtime dependencies.
- `src/infrastructure/observability.rs`
  - New module for subscriber setup, correlation IDs, and atomic metrics counters.
- `src/main.rs`
  - Installs logging, opens a root span, and emits a final metrics summary.
- `src/adapters/primary/cli.rs`
  - Adds `health` and `ready` subcommands.
  - Records command/health metrics.

## What is still intentionally not added

- No HTTP `/health` or `/ready` endpoints were added because the repository is CLI-first and does not expose a server process.
- No external metrics exporter was added. The hook is intentionally backend-agnostic so it can be wired into Prometheus, OTel, or internal telemetry later.
- No request tracing middleware was added because there is no inbound network service to wrap.

## Recommended next diffs if this becomes a daemon

1. Add a small HTTP status endpoint with `/health` and `/ready`.
2. Export the atomic metrics hook through a real backend.
3. Propagate `request_id` into any future API or job runner adapters.
