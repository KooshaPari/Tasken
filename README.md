# Tasken

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

Tasken integrates with `phenotype-bus` for event streaming and works alongside Sidekick for agent-driven task distribution. Use Stashly's state machine for workflow state management.

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
