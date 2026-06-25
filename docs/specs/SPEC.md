# Tasken Specification — Functional & Non-Functional Requirements

> **Oracle file**: specs/acceptance/ encodes the pass/fail criteria.
> **Authoring lane**: spec/tasken-oracle — derived from actual source at d20e205.

---

## 1. Functional Requirements

### FR-1: Single Task Execution
**Description**: The system SHALL execute a single named task via the CLI, running its associated command in a subprocess and streaming stdout/stderr to the user.
**Acceptance Criterion**: Given a registered task `"build"` with command `"cargo build"`, when the user runs `tasken run build`, then the command SHALL be spawned and its exit code SHALL be returned to the caller.
**Traceability**: `src/main.rs` (dispatch), `src/adapters/primary/cli.rs` (`Cmd::Run` handler), `src/domain/runners.rs` (`ShellRunner::run`), `src/domain/tasks.rs` (`Task` struct, `Command` field).

### FR-2: Task Definition & Listing
**Description**: The system SHALL allow users to define tasks (name, command, metadata) and list all registered tasks.
**Acceptance Criterion**: Given one or more registered tasks, when the user runs `tasken list`, then the output SHALL contain the name and a one-line summary of each registered task. When the user runs `tasken show <name>`, the output SHALL contain the full task definition.
**Traceability**: `src/adapters/primary/cli.rs` (`Cmd::List`, `Cmd::Show`), `src/application/queries.rs` (`list_tasks`, `get_task`), `src/domain/tasks.rs`.

### FR-3: Cron-Based Scheduling
**Description**: The system SHALL support recurring execution of tasks via cron expressions, managed by a scheduler daemon.
**Acceptance Criterion**: Given a schedule `"0 9 * * 1-5"` on a task, when the scheduler evaluates the cron expression, then the task SHALL be dispatched at the matching times with sub-minute precision.
**Traceability**: `src/domain/scheduler.rs` (`Scheduler`, `CronSchedule`), `src/domain/tasks.rs` (`Task::schedule`), `src/adapters/primary/cli.rs` (`Cmd::Schedule`).

### FR-4: DAG Workflow Execution
**Description**: The system SHALL load a workflow definition (YAML/TOML) specifying a DAG of tasks with dependencies, and execute them in dependency order, parallelising independent steps.
**Acceptance Criterion**: Given a workflow with `step_a -> step_b -> step_c` and `step_d` independent, when the workflow is executed, then `step_b` SHALL NOT start before `step_a` completes, `step_c` SHALL NOT start before `step_b` completes, and `step_d` MAY run concurrently with any other step.
**Traceability**: `src/domain/workflows.rs` (`WorkflowDefinition`, `DagExecutor`), `src/domain/scheduler.rs` (`WaveScheduler`), `src/domain/runners.rs`, `src/adapters/primary/cli.rs` (`Cmd::Workflow`).

### FR-5: Task Grouping & Hierarchical Organization
**Description**: The system SHALL allow tasks to be organised into named groups, with optional nesting (sub-groups), and support group-level operations (list, run-all).
**Acceptance Criterion**: Given two groups `"frontend"` and `"backend"` each containing multiple tasks, when the user runs `tasken run frontend`, then all tasks in that group SHALL execute. When the user runs `tasken groups`, both groups SHALL appear in the output.
**Traceability**: `src/domain/groups.rs` (`TaskGroup`, `GroupHierarchy`), `src/adapters/primary/cli.rs` (`Cmd::Groups`), `src/application/services.rs`.

### FR-6: Multi-Backend Task Runners
**Description**: The system SHALL support at least three execution backends: shell subprocess, Docker container, and plugin/wasm-based runner.
**Acceptance Criterion**: Given a task with `runner: "docker"` and an image name, when executed, the command SHALL run inside a Docker container and stream logs back. Given a task with `runner: "plugin"` and a `.wasm` path, when executed, the plugin host SHALL load and invoke the WASM module.
**Traceability**: `src/domain/runners.rs` (`ShellRunner`, `DockerRunner`, `PluginRunner`), `src/domain/plugins.rs` (`PluginHost`, `WasmPlugin`).

