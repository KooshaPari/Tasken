# Phenotype Forge

> High-Performance CLI Task Runner and Build Orchestrator

A Rust-native task runner with parallel execution, dependency graph resolution, hot reload, and a powerful plugin system for build automation and workflow orchestration.

## Philosophy

**Build tools should be fast, reliable, and delightful to use.**

- **Speed**: Parallel execution with topological sorting
- **Correctness**: Deterministic builds with proper dependency tracking
- **Ergonomics**: Simple syntax, powerful features
- **Extensibility**: Plugin system for custom tasks
- **Observability**: Clear progress, detailed logs, timing metrics

## Features

| Feature | Description | Status |
|---------|-------------|--------|
| **Parallel Execution** | Run independent tasks concurrently | Stable |
| **Dependency Graph** | Automatic topological sort | Stable |
| **Hot Reload** | Watch files and restart on changes | Stable |
| **Plugin System** | Extend with WASM plugins | Stable |
| **Incremental Builds** | Only rebuild changed dependencies | Stable |
| **Caching** | Cache task outputs | Stable |
| **Remote Execution** | Distribute tasks to workers | Beta |
| **Profiling** | Task execution timing and analysis | Stable |

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Phenotype Forge                                      │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         Task Definition                              │   │
│  │                                                                      │   │
│  │   #[task]                                                            │   │
│  │   fn build() { /* compile */ }                                       │   │
│  │                                                                      │   │
│  │   #[task]                                                            │   │
│  │   #[deps(build)]                                                     │   │
│  │   fn test() { /* test */ }                                           │   │
│  │                                                                      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┼───────────────────────────────────────┐ │
│  │                    Dependency Graph Engine                             │ │
│  │                                                                      │ │
│  │   ┌──────────┐  ┌──────────┐  ┌──────────┐                         │ │
│  │   │   build  │──│   test   │──│  deploy  │                         │ │
│  │   └────┬─────┘  └──────────┘  └──────────┘                         │ │
│  │        │                                                             │ │
│  │   ┌────┴─────┐                                                     │ │
│  │   │  compile │                                                     │ │
│  │   └──────────┘                                                     │ │
│  │                                                                      │ │
│  │   Topological Sort → Parallel Execution                              │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│                                    │                                        │
│  ┌─────────────────────────────────┼───────────────────────────────────────┐ │
│  │                      Execution Engine                                  │ │
│  │                                                                      │ │
│  │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │ │
│  │   │  Worker 1    │  │  Worker 2    │  │  Worker 3    │           │ │
│  │   │  (compile)   │  │  (test)      │  │  (lint)      │           │ │
│  │   └──────────────┘  └──────────────┘  └──────────────┘           │ │
│  │                                                                      │ │
│  │   Thread Pool / Process Pool / Remote Workers                        │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Quick Start

```rust
use forge::{task, deps, Forge};

// Define tasks with attributes
#[task]
fn compile() {
    println!("Compiling...");
    // Your build logic
}

#[task]
#[deps(compile)]
fn test() {
    println!("Running tests...");
    // Your test logic
}

#[task]
#[deps(compile)]
fn lint() {
    println!("Linting...");
    // Your lint logic
}

#[task]
#[deps(test, lint)]
fn package() {
    println!("Packaging...");
    // Your package logic
}

// Main entry point
fn main() {
    Forge::new()
        .register(compile)
        .register(test)
        .register(lint)
        .register(package)
        .run();
}
```

Run with:
```bash
# Run specific task
forge compile

# Run with dependencies
forge test       # Runs compile first

# Run default task
forge            # Runs all default tasks

# Watch mode
forge --watch    # Rebuild on file changes

# Parallel execution (default)
forge --parallel # Run independent tasks concurrently

# Dry run
forge --dry-run  # Show what would be executed
```

## Task Definition

### Basic Tasks

```rust
#[task]
fn clean() {
    fs::remove_dir_all("./target").ok();
}

#[task]
#[deps(clean)]
fn build() {
    Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .unwrap();
}
```

### Tasks with Arguments

