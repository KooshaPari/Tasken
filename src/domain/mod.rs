//! Domain layer - pure business logic with no external dependencies.
//!
//! This layer contains:
//! - **Entities**: Task, Workflow, Schedule
//! - **Value Objects**: TaskId, Priority, Timeout, RetryPolicy
//! - **Ports**: Interface definitions for external dependencies
//! - **Services**: Domain services for task orchestration
//! - **Errors**: Domain-specific error types

pub mod tasks;
pub mod workflows;
pub mod scheduler;
pub mod runners;
pub mod ports;
pub mod errors;
pub mod events;

// Re-exports
pub use tasks::{Task, TaskState, TaskResult};
pub use workflows::{Workflow, WorkflowState, WorkflowStep};
pub use scheduler::{Schedule, ScheduleKind, Scheduler};
pub use runners::{TaskRunner, SyncRunner, AsyncRunner, BackgroundRunner};
pub use ports::{TaskPort, StoragePort, QueuePort, NotificationPort};
pub use errors::TaskError;
pub use events::{TaskEvent, TaskEventKind};