### FR-7: Task Dependencies & Ordering
**Description**: The system SHALL allow tasks to declare dependencies on other tasks by name, and the runner SHALL resolve the dependency graph before executing.
**Acceptance Criterion**: Given task `"deploy"` depends on `"test"` and `"build"`, when `tasken run deploy` is invoked, then `"build"` and `"test"` SHALL run first (in any order), and `"deploy"` SHALL run only after both complete successfully.
**Traceability**: `src/domain/tasks.rs` (`Task::dependencies`), `src/domain/workflows.rs`, `src/application/services.rs`.

### FR-8: Task Visualization
**Description**: The system SHALL render a visual graph (Mermaid or DOT) of a workflow or task dependency chain for user inspection.
**Acceptance Criterion**: Given a workflow with three steps, when `tasken visualize <workflow>` is executed, then the output SHALL be a valid Mermaid flowchart string that can be rendered by a Mermaid renderer.
**Traceability**: `src/application/visualize.rs` (`WorkflowVisualizer`, `to_mermaid`), `src/adapters/primary/cli.rs` (`Cmd::Visualize`).

### FR-9: File/Directory Watch & Trigger
**Description**: The system SHALL watch a file or directory for changes (create/modify/delete) and trigger a configured task when a matching event occurs.
**Acceptance Criterion**: Given a watch on `"./src/"` with glob `"*.rs"` and action `"build"`, when a `.rs` file is modified, then the `"build"` task SHALL be dispatched within 1 second.
**Traceability**: `src/application/watcher.rs` (`FileWatcher`, `WatchEvent`, `Trigger`), `src/adapters/primary/cli.rs` (`Cmd::Watch`).

### FR-10: Rate Limiting & Throttling
**Description**: The system SHALL support per-task and global rate limiting (max executions per time window, concurrency caps) to prevent resource exhaustion.
**Acceptance Criterion**: Given a rate limit of `5/min` on a task, when 6 execution requests arrive within one minute, the 6th SHALL be queued or rejected with a `RateLimited` error.
**Traceability**: `src/domain/rate_limiter.rs` (`RateLimiter`, `TokenBucket`), `src/domain/tasks.rs` (`Task::rate_limit`).

### FR-11: Event Bus & Observability
**Description**: The system SHALL emit structured events (task started/completed/failed, schedule triggered, error) onto an internal event bus, with optional OpenTelemetry export.
**Acceptance Criterion**: When a task starts and completes, exactly two events (`TaskStarted`, `TaskCompleted`) SHALL be emitted on the bus and, when the `otel` feature is enabled, a matching span tree SHALL be exported.
**Traceability**: `src/domain/events.rs` (`Event`, `EventBus`, `TaskStarted`, `TaskCompleted`), `src/infrastructure/otel.rs` (`OtelTelemetry`, `SpanExporter`), `Cargo.toml` (`otel` feature flag).

### FR-12: Recipe / Blueprint System
**Description**: The system SHALL support recipe definitions — reusable task blueprints parameterised with variables — that can be instantiated into concrete tasks at load time.
**Acceptance Criterion**: Given a recipe `"compile"` with variable `{{lang}}` and a configuration mapping `lang=rust`, when tasks are loaded, a concrete task SHALL be created with the command resolved to the Rust compiler invocation.
**Traceability**: `src/domain/recipe.rs` (`Recipe`, `RecipeInstance`, `VariableResolver`), `src/config/mod.rs`.

### FR-13: Plugin Management
**Description**: The system SHALL allow users to install, list, and remove plugins that extend the runner or add new task types.
**Acceptance Criterion**: Given a valid plugin package, when `tasken plugin install <path>` is run, the plugin SHALL be registered and listed by `tasken plugin list`. When `tasken plugin remove <name>` is run, the plugin SHALL be unregistered.
**Traceability**: `src/domain/plugins.rs` (`PluginHost`, `PluginRegistry`), `src/adapters/primary/cli.rs` (`Cmd::Plugin`).

