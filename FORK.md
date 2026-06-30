# FORK.md — Tasken fork governance

## Upstream

Tasken is a Phenotype-internal fork that extends the `taskkit` crate namespace.
There is no single public upstream; the hexagonal architecture, async runtime,
and CI baseline are Phenotype originals.

## Local delta (what this fork adds)

| Area | Description |
|------|-------------|
| `cron_parser.rs` | Full cron expression parser wrapping the `cron-parser` crate |
| `domain/events.rs` | Event-sourcing model (`TaskEvent` enum + append/load ports) |
| `domain/rate_limiter.rs` | Token-bucket admission throttle |
| `infrastructure/cache.rs` + `persistent_cache.rs` | TTL-aware in-memory and disk-backed result caches |
| `infrastructure/observability.rs` | Structured logging (JSON/compact) via `tracing-subscriber` |
| `infrastructure/otel.rs` | Optional OpenTelemetry span layer (feature `otel`) |
| `adapters/secondary/file.rs` | JSON-on-disk storage adapter |
| Justfile + `phenotype.just` recipes | Shared Phenotype build/verify/release targets |
| SLSA / release attestation workflows | Provenance + SBOM generation in CI |
| `AGENTS.md` / `CLAUDE.md` / `VERIFICATION_POLICY.md` | Agent-readiness and verification surface |

## Language policy

- **Rust** is the primary language for all runtime and library code.
- **Python** (`python/`) is used only for doc-tooling scripts; no new Python
  runtime surfaces without an explicit ADR.
- **TypeScript** (`package.json`) is used only for VitePress docs; no new TS
  surfaces without an explicit ADR.

## Security model

Tasken is a **local-only, single-operator CLI and library** with no network
server, multi-user surface, or credential storage.

- No authentication or authorization is required or implemented.
- No tenant isolation is required or implemented.
- No cryptographic key management is in scope.
- If Tasken is ever exposed as a multi-user service, an ADR must be filed
  covering authn/authz, tenant scoping, and a STRIDE threat model before
  shipping.

## Sync cadence

There is no external upstream to track.  Internal changes are reviewed in
the normal PR flow; breaking changes to the `StoragePort` / `QueuePort`
traits require a semver bump and an ADR entry.

## Known gaps / roadmap

| Gap | Tracking |
|-----|----------|
| `FileStorage::load_events` does not deserialize — requires `Deserialize` on `TaskEvent` | Follow-up PR |
| `pheno-otel` optional dep not yet published; feature `otel` omits it until then | See `Cargo.toml` comment |
| OTel `init_tracer` exporter is a stub — wire to stdout/OTLP exporter | Follow-up PR |
| Journey manifests in `docs/journeys/manifests/` — CI gate currently stub | Follow-up PR |

## Attribution

All code in this repository is authored by the Phenotype organization
(`dev@kooshapari.com`). See `CHANGELOG.md` for per-release attribution.
