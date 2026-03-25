//! Task runner implementations.

use async_trait::async_trait;
use std::time::{Duration, Instant};
use super::{Task, TaskState, TaskResult, TaskError};
use super::events::{TaskEvent, TaskEventKind};

/// Trait for task runners.
#[async_trait]
pub trait TaskRunner: Send + Sync {
    /// Execute a task synchronously.
    fn execute(&self, task: &mut Task) -> Result<TaskResult, TaskError>;

    /// Execute a task asynchronously.
    async fn execute_async(&self, task: &mut Task) -> Result<TaskResult, TaskError>;
}

/// Synchronous task runner.
pub struct SyncRunner;

impl SyncRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SyncRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRunner for SyncRunner {
    fn execute(&self, task: &mut Task) -> Result<TaskResult, TaskError> {
        // Transition to running
        task.transition_to(TaskState::Running)?;

        let start = Instant::now();

        // Simulate task execution
        // In real implementation, execute the task action
        std::thread::sleep(Duration::from_millis(10));

        let duration = start.elapsed();

        // Transition to completed
        task.transition_to(TaskState::Completed)?;

        Ok(task.success_result(serde_json::json!({"status": "ok"}), duration))
    }

    async fn execute_async(&self, task: &mut Task) -> Result<TaskResult, TaskError> {
        // For sync runner, just run in blocking thread
        let task_id = task.id.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut t = Task::new("temp");
            t.id = task_id;
            self.execute(&mut t)
        })
        .await
        .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;

        result
    }
}

/// Asynchronous task runner.
pub struct AsyncRunner;

impl AsyncRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AsyncRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRunner for AsyncRunner {
    fn execute(&self, task: &mut Task) -> Result<TaskResult, TaskError> {
        // Use tokio runtime for async execution
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;
        rt.block_on(self.execute_async(task))
    }

    async fn execute_async(&self, task: &mut Task) -> Result<TaskResult, TaskError> {
        task.transition_to(TaskState::Running)?;

        let start = Instant::now();

        // Simulate async work
        tokio::time::sleep(Duration::from_millis(10)).await;

        let duration = start.elapsed();

        task.transition_to(TaskState::Completed)?;

        Ok(task.success_result(serde_json::json!({"status": "ok"}), duration))
    }
}

/// Background task runner with queue.
pub struct BackgroundRunner {
    queue: std::sync::Arc<std::sync::Mutex<Vec<Task>>>,
}

impl BackgroundRunner {
    pub fn new() -> Self {
        Self {
            queue: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn enqueue(&self, task: Task) {
        self.queue.lock().unwrap().push(task);
    }

    pub fn queue_len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}

impl Default for BackgroundRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRunner for BackgroundRunner {
    fn execute(&self, _task: &mut Task) -> Result<TaskResult, TaskError> {
        Err(TaskError::InvalidOperation(
            "BackgroundRunner requires async execution".to_string(),
        ))
    }

    async fn execute_async(&self, task: &mut Task) -> Result<TaskResult, TaskError> {
        task.transition_to(TaskState::Running)?;

        let start = Instant::now();

        // Simulate background work
        tokio::time::sleep(Duration::from_millis(10)).await;

        let duration = start.elapsed();

        task.transition_to(TaskState::Completed)?;

        Ok(task.success_result(serde_json::json!({"status": "ok"}), duration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_runner() {
        let runner = SyncRunner::new();
        let mut task = Task::new("test");

        let result = runner.execute(&mut task);
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_background_runner_enqueue() {
        let runner = BackgroundRunner::new();
        let task = Task::new("test");

        runner.enqueue(task);
        assert_eq!(runner.queue_len(), 1);
    }
}