### FR-14: Task Export / Interoperability
**Description**: The system SHALL export task definitions to common interchange formats (JSON, YAML, TOML) for integration with external tools.
**Acceptance Criterion**: Given a configured task, when `tasken export json` is run, the output SHALL be valid JSON containing the task name, command, schedule, and dependencies.
**Traceability**: `src/adapters/primary/cli.rs` (`Cmd::Export`), `src/application/services.rs` (`export_tasks`).

### FR-15: Persistent Task Store — In-Memory & File Backed
**Description**: The system SHALL support two storage backends: in-memory (for ephemeral/testing usage) and file-based (for persistence across restarts).
**Acceptance Criterion**: Given a file-backed store, when a task is created and the process restarts, the task SHALL still be present after reload. Given an in-memory store, a restart SHALL return an empty store.
**Traceability**: `src/adapters/secondary/memory.rs` (`InMemoryStore`), `src/adapters/secondary/file.rs` (`FileStore`), `src/domain/ports.rs` (`TaskRepository` trait).

### FR-16: Caching Layer
**Description**: The system SHALL provide an in-memory (TTL-eviction) and persistent (disk-backed) cache for task outputs and intermediate results.
**Acceptance Criterion**: Given a cached task result with TTL 60s, when the result is retrieved within 60s, it SHALL be returned from cache. After 60s, the cache SHALL return a miss.
**Traceability**: `src/infrastructure/cache.rs` (`Cache`, `TtlCache`), `src/infrastructure/persistent_cache.rs` (`PersistentCache`).

### FR-17: Python SDK / Bindings
**Description**: The system SHALL expose a Python package (`tasken`) with functions to run, list, and schedule tasks programmatically.
**Acceptance Criterion**: Given a Python environment with the `tasken` packages installed, when `tasken.run("build")` is called, it SHALL dispatch the task and return a result dict with `exit_code` and `output`.
**Traceability**: `python/__init__.py`, `python/task.py` (`run_task`), `python/run.py` (`execute`), `python/execute_task.py`.

---

## 2. Non-Functional Requirements

### NFR-1: CLI Responsiveness
**Description**: The CLI SHALL display usage output within 500 ms for non-execution commands (list, show, help, groups).
**Acceptance Criterion**: When `tasken list` is invoked on a store with ≤100 tasks, the command SHALL exit within 500 ms wall-clock time.

### NFR-2: Execution Overhead
**Description**: The overhead of launching a task (parsing config, resolving dependencies, spawning runner) SHALL be minimal.
**Acceptance Criterion**: Given a no-op task (`command: "true"` on Unix, `command: "exit 0"` on Windows), the end-to-end execution time SHALL be ≤ 50 ms.

### NFR-3: Workflow DAG Correctness
**Description**: The DAG executor SHALL produce a topological ordering that respects all declared dependencies.
**Acceptance Criterion**: For any workflow with a valid DAG (acyclic), the execution order SHALL be a valid topological sort verified by comparing against the transitive closure of declared dependencies. If a cycle is detected, the executor SHALL reject the workflow with a `CycleDetected` error.

### NFR-4: Concurrent Execution Safety
**Description**: The system SHALL support concurrent execution of independent tasks without data races or corruption.
**Acceptance Criterion**: When 10 independent tasks are dispatched concurrently to the same store, all 10 SHALL complete, and the store state SHALL be consistent (no missing or duplicated entries).

### NFR-5: OpenTelemetry Correctness (feature-gated)
**Description**: When the `otel` Cargo feature is enabled, every task execution SHALL produce a complete span tree with correct parent-child relationships.
**Acceptance Criterion**: Given `otel` enabled, a single task execution SHALL produce spans `tasken.run`, `tasken.resolve`, `tasken.execute` where `tasken.execute` is a child of `tasken.run`.

### NFR-6: Plugin Isolation
**Description**: Plugin execution (WASM) SHALL be sandboxed so that a misbehaving plugin cannot crash the host or access arbitrary filesystem resources.
**Acceptance Criterion**: Given a WASM plugin that attempts an out-of-bounds memory access or a host function call not in its import table, the runtime SHALL trap with a `PluginViolation` error and the host SHALL remain operational.

