// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for retry logic under concurrent and sequential scenarios.
//!
//! These tests verify that:
//!   - Tasks with transient failures are retried (retry_count incremented,
//!     state reset to Pending).
//!   - Tasks with permanent failures are NOT retried beyond max_retries.
//!   - The retry system handles 10 concurrent tasks without races or data
//!     corruption (retry storm).
//!   - Exponential backoff delays are computed correctly.
//!   - Retry limits are enforced at the domain level.

use std::sync::Arc;
use std::time::Duration;

use taskkit::adapters::secondary::memory::MemoryStorage;
use taskkit::application::services::TaskService;
use taskkit::application::CreateTask;
use taskkit::domain::tasks::{RetryPolicy, Task, TaskId, TaskState};
use taskkit::infrastructure::PersistentTaskCache;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn setup_service() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    let cache = Arc::new(PersistentTaskCache::ephemeral(
        Duration::from_secs(300),
    ));
    Arc::new(TaskService::with_cache(storage, queue, cache))
}

/// Helper to create a task in the Failed state with a retry policy.
async fn make_failed_task(
    service: &TaskService,
    name: &str,
    max_attempts: u32,
    command: &str,
) -> Task {
    let policy = RetryPolicy {
        max_attempts,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_secs(1),
        jitter: 0.0,
    };

    let cmd = CreateTask::new(name)
        .with_command(command)
        .with_retry_policy(policy);
    let task = service.create_task(cmd).await.unwrap();

    // Run once — it will fail because the command succeeds (or fails)
    // For "false" it'll fail; for "echo" it'll succeed — we need to
    // manually set the state for deterministic testing.
    let mut failed = task;
    failed.state = TaskState::Failed;
    failed.retry_count = 0;
    failed.error = Some("simulated transient failure".to_string());
    service.storage.save_task(&failed).await.unwrap();
    failed
}

// ---------------------------------------------------------------------------
// Single-task retry logic
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_transient_failure_retried() {
    // A task with max_attempts >= 2 that has failed once should be
    // retried: retry_count goes from 0 → 1, state resets to Pending.
    let service = setup_service();
    let task = make_failed_task(&service, "transient", 3, "echo ok").await;

    assert_eq!(task.state, TaskState::Failed);
    assert_eq!(task.retry_count, 0);

    let retried = service.retry_task(task.id.clone()).await.unwrap();

    assert_eq!(
        retried.state,
        TaskState::Pending,
        "retried task must be reset to Pending"
    );
    assert_eq!(
        retried.retry_count, 1,
        "retry_count must be incremented from 0 to 1"
    );
    assert!(
        retried.error.is_none(),
        "error must be cleared on retry"
    );
}

#[tokio::test]
async fn test_retry_after_failed_run_completes_successfully() {
    // Full cycle: create task, run (succeeds), manually set to failed,
    // retry, then run again successfully.
    let service = setup_service();

    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_secs(1),
        jitter: 0.0,
    };

    let cmd = CreateTask::new("retry-cycle")
        .with_command("echo hello")
        .with_retry_policy(policy);
    let task = service.create_task(cmd).await.unwrap();
    let result = service.run_task(&task.id, false).await.unwrap();
    assert!(result.success, "first run must succeed");

    // Now simulate a failure + retry
    let mut failed = taskclone_for_retry(&service, &task.id).await;
    failed.state = TaskState::Failed;
    failed.error = Some("simulated".to_string());
    failed.retry_count = 0;
    service.storage.save_task(&failed).await.unwrap();

    // Retry it
    let _retried = service.retry_task(task.id.clone()).await.unwrap();

    // Run again — should succeed
    let result2 = service.run_task(&task.id, false).await.unwrap();
    assert!(result2.success, "retried task must run successfully");
}

/// Helper: fetch the current task from storage for modification.
async fn taskclone_for_retry(service: &TaskService, task_id: &TaskId) -> Task {
    service
        .get_task(task_id)
        .await
        .unwrap()
        .expect("task must exist")
}

// ---------------------------------------------------------------------------
// Permanent failure — retry limit exceeded
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_permanent_failure_not_retried_beyond_max() {
    // A task with max_attempts=1 can be retried exactly once (the first
    // retry succeeds).  A second retry must fail with RetryLimitExceeded.
    let service = setup_service();
    let task = make_failed_task(&service, "permanent", 1, "echo no").await;

    // First retry: should work (retry_count goes 0 → 1)
    let retried = service.retry_task(task.id.clone()).await.unwrap();
    assert_eq!(retried.retry_count, 1);

    // Re-fail it for second retry attempt
    let mut refailed = taskclone_for_retry(&service, &task.id).await;
    refailed.state = TaskState::Failed;
    refailed.error = Some("still failing".to_string());
    service.storage.save_task(&refailed).await.unwrap();

    // Second retry: must be rejected — max_attempts=1 means only 1 retry
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("Retry"),
        "error must mention retry limit, got: {msg}"
    );
}

