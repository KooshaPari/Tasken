<!-- AI-DD-META:START -->
<!-- This repository is planned, maintained, and managed by AI Agents only. -->
<!-- Slop issues are expected and intentionally present as part of an HITL-less -->
<!-- /minimized AI-DD metaproject of learning, refining, and building brute-force -->
<!-- training for both agents and the human operator. -->
![Downloads](https://img.shields.io/github/downloads/KooshaPari/Tasken/total?style=flat-square&label=downloads&color=blue)
![GitHub release](https://img.shields.io/github/v/release/KooshaPari/Tasken?style=flat-square&label=release)
![License](https://img.shields.io/github/license/KooshaPari/Tasken?style=flat-square)
![AI-Slop](https://img.shields.io/badge/AI--DD-Slop%20Expected-orange?style=flat-square)
![AI-Only-Maintained](https://img.shields.io/badge/Planned%20%26%20Maintained%20by-AI%20Agents%20Only-red?style=flat-square)
![HITL-less](https://img.shields.io/badge/HITL--less%20AI--DD-metaproject-yellow?style=flat-square)

> ⚠️ **AI-Agent-Only Repository**
>
> This repo is **planned, maintained, and managed exclusively by AI Agents**.
> Slop issues, rough edges, and AI artifacts are **expected and intentionally
> present** as part of an **HITL-less / minimized AI-DD** metaproject focused
> on learning, refining, and brute-force training both the agents and the
> human operator. Bug reports and contributions are still welcome, but please
> expect AI-generated code, comments, and documentation throughout.
<!-- AI-DD-META:END -->
> **Work state:** ACTIVE · **Progress:** `█████░░░░░ 45%`
> Rust task engine; scaffold-to-active, governance complete · updated 2026-06-02

> **Pinned references (Phenotype-org)**
> - MSRV: see rust-toolchain.toml
> - cargo-deny config: see deny.toml
> - cargo-audit: rustsec/audit-check@v2 weekly
> - Branch protection: 1 reviewer required, no force-push
> - Authority: phenotype-org-governance/SUPERSEDED.md

# Tasken

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![CI](https://github.com/KooshaPari/Tasken/actions/workflows/ci.yml/badge.svg)](https://github.com/KooshaPari/Tasken/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

**Universal task execution framework with scheduling, workflow orchestration, DAG support, and plugin system.**

A comprehensive task execution engine with implementations in Rust and Python.

## Implementations

| Language | Directory | Description |
|---------|----------|-------------|
| **Rust** | `src/` | High-performance hexagonal architecture |
| **Python** | `python/` | Async task orchestration with dependency management |

## Features

### Rust Implementation
- **Task Scheduling**: Cron, interval, one-shot, and delayed execution
- **Workflow Orchestration**: DAG-based workflows with parallel/sequential execution
- **Plugin System**: Extend task types and integrations via plugins
- **Multiple Runners**: Sync, async, background, and queue-based execution
- **Observability**: Built-in metrics, tracing, and structured logging
- **Hexagonal Architecture**: Clean separation of domain, application, and infrastructure

### Python Implementation
- **Async/Await**: Full async execution with asyncio
- **Dependency Graph**: DAG-based task dependencies
- **Retry Logic**: Exponential backoff with jitter
- **Parallel Execution**: Maximize resource utilization
- **Distributed**: Support for multi-node execution
- **Observability**: Tracing and metrics for all tasks

## Architecture

```
tasken/
├── src/                     # Rust implementation
│   ├── domain/             # Core domain logic (pure)
│   │   ├── tasks/          # Task definitions and state machine
│   │   ├── workflows/      # Workflow and DAG definitions
│   │   ├── scheduler/      # Scheduling logic
│   │   ├── runners/        # Execution runners
│   │   ├── ports/          # Interface definitions
│   │   └── errors/         # Domain errors
│   ├── application/         # Application services
│   │   ├── commands/        # Command handlers
│   │   └── queries/         # Query handlers
│   ├── adapters/            # Infrastructure adapters
│   │   ├── primary/         # Primary adapters (CLI, API)
│   │   ├── secondary/       # Secondary adapters (storage, queue)
│   │   └── plugins/         # Plugin system
│   └── infrastructure/      # Cross-cutting concerns
├── python/                  # Python implementation
│   ├── task.py              # Core task definitions
│   ├── execute_task.py      # Task execution engine
│   ├── run.py               # CLI entry point
│   └── ...
├── tests/                  # Integration tests
├── examples/                # Usage examples
└── benches/                # Benchmarks
```

## Quick Start

### Rust

```toml
[dependencies]
tasken = "0.1"
```

```rust
use tasken::{Task, TaskRunner, SyncRunner};

let task = Task::new("hello")
    .with_action(|| println!("Hello, Tasken!"))
    .with_timeout(Duration::from_secs(30));

let runner = SyncRunner::new();
runner.execute(task)?;
```

### Python

```bash
pip install tasken
```

```python
from tasken import Task, execute_task

async def main():
    task = Task(name="hello", action=lambda: print("Hello, Tasken!"))
    await execute_task(task)

asyncio.run(main())
```

## Governance & Development

**AgilePlus Integration**: All work tracked in `/repos/AgilePlus`. Review `CLAUDE.md` for development policies and standards.

**Quality Gates**:
```bash
cargo test --workspace           # Test suite (min 80% coverage)
cargo clippy --workspace -- -D warnings  # Linting (zero warnings)
cargo fmt --check                # Format validation
cargo doc --open                 # Documentation generation
```

**Architecture Pattern**: Tasken follows hexagonal (ports & adapters) architecture to maintain clean separation between domain logic, application services, and infrastructure concerns.

## Performance & Observability

- **Built-in Metrics**: Task execution times, retry counts, and workflow DAG metrics
- **Structured Logging**: Full execution tracing for debugging distributed workflows
- **Benchmarking**: Dedicated `benches/` directory for performance profiling

## Cross-Repo Integration

Tasken emits task and workflow lifecycle events through `phenotype-event-bus` where cross-repo event streaming is required. Agent-driven distribution and workflow-state integrations should be documented against their current owning crates and services rather than stale retired bus assumptions.

## Related Phenotype Projects

- **[Sidekick](../Sidekick)** — Agent dispatch for task execution
- **[Stashly](../Stashly)** — State machines & event sourcing
- **[phenotype-shared](../phenotype-shared)** — Shared utilities
- **[AgilePlus](../AgilePlus)** — Specification & planning

## License

MIT OR Apache-2.0

**Status**: Active development  
**Maintained by**: Phenotype Org  
**Last Updated**: 2026-04-24

## Documentation

This repository includes the following cross-cutting documents:

- [`AGENTS.md`](AGENTS.md) — operating instructions for AI agents and human contributors
- [`SPEC.md`](SPEC.md) — formal specification of behavior and contracts
- [`docs/`](docs/) — design notes, ADRs, and supporting documentation (see [`docs/index.md`](docs/index.md))


## Absorbed phenoForge contract

phenoForge build-orchestrator research and product intent is preserved under docs/history/archived-repos/phenoForge/. Tasken remains the canonical active task orchestration product; phenoForge material is historical input for build-runner, DAG, caching, plugin, and remote execution requirements.

<!-- ci: retrigger deploy after enabling GitHub Pages (2026-06-24T08:16:03Z) -->
