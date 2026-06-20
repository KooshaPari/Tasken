# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- L7-001 intent + boundary snapshot documentation
- Parallel wave-based DAG execution engine for workflows
- Persistent task cache with TTL-based result caching
- Cron expression parser (W3b spike, zero external dependencies)
- SOTA feature implementation: argument forwarding, DAG dependency resolution, persistent cache, stdout/stderr stream separation, contextual error types
- CLI framework via clap-ext integration
- Coverage tooling recipes (Justfile + Taskfile.yml)
- Journey traceability and iconography system
- SLSA Build L2 attestation workflow
- SUPPORT.md and SECURITY.md documentation for issue routing
- OpenSSF Scorecard policy with issue templates
- GitHub Discussions ideas template
- AI-DD metadata badge block
- SPEC.md and PLAN.md project documentation
- Deep journeys, stories, and traceability documentation
- Additional CI workflows: Dependabot, pre-commit, benchmarks, coverage, QA matrices
- Justfile targets and CI/release workflow definitions

### Changed
- Refreshed L7-001 intent + boundary snapshot for cross-repo consistency
- Recorded phenoForge absorption and forge stub supersession in project history
- Formatted CLI source with cargo fmt (long match arm line-wrapping)
- Moved domain/tasks test module to bottom of file per Rust conventions
- WASM SQLite driver research spike (W3a)
- Updated cron-parser dependency from 0.8 to 0.11
- Workflow infrastructure: ubuntu-24.04 runner, explicit permissions, reusable deduplication via phenoShared
- README badge header, worklog scaffolding, AGENTS.md harmonization to thin pointer
- Applied Rust code style: prefixed unused default_ttl param with underscore

### Fixed
- All test suite failures: added scheduled task state, fixed compose_command empty base edge case, added comma metacharacter handling, corrected TTL expiration logic
- Clippy errors across the codebase
- `run_with_streams` compilation error (resolved ambiguous path resolution)
- CI workflow issues: malformed matrix syntax, broken checkout refs, double-SHA action refs, trufflehog configuration (wrong repo path + fake SHA)
- Missing workflow-level permissions and concurrency groups across CI pipelines
- Declared workspace header for standalone Tasken package
- Updated tests to use PersistentTaskCache (was TaskCache)

### Security
- SHA-pinned all GitHub Actions to immutable commit refs across 8+ workflows
- Added CodeQL Rust analysis workflow (weekly + on-demand triggers)
- Added cargo-audit (RustSec advisory) scheduled workflow
- Added trufflehog secrets scanning pipeline
- Standardized workflow suite to ci/audit/deny/scorecard/release
- Removed stale RUSTSEC advisory ignores (no longer applicable)

### Removed
- Stale RUSTSEC advisory ignores

## [0.1.0] - 2026-03-25

### Added
- Initial project scaffold
- Hexagonal architecture structure
- Domain layer with tasks, workflows, scheduler
- Task state machine with transitions
- Workflow DAG definition
- Schedule management (once, interval, cron)
- Task runner implementations (sync, async, background)
- Port definitions (Storage, Queue, Notification)
- Application layer with commands and queries
- Domain events for event sourcing
- Basic tests
- CI/CD workflow
- STANDARDS.md with 126 xDD methodologies
- ADR directory for architecture decisions

### Planned
- Storage adapter implementations
- Queue adapter implementations
- Workflow persistence
- Distributed execution
- Webhook integrations
- CLI adapter
- API adapter
