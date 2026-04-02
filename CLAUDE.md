# CLAUDE.md — Tasken

## Project Identity

- **Name**: Tasken
- **Description**: Task execution framework with scheduling and workflow support
- **Location**: `remote-clones/Tasken/`
- **Language**: Rust
- **License**: MIT OR Apache-2.0

## Architecture

Hexagonal (Ports & Adapters):
- Task trait is the port (interface)
- Executor implementations are adapters
- Scheduler for workflow orchestration

## Quick Commands

```bash
# Build
cargo build

# Test
cargo test

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --check

# Documentation
cargo doc --open
```

## Key Files

| Path | Purpose |
|------|---------|
| `src/lib.rs` | Main library entry |
| `src/executor.rs` | Task execution engine |
| `src/scheduler.rs` | Scheduling logic |
| `src/workflow.rs` | Workflow definitions |
| `src/task.rs` | Task traits and types |

## Testing Requirements

- Unit tests for all public APIs
- Integration tests for workflow execution
- Property-based tests for scheduler
- Minimum 80% code coverage
