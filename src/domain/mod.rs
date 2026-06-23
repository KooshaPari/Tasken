// SPDX-License-Identifier: MIT OR Apache-2.0
//! Domain layer - pure business logic with no external dependencies.
//!
//! This layer contains:
//! - **Entities**: Task, Workflow, Schedule
//! - **Value Objects**: TaskId, Priority, Timeout, RetryPolicy
//! - **Ports**: Interface definitions for external dependencies
//! - **Services**: Domain services for task orchestration
//! - **Errors**: Domain-specific error types

pub mod errors;
pub mod events;
pub mod groups;
pub mod plugins;
pub mod ports;
pub mod rate_limiter;
pub mod recipe;
pub mod recipes;
pub mod runners;
pub mod scheduler;
pub mod stream_runner;
pub mod tasks;
pub mod workflows;

// Re-exports
pub use errors::PortError;
pub use errors::TaskError;
pub use events::{TaskEvent, TaskEventKind};
pub use groups::{Group, GroupId};
pub use plugins::{
    NoopPlugin, PluginContext, PluginRegistry, PluginResult, RunnerPlugin, ShellPlugin,
};
pub use ports::{NotificationPort, QueuePort, StoragePort, TaskPort};
pub use rate_limiter::{parse_rate_limit, TokenBucket};
pub use recipe::{ParseError, Recipe, RecipeFile, RecipeTask, TaskStepDef, TaskenfileParser};
pub use recipes::{
    evaluate_condition, interpolate, interpolate_strict, predefined_vars, InterpolationError,
    Settings, VarDefinition, VarType, Vars,
};
pub use runners::{AsyncRunner, BackgroundRunner, ShellRunner, SyncRunner, TaskRunner};
pub use scheduler::{Schedule, ScheduleKind, Scheduler};
pub use stream_runner::{run_with_streams, StreamResult, StreamRunner};
pub use tasks::{topological_sort_tasks, Task, TaskResult, TaskState};
pub use workflows::{Workflow, WorkflowState, WorkflowStep};
