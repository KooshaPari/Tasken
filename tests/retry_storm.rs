// SPDX-License-Identifier: MIT OR Apache-2.0
// Integration tests for retry-storm scenarios.
//
// These tests verify correct behaviour when multiple tasks are retried
// concurrently, when retry limits are exhausted, and when state
// transitions are invalid for retry.
//
// Run with: `cargo test --test integration`

use std::sync::Arc;
use std::time::Duration;

use taskkit::adapters::secondary::memory::MemoryStorage;
use taskkit::application::services::TaskService;
use taskkit::application::CreateTask;
use taskkit::domain::tasks::{RetryPolicy, TaskState};

fn setup_service() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    Arc::new(TaskService::new(storage, queue))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a task with a command that will fail, run it, and return the task
/// after it transitions to Failed state.
async fn create_failed_task(service: &TaskService, name: &str, policy: RetryPolicy) -> taskkit::domain::tasks::Task {
    let cmd = CreateTask::new(name)
        .with_command("false")  // `false` always exits non-zero
        .with_retry_policy(policy);
    let task = service.create_task(cmd).await.unwrap();
    let result = service.run_task(&task.id, false).await.unwrap();

    // The task state should be Failed (command exited non-zero)
    assert!(!result.success, "task should have failed: {}", name);
    task
}

// ---------------------------------------------------------------------------
// Retry limits
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_limit_exceeded_after_max_attempts() {
    let service = setup_service();

    let policy = RetryPolicy {
        max_attempts: 1, // only 1 attempt allowed
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(10),
        jitter: 0.0,
    };
    let cmd = CreateTask::new("retry-limit-1")
        .with_command("false")
        .with_retry_policy(policy);
    let task = service.create_task(cmd).await.unwrap();

    // Run the task — it will exit non-zero and transition to Failed
    let result = service.run_task(&task.id, false).await.unwrap();
    assert!(!result.success, "task should have failed");

    // First retry — should succeed (move back to Pending)
    let retried = service.retry_task(task.id.clone()).await.unwrap();
    assert_eq!(retried.state, TaskState::Pending);
    assert_eq!(retried.retry_count, 1);

    // Run again — should fail again
    let result2 = service.run_task(&task.id, false).await.unwrap();
    assert!(!result2.success, "task should fail on second run");

    // Second retry — should fail (max_attempts=1 already consumed)
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("exceeded") || msg.to_lowercase().contains("retry"),
        "expected retry-limit error, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Retry on a task that hasn't failed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_non_failed_task_is_error() {
    let service = setup_service();
    let cmd = CreateTask::new("non-failed-retry");
    let task = service.create_task(cmd).await.unwrap();

    // Task is Pending, not Failed — retry should error
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("transition") || msg.contains("Invalid") || msg.contains("state"),
        "expected state-transition error, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Concurrent retry of multiple failed tasks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_concurrent_retry_multiple_tasks() {
    let service = setup_service();
    let mut ids = Vec::new();

    // Create 5 tasks that fail, each with retry policy
    for i in 0..5 {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            jitter: 0.0,
        };
        let task = create_failed_task(&service, &format!("concurrent-{}", i), policy).await;
        ids.push(task.id.clone());
    }

    // Retry all 5 concurrently
    let mut handles = Vec::new();
    for id in ids {
        let svc = service.clone();
        handles.push(tokio::spawn(async move {
            svc.retry_task(id).await
        }));
    }

    let mut success_count = 0;
    for handle in handles {
        let result = handle.await.unwrap();
        if result.is_ok() {
            success_count += 1;
        }
    }

    // All 5 should retry successfully
    assert_eq!(success_count, 5, "all 5 concurrent retries should succeed");
}

// ---------------------------------------------------------------------------
// Retry with no retry policy set — should fail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_retry_without_policy_fails() {
    let service = setup_service();
    // Create a task that has no retry policy by default (CreateTask without policy)
    let cmd = CreateTask::new("no-policy-retry").with_command("false");
    let task = service.create_task(cmd).await.unwrap();

    // Run — it will fail
    let result = service.run_task(&task.id, false).await.unwrap();
    assert!(!result.success);

    // Now retry — should fail because there's no retry policy
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("limit") || msg.contains("exceeded") || msg.to_lowercase().contains("retry"),
        "expected retry-limit error without policy, got: {msg}"
    );
}