#[tokio::test]
async fn test_retry_limit_exact_max() {
    // max_attempts=3: retry 3 times → 4th attempt must fail.
    let service = setup_service();
    let task = make_failed_task(&service, "limit-exact", 3, "echo x").await;

    // Retry #1: 0 → 1
    let r1 = service.retry_task(task.id.clone()).await.unwrap();
    assert_eq!(r1.retry_count, 1);
    refail_task(&service, &task.id).await;

    // Retry #2: 1 → 2
    let r2 = service.retry_task(task.id.clone()).await.unwrap();
    assert_eq!(r2.retry_count, 2);
    refail_task(&service, &task.id).await;

    // Retry #3: 2 → 3  (this is the last allowed — max_attempts=3)
    let r3 = service.retry_task(task.id.clone()).await.unwrap();
    assert_eq!(r3.retry_count, 3);
    refail_task(&service, &task.id).await;

    // Retry #4: must fail — retry_count (3) == max_attempts (3)
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("Retry"),
        "error must mention retry limit, got: {msg}"
    );
}

/// Helper: re-fail a task in storage so it can be retried again.
async fn refail_task(service: &TaskService, task_id: &TaskId) {
    let mut task = taskclone_for_retry(service, task_id).await;
    task.state = TaskState::Failed;
    task.error = Some("refailed".to_string());
    service.storage.save_task(&task).await.unwrap();
}

// ---------------------------------------------------------------------------
// Retry of a successful task  (invalid state transition)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_successful_task_rejected() {
    // retry_task requires the task to be in Failed state.  A task in any
    // other state must be rejected.
    let service = setup_service();
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_secs(1),
        jitter: 0.0,
    };

    let cmd = CreateTask::new("successful")
        .with_command("echo ok")
        .with_retry_policy(policy);
    let task = service.create_task(cmd).await.unwrap();
    service.run_task(&task.id, false).await.unwrap();

    // Task is now in Completed state — retry must fail
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    assert!(
        err.to_string().contains("Invalid"),
        "retry of a successful task must give InvalidStateTransition, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Retry storm — 10 concurrent retries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_concurrent_retry_storm() {
    let service = setup_service();
    let count = 10;

    // Create 10 failed tasks with generous retry limits.
    let mut task_ids = Vec::with_capacity(count);
    for i in 0..count {
        let task = make_failed_task(
            &service,
            &format!("storm-{}", i),
            5,
            "echo ok",
        )
        .await;
        task_ids.push(task.id.clone());
    }

    // Fire off 10 concurrent retry attempts.
    let mut handles = Vec::with_capacity(count);
    for tid in &task_ids {
        let svc = service.clone();
        let id = tid.clone();
        handles.push(tokio::spawn(async move {
            svc.retry_task(id).await
        }));
    }

    // Join all and collect results.
    let mut successes = 0u32;
    let mut failures = 0u32;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(task) => {
                successes += 1;
                assert_eq!(
                    task.state,
                    TaskState::Pending,
                    "each retried task must be Pending"
                );
                assert_eq!(
                    task.retry_count, 1,
                    "each task must have retry_count = 1 after first retry"
                );
            }
            Err(e) => {
                failures += 1;
                eprintln!("retry storm task failed: {e}");
            }
        }
    }

    assert_eq!(
        successes, count,
        "all {count} concurrent retries must succeed"
    );
    assert_eq!(failures, 0, "zero retry failures expected");
}

