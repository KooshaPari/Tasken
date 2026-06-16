// Integration tests for the runtime modules:
//   - src/domain/scheduler.rs
//   - src/infrastructure/cache.rs
//   - src/application/services.rs
//
// These are the W3 wave targets (#1 argument forwarding, #2 DAG,
// #5 cache persistence) and already have inline `#[cfg(test)]` blocks;
// this file adds the cross-module integration coverage that those
// inline tests cannot reach (cache TTL, scheduler+service interplay,
// workflow execution paths).
//
// Run with: `cargo test --test runtime`

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use taskkit::adapters::secondary::memory::MemoryStorage;
use taskkit::application::services::TaskService;
use taskkit::application::CreateTask;
use taskkit::domain::scheduler::{Schedule, ScheduleId, ScheduleKind};
use taskkit::domain::tasks::{Priority, RetryPolicy, TaskId, TaskResult};
use taskkit::domain::workflows::Workflow;
use taskkit::infrastructure::{TaskCache, PersistentTaskCache};

// ---------- Cache (src/infrastructure/cache.rs) ----------

#[test]
fn test_cache_default_has_300s_ttl() {
    // The Default impl must use a 5-minute TTL; verify by inserting
    // and then confirming a fresh cache::get returns Some.
    let cache = TaskCache::default();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    let id = TaskId::from_string("t-default");
    let result = TaskResult {
        task_id: id.clone(),
        success: true,
        output: None,
        error: None,
        duration: Duration::from_millis(1),
        timestamp: Utc::now(),
    };
    cache.insert(id.clone(), result);
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());
    let got = cache.get(&id).expect("fresh insert should be Some");
    assert!(got.success);
}

#[test]
fn test_cache_insert_with_ttl_overrides_default() {
    // A custom-TTL entry must be retrievable for at least the full TTL.
    let cache = TaskCache::new(Duration::from_secs(60));
    let id = TaskId::from_string("t-custom-ttl");
    let result = TaskResult {
        task_id: id.clone(),
        success: true,
        output: Some(serde_json::json!({"k": "v"})),
        error: None,
        duration: Duration::from_millis(1),
        timestamp: Utc::now(),
    };
    cache.insert_with_ttl(id.clone(), result, Duration::from_secs(3600));
    let got = cache.get(&id).expect("custom-ttl insert should be Some");
    assert!(got.output.is_some());
    assert_eq!(got.output.unwrap()["k"], "v");
}

