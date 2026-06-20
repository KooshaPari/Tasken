// SPDX-License-Identifier: MIT OR Apache-2.0
//! Task application service.

use super::commands::CreateTask;
use crate::config::TaskenConfig;
use crate::domain::errors::TaskError;
use crate::domain::ports::{QueuePort, StoragePort};
use crate::domain::runners::{ShellRunner, TaskRunner};
use crate::domain::tasks::{Task, TaskId, TaskState};
use crate::domain::workflows::{Workflow, WorkflowId};
use crate::domain::{events::TaskEvent, Group, GroupId, TaskResult};
use chrono::Utc;
use std::sync::Arc;

/// Task application service.
#[derive(Clone)]
pub struct TaskService {
    pub(crate) storage: Arc<dyn StoragePort>,
    pub(crate) queue: Arc<dyn QueuePort>,
    cache: Arc<crate::infrastructure::PersistentTaskCache>,
}

impl TaskService {
    /// Create a new task service with default configuration.
    pub fn new(storage: Arc<dyn StoragePort>, queue: Arc<dyn QueuePort>) -> Self {
        let config = TaskenConfig::default();
        Self::with_config(storage, queue, &config)
    }

    /// Create a new task service using the given configuration.
    ///
    /// The cache path and TTL are derived from `config`.
    pub fn with_config(
        storage: Arc<dyn StoragePort>,
        queue: Arc<dyn QueuePort>,
        config: &TaskenConfig,
    ) -> Self {
        let cache_path = config.cache_path();
        let cache_ttl = config.cache_ttl();

        // Ensure cache parent directory exists
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let cache = crate::infrastructure::PersistentTaskCache::open(
            &cache_path,
            cache_ttl,
        )
        .unwrap_or_else(|_| {
            crate::infrastructure::PersistentTaskCache::ephemeral(cache_ttl)
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
    ///
    /// When `dry_run` is `true`, the task command is printed to stderr
    /// and no side effects (shell execution, storage writes, caching)
    /// are performed. A simulated successful result is returned.
    pub async fn run_task(
        &self,
        task_id: &TaskId,
        dry_run: bool,
    ) -> Result<TaskResult, TaskError> {
        // Check cache first (disk-backed persistent cache)
        if !dry_run {
            if let Some(cached) = self.cache.get(task_id) {
                return Ok(cached);
            }
        }

        let task = self
            .storage
            .load_task(&task_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(task_id.0.clone()))?;

        if dry_run {
            let cmd = task
                .data
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            eprintln!("[dry-run] would run task '{}'", task_id.0);
            eprintln!("[dry-run]   command: {}", cmd);
            return Ok(TaskResult {
                task_id: task_id.clone(),
                success: true,
                output: None,
                error: None,
                duration: std::time::Duration::ZERO,
                timestamp: chrono::Utc::now(),
            });
        }

        let mut task = task;
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

    /// Create a group.
    pub async fn create_group(&self, group: Group) -> Result<Group, TaskError> {
        self.storage.save_group(&group).await?;
        Ok(group)
    }

    /// Get a group by ID.
    pub async fn get_group(&self, id: &GroupId) -> Result<Option<Group>, TaskError> {
        self.storage.load_group(&id.0).await.map_err(Into::into)
    }

    /// List all groups.
    pub async fn list_groups(&self) -> Result<Vec<Group>, TaskError> {
        self.storage.list_groups().await.map_err(Into::into)
    }

    /// Run all tasks in a group.
    ///
    /// Loads each task referenced by the group and runs it sequentially.
    /// Returns the aggregate results in task order.
    pub async fn run_group(&self, group_id: &GroupId, dry_run: bool) -> Result<Vec<TaskResult>, TaskError> {
        let group = self
            .storage
            .load_group(&group_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(group_id.0.clone()))?;

        let mut results = Vec::with_capacity(group.task_ids.len());
        for task_id in &group.task_ids {
            let result = self.run_task(task_id, dry_run).await?;
            results.push(result);
        }
        Ok(results)
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

    /// Execute a workflow with parallel step execution.
    ///
    /// Steps whose dependencies are all satisfied run concurrently in each wave.
    /// Waves advance as steps complete, respecting the DAG's partial order.
    ///
    /// When `dry_run` is `true`, each step's command is printed to stderr
    /// instead of being executed (passed through to [`Self::run_task`]).
    pub async fn execute_workflow(
        &self,
        workflow_id: &WorkflowId,
        dry_run: bool,
    ) -> Result<Vec<TaskResult>, TaskError> {
        let mut workflow = self
            .storage
            .load_workflow(&workflow_id.0)
            .await?
            .ok_or_else(|| TaskError::NotFound(workflow_id.0.clone()))?;

        workflow.build_dag().map_err(|e| TaskError::InvalidOperation(e))?;

        let mut completed_step_ids: Vec<String> = Vec::new();
        let mut all_results: Vec<TaskResult> = Vec::new();
        let total_steps = workflow.steps.len();

        // Wave-based parallel execution: each wave is the set of steps whose
        // dependencies are fully satisfied by the previously-completed waves.
        while completed_step_ids.len() < total_steps {
            let ready: Vec<_> = workflow
                .ready_steps(&completed_step_ids)
                .into_iter()
                .cloned()
                .collect();

            if ready.is_empty() {
                // No runnable steps but not all done — cycle or broken DAG.
                return Err(TaskError::InvalidOperation(
                    "Workflow stalled: no ready steps but execution is incomplete".to_string(),
                ));
            }

            // Launch all ready steps concurrently.
            let wave_futures: Vec<_> = ready
                .iter()
                .filter_map(|step| step.task_id.as_ref().map(|tid| self.run_task(tid, dry_run)))
                .collect();

            let wave_results = futures::future::try_join_all(wave_futures).await?;

            // Record which steps finished (whether or not they had a task_id).
            for step in &ready {
                completed_step_ids.push(step.id.clone());
            }
            all_results.extend(wave_results);
        }

        Ok(all_results)
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
        TaskService::with_cache(
            storage,
            queue,
            Arc::new(crate::infrastructure::PersistentTaskCache::ephemeral(
                std::time::Duration::from_secs(300),
            )),
        )
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

        let result = service.run_task(&task.id, false).await.unwrap();
        assert!(result.success);
        let output = result.output.unwrap();
        assert!(output.get("stdout").unwrap().as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_run_task_not_found() {
        let service = setup_service();
        let result = service.run_task(&TaskId::from_string("missing"), false).await;
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
        let result1 = service.run_task(&task.id, false).await.unwrap();
        assert!(result1.success);

        // Second run should return cached result
        let result2 = service.run_task(&task.id, false).await.unwrap();
        assert!(result2.success);
        // Should be the same result (cached)
        assert_eq!(result1.timestamp, result2.timestamp);
    }

    #[tokio::test]
    async fn test_run_task_failure_not_cached() {
        let service = setup_service();
        let cmd = CreateTask::new("fail-cache").with_command("false");
        let task = service.create_task(cmd).await.unwrap();

        let result1 = service.run_task(&task.id, false).await.unwrap();
        assert!(!result1.success);

        // Failure should not be cached
        let result2 = service.run_task(&task.id, false).await.unwrap();
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
        let results = service.execute_workflow(&created.id, false).await.unwrap();

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

    /// Diamond DAG: A → (B, C) → D.
    /// B and C have no inter-dependency and must run in the same wave.
    #[tokio::test]
    async fn test_execute_workflow_parallel_diamond() {
        let service = setup_service();

        let task_a = service.create_task(CreateTask::new("a").with_command("echo a")).await.unwrap();
        let task_b = service.create_task(CreateTask::new("b").with_command("echo b")).await.unwrap();
        let task_c = service.create_task(CreateTask::new("c").with_command("echo c")).await.unwrap();
        let task_d = service.create_task(CreateTask::new("d").with_command("echo d")).await.unwrap();

        // Diamond: a → b, a → c, (b ∧ c) → d
        let workflow = Workflow::new("diamond")
            .with_step(WorkflowStep::new("a").with_task(task_a.id.clone()))
            .with_step(WorkflowStep::new("b").with_task(task_b.id.clone()).with_dependency("a"))
            .with_step(WorkflowStep::new("c").with_task(task_c.id.clone()).with_dependency("a"))
            .with_step(
                WorkflowStep::new("d")
                    .with_task(task_d.id.clone())
                    .with_dependency("b")
                    .with_dependency("c"),
            );

        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();

        // All four tasks must succeed.
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|r| r.success));
    }

    /// Fork DAG: A → (B, C, D) — three independent branches off a single root.
    #[tokio::test]
    async fn test_execute_workflow_parallel_fork() {
        let service = setup_service();

        let task_a = service.create_task(CreateTask::new("root").with_command("echo root")).await.unwrap();
        let task_b = service.create_task(CreateTask::new("branch-1").with_command("echo b1")).await.unwrap();
        let task_c = service.create_task(CreateTask::new("branch-2").with_command("echo b2")).await.unwrap();
        let task_d = service.create_task(CreateTask::new("branch-3").with_command("echo b3")).await.unwrap();

        let workflow = Workflow::new("fork")
            .with_step(WorkflowStep::new("a").with_task(task_a.id.clone()))
            .with_step(WorkflowStep::new("b").with_task(task_b.id.clone()).with_dependency("a"))
            .with_step(WorkflowStep::new("c").with_task(task_c.id.clone()).with_dependency("a"))
            .with_step(WorkflowStep::new("d").with_task(task_d.id.clone()).with_dependency("a"));

        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();

        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|r| r.success));
    }

    /// Stall detection: artificially inject an impossible dependency to trigger the stall error.
    #[tokio::test]
    async fn test_execute_workflow_stall_is_detected() {
        use crate::domain::workflows::WorkflowStep;

        let service = setup_service();
        let task_a = service.create_task(CreateTask::new("alone").with_command("echo alone")).await.unwrap();

        // Manually build a workflow where "a" depends on a ghost step that never completes.
        let mut ghost_step = WorkflowStep::new("ghost");
        ghost_step.task_id = None; // no task — the wave executor skips it for results but still marks it done
        // We actually want to test a broken dependency reference (step depends on non-existent step).
        // Simulate by building the workflow with a step that references a dep not in the graph.
        let mut bad_step = WorkflowStep::new("bad");
        bad_step.task_id = Some(task_a.id.clone());
        bad_step.depends_on = vec!["nonexistent-step".to_string()];

        let workflow = Workflow::new("stall-test")
            .with_step(bad_step);

        // build_dag succeeds (the missing dep is simply not added as an edge),
        // but ready_steps will never return "bad" because "nonexistent-step" is never completed.
        // This exercises the stall-detection branch.
        let created = service.create_workflow(workflow).await.unwrap();
        let err = service.execute_workflow(&created.id, false).await.unwrap_err();
        assert!(
            matches!(err, TaskError::InvalidOperation(_)),
            "expected InvalidOperation stall error, got: {err:?}"
        );
    }
}
