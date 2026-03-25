# taskkit

**Universal task execution framework with scheduling, workflow orchestration, and plugin support.**

A hexagonal architecture-based task execution engine supporting:

- **Task Scheduling**: Cron, interval, one-shot, and delayed execution
- **Workflow Orchestration**: DAG-based workflows with parallel/sequential execution
- **Plugin System**: Extend task types and integrations via plugins
- **Multiple Runners**: Sync, async, background, and queue-based execution
- **Observability**: Built-in metrics, tracing, and structured logging

## Architecture

```
taskkit/
├── src/
│   ├── domain/          # Core domain logic (pure)
│   │   ├── tasks/      # Task definitions and state machine
│   │   ├── workflows/  # Workflow and DAG definitions
│   │   ├── scheduler/  # Scheduling logic
│   │   ├── runners/    # Execution runners
│   │   ├── ports/      # Interface definitions
│   │   └── errors/     # Domain errors
│   ├── application/    # Application services
│   │   ├── commands/  # Command handlers
│   │   └── queries/    # Query handlers
│   ├── adapters/      # Infrastructure adapters
│   │   ├── primary/    # Primary adapters (CLI, API)
│   │   ├── secondary/  # Secondary adapters (storage, queue)
│   │   └── plugins/    # Plugin system
│   └── infrastructure/ # Cross-cutting concerns
│       ├── error.rs    # Error handling
│       ├── logging.rs  # Structured logging
│       └── tracing.rs # Distributed tracing
├── tests/             # Integration tests
├── examples/          # Usage examples
└── benches/           # Benchmarks
```

## Features

- [x] Task definitions with configurable retry, timeout, priority
- [x] DAG-based workflow orchestration
- [x] Cron and interval scheduling
- [x] Plugin system for custom task types
- [x] Multiple execution runners (sync, async, background, queue)
- [x] Event sourcing for task state changes
- [x] Built-in observability (metrics, logs, traces)
- [ ] Workflow persistence and recovery
- [ ] Distributed execution support
- [ ] Webhook integrations

## Installation

```toml
[dependencies]
taskkit = "0.1"
```

## Quick Start

```rust
use taskkit::{Task, TaskRunner, SyncRunner};

let task = Task::new("hello")
    .with_action(|| println!("Hello, TaskKit!"))
    .with_timeout(Duration::from_secs(30));

let runner = SyncRunner::new();
runner.execute(task)?;
```

## Documentation

- [API Documentation](https://docs.rs/taskkit)
- [User Guide](https://taskkit.dev/guide)
- [Architecture Guide](https://taskkit.dev/architecture)
- [xDD Methodologies](STANDARDS.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT OR Apache-2.0