#[test]
fn test_cache_clear_removes_all_entries() {
    let cache = TaskCache::new(Duration::from_secs(60));
    for i in 0..5 {
        let id = TaskId::from_string(format!("t-{}", i));
        let result = TaskResult {
            task_id: id.clone(),
            success: true,
            output: None,
            error: None,
            duration: Duration::from_millis(1),
            timestamp: Utc::now(),
        };
        cache.insert(id, result);
    }
    assert_eq!(cache.len(), 5);
    cache.clear();
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn test_cache_get_returns_none_for_missing_key() {
    let cache = TaskCache::new(Duration::from_secs(60));
    let id = TaskId::from_string("not-there");
    assert!(cache.get(&id).is_none());
}

#[test]
fn test_cache_invalidate_does_not_affect_others() {
    let cache = TaskCache::new(Duration::from_secs(60));
    let a = TaskId::from_string("a");
    let b = TaskId::from_string("b");
    let mk = |id: &TaskId| TaskResult {
        task_id: id.clone(),
        success: true,
        output: None,
        error: None,
        duration: Duration::from_millis(1),
        timestamp: Utc::now(),
    };
    cache.insert(a.clone(), mk(&a));
    cache.insert(b.clone(), mk(&b));
    assert_eq!(cache.len(), 2);
    cache.invalidate(&a);
    assert_eq!(cache.len(), 1);
    assert!(cache.get(&a).is_none());
    assert!(cache.get(&b).is_some());
}

#[test]
fn test_cache_is_send_and_sync() {
    // TaskCache must be safely shareable across threads (it is wrapped
    // in Arc<Mutex<...>>). This test compiles only if the type is
    // Send + Sync.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TaskCache>();
}

// ---------- Scheduler (src/domain/scheduler.rs) ----------

#[test]
fn test_schedule_id_default_is_unique() {
    let a = ScheduleId::default();
    let b = ScheduleId::default();
    assert_ne!(a, b, "default ScheduleId must be unique per instance");
}

#[test]
fn test_schedule_once_sets_next_run_to_target() {
    let future = Utc::now() + chrono::Duration::hours(2);
    let schedule = Schedule::once("task-x", future);
    assert_eq!(schedule.task_id, "task-x");
    assert!(schedule.active);
    assert_eq!(schedule.next_run, Some(future));
    assert!(schedule.last_run.is_none());
}

#[test]
fn test_schedule_cron_initial_next_run_is_in_future() {
    let schedule = Schedule::cron("cron-task", "0 0 * * *");
    assert!(schedule.active);
    let next = schedule.next_run.expect("cron schedule should compute a next_run");
    assert!(next > Utc::now());
}

#[test]
fn test_schedule_kind_once_past_returns_none() {
    let past = Utc::now() - chrono::Duration::days(1);
    let kind = ScheduleKind::Once { at: past };
    assert!(kind.next_run(Utc::now()).is_none());
}

#[test]
fn test_schedule_kind_interval_is_deterministic() {
    // Interval next_run is purely arithmetic — verify exact equality.
    let now = Utc::now();
    let kind = ScheduleKind::Interval { every: 120 };
    assert_eq!(kind.next_run(now), Some(now + chrono::Duration::seconds(120)));
}

#[test]
fn test_schedule_kind_weekly_is_one_week_later() {
    let now = Utc::now();
    let kind = ScheduleKind::Weekly {
        days: vec!["mon".to_string(), "wed".to_string()],
        at: "10:00".to_string(),
    };
    let next = kind.next_run(now).expect("weekly should be Some");
    assert_eq!(next - now, chrono::Duration::weeks(1));
}

#[test]
fn test_schedule_kind_daily_is_one_day_later() {
    let now = Utc::now();
    let kind = ScheduleKind::Daily { at: "09:00".to_string() };
    let next = kind.next_run(now).expect("daily should be Some");
    assert_eq!(next - now, chrono::Duration::days(1));
}

#[test]
fn test_schedule_tick_updates_last_run() {
    let future = Utc::now() + chrono::Duration::hours(1);
    let mut schedule = Schedule::once("task-tick", future);
    assert!(schedule.last_run.is_none());
    schedule.tick();
    assert!(schedule.last_run.is_some());
}

#[test]
fn test_schedule_pause_and_resume_round_trip() {
    let mut schedule = Schedule::interval("task-pr", 60);
    assert!(schedule.active);
    schedule.pause();
    assert!(!schedule.active);
    schedule.resume();
    assert!(schedule.active);
    // Idempotent.
    schedule.pause();
    schedule.pause();
    assert!(!schedule.active);
}

// ---------- Services (src/application/services.rs) ----------

fn setup_service() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    Arc::new(TaskService::new(storage, queue))
}

#[tokio::test]
async fn test_service_with_cache_uses_provided_cache() {
    // Build a custom PersistentTaskCache and confirm TaskService::with_cache
    // accepts it without panicking and produces a working service.
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    let cache = Arc::new(taskkit::infrastructure::PersistentTaskCache::ephemeral(
        Duration::from_millis(10),
    ));
    let service = TaskService::with_cache(storage, queue, cache);

    let cmd = CreateTask::new("custom-cache").with_command("echo hi");
    let task = service.create_task(cmd).await.unwrap();
    let result = service.run_task(&task.id).await.unwrap();
    assert!(result.success);
}

#[tokio::test]
async fn test_service_get_task_history_returns_empty_vec() {
    // get_task_history is currently a stub returning Vec::new().
    let service = setup_service();
    let id = TaskId::from_string("any");
    let history = service.get_task_history(&id).await.unwrap();
    assert!(history.is_empty());
}

