// SPDX-License-Identifier: MIT OR Apache-2.0
//! Task execution framework with scheduling and workflow orchestration.
//!
//! # Architecture
//!
//! taskkit follows hexagonal architecture:
//!
//! - **Domain**: Pure business logic (tasks, workflows, scheduling)
//! - **Application**: Use cases and command/query handlers
//! - **Adapters**: Primary (CLI, API) and secondary (storage, queue) adapters
//! - **Infrastructure**: Cross-cutting concerns (logging, tracing, metrics)
//!
//! # Quick Start
//!
//! ```
//! use taskkit::{Task, SyncRunner, TaskRunner};
//!
//! let mut task = Task::new("hello");
//! let runner = SyncRunner::new();
//! let result = runner.execute(&mut task);
//! assert!(result.is_ok());
//! ```

pub mod adapters;
pub mod application;
pub mod config;
pub mod cron_parser;
pub mod domain;
pub mod infrastructure;

// Re-exports for convenience
pub use application::services::TaskService;
pub use config::TaskenConfig;
pub use domain::errors::TaskError;
pub use domain::{
    Schedule, Scheduler, Task, TaskResult, TaskRunner, TaskState, Workflow,
};
pub use domain::tasks::{Priority, RetryPolicy, TaskId};
pub use domain::groups::{Group, GroupId};
pub use domain::recipes::{interpolate, interpolate_strict, predefined_vars, Settings, VarDefinition, VarType, Vars};
pub use domain::runners::{AsyncRunner, BackgroundRunner, SyncRunner};
pub use infrastructure::{TaskCache, TaskKitError};

/// Framework version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
