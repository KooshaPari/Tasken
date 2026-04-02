//! Task application service.

use super::commands::{CancelTask, CreateTask, RetryTask};
use super::queries::{GetTask, GetTaskHistory, ListTasks};
use crate::domain::errors::TaskError;
use crate::domain::{
    events::TaskEvent,
    ports::{QueuePort, StoragePort},
    Priority, RetryPolicy, Task, TaskId, TaskResult, TaskState,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;

/// Task application service.
pub struct TaskService {
    storage: Arc<dyn StoragePort>,
    queue: Arc<dyn QueuePort>,
}

impl TaskService {
    /// Create a new task service.
    pub fn new(storage: Arc<dyn StoragePort>, queue: Arc<dyn QueuePort>) -> Self {
        Self { storage, queue }
    }

    /// Create a new task.
    pub async fn create_task(&self, cmd: CreateTask) -> Result<Task, TaskError> {
        let mut task = Task::new(cmd.name);

        if let Some(desc) = cmd.description {
            task = task.with_description(desc);
        }

        if let Some(priority) = cmd.priority {
            task = task.with_priority(priority);
        }

        if let Some(timeout_secs) = cmd.timeout_seconds {
            task = task.with_timeout(std::time::Duration::from_secs(timeout_secs));
        }

        if let Some(policy) = cmd.retry_policy {
            task = task.with_retry_policy(policy);
        }

        for tag in cmd.tags {
            task = task.with_tag(tag);
        }

        task = task.with_data(cmd.data);

        // Persist the task
        self.storage
            .save_task(&task)
            .await
            .map_err(|e| TaskError::StorageError(e))?;

        Ok(task)
    }

    /// Get a task by ID.
    pub async fn get_task(&self, task_id: &TaskId) -> Result<Option<Task>, TaskError> {
        self.storage
            .load_task(&task_id.0)
            .await
            .map_err(|e| TaskError::StorageError(e))
    }

    /// List tasks with optional filters.
    pub async fn list_tasks(
        &self,
        state_filter: Option<TaskState>,
        tag_filter: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<Task>, TaskError> {
        let mut tasks = self
            .storage
            .list_tasks()
            .await
            .map_err(|e| TaskError::StorageError(e))?;

        // Apply filters
        if let Some(state) = state_filter {
            tasks.retain(|t| t.state == state);
        }

        if let Some(tag) = tag_filter {
            tasks.retain(|t| t.tags.contains(&tag));
        }

        // Apply limit
        if let Some(limit) = limit {
            tasks.truncate(limit);
        }

        Ok(tasks)
    }

    /// Cancel a task.
    pub async fn cancel_task(
        &self,
        task_id: TaskId,
        reason: Option<String>,
    ) -> Result<(), TaskError> {
        let mut task = self
            .storage
            .load_task(&task_id.0)
            .await
            .map_err(|e| TaskError::StorageError(e))?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        task.transition_to(TaskState::Cancelled)?;

        self.storage
            .save_task(&task)
            .await
            .map_err(|e| TaskError::StorageError(e))?;

        Ok(())
    }

    /// Retry a failed task.
    pub async fn retry_task(&self, task_id: TaskId) -> Result<Task, TaskError> {
        let mut task = self
            .storage
            .load_task(&task_id.0)
            .await
            .map_err(|e| TaskError::StorageError(e))?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        if task.state != TaskState::Failed {
            return Err(TaskError::InvalidStateTransition {
                from: task.state,
                to: TaskState::Pending,
            });
        }

        if !task.can_retry() {
            return Err(TaskError::RetryLimitExceeded(task.retry_count));
        }

        task.retry_count += 1;
        task.state = TaskState::Pending;
        task.error = None;
        task.updated_at = Utc::now();

        self.storage
            .save_task(&task)
            .await
            .map_err(|e| TaskError::StorageError(e))?;

        Ok(task)
    }

    /// Get task event history.
    pub async fn get_task_history(&self, _task_id: &TaskId) -> Result<Vec<TaskEvent>, TaskError> {
        // In a real implementation, load events from storage
        Ok(Vec::new())
    }

    /// Execute a task.
    pub async fn execute_task(&self, task_id: &TaskId) -> Result<TaskResult, TaskError> {
        let mut task = self
            .storage
            .load_task(&task_id.0)
            .await
            .map_err(|e| TaskError::StorageError(e))?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        // Execute using the queue
        self.queue
            .enqueue(task.clone())
            .await
            .map_err(|e| TaskError::StorageError(e))?;

        // In a real implementation, the queue worker would execute and store the result
        Ok(task.success_result(
            serde_json::json!({"status": "queued"}),
            std::time::Duration::ZERO,
        ))
    }
}