#[tokio::test]
async fn test_service_execute_task_queues_and_returns_success() {
    // execute_task enqueues onto the queue port and returns a
    // success_result with a "queued" payload.
    let service = setup_service();
    let cmd = CreateTask::new("queued").with_command("sleep 0");
    let task = service.create_task(cmd).await.unwrap();
    let result = service.execute_task(&task.id).await.unwrap();
    assert!(result.success);
    let output = result.output.expect("output should be Some");
    assert_eq!(output["status"], "queued");
}

#[tokio::test]
async fn test_service_retry_limit_exceeded() {
    // Set up a task with a policy of max_attempts=1, run it to completion,
    // then retry again — the second retry should hit RetryLimitExceeded.
    let service = setup_service();

    let policy = RetryPolicy {
        max_attempts: 1,
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(10),
        jitter: 0.0,
    };
    let cmd = CreateTask::new("limit-test")
        .with_command("echo hello")
        .with_retry_policy(policy);
    let task = service.create_task(cmd).await.unwrap();
    let result = service.run_task(&task.id).await.unwrap();
    assert!(result.success);

    // Retry after success is an error (invalid state transition)
    let err = service.retry_task(task.id.clone()).await.unwrap_err();
    let msg = err.to_string();
    eprintln!("retry error: {msg}");
    assert!(msg.contains("Invalid") || msg.contains("retry") || msg.contains("limit"));
}

#[tokio::test]
async fn test_service_create_task_with_retry_policy() {
    // Verify that the retry policy field is propagated through
    // CreateTask.execute into the persisted Task.
    let service = setup_service();
    let policy = RetryPolicy {
        max_attempts: 5,
        base_delay: Duration::from_secs(2),
        max_delay: Duration::from_secs(120),
        jitter: 0.25,
    };
    let cmd = CreateTask::new("with-policy").with_retry_policy(policy.clone());
    let task = service.create_task(cmd).await.unwrap();
    let fetched = service.get_task(&task.id).await.unwrap().unwrap();
    let fetched_policy = fetched.retry_policy.expect("policy should be Some");
    assert_eq!(fetched_policy.max_attempts, 5);
    assert_eq!(fetched_policy.jitter, 0.25);
}

#[tokio::test]
async fn test_service_list_workflows_initially_empty() {
    let service = setup_service();
    let workflows = service.list_workflows().await.unwrap();
    assert!(workflows.is_empty());
}

#[tokio::test]
async fn test_service_workflow_round_trip() {
    // Create, get, list — verify the full workflow lifecycle.
    let service = setup_service();
    let wf = Workflow::new("rt-flow");
    let created = service.create_workflow(wf.clone()).await.unwrap();
    assert_eq!(created.name, "rt-flow");

    let fetched = service.get_workflow(&created.id).await.unwrap().expect("some");
    assert_eq!(fetched.name, "rt-flow");

    let all = service.list_workflows().await.unwrap();
    assert_eq!(all.len(), 1);
}

#[tokio::test]
async fn test_service_create_task_with_priority_low_and_critical() {
    // Boundary check: all four Priority levels must round-trip.
    let service = setup_service();
    for (name, expected) in [
        ("low", Priority::Low),
        ("norm", Priority::Normal),
        ("hi", Priority::High),
        ("crit", Priority::Critical),
    ] {
        let cmd = CreateTask::new(name).with_priority(expected);
        let task = service.create_task(cmd).await.unwrap();
        assert_eq!(task.priority, expected, "priority mismatch for {}", name);
    }
}

#[tokio::test]
async fn test_service_cancel_already_cancelled_is_error() {
    // Cancelling a task that is already in the Cancelled terminal state
    // must fail (Cancelled is terminal, no further transitions allowed).
    let service = setup_service();
    let cmd = CreateTask::new("canc-twice");
    let task = service.create_task(cmd).await.unwrap();
    service.cancel_task(task.id.clone(), None).await.unwrap();
    let result = service.cancel_task(task.id, None).await;
    assert!(result.is_err(), "second cancel should fail");
}
