# PRD — Tasken

## Overview

`Tasken` is a task execution framework with scheduling and workflow support for Rust, following hexagonal architecture principles.

## Goals

- Provide task execution abstraction
- Support scheduled task execution
- Workflow orchestration with parallel/sequential execution
- Retry policies and timeout handling
- Task state management

## Epics

### E1 — Task Abstraction
- E1.1 Define Task trait with execute method
- E1.2 Support task inputs and outputs
- E1.3 Task metadata and tags

### E2 — Scheduling
- E2.1 Cron-style scheduling
- E2.2 Interval-based scheduling
- E2.3 One-time delayed execution

### E3 — Workflows
- E3.1 Sequential workflow execution
- E3.2 Parallel workflow execution
- E3.3 Conditional branching

### E4 — Error Handling
- E4.1 Retry policies (fixed, exponential backoff)
- E4.2 Timeout handling
- E4.3 Error aggregation

## Acceptance Criteria

- Tasks execute correctly
- Schedules trigger at correct times
- Workflows execute in correct order
- Retry policies work correctly
- Timeouts are enforced
