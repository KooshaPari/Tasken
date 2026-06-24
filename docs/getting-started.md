---
title: Getting Started
---

# Getting Started

**Universal task execution framework with scheduling, workflow orchestration, DAG support, and plugin system.**

A comprehensive task execution engine with implementations in Rust and Python.

## Implementations

| Language   | Directory   | Description                                       |
| ---------- | ----------- | ------------------------------------------------- |
| **Rust**   | `src/`      | High-performance hexagonal architecture            |
| **Python** | `python/`   | Async task orchestration with dependency management |

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

## Quality Gates

```bash
cargo test --workspace           # Test suite (min 80% coverage)
cargo clippy --workspace -- -D warnings  # Linting (zero warnings)
cargo fmt --check                # Format validation
cargo doc --open                 # Documentation generation
```

## License

MIT OR Apache-2.0
