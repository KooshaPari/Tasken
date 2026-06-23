// SPDX-License-Identifier: MIT OR Apache-2.0
// Integration tests for dependency cycle detection and retry storm scenarios.
//
// These tests exercise cross-module behaviour that inline `#[cfg(test)]`
// blocks cannot reach: multi-task service integration with concurrent
// retry logic, workflow DAG cycle detection, and topological sort
// validation at the integration level.
//
// Run with: `cargo test --test integration`

mod dependency_cycles {
    use std::sync::Arc;

    use taskkit::adapters::secondary::memory::MemoryStorage;
    use taskkit::application::services::TaskService;
    use taskkit::application::CreateTask;
    use taskkit::domain::tasks::topological_sort_tasks;
    use taskkit::domain::workflows::{Workflow, WorkflowStep};
    use taskkit::infrastructure::PersistentTaskCache;

    fn setup_service() -> Arc<TaskService> {
        let storage = Arc::new(MemoryStorage::new());
        let queue = Arc::new(MemoryStorage::new());
        Arc::new(TaskService::new(storage, queue))
    }

    fn setup_service_with_cache() -> Arc<TaskService> {
        let storage = Arc::new(MemoryStorage::new());
        let queue = Arc::new(MemoryStorage::new());
        let cache = Arc::new(PersistentTaskCache::ephemeral(std::time::Duration::from_secs(300)));
        Arc::new(TaskService::with_cache(storage, queue, cache))
    }

    // -----------------------------------------------------------------------
    // Linear DAG  (a → b → c)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_linear_dag_workflow() {
        let service = setup_service_with_cache();
        let task_a =
            service.create_task(CreateTask::new("a").with_command("echo a")).await.unwrap();
        let task_b =
            service.create_task(CreateTask::new("b").with_command("echo b")).await.unwrap();
        let task_c =
            service.create_task(CreateTask::new("c").with_command("echo c")).await.unwrap();

        let workflow = Workflow::new("linear")
            .with_step(WorkflowStep::new("step-a").with_task(task_a.id.clone()))
            .with_step(
                WorkflowStep::new("step-b").with_task(task_b.id.clone()).with_dependency("step-a"),
            )
            .with_step(
                WorkflowStep::new("step-c").with_task(task_c.id.clone()).with_dependency("step-b"),
            );

        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();

