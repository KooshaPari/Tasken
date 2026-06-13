//! Port definitions - interfaces for external dependencies.

use super::errors::PortError;
use super::{Schedule, Task, TaskResult, Workflow};
use async_trait::async_trait;

/// Port for task storage operations.
#[async_trait]
pub trait StoragePort: Send + Sync {
    /// Save a task.
    async fn save_task(&self, task: &Task) -> Result<(), PortError>;

    /// Load a task by ID.
    async fn load_task(&self, id: &str) -> Result<Option<Task>, PortError>;

    /// Delete a task.
    async fn delete_task(&self, id: &str) -> Result<(), PortError>;

    /// List all tasks.
    async fn list_tasks(&self) -> Result<Vec<Task>, PortError>;

    /// Save a workflow.
    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), PortError>;

    /// Load a workflow.
    async fn load_workflow(&self, id: &str) -> Result<Option<Workflow>, PortError>;

    /// List all workflows.
    async fn list_workflows(&self) -> Result<Vec<Workflow>, PortError>;

    /// Save a schedule.
    async fn save_schedule(&self, schedule: &Schedule) -> Result<(), PortError>;

    /// Load a schedule.
    async fn load_schedule(&self, id: &str) -> Result<Option<Schedule>, PortError>;

    /// List all schedules.
    async fn list_schedules(&self) -> Result<Vec<Schedule>, PortError>;
}

/// Port for task queue operations.
#[async_trait]
pub trait QueuePort: Send + Sync {
    /// Enqueue a task.
    async fn enqueue(&self, task: Task) -> Result<(), PortError>;

    /// Dequeue a task.
    async fn dequeue(&self) -> Result<Option<Task>, PortError>;

    /// Get queue length.
    async fn len(&self) -> Result<usize, PortError>;

    /// Check if queue is empty.
    async fn is_empty(&self) -> Result<bool, PortError>;
}

/// Port for task execution notifications.
#[async_trait]
pub trait NotificationPort: Send + Sync {
    /// Notify task started.
    async fn notify_started(&self, task_id: &str) -> Result<(), PortError>;

    /// Notify task completed.
    async fn notify_completed(&self, result: &TaskResult) -> Result<(), PortError>;

    /// Notify task failed.
    async fn notify_failed(&self, task_id: &str, error: &str) -> Result<(), PortError>;

    /// Notify schedule due.
    async fn notify_schedule_due(&self, schedule_id: &str) -> Result<(), PortError>;
}

/// Combined task port for dependency injection.
#[async_trait]
pub trait TaskPort: Send + Sync {
    /// Execute a task.
    async fn execute(&self, task: Task) -> Result<TaskResult, PortError>;

    /// Cancel a task.
    async fn cancel(&self, task_id: &str) -> Result<(), PortError>;

    /// Get task status.
    async fn status(&self, task_id: &str) -> Result<Option<Task>, PortError>;
}
