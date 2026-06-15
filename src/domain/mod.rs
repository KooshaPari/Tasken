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
pub mod ports;
pub mod runners;
pub mod scheduler;
pub mod stream_runner;
pub mod tasks;
pub mod workflows;

// Re-exports
pub use errors::TaskError;
pub use errors::PortError;
pub use events::{TaskEvent, TaskEventKind};
pub use ports::{NotificationPort, QueuePort, StoragePort, TaskPort};
pub use runners::{AsyncRunner, BackgroundRunner, ShellRunner, SyncRunner, TaskRunner};
pub use scheduler::{Schedule, ScheduleKind, Scheduler};
pub use stream_runner::{run_with_streams, StreamResult, StreamRunner};
pub use tasks::{Task, TaskResult, TaskState};
pub use workflows::{Workflow, WorkflowState, WorkflowStep};