// ---------------------------------------------------------------------------
// Retry storm — mixed transient and permanent failures
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_concurrent_retry_mixed_failures() {
    let service = setup_service();

    // 5 tasks with max_attempts=3 (transient — many retries left)
    // 5 tasks with max_attempts=1 (permanent — only 1 retry allowed)
    let transient_ids: Vec<_> = (0..5)
        .map(|i| make_failed_task(&service, &format!("trans-{}", i), 3, "echo t"))
        .collect();
    let permanent_ids: Vec<_> = (0..5)
        .map(|i| make_failed_task(&service, &format!("perm-{}", i), 1, "echo p"))
        .collect();

    // Helper to retry a batch of tasks concurrently.
    async fn retry_all(service: &TaskService, ids: &[TaskId]) -> Vec<Result<Task, taskkit::domain::errors::TaskError>> {
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

    // First retry wave — all 10 should succeed (all within limits)
    let all_ids: Vec<TaskId> = transient_ids
        .iter()
        .chain(permanent_ids.iter())
        .map(|t| t.id.clone())
        .collect();

    let wave1 = retry_all(&service, &all_ids).await;
    let wave1_ok = wave1.iter().filter(|r| r.is_ok()).count();
    let wave1_err = wave1.iter().filter(|r| r.is_err()).count();
    assert_eq!(
        wave1_ok, 10,
        "first retry wave: all 10 tasks must succeed (0 errors), got {} ok / {} err",
        wave1_ok, wave1_err
    );

    // Re-fail all tasks for a second wave
    for tid in &all_ids {
        refail_task(&service, tid).await;
    }

    // Second retry wave
    // - Transient tasks (max_attempts=3): retry #2 should succeed
    // - Permanent tasks (max_attempts=1): retry #2 must fail (limit exceeded)
    let wave2 = retry_all(&service, &all_ids).await;

    let trans_ok = wave2[0..5].iter().filter(|r| r.is_ok()).count();
    let trans_err = wave2[0..5].iter().filter(|r| r.is_err()).count();
    let perm_ok = wave2[5..10].iter().filter(|r| r.is_ok()).count();
    let perm_err = wave2[5..10].iter().filter(|r| r.is_err()).count();

    assert_eq!(
        trans_ok, 5,
        "transient tasks: all 5 must still succeed on retry #2"
    );
    assert_eq!(trans_err, 0);
    assert_eq!(
        perm_ok, 0,
        "permanent tasks: 0 must succeed on retry #2 (limit exceeded)"
    );
    assert_eq!(
        perm_err, 5,
        "permanent tasks: all 5 must fail on retry #2"
    );
}

// ---------------------------------------------------------------------------
// Retry with no retry policy  (can_retry returns false)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_without_policy_rejected() {
    let service = setup_service();
    let cmd = CreateTask::new("no-policy").with_command("echo hi");
    let task = service.create_task(cmd).await.unwrap();

    // Manually fail the task (no retry policy set)
    let mut failed = taskclone_for_retry(&service, &task.id).await;
    failed.state = TaskState::Failed;
    failed.error = Some("no policy".to_string());
    failed.retry_policy = None;
    service.storage.save_task(&failed).await.unwrap();

    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("Retry"),
        "retry without policy must fail, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Exponential backoff calculation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_exponential_backoff_delays() {
    use taskkit::domain::tasks::RetryPolicy;
    use std::time::Duration;

    let policy = RetryPolicy {
        max_attempts: 10,
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(60),
        jitter: 0.0,
    };

    let mut task = Task::new("backoff-test").with_retry_policy(policy);

    // retry_count = 0 → 1s * 2^0 = 1s
    task.retry_count = 0;
    assert_eq!(task.retry_delay(), Duration::from_secs(1));

    // retry_count = 1 → 1s * 2^1 = 2s
    task.retry_count = 1;
    assert_eq!(task.retry_delay(), Duration::from_secs(2));

    // retry_count = 2 → 1s * 2^2 = 4s
    task.retry_count = 2;
    assert_eq!(task.retry_delay(), Duration::from_secs(4));

    // retry_count = 3 → 1s * 2^3 = 8s
    task.retry_count = 3;
    assert_eq!(task.retry_delay(), Duration::from_secs(8));

    // retry_count = 4 → 1s * 2^4 = 16s
    task.retry_count = 4;
    assert_eq!(task.retry_delay(), Duration::from_secs(16));

    // retry_count = 5 → 1s * 2^5 = 32s
    task.retry_count = 5;
    assert_eq!(task.retry_delay(), Duration::from_secs(32));

    // retry_count = 6 → 1s * 2^6 = 64s, capped at max_delay (60s)
    task.retry_count = 6;
    assert_eq!(task.retry_delay(), Duration::from_secs(60));

    // retry_count = 7 → still capped at 60s
    task.retry_count = 7;
    assert_eq!(task.retry_delay(), Duration::from_secs(60));
}

#[tokio::test]
async fn test_no_retry_policy_delay_fallback() {
    // When there is no retry policy, retry_delay() returns 1 second.
    let task = Task::new("no-policy-delay");
    assert_eq!(
        task.retry_delay(),
        Duration::from_secs(1),
        "without a policy, retry_delay must fall back to 1s"
    );
}

// ---------------------------------------------------------------------------
// Concurrent storm with run + retry cycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_run_and_retry_concurrent_storm() {
    let service = setup_service();
    let count = 10;

    // Create 10 tasks with retry policies and fail them.
    let mut task_ids = Vec::with_capacity(count);
    for i in 0..count {
        let task = make_failed_task(
            &service,
            &format!("run-retry-{}", i),
            3,
            "echo wave",
        )
        .await;
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

    // Wave 2: run all 10 concurrently — since their commands ("echo wave")
    // succeed, all should be successful.
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
        assert!(
            result.is_ok(),
            "run after retry must succeed"
        );
        let task_result = result.unwrap();
        assert!(task_result.success, "task execution must succeed after retry");
    }
}
