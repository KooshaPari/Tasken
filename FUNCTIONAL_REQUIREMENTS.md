# Functional Requirements — Tasken

## FR-TASK: Task Operations

### FR-TASK-001: Task Trait
The system SHALL define a `Task` trait with `execute` method.
**Traces to:** E1.1
**Code Location:** `src/domain/task.rs`

### FR-TASK-002: Task Inputs/Outputs
The system SHALL support typed inputs and outputs for tasks.
**Traces to:** E1.2
**Code Location:** `src/domain/task.rs`

### FR-TASK-003: Task Metadata
The system SHALL support metadata and tags for task identification.
**Traces to:** E1.3
**Code Location:** `src/domain/task.rs`

## FR-SCHED: Scheduling

### FR-SCHED-001: Cron Scheduling
The system SHALL support cron-style scheduling expressions.
**Traces to:** E2.1
**Code Location:** `src/domain/scheduler.rs`

### FR-SCHED-002: Interval Scheduling
The system SHALL support interval-based scheduling (every N seconds/minutes/hours).
**Traces to:** E2.2
**Code Location:** `src/domain/scheduler.rs`

### FR-SCHED-003: Delayed Execution
The system SHALL support one-time delayed execution.
**Traces to:** E2.3
**Code Location:** `src/domain/scheduler.rs`

## FR-WORKFLOW: Workflows

### FR-WORKFLOW-001: Sequential Execution
The system SHALL execute tasks sequentially in defined order.
**Traces to:** E3.1
**Code Location:** `src/domain/workflow.rs`

### FR-WORKFLOW-002: Parallel Execution
The system SHALL execute tasks in parallel using async.
**Traces to:** E3.2
**Code Location:** `src/domain/workflow.rs`

### FR-WORKFLOW-003: Conditional Branching
The system SHALL support conditional branching based on task results.
**Traces to:** E3.3
**Code Location:** `src/domain/workflow.rs`

## FR-ERROR: Error Handling

### FR-ERROR-001: Retry Policies
The system SHALL support fixed and exponential backoff retry policies.
**Traces to:** E4.1
**Code Location:** `src/domain/retry.rs`

### FR-ERROR-002: Timeout Handling
The system SHALL enforce timeout limits on task execution.
**Traces to:** E4.2
**Code Location:** `src/domain/timeout.rs`

### FR-ERROR-003: Error Aggregation
The system SHALL aggregate errors from multiple task executions.
**Traces to:** E4.3
**Code Location:** `src/application/`