        assert_eq!(results.len(), 3, "all three steps must produce a result");
        assert!(results.iter().all(|r| r.success), "every step must succeed");
    }

    #[tokio::test]
    async fn test_linear_dag_topological_sort() {
        let service = setup_service();
        let task_a =
            service.create_task(CreateTask::new("build").with_command("echo build")).await.unwrap();
        let task_b = service
            .create_task(
                CreateTask::new("test")
                    .with_command("echo test")
                    .with_dependency(task_a.id.clone()),
            )
            .await
            .unwrap();
        let task_c = service
            .create_task(
                CreateTask::new("deploy")
                    .with_command("echo deploy")
                    .with_dependency(task_b.id.clone()),
            )
            .await
            .unwrap();
        let _ = (task_a, task_b, task_c);

        let sorted = service.list_tasks_sorted(None, None).await.unwrap();
        assert_eq!(sorted.len(), 3, "all three tasks must appear in sorted order");

        let names: Vec<&str> = sorted.iter().map(|t| t.name.as_str()).collect();
        let pos_build = names.iter().position(|n| *n == "build").unwrap();
        let pos_test = names.iter().position(|n| *n == "test").unwrap();
        let pos_deploy = names.iter().position(|n| *n == "deploy").unwrap();
        assert!(pos_build < pos_test, "build must be sorted before test");
        assert!(pos_test < pos_deploy, "test must be sorted before deploy");
    }

    // -----------------------------------------------------------------------
    // Diamond DAG  (a → {b, c} → d)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_diamond_dag_workflow() {
        let service = setup_service_with_cache();
        let task_a =
            service.create_task(CreateTask::new("root").with_command("echo root")).await.unwrap();
        let task_b =
            service.create_task(CreateTask::new("left").with_command("echo left")).await.unwrap();
        let task_c =
            service.create_task(CreateTask::new("right").with_command("echo right")).await.unwrap();
        let task_d =
            service.create_task(CreateTask::new("leaf").with_command("echo leaf")).await.unwrap();

        let workflow = Workflow::new("diamond")
            .with_step(WorkflowStep::new("root").with_task(task_a.id.clone()))
            .with_step(
                WorkflowStep::new("left").with_task(task_b.id.clone()).with_dependency("root"),
            )
            .with_step(
                WorkflowStep::new("right").with_task(task_c.id.clone()).with_dependency("root"),
            )
            .with_step(
                WorkflowStep::new("leaf")
                    .with_task(task_d.id.clone())
                    .with_dependency("left")
                    .with_dependency("right"),
            );

        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();

        assert_eq!(results.len(), 4, "all four steps must produce a result");
        assert!(results.iter().all(|r| r.success), "every step must succeed");
    }

    // -----------------------------------------------------------------------
    // Circular dependency detection  (a → b → a)
    // -----------------------------------------------------------------------

    #[test]
    fn test_circular_dependency_detected_via_topological_sort() {
        let mut t1 = taskkit::domain::tasks::Task::new("a");
        let t2 = taskkit::domain::tasks::Task::new("b").with_dependency(t1.id.clone());
        t1.depends_on.push(t2.id.clone());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            topological_sort_tasks(&[t1, t2]);
        }));
        assert!(result.is_err(), "topological_sort_tasks must panic on a circular dependency");
    }

    #[tokio::test]
    async fn test_circular_dependency_detected_via_workflow() {
        let _service = setup_service();
        let mut step_a = WorkflowStep::new("a");
        step_a.depends_on = vec!["b".to_string()];
        let mut step_b = WorkflowStep::new("b");
        step_b.depends_on = vec!["a".to_string()];

        let mut workflow = Workflow::new("cycle-test").with_step(step_a).with_step(step_b);

        let err = workflow.build_dag().unwrap_err();
        assert!(err.contains("cycle"), "build_dag error must mention 'cycle', got: {err}");
    }

    #[tokio::test]
    async fn test_circular_dependency_detected_via_execute() {
        let service = setup_service();
        let task_a =
            service.create_task(CreateTask::new("a").with_command("echo a")).await.unwrap();
        let task_b =
            service.create_task(CreateTask::new("b").with_command("echo b")).await.unwrap();

        let mut workflow = Workflow::new("cycle-exec")
            .with_step(WorkflowStep::new("a").with_task(task_a.id.clone()).with_dependency("b"))
            .with_step(WorkflowStep::new("b").with_task(task_b.id.clone()).with_dependency("a"));

        let err = workflow.build_dag().unwrap_err();
        assert!(err.contains("cycle"), "execute_workflow must reject cycle, got: {err}");
    }

    // -----------------------------------------------------------------------
    // Self-referencing cycle  (a → a)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_self_referencing_cycle_detected() {
        let service = setup_service();
        let task_a =
            service.create_task(CreateTask::new("a").with_command("echo a")).await.unwrap();

        let mut workflow = Workflow::new("self-cycle")
            .with_step(WorkflowStep::new("a").with_task(task_a.id.clone()).with_dependency("a"));

        let err = workflow.build_dag().unwrap_err();
        assert!(
            err.contains("cycle"),
            "self-referencing step must be detected as a cycle, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Complex multi-node cycle  (a → b → c → a)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_complex_cycle_detected() {
        let service = setup_service();
        let task_a =
            service.create_task(CreateTask::new("a").with_command("echo a")).await.unwrap();
        let task_b =
            service.create_task(CreateTask::new("b").with_command("echo b")).await.unwrap();
        let task_c =
            service.create_task(CreateTask::new("c").with_command("echo c")).await.unwrap();

        let mut workflow = Workflow::new("complex-cycle")
            .with_step(WorkflowStep::new("a").with_task(task_a.id.clone()).with_dependency("c"))
            .with_step(WorkflowStep::new("b").with_task(task_b.id.clone()).with_dependency("a"))
            .with_step(WorkflowStep::new("c").with_task(task_c.id.clone()).with_dependency("b"));

        let err = workflow.build_dag().unwrap_err();
        assert!(err.contains("cycle"), "a → b → c → a must be detected as a cycle, got: {err}");
    }

    // -----------------------------------------------------------------------
    // No dependencies  (empty dep list, independent tasks)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_independent_tasks_topological_sort() {
        let service = setup_service();
        let task_a =
            service.create_task(CreateTask::new("alpha").with_command("echo alpha")).await.unwrap();
        let task_b =
            service.create_task(CreateTask::new("beta").with_command("echo beta")).await.unwrap();
        let task_c =
            service.create_task(CreateTask::new("gamma").with_command("echo gamma")).await.unwrap();
        let _ = (task_a, task_b, task_c);

        let sorted = service.list_tasks_sorted(None, None).await.unwrap();
        assert_eq!(sorted.len(), 3, "all three independent tasks must appear");
    }

    // -----------------------------------------------------------------------
    // Fork DAG  (a → {b, c, d})
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_fork_dag_workflow() {
        let service = setup_service_with_cache();
        let task_a =
            service.create_task(CreateTask::new("root").with_command("echo root")).await.unwrap();
        let task_b =
            service.create_task(CreateTask::new("b1").with_command("echo b1")).await.unwrap();
        let task_c =
            service.create_task(CreateTask::new("b2").with_command("echo b2")).await.unwrap();
        let task_d =
            service.create_task(CreateTask::new("b3").with_command("echo b3")).await.unwrap();

        let workflow = Workflow::new("fork")
            .with_step(WorkflowStep::new("root").with_task(task_a.id.clone()))
            .with_step(WorkflowStep::new("b1").with_task(task_b.id.clone()).with_dependency("root"))
            .with_step(WorkflowStep::new("b2").with_task(task_c.id.clone()).with_dependency("root"))
            .with_step(
                WorkflowStep::new("b3").with_task(task_d.id.clone()).with_dependency("root"),
            );

        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();

        assert_eq!(results.len(), 4, "all four fork steps must succeed");
        assert!(results.iter().all(|r| r.success));
    }

    // -----------------------------------------------------------------------
    // Empty workflow
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_empty_workflow() {
        let service = setup_service();
        let workflow = Workflow::new("empty");
        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();
        assert!(results.is_empty(), "empty workflow must produce zero results");
    }

    // -----------------------------------------------------------------------
    // Single-step workflow
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_single_step_workflow() {
        let service = setup_service_with_cache();
        let task =
            service.create_task(CreateTask::new("solo").with_command("echo solo")).await.unwrap();
        let workflow =
            Workflow::new("single").with_step(WorkflowStep::new("only").with_task(task.id.clone()));
        let created = service.create_workflow(workflow).await.unwrap();
        let results = service.execute_workflow(&created.id, false).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }
}

