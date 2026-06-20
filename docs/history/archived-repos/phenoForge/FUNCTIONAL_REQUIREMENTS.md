# Functional Requirements — phenotype-forge

## FR-TASK — Task Definition

| ID | Requirement |
|----|-------------|
| FR-TASK-001 | The system SHALL allow tasks to be defined as Rust structs implementing a Task trait. |
| FR-TASK-002 | The system SHALL support declaring task dependencies as a directed acyclic graph. |
| FR-TASK-003 | The system SHALL validate the task graph for cycles at startup and exit with an error if found. |
| FR-TASK-004 | The system SHALL support environment variable injection per task. |
| FR-TASK-005 | The system SHALL support secret resolution from environment or vault per task. |

## FR-EXEC — Execution

| ID | Requirement |
|----|-------------|
| FR-EXEC-001 | The system SHALL execute tasks in topological dependency order. |
| FR-EXEC-002 | The system SHALL execute independent tasks in parallel up to a configurable concurrency limit. |
| FR-EXEC-003 | The system SHALL stream stdout/stderr to the terminal with task-labeled prefixes. |
| FR-EXEC-004 | The system SHALL fail fast on first error by default. |
| FR-EXEC-005 | The system SHALL support --continue-on-error to execute remaining independent tasks after a failure. |

## FR-CLI — CLI

| ID | Requirement |
|----|-------------|
| FR-CLI-001 | The system SHALL provide a `forge run <task>` command. |
| FR-CLI-002 | The system SHALL provide a `forge list` command showing all tasks with descriptions. |
| FR-CLI-003 | The system SHALL provide a `forge graph` command rendering the task DAG. |
| FR-CLI-004 | The system SHALL provide a `forge check` command validating the task graph. |

## FR-DIST — Distribution

| ID | Requirement |
|----|-------------|
| FR-DIST-001 | The system SHALL ship as a statically linked binary for Linux, macOS, and Windows. |
| FR-DIST-002 | The system SHALL read project configuration from `.forge/config.toml`. |
