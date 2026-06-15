//! Task application service.

use super::commands::CreateTask;
use crate::domain::errors::TaskError;
use crate::domain::ports::{QueuePort, StoragePort};
use crate::domain::runners::{ShellRunner, TaskRunner};
use crate::domain::tasks::{Task, TaskId, TaskState};
use crate::domain::workflows::{Workflow, WorkflowId};
use crate::domain::{events::TaskEvent, TaskResult};
use chrono::Utc;
use std::sync::Arc;

/// Task application service.
#[derive(Clone)]
pub struct TaskService {
    storage: Arc<dyn StoragePort>,
    queue: Arc<dyn QueuePort>,
    cache: Arc<crate::infrastructure::PersistentTaskCache>,
}

impl TaskService {
    /// Create a new task service with a disk-backed cache at ~/.taskkit/cache.json.
    pub fn new(storage: Arc<dyn StoragePort>, queue: Arc<dyn QueuePort>) -> Self {
        let cache_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".taskkit")
            .join("cache.json");
        let cache = crate::infrastructure::PersistentTaskCache::open(
            &cache_path,
            std::time::Duration::from_secs(300),
        )
        .unwrap_or_else(|_| {
            crate::infrastructure::PersistentTaskCache::ephemeral(
                std::time::Duration::from_secs(300),
            )
        });
        Self {
            storage,
            queue,
            cache: Arc::new(cache),
        }
    }

    /// Create a new task service with a custom persistent cache.
    pub fn with_cache(
        storage: Arc<dyn StoragePort>,
        queue: Arc<dyn QueuePort>,
        cache: Arc<crate::infrastructure::PersistentTaskCache>,
    ) -> Self {
        Self { storage, queue, cache }
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

        // Set dependencies
        for dep in cmd.depends_on {
            task = task.with_dependency(dep);
        }

        // Persist the task
        self.storage.save_task(&task).await?;

        Ok(task)
    }

    /// Get a task by ID.
    pub async fn get_task(&self, task_id: &TaskId) -> Result<Option<Task>, TaskError> {
        self.storage.load_task(&task_id.0).await.map_err(Into::into)
    }

    /// List tasks with optional filters.
    pub async fn list_tasks(
        &self,
        state_filter: Option<TaskState>,
        tag_filter: Option<String>,
        limit: Option<usize>,
    ) -> Result<Vec<Task>, TaskError> {
        let mut tasks = self.storage.list_tasks().await?;

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
        _reason: Option<String>,
    ) -> Result<(), TaskError> {
        let mut task = self
            .storage
            .load_task(&task_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        task.transition_to(TaskState::Cancelled)?;

        self.storage.save_task(&task).await?;

        Ok(())
    }

    /// Retry a failed task.
    pub async fn retry_task(&self, task_id: TaskId) -> Result<Task, TaskError> {
        let mut task = self
            .storage
            .load_task(&task_id.0)
            .await?
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

        self.storage.save_task(&task).await?;

        Ok(task)
    }

    /// Get task event history.
    pub async fn get_task_history(&self, _task_id: &TaskId) -> Result<Vec<TaskEvent>, TaskError> {
        // In a real implementation, load events from storage
        Ok(Vec::new())
    }

    /// Execute a task by queuing it.
    pub async fn execute_task(&self, task_id: &TaskId) -> Result<TaskResult, TaskError> {
        let task = self
            .storage
            .load_task(&task_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        // Execute using the queue
        self.queue.enqueue(task.clone()).await?;

        // In a real implementation, the queue worker would execute and store the result
        Ok(task.success_result(
            serde_json::json!({"status": "queued"}),
            std::time::Duration::ZERO,
        ))
    }

    /// Run a task synchronously using the shell runner with disk-backed cache.
    pub async fn run_task(&self, task_id: &TaskId) -> Result<TaskResult, TaskError> {
        // Check cache first (disk-backed persistent cache)
        if let Some(cached) = self.cache.get(task_id) {
            return Ok(cached);
        }

        let mut task = self
            .storage
            .load_task(&task_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        let runner = ShellRunner::new();
        let result = runner.execute(&mut task)?;

        // Save updated task state back to storage
        self.storage.save_task(&task).await?;

        // Cache successful results to disk
        if result.success {
            self.cache
                .insert(task_id.clone(), result.clone())
                .map_err(|e| {
                    TaskError::StorageError(format!("cache persist failed: {e}"))
                })?;
        }

        Ok(result)
    }

    /// Save a task directly (primarily for testing).
    pub async fn save_task(&self, task: &Task) -> Result<(), TaskError> {
        self.storage.save_task(task).await?;
        Ok(())
    }

    /// Create a workflow.
    pub async fn create_workflow(&self, workflow: Workflow) -> Result<Workflow, TaskError> {
        self.storage.save_workflow(&workflow).await?;
        Ok(workflow)
    }

    /// Get a workflow by ID.
    pub async fn get_workflow(&self, id: &WorkflowId) -> Result<Option<Workflow>, TaskError> {
        self.storage.load_workflow(&id.0).await.map_err(Into::into)
    }

    /// List all workflows.
    pub async fn list_workflows(&self) -> Result<Vec<Workflow>, TaskError> {
        self.storage.list_workflows().await.map_err(Into::into)
    }

    /// Execute a workflow.
    pub async fn execute_workflow(&self, workflow_id: &WorkflowId) -> Result<Vec<TaskResult>, TaskError> {
        let mut workflow = self
            .storage
            .load_workflow(&workflow_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(workflow_id.0.clone()))?;

        workflow.build_dag().map_err(|e| TaskError::InvalidOperation(e))?;

        let order = workflow.execution_order().map_err(|e| TaskError::InvalidOperation(e))?;
        let mut results = Vec::new();

        for step in order {
            if let Some(ref task_id) = step.task_id {
                let result = self.run_task(task_id).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// List tasks in dependency order (topological sort).
    /// Tasks with no dependencies come first.
    pub async fn list_tasks_sorted(
        &self,
        state_filter: Option<TaskState>,
        tag_filter: Option<String>,
    ) -> Result<Vec<Task>, TaskError> {
        let mut tasks = self.storage.list_tasks().await?;

        // Apply filters
        if let Some(state) = state_filter {
            tasks.retain(|t| t.state == state);
        }
        if let Some(tag) = tag_filter {
            tasks.retain(|t| t.tags.contains(&tag));
        }

        // Topological sort by dependency graph
        Ok(crate::domain::topological_sort_tasks(&tasks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::secondary::memory::MemoryStorage;
    use crate::domain::tasks::Priority;
    use crate::domain::workflows::WorkflowStep;

    fn setup_service() -> TaskService {
        let storage = Arc::new(MemoryStorage::new());
        let queue = Arc::new(MemoryStorage::new());
        TaskService::new(storage, queue)
    }

    #[tokio::test]
    async fn test_create_task() {
        let service = setup_service();
        let cmd = CreateTask::new("test-task").with_priority(Priority::High);
        let task = service.create_task(cmd).await.unwrap();

        assert_eq!(task.name, "test-task");
        assert_eq!(task.priority, Priority::High);
        assert_eq!(task.state, TaskState::Pending);
    }

    #[tokio::test]
    async fn test_get_task() {
        let service = setup_service();
        let cmd = CreateTask::new("test-task");
        let created = service.create_task(cmd).await.unwrap();

        let found = service.get_task(&created.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test-task");

        let not_found = service.get_task(&TaskId::from_string("missing")).await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_list_tasks_with_filters() {
        let service = setup_service();
        let cmd1 = CreateTask::new("task-1").with_tag("dev");
        let task1 = service.create_task(cmd1).await.unwrap();

        let cmd2 = CreateTask::new("task-2").with_tag("prod");
        let _task2 = service.create_task(cmd2).await.unwrap();

        // Filter by state
        let pending = service.list_tasks(Some(TaskState::Pending), None, None).await.unwrap();
        assert_eq!(pending.len(), 2);

        // Filter by tag
        let dev = service.list_tasks(None, Some("dev".to_string()), None).await.unwrap();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].id, task1.id);

        // Filter by limit
        let limited = service.list_tasks(None, None, Some(1)).await.unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let service = setup_service();
        let cmd = CreateTask::new("cancel-me");
        let task = service.create_task(cmd).await.unwrap();

        service.cancel_task(task.id.clone(), Some("test".to_string())).await.unwrap();

        let found = service.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(found.state, TaskState::Cancelled);
    }

    #[tokio::test]
    async fn test_cancel_task_not_found() {
        let service = setup_service();
        let result = service.cancel_task(TaskId::from_string("missing"), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_retry_task() {
        let service = setup_service();
        let mut task = Task::new("retry-test");
        task.retry_policy = Some(crate::domain::tasks::RetryPolicy::default());
        task.state = TaskState::Failed;
        service.storage.save_task(&task).await.unwrap();

        let retried = service.retry_task(task.id.clone()).await.unwrap();
        assert_eq!(retried.state, TaskState::Pending);
        assert_eq!(retried.retry_count, 1);
    }

    #[tokio::test]
    async fn test_retry_task_not_failed() {
        let service = setup_service();
        let cmd = CreateTask::new("not-failed");
        let task = service.create_task(cmd).await.unwrap();

        let result = service.retry_task(task.id.clone()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_task() {
        let service = setup_service();
        let cmd = CreateTask::new("run-test").with_command("echo hello");
        let task = service.create_task(cmd).await.unwrap();

        let result = service.run_task(&task.id).await.unwrap();
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.get("stdout").unwrap().as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_run_task_not_found() {
        let service = setup_service();
        let result = service.run_task(&TaskId::from_string("missing")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_create_and_get_workflow() {
        let service = setup_service();
        let workflow = Workflow::new("test-workflow");
        let created = service.create_workflow(workflow.clone()).await.unwrap();

        let found = service.get_workflow(&created.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test-workflow");
    }

    #[tokio::test]
    async fn test_run_task_caches_successful_results() {
        let service = setup_service();
        let cmd = CreateTask::new("cache-test").with_command("echo cached");
        let task = service.create_task(cmd).await.unwrap();

        // First run should execute
        let result1 = service.run_task(&task.id).await.unwrap();
        assert!(result1.success);

        // Second run should return cached result
        let result2 = service.run_task(&task.id).await.unwrap();
        assert!(result2.success);
        // Should be the same result (cached)
        assert_eq!(result1.timestamp, result2.timestamp);
    }

    #[tokio::test]
    async fn test_run_task_failure_not_cached() {
        let service = setup_service();
        let cmd = CreateTask::new("fail-cache").with_command("false");
        let task = service.create_task(cmd).await.unwrap();

        let result1 = service.run_task(&task.id).await.unwrap();
        assert!(!result1.success);

        // Failure should not be cached
        let result2 = service.run_task(&task.id).await.unwrap();
        assert!(!result2.success);
        // Timestamps should differ since it re-ran
        assert_ne!(result1.timestamp, result2.timestamp);
    }

    #[tokio::test]
    async fn test_execute_workflow() {
        let service = setup_service();
        let cmd = CreateTask::new("step-a").with_command("echo step-a");
        let task_a = service.create_task(cmd).await.unwrap();

        let cmd = CreateTask::new("step-b").with_command("echo step-b");
        let task_b = service.create_task(cmd).await.unwrap();

        let workflow = Workflow::new("test-flow")
            .with_step(WorkflowStep::new("step-a").with_task(task_a.id.clone()))
            .with_step(WorkflowStep::new("step-b").with_task(task_b.id.clone()).with_dependency("step-a"));

        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].success);
        assert!(results[1].success);
    }

    #[tokio::test]
    async fn test_list_tasks_sorted_by_dependency() {
        let service = setup_service();

        let t1 = service
            .create_task(CreateTask::new("build").with_command("echo build"))
            .await
            .unwrap();

        let t2 = service
            .create_task(
                CreateTask::new("test").with_command("echo test").with_dependency(t1.id.clone()),
            )
            .await
            .unwrap();

        let t3 = service
            .create_task(
                CreateTask::new("deploy")
                    .with_command("echo deploy")
                    .with_dependency(t2.id.clone()),
            )
            .await
            .unwrap();

        let sorted = service.list_tasks_sorted(None, None).await.unwrap();
        assert_eq!(sorted.len(), 3);
        // build must come before test, test before deploy
        assert_eq!(sorted[0].name, "build");
        assert_eq!(sorted[1].name, "test");
        assert_eq!(sorted[2].name, "deploy");
    }

    #[tokio::test]
    async fn test_list_tasks_sorted_with_state_filter() {
        let service = setup_service();

        let t1 = service
            .create_task(CreateTask::new("build").with_command("echo build"))
            .await
            .unwrap();

        let t2 = service
            .create_task(
                CreateTask::new("test").with_command("echo test").with_dependency(t1.id.clone()),
            )
            .await
            .unwrap();

        // Cancel t2
        service.cancel_task(t2.id.clone(), Some("filtered".to_string())).await.unwrap();

        // Only pending tasks should be returned
        let sorted = service
            .list_tasks_sorted(Some(TaskState::Pending), None)
            .await
            .unwrap();
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].name, "build");
    }
}