mod retry_storm {
    use std::sync::Arc;
    use std::time::Duration;

    use taskkit::adapters::secondary::memory::MemoryStorage;
    use taskkit::application::services::TaskService;
    use taskkit::application::CreateTask;
    use taskkit::domain::tasks::{RetryPolicy, Task, TaskId, TaskState};
    use taskkit::infrastructure::PersistentTaskCache;

    fn setup_service() -> Arc<TaskService> {
        let storage = Arc::new(MemoryStorage::new());
        let queue = Arc::new(MemoryStorage::new());
        let cache = Arc::new(PersistentTaskCache::ephemeral(Duration::from_secs(300)));
        Arc::new(TaskService::with_cache(storage, queue, cache))
    }

    async fn make_failed_task(service: &TaskService, name: &str, max_attempts: u32) -> Task {
        let policy = RetryPolicy {
            max_attempts,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_secs(1),
            jitter: 0.0,
        };
        let cmd = CreateTask::new(name).with_command("echo ok").with_retry_policy(policy);
        let task = service.create_task(cmd).await.unwrap();
        let mut failed = task;
        failed.state = TaskState::Failed;
        failed.retry_count = 0;
        failed.error = Some("simulated transient failure".to_string());
        service.save_task(&failed).await.unwrap();
        failed
    }

    async fn taskclone_for_retry(service: &TaskService, task_id: &TaskId) -> Task {
        service.get_task(task_id).await.unwrap().expect("task must exist")
    }

    async fn refail_task(service: &TaskService, task_id: &TaskId) {
        let mut task = taskclone_for_retry(service, task_id).await;
        task.state = TaskState::Failed;
        task.error = Some("refailed".to_string());
        service.save_task(&task).await.unwrap();
    }

    // -----------------------------------------------------------------------
    // Single-task retry logic
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_transient_failure_retried() {
        let service = setup_service();
        let task = make_failed_task(&service, "transient", 3).await;

        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.retry_count, 0);

        let retried = service.retry_task(task.id.clone()).await.unwrap();

