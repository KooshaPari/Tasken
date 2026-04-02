//! Task command definitions.

use super::super::domain::errors::TaskError;
use super::super::domain::{Priority, RetryPolicy, Task, TaskId, TaskState};
use super::services::TaskService;

/// Command to create a new task.
pub struct CreateTask {
    pub name: String,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub timeout_seconds: Option<u64>,
    pub retry_policy: Option<RetryPolicy>,
    pub tags: Vec<String>,
    pub data: serde_json::Value,
}

impl CreateTask {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            priority: None,
            timeout_seconds: None,
            retry_policy: None,
            tags: Vec::new(),
            data: serde_json::Value::Null,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }

    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Execute the command.
    pub async fn execute(self, service: &TaskService) -> Result<Task, TaskError> {
        service.create_task(self).await
    }
}

/// Command to cancel a task.
pub struct CancelTask {
    pub task_id: TaskId,
    pub reason: Option<String>,
}

impl CancelTask {
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id,
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Execute the command.
    pub async fn execute(self, service: &TaskService) -> Result<(), TaskError> {
        service.cancel_task(self.task_id, self.reason).await
    }
}

/// Command to retry a failed task.
pub struct RetryTask {
    pub task_id: TaskId,
}

impl RetryTask {
    pub fn new(task_id: TaskId) -> Self {
        Self { task_id }
    }

    /// Execute the command.
    pub async fn execute(self, service: &TaskService) -> Result<Task, TaskError> {
        service.retry_task(self.task_id).await
    }
}
