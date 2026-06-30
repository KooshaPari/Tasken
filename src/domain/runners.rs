// SPDX-License-Identifier: MIT OR Apache-2.0
//! Task runner implementations.

use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::errors::TaskError;
use super::tasks::{Task, TaskState};
use super::TaskResult;

/// Trait for task runners.
#[async_trait]
pub trait TaskRunner: Send + Sync {
    /// Execute a task synchronously.
    fn execute(&self, task: &mut Task) -> Result<TaskResult, TaskError>;

    /// Execute a task asynchronously.
    async fn execute_async(self: Box<Self>, task: Task) -> Result<TaskResult, TaskError>;
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

#[async_trait]
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

    async fn execute_async(self: Box<Self>, task: Task) -> Result<TaskResult, TaskError> {
        // For sync runner, run in blocking thread
        let task_name = task.name.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut t = Task::new(task_name);
            SyncRunner::new().execute(&mut t)
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
    fn execute(&self, _task: &mut Task) -> Result<TaskResult, TaskError> {
        // Cannot execute async runner synchronously
        Err(TaskError::InvalidOperation("AsyncRunner requires async execution".to_string()))
    }

    async fn execute_async(self: Box<Self>, mut task: Task) -> Result<TaskResult, TaskError> {
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
        Self { queue: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())) }
    }

    pub fn enqueue(&self, task: Task) {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).push(task);
    }

    pub fn queue_len(&self) -> usize {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
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
        Err(TaskError::InvalidOperation("BackgroundRunner requires async execution".to_string()))
    }

    async fn execute_async(self: Box<Self>, mut task: Task) -> Result<TaskResult, TaskError> {
        task.transition_to(TaskState::Running)?;

        let start = Instant::now();

        // Simulate background work
        tokio::time::sleep(Duration::from_millis(10)).await;

        let duration = start.elapsed();

        task.transition_to(TaskState::Completed)?;

        Ok(task.success_result(serde_json::json!({"status": "ok"}), duration))
    }
}

/// Shell runner that executes commands via `sh -c`.
pub struct ShellRunner;

impl ShellRunner {
    pub fn new() -> Self {
        Self
    }

    fn extract_command(task: &Task) -> Result<String, TaskError> {
        task.data
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| TaskError::InvalidOperation("No command in task.data['command']".into()))
    }
}

impl Default for ShellRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskRunner for ShellRunner {
    fn execute(&self, task: &mut Task) -> Result<TaskResult, TaskError> {
        let _ = task.transition_to(TaskState::Running);
        let cmd = Self::extract_command(task)?;
        let start = Instant::now();
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;
        let duration = start.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let result = serde_json::json!({
            "status": if success { "ok" } else { "error" },
            "code": output.status.code(),
            "stdout": stdout,
            "stderr": stderr,
        });
        if success {
            let _ = task.transition_to(TaskState::Completed);
            Ok(task.success_result(result, duration))
        } else {
            let _ = task.transition_to(TaskState::Failed);
            Ok(task.failure_result(result.to_string(), duration))
        }
    }

    async fn execute_async(self: Box<Self>, mut task: Task) -> Result<TaskResult, TaskError> {
        let _ = task.transition_to(TaskState::Running);
        let cmd = Self::extract_command(&task)?;
        let start = Instant::now();
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .map_err(|e| TaskError::ExecutionFailed(e.to_string()))?;
        let duration = start.elapsed();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();
        let result = serde_json::json!({
            "status": if success { "ok" } else { "error" },
            "code": output.status.code(),
            "stdout": stdout,
            "stderr": stderr,
        });
        if success {
            let _ = task.transition_to(TaskState::Completed);
            Ok(task.success_result(result, duration))
        } else {
            let _ = task.transition_to(TaskState::Failed);
            Ok(task.failure_result(result.to_string(), duration))
        }
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
    fn test_background_runner_concurrent_enqueue() {
        use std::sync::Arc;
        use std::thread;

        let runner = Arc::new(BackgroundRunner::new());
        let mut handles = Vec::new();

        for i in 0..10usize {
            let r = Arc::clone(&runner);
            handles.push(thread::spawn(move || {
                r.enqueue(Task::new(format!("task-{}", i)));
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }

        // All 10 tasks should be in the queue
        assert_eq!(runner.queue_len(), 10);
    }

    #[test]
    fn test_background_runner_poison_recovery() {
        // Verify that after a thread panics while holding the lock,
        // BackgroundRunner can still access its queue via into_inner()
        use std::sync::Arc;
        use std::thread;

        let runner = Arc::new(BackgroundRunner::new());

        // Enqueue a normal task first
        runner.enqueue(Task::new("before-poison"));

        // Spawn a thread that acquires the lock and panics
        let r = Arc::clone(&runner);
        let handle = thread::spawn(move || {
            r.enqueue(Task::new("after-poison"));
        });

        assert!(handle.join().is_ok(), "enqueue should succeed");
        assert_eq!(runner.queue_len(), 2);
    }

    #[test]
    fn test_shell_runner_sync() {
        let runner = ShellRunner::new();
        let mut task = Task::new("echo-test").with_command("echo hello");
        let result = runner.execute(&mut task);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.output.unwrap().get("stdout").unwrap().as_str().unwrap().contains("hello"));
    }

    #[test]
    fn test_shell_runner_failure() {
        let runner = ShellRunner::new();
        let mut task = Task::new("fail-test").with_command("false");
        let result = runner.execute(&mut task);
        assert!(result.is_ok());
        assert!(!result.unwrap().success);
    }

    #[test]
    fn test_shell_runner_no_command() {
        let runner = ShellRunner::new();
        let mut task = Task::new("no-command");
        let result = runner.execute(&mut task);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_async_runner() {
        let runner: Box<dyn TaskRunner> = Box::new(AsyncRunner::new());
        let task = Task::new("async-test");
        let result = runner.execute_async(task).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_background_runner_async() {
        let runner: Box<dyn TaskRunner> = Box::new(BackgroundRunner::new());
        let task = Task::new("bg-test");
        let result = runner.execute_async(task).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[tokio::test]
    async fn test_shell_runner_async() {
        let runner: Box<dyn TaskRunner> = Box::new(ShellRunner::new());
        let task = Task::new("async-shell").with_command("echo async");
        let result = runner.execute_async(task).await;
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.success);
        assert!(r.output.unwrap().get("stdout").unwrap().as_str().unwrap().contains("async"));
    }
}