        assert_eq!(retried.state, TaskState::Pending, "retried task must be reset to Pending");
        assert_eq!(retried.retry_count, 1, "retry_count must be incremented from 0 to 1");
        assert!(retried.error.is_none(), "error must be cleared on retry");
    }

    #[tokio::test]
    async fn test_retry_after_failed_run_completes_successfully() {
        let service = setup_service();
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_secs(1),
            jitter: 0.0,
        };

        let cmd =
            CreateTask::new("retry-cycle").with_command("echo hello").with_retry_policy(policy);
        let task = service.create_task(cmd).await.unwrap();
        let result = service.run_task(&task.id, false).await.unwrap();
        assert!(result.success, "first run must succeed");

        let mut failed = taskclone_for_retry(&service, &task.id).await;
        failed.state = TaskState::Failed;
        failed.error = Some("simulated".to_string());
        failed.retry_count = 0;
        service.save_task(&failed).await.unwrap();

        let _retried = service.retry_task(task.id.clone()).await.unwrap();
        let result2 = service.run_task(&task.id, false).await.unwrap();
        assert!(result2.success, "retried task must run successfully");
    }

    // -----------------------------------------------------------------------
    // Permanent failure — retry limit exceeded
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_permanent_failure_not_retried_beyond_max() {
        let service = setup_service();
        let task = make_failed_task(&service, "permanent", 1).await;

        let retried = service.retry_task(task.id.clone()).await.unwrap();
        assert_eq!(retried.retry_count, 1);

        let mut refailed = taskclone_for_retry(&service, &task.id).await;
        refailed.state = TaskState::Failed;
        refailed.error = Some("still failing".to_string());
        service.save_task(&refailed).await.unwrap();

        let err = service.retry_task(task.id.clone()).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("limit") || msg.contains("Retry"),
            "error must mention retry limit, got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_retry_limit_exact_max() {
        let service = setup_service();
        let task = make_failed_task(&service, "limit-exact", 3).await;

        let r1 = service.retry_task(task.id.clone()).await.unwrap();
        assert_eq!(r1.retry_count, 1);
        refail_task(&service, &task.id).await;

        let r2 = service.retry_task(task.id.clone()).await.unwrap();
        assert_eq!(r2.retry_count, 2);
        refail_task(&service, &task.id).await;

        let r3 = service.retry_task(task.id.clone()).await.unwrap();
        assert_eq!(r3.retry_count, 3);
        refail_task(&service, &task.id).await;

        let err = service.retry_task(task.id.clone()).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("limit") || msg.contains("Retry"),
            "error must mention retry limit, got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Retry of a successful task  (invalid state transition)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_retry_successful_task_rejected() {
        let service = setup_service();
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_secs(1),
            jitter: 0.0,
        };

        let cmd = CreateTask::new("successful").with_command("echo ok").with_retry_policy(policy);
        let task = service.create_task(cmd).await.unwrap();
        service.run_task(&task.id, false).await.unwrap();

        let err = service.retry_task(task.id.clone()).await.unwrap_err();
        assert!(
            err.to_string().contains("Invalid"),
            "retry of a successful task must give InvalidStateTransition, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Retry storm — 10 concurrent retries
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_concurrent_retry_storm() {
        let service = setup_service();
        let count = 10;

        let mut task_ids = Vec::with_capacity(count);
        for i in 0..count {
            let task = make_failed_task(&service, &format!("storm-{i}"), 5).await;
            task_ids.push(task.id.clone());
        }

        let mut handles = Vec::with_capacity(count);
        for tid in &task_ids {
            let svc = service.clone();
            let id = tid.clone();
            handles.push(tokio::spawn(async move { svc.retry_task(id).await }));
        }

        let mut successes = 0usize;
        for handle in handles {
            match handle.await.unwrap() {
                Ok(task) => {
                    successes += 1;
                    assert_eq!(task.state, TaskState::Pending, "each retried task must be Pending");
                    assert_eq!(
                        task.retry_count, 1,
                        "each task must have retry_count = 1 after first retry"
                    );
                }
                Err(e) => {
                    eprintln!("retry storm task failed: {e}");
                }
            }
        }

        assert_eq!(successes, count, "all {count} concurrent retries must succeed");
    }

    // -----------------------------------------------------------------------
    // Retry storm — mixed transient and permanent failures
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_concurrent_retry_mixed_failures() {
        let service = setup_service();

        let mut transient_ids = Vec::with_capacity(5);
        for i in 0..5 {
            transient_ids.push(make_failed_task(&service, &format!("trans-{i}"), 3).await);
        }
        let mut permanent_ids = Vec::with_capacity(5);
        for i in 0..5 {
            permanent_ids.push(make_failed_task(&service, &format!("perm-{i}"), 1).await);
        }

        async fn retry_all(
            service: &TaskService,
            ids: &[TaskId],
        ) -> Vec<Result<Task, taskkit::domain::errors::TaskError>> {
            let handles: Vec<_> = ids
                .iter()
                .map(|tid| {
                    let svc = service.clone();
                    let id = tid.clone();
                    tokio::spawn(async move { svc.retry_task(id).await })
                })
                .collect();
            let mut results = Vec::with_capacity(ids.len());
            for h in handles {
                results.push(h.await.unwrap());
            }
            results
        }

        let all_ids: Vec<TaskId> =
            transient_ids.iter().chain(permanent_ids.iter()).map(|t| t.id.clone()).collect();

        let wave1 = retry_all(&service, &all_ids).await;
        let wave1_ok = wave1.iter().filter(|r| r.is_ok()).count();
        assert_eq!(wave1_ok, 10, "first retry wave: all 10 tasks must succeed");

        for tid in &all_ids {
            refail_task(&service, tid).await;
        }

        let wave2 = retry_all(&service, &all_ids).await;

        let trans_ok = wave2[0..5].iter().filter(|r| r.is_ok()).count();
        let perm_err = wave2[5..10].iter().filter(|r| r.is_err()).count();

        assert_eq!(trans_ok, 5, "transient tasks: all 5 must still succeed on retry #2");
        assert_eq!(perm_err, 5, "permanent tasks: all 5 must fail on retry #2");
    }

    // -----------------------------------------------------------------------
    // Retry with no retry policy  (can_retry returns false)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_retry_without_policy_rejected() {
        let service = setup_service();
        let cmd = CreateTask::new("no-policy").with_command("echo hi");
        let task = service.create_task(cmd).await.unwrap();

        let mut failed = taskclone_for_retry(&service, &task.id).await;
        failed.state = TaskState::Failed;
        failed.error = Some("no policy".to_string());
        failed.retry_policy = None;
        service.save_task(&failed).await.unwrap();

        let err = service.retry_task(task.id.clone()).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("limit") || msg.contains("Retry"),
            "retry without policy must fail, got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Exponential backoff calculation
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_exponential_backoff_delays() {
        let policy = RetryPolicy {
            max_attempts: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            jitter: 0.0,
        };

        let mut task = Task::new("backoff-test").with_retry_policy(policy);

        task.retry_count = 0;
        assert_eq!(task.retry_delay(), Duration::from_secs(1));
        task.retry_count = 1;
        assert_eq!(task.retry_delay(), Duration::from_secs(2));
        task.retry_count = 2;
        assert_eq!(task.retry_delay(), Duration::from_secs(4));
        task.retry_count = 3;
        assert_eq!(task.retry_delay(), Duration::from_secs(8));
        task.retry_count = 4;
        assert_eq!(task.retry_delay(), Duration::from_secs(16));
        task.retry_count = 5;
        assert_eq!(task.retry_delay(), Duration::from_secs(32));
        task.retry_count = 6;
        assert_eq!(task.retry_delay(), Duration::from_secs(60)); // capped at max_delay
        task.retry_count = 7;
        assert_eq!(task.retry_delay(), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_no_retry_policy_delay_fallback() {
        let task = Task::new("no-policy-delay");
        assert_eq!(
            task.retry_delay(),
            Duration::from_secs(1),
            "without a policy, retry_delay must fall back to 1s"
        );
    }

    // -----------------------------------------------------------------------
    // Concurrent storm with run + retry cycle
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_run_and_retry_concurrent_storm() {
        let service = setup_service();
        let count = 10;

        let mut task_ids = Vec::with_capacity(count);
        for i in 0..count {
            let task = make_failed_task(&service, &format!("run-retry-{i}"), 3).await;
            task_ids.push(task.id.clone());
        }

        // Wave 1: retry all 10 concurrently
        let handles1: Vec<_> = task_ids
            .iter()
            .map(|tid| {
                let svc = service.clone();
                let id = tid.clone();
                tokio::spawn(async move { svc.retry_task(id).await })
            })
            .collect();

        for h in handles1 {
            let result = h.await.unwrap();
            assert!(result.is_ok(), "retry must succeed in wave 1");
            let task = result.unwrap();
            assert_eq!(task.state, TaskState::Pending);
        }

        // Wave 2: run all 10 concurrently
        let handles2: Vec<_> = task_ids
            .iter()
            .map(|tid| {
                let svc = service.clone();
                let id = tid.clone();
                tokio::spawn(async move { svc.run_task(&id, false).await })
            })
            .collect();

        for h in handles2 {
            let result = h.await.unwrap();
            assert!(result.is_ok(), "run after retry must succeed");
            let task_result = result.unwrap();
            assert!(task_result.success, "task execution must succeed after retry");
        }
    }
}