### NFR-7: Configuration Loading — Graceful Degradation
**Description**: The system SHALL start with sensible defaults even when the config file is missing, malformed, or absent.
**Acceptance Criterion**: When no config file is found, the system SHALL start with a default in-memory store and no registered tasks. When a malformed YAML config is provided, the system SHALL emit a structured error and exit with code 78 (EX_CONFIG).

### NFR-8: Watch Responsiveness
**Description**: The file watcher SHALL detect filesystem events and dispatch the trigger task promptly.
**Acceptance Criterion**: Given a watch on a directory, when a matching file is written, the trigger task SHALL be dispatched within 1 second (measured from kernel event to task start).

---

## 3. Traceability Matrix

| FR ID | Source File(s) | Key Symbol(s) |
|-------|----------------|---------------|
| FR-1 | `src/main.rs`, `src/adapters/primary/cli.rs`, `src/domain/runners.rs`, `src/domain/tasks.rs` | `Cmd::Run`, `ShellRunner::run`, `Task` |
| FR-2 | `src/adapters/primary/cli.rs`, `src/application/queries.rs`, `src/domain/tasks.rs` | `Cmd::List`, `Cmd::Show`, `list_tasks`, `get_task` |
| FR-3 | `src/domain/scheduler.rs`, `src/domain/tasks.rs`, `src/adapters/primary/cli.rs` | `Scheduler`, `CronSchedule`, `Cmd::Schedule` |
| FR-4 | `src/domain/workflows.rs`, `src/domain/scheduler.rs`, `src/domain/runners.rs`, `src/adapters/primary/cli.rs` | `WorkflowDefinition`, `DagExecutor`, `WaveScheduler`, `Cmd::Workflow` |
| FR-5 | `src/domain/groups.rs`, `src/adapters/primary/cli.rs`, `src/application/services.rs` | `TaskGroup`, `GroupHierarchy`, `Cmd::Groups` |
| FR-6 | `src/domain/runners.rs`, `src/domain/plugins.rs` | `ShellRunner`, `DockerRunner`, `PluginRunner`, `PluginHost` |
| FR-7 | `src/domain/tasks.rs`, `src/domain/workflows.rs`, `src/application/services.rs` | `Task::dependencies`, dependency graph resolution |
| FR-8 | `src/application/visualize.rs`, `src/adapters/primary/cli.rs` | `WorkflowVisualizer`, `to_mermaid`, `Cmd::Visualize` |
| FR-9 | `src/application/watcher.rs`, `src/adapters/primary/cli.rs` | `FileWatcher`, `WatchEvent`, `Trigger`, `Cmd::Watch` |
| FR-10 | `src/domain/rate_limiter.rs`, `src/domain/tasks.rs` | `RateLimiter`, `TokenBucket`, `Task::rate_limit` |
| FR-11 | `src/domain/events.rs`, `src/infrastructure/otel.rs`, `Cargo.toml` | `EventBus`, `TaskStarted`, `TaskCompleted`, `OtelTelemetry`, `otel` feature |
| FR-12 | `src/domain/recipe.rs`, `src/config/mod.rs` | `Recipe`, `RecipeInstance`, `VariableResolver` |
| FR-13 | `src/domain/plugins.rs`, `src/adapters/primary/cli.rs` | `PluginHost`, `PluginRegistry`, `Cmd::Plugin` |
| FR-14 | `src/adapters/primary/cli.rs`, `src/application/services.rs` | `Cmd::Export`, `export_tasks` |
| FR-15 | `src/adapters/secondary/memory.rs`, `src/adapters/secondary/file.rs`, `src/domain/ports.rs` | `InMemoryStore`, `FileStore`, `TaskRepository` |
| FR-16 | `src/infrastructure/cache.rs`, `src/infrastructure/persistent_cache.rs` | `Cache`, `TtlCache`, `PersistentCache` |
| FR-17 | `python/__init__.py`, `python/task.py`, `python/run.py`, `python/execute_task.py` | `run_task`, `execute` |

---

## 4. Version History

| Version | Date       | Author              | Change |
|---------|------------|---------------------|--------|
| 1.0     | 2026-06-24 | spec/tasken-oracle  | Initial FR/NFR oracle derived from source at d20e205 |
