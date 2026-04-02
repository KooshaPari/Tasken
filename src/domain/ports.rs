//! Port definitions - interfaces for external dependencies.

use super::{Schedule, Task, TaskResult, Workflow};
use async_trait::async_trait;

/// Port for task storage operations.
#[async_trait]
pub trait StoragePort: Send + Sync {
    /// Save a task.
    async fn save_task(&self, task: &Task) -> Result<(), String>;

    /// Load a task by ID.
    async fn load_task(&self, id: &str) -> Result<Option<Task>, String>;

    /// Delete a task.
    async fn delete_task(&self, id: &str) -> Result<(), String>;

    /// List all tasks.
    async fn list_tasks(&self) -> Result<Vec<Task>, String>;

    /// Save a workflow.
    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), String>;

    /// Load a workflow.
    async fn load_workflow(&self, id: &str) -> Result<Option<Workflow>, String>;

    /// Save a schedule.
    async fn save_schedule(&self, schedule: &Schedule) -> Result<(), String>;

    /// Load a schedule.
    async fn load_schedule(&self, id: &str) -> Result<Option<Schedule>, String>;
}

/// Port for task queue operations.
#[async_trait]
pub trait QueuePort: Send + Sync {
    /// Enqueue a task.
    async fn enqueue(&self, task: Task) -> Result<(), String>;

    /// Dequeue a task.
    async fn dequeue(&self) -> Result<Option<Task>, String>;

    /// Get queue length.
    async fn len(&self) -> Result<usize, String>;

    /// Check if queue is empty.
    async fn is_empty(&self) -> Result<bool, String>;
}

/// Port for task execution notifications.
#[async_trait]
pub trait NotificationPort: Send + Sync {
    /// Notify task started.
    async fn notify_started(&self, task_id: &str) -> Result<(), String>;

    /// Notify task completed.
    async fn notify_completed(&self, result: &TaskResult) -> Result<(), String>;

    /// Notify task failed.
    async fn notify_failed(&self, task_id: &str, error: &str) -> Result<(), String>;

    /// Notify schedule due.
    async fn notify_schedule_due(&self, schedule_id: &str) -> Result<(), String>;
}

/// Combined task port for dependency injection.
#[async_trait]
pub trait TaskPort: Send + Sync {
    /// Execute a task.
    async fn execute(&self, task: Task) -> Result<TaskResult, String>;

    /// Cancel a task.
    async fn cancel(&self, task_id: &str) -> Result<(), String>;

    /// Get task status.
    async fn status(&self, task_id: &str) -> Result<Option<Task>, String>;
}