```rust
#[task]
fn build_target(target: String, features: Vec<String>) {
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    cmd.arg("--target").arg(target);
    
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }
    
    cmd.status().unwrap();
}
```

### Conditional Dependencies

```rust
#[task]
#[deps(compile)]
#[deps(test, if = "cfg(test)")]
fn build() {
    // Compiles always, tests only in test mode
}
```

## Configuration

```toml
# forge.toml
[project]
name = "my-project"
version = "1.0.0"

[tasks]
default = ["build", "test"]

[execution]
parallel = true
workers = 8
timeout = "10m"

[cache]
enabled = true
directory = ".forge-cache"
ttl = "24h"

[watch]
paths = ["src/", "Cargo.toml"]
ignore = ["target/", ".git/"]
debounce = 500  # ms

[plugins]
directory = "./plugins"
preload = ["plugin-a", "plugin-b"]

[remote]
enabled = false
workers = ["worker-1.example.com", "worker-2.example.com"]
```

## Plugin System

### WASM Plugins

```rust
// plugin.rs
use forge_plugin::*;

#[plugin]
fn custom_task() {
    println!("Running custom task from plugin!");
}

#[plugin(name = "format-code")]
fn formatter(paths: Vec<String>) {
    for path in paths {
        format_file(&path);
    }
}
```

### Using Plugins

```toml
# forge.toml
[plugins]
my-plugin = { git = "https://github.com/example/forge-plugin" }
local-plugin = { path = "./plugins/local" }
```

```bash
# Run plugin task
forge my-plugin::custom_task
forge format-code --paths "src/**/*.rs"
```

## Advanced Features

### Incremental Builds

```rust
#[task]
#[cache]
fn compile() -> TaskResult {
    let output = compile_sources();
    
    // Cache key based on inputs
    TaskResult::ok(output)
        .with_cache_key(file_hash("src/"))
        .with_ttl(Duration::from_hours(24))
}
```

### Remote Execution

```rust
#[task]
#[remote]
fn heavy_computation() {
    // Automatically distributed to remote workers
    compute_intensive_operation()
}
```

### Custom Execution Environment

```rust
#[task]
#[env(PATH = "/opt/custom/bin", RUST_LOG = "debug")]
#[cwd("./subdir")]
fn custom_env() {
    // Runs in custom environment
}
```

### Shell Tasks

```rust
#[task]
#[shell("""
    echo "Building..."
    cargo build --release
    echo "Done!"
""")]
fn build() {}
```

## CLI Commands

| Command | Description | Example |
|---------|-------------|---------|
| `forge <task>` | Run task | `forge build` |
| `forge --list` | List all tasks | `forge --list` |
| `forge --graph` | Show dependency graph | `forge --graph` |
| `forge --watch` | Watch mode | `forge --watch` |
| `forge --dry-run` | Dry run | `forge --dry-run` |
| `forge --profile` | Profile execution | `forge --profile` |
| `forge --clean` | Clean cache | `forge --clean` |
| `forge --remote` | Use remote workers | `forge --remote` |

## Performance

| Metric | Typical Value | Notes |
|--------|---------------|-------|
| Task startup | < 1ms | Minimal overhead |
| Parallel tasks | 10-100x | Depends on task count |
| Cache hit | < 100ms | Near-instant |
| Watch mode | < 50ms | File change detection |
| Plugin load | < 10ms | WASM instantiation |

## References

- Cargo: https://doc.rust-lang.org/cargo/
- Make: https://www.gnu.org/software/make/
- Just: https://github.com/casey/just
- Task: https://taskfile.dev/
- Nx: https://nx.dev/
- Turborepo: https://turbo.build/
- Buck2: https://buck2.build/
- Bazel: https://bazel.build/

## Documentation

This repository includes the following cross-cutting documents:

- [`AGENTS.md`](AGENTS.md) — operating instructions for AI agents and human contributors
- [`SPEC.md`](SPEC.md) — formal specification of behavior and contracts
- [`docs/`](docs/) — design notes, ADRs, and supporting documentation (see [`docs/index.md`](docs/index.md))

