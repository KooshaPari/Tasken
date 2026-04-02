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
//! use taskkit::{Task, TaskRunner, SyncRunner};
//!
//! let task = Task::new("hello")
//!     .with_action(|| println!("Hello!"));
//!
//! let runner = SyncRunner::new();
//! runner.execute(task).unwrap();
//! ```

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infrastructure;

// Re-exports for convenience
pub use application::services::TaskService;
pub use domain::errors::TaskError;
pub use domain::{
    RetryPolicy, Schedule, Scheduler, Task, TaskResult, TaskRunner, TaskState, Timeout, Workflow,
};
pub use infrastructure::error::TaskKitError;

/// Framework version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
