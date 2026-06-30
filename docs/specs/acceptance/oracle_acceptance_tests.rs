//! Acceptance oracle for Tasken.
//! Real feature tests are executable; non-implemented gaps remain explicit `ignore`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use taskkit::adapters::secondary::memory::MemoryStorage;
use taskkit::application::visualize::generate_mermaid;
use taskkit::application::watcher::FileWatcher;
use taskkit::application::{CreateTask, TaskService};
use taskkit::domain::rate_limiter::TokenBucket;
use taskkit::domain::recipe::TaskenfileParser;
use taskkit::domain::tasks::{topological_sort_tasks, Task};
use taskkit::domain::workflows::{Workflow, WorkflowStep};
use taskkit::domain::Group;
use taskkit::infrastructure::PersistentTaskCache;
use tempfile::tempdir;
use tokio::sync::Barrier;

fn setup_service() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    Arc::new(TaskService::with_cache(
        storage,
        queue,
        Arc::new(PersistentTaskCache::ephemeral(Duration::from_secs(300))),
    ))
}

fn assert_file_watch_pulse(timeout: Duration) -> bool {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("trigger.txt");
    let watcher = Arc::new(FileWatcher::new().with_debounce(80));
    let watcher_for_thread = Arc::clone(&watcher);

    let fired = Arc::new(AtomicBool::new(false));
    let fired_for_cb = Arc::clone(&fired);
    let handle = std::thread::spawn(move || {
        watcher_for_thread.watch_and_run(dir.path(), move || {
            fired_for_cb.store(true, Ordering::SeqCst);
        })
    });

    std::thread::sleep(Duration::from_millis(250));
    std::fs::write(&path, b"accepted").expect("touch file");
    let start = Instant::now();
    while start.elapsed() < timeout && !fired.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(25));
    }

    watcher.stop();
    handle.join().expect("watcher thread").expect("watcher run");
    fired.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// Functional Requirements
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fr_01_single_task_execution() {
    let service = setup_service();
    let task = service
        .create_task(CreateTask::new("build").with_command("echo one"))
        .await
        .expect("create task");
    let result = service.run_task(&task.id, false).await.expect("run task");

    assert!(result.success);
    let stdout = result.output.as_ref().and_then(|o| o.get("stdout")).and_then(|v| v.as_str());
    assert!(stdout.unwrap_or("").contains("one"));
}

#[tokio::test]
async fn fr_02_task_listing() {
    let service = setup_service();
    let _ = service
        .create_task(CreateTask::new("task-a").with_command("echo a"))
        .await
        .expect("create a");
    let task_b = service
        .create_task(CreateTask::new("task-b").with_command("echo b").with_tag("list"))
        .await
        .expect("create b");

    let all = service.list_tasks(None, None, None).await.expect("list");
    let names: HashSet<_> = all.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains("task-a"));
    assert!(names.contains("task-b"));

    let found = service.get_task(&task_b.id).await.expect("get");
    let found = found.expect("task exists");
    assert_eq!(found.name, "task-b");
    assert!(found.tags.contains(&"list".to_string()));
}

#[test]
#[ignore = "FR-3 not implemented: cron schedule dispatcher loop not wired"]
fn fr_03_cron_scheduling() {
    // Schedule parse exists; runtime dispatcher is not yet implemented.
}

#[tokio::test]
async fn fr_04_dag_workflow_execution() {
    let service = setup_service();
    let task_a =
        service.create_task(CreateTask::new("a").with_command("echo a")).await.expect("create a");
    let task_b =
        service.create_task(CreateTask::new("b").with_command("echo b")).await.expect("create b");

    let workflow = Workflow::new("chain")
        .with_step(WorkflowStep::new("a").with_task(task_a.id.clone()))
        .with_step(WorkflowStep::new("b").with_task(task_b.id.clone()).with_dependency("a"));
    let workflow = service.create_workflow(workflow).await.expect("create workflow");
    let results = service.execute_workflow(&workflow.id, false).await.expect("execute workflow");

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| result.success));
    assert_eq!(results[0].task_id, task_a.id);
    assert_eq!(results[1].task_id, task_b.id);
}

#[tokio::test]
async fn fr_05_task_grouping() {
    let service = setup_service();
    let lint = service
        .create_task(CreateTask::new("lint").with_command("echo lint"))
        .await
        .expect("create lint");
    let build = service
        .create_task(CreateTask::new("build").with_command("echo build"))
        .await
        .expect("create build");

    let group = Group::new("frontend").with_task(lint.id.clone()).with_task(build.id.clone());
    let group = service.create_group(group).await.expect("create group");

    let listed = service.list_groups().await.expect("list groups");
    assert!(listed.iter().any(|candidate| candidate.id == group.id));

    let stored = service.get_group(&group.id).await.expect("get group").expect("group exists");
    assert_eq!(stored.task_ids, vec![lint.id.clone(), build.id.clone()]);

    let results = service.run_group(&group.id, false).await.expect("run group");
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| result.success));
    assert_eq!(results[0].task_id, lint.id);
    assert_eq!(results[1].task_id, build.id);
}

#[test]
#[ignore = "FR-6 not implemented: docker runner and wasm plugin runtime are not wired"]
fn fr_06_multi_backend_runners() {}

#[tokio::test]
async fn fr_07_task_dependencies() {
    let service = setup_service();
    let build = service
        .create_task(CreateTask::new("build").with_command("echo build"))
        .await
        .expect("create build");
    let test = service
        .create_task(
            CreateTask::new("test").with_command("echo test").with_dependency(build.id.clone()),
        )
        .await
        .expect("create test");
    let deploy = service
        .create_task(
            CreateTask::new("deploy")
                .with_command("echo deploy")
                .with_dependency(test.id.clone())
                .with_dependency(build.id.clone()),
        )
        .await
        .expect("create deploy");

    let sorted = service.list_tasks_sorted(None, None).await.expect("sorted");
    let positions: Vec<_> = sorted.iter().map(|task| task.id.clone()).collect();
    let build_pos = positions.iter().position(|id| *id == build.id).expect("build in sorted");
    let test_pos = positions.iter().position(|id| *id == test.id).expect("test in sorted");
    let deploy_pos = positions.iter().position(|id| *id == deploy.id).expect("deploy in sorted");

    assert!(build_pos < test_pos);
    assert!(test_pos < deploy_pos);
}

#[test]
fn fr_08_task_visualization() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("Taskenfile.toml");
    std::fs::write(
        &path,
        r#"
name = "viz"
[[tasks]]
name = "lint"
command = "echo lint"
[[tasks]]
name = "build"
command = "echo build"
depends_on = ["lint"]
"#,
    )
    .expect("write taskenfile");

    let recipe = TaskenfileParser::parse_file(&path).expect("parse file");
    let mermaid = generate_mermaid(&recipe.tasks);
    assert!(mermaid.contains("flowchart LR"));
    assert!(mermaid.contains("lint --> build"));
}

#[test]
fn fr_09_file_watch_trigger() {
    assert!(assert_file_watch_pulse(Duration::from_millis(500)));
}

#[tokio::test]
async fn fr_10_rate_limiting() {
    let service = setup_service();
    let task = service
        .create_task(CreateTask::new("rate").with_command("true"))
        .await
        .expect("create task");

    let limiter = TokenBucket::with_starting_tokens(5, 0.0, Some(Duration::from_millis(50)), 5);
    service.set_rate_limiter(limiter).await;

    for _ in 0..5 {
        let result = service.run_task(&task.id, false).await;
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    let blocked =
        tokio::time::timeout(Duration::from_millis(200), service.run_task(&task.id, false));
    assert!(blocked.await.is_err(), "6th call should be backpressured by a zero-rate limiter");
}

#[test]
#[ignore = "FR-11 not implemented: event bus + OTEL span assertions are not currently wired"]
fn fr_11_event_bus_observability() {}

#[test]
#[ignore = "FR-12 not implemented: recipe variables are not mapped through a user-facing resolver contract"]
fn fr_12_recipe_blueprint() {}

#[test]
#[ignore = "FR-13 not implemented: plugin install/list/remove lifecycle is incomplete"]
fn fr_13_plugin_management() {}

#[test]
#[ignore = "FR-14 not implemented: export commands are not implemented yet"]
fn fr_14_task_export() {}

#[test]
#[ignore = "FR-15 not implemented: durable restart/restore harness not implemented for acceptance criteria"]
fn fr_15_persistent_task_store() {}

#[test]
#[ignore = "FR-16 not implemented: cache TTL eviction + persistence behavior not yet asserted in harness"]
fn fr_16_caching_layer() {}

#[test]
#[ignore = "FR-17 not implemented: Python SDK binding is not part of this workspace"]
fn fr_17_python_sdk() {}

// ---------------------------------------------------------------------------
// Non-Functional Requirements
// ---------------------------------------------------------------------------

#[test]
#[ignore = "NFR-1 not implemented: benchmark harness for command-latency SLIs is not present"]
fn nfr_01_cli_responsiveness() {}

#[test]
#[ignore = "NFR-2 not implemented: no strict overhead harness in scope"]
fn nfr_02_execution_overhead() {}

#[tokio::test]
async fn nfr_03_dag_correctness() {
    let t1 = Task::new("root").with_command("echo root");
    let t2 = Task::new("child").with_dependency(t1.id.clone()).with_command("echo child");
    let t3 = Task::new("child2").with_dependency(t2.id.clone()).with_command("echo child2");
    let sorted = topological_sort_tasks(&[t1.clone(), t2.clone(), t3.clone()]);
    assert_eq!(sorted.len(), 3);
    let ids: Vec<_> = sorted.iter().map(|t| t.id.clone()).collect();
    let p1 = ids.iter().position(|id| *id == t1.id).expect("root");
    let p2 = ids.iter().position(|id| *id == t2.id).expect("child");
    let p3 = ids.iter().position(|id| *id == t3.id).expect("child2");
    assert!(p1 < p2 && p2 < p3);

    let mut cycle_a = Task::new("a");
    let mut cycle_b = Task::new("b");
    cycle_a.depends_on.push(cycle_b.id.clone());
    cycle_b.depends_on.push(cycle_a.id.clone());
    assert!(std::panic::catch_unwind(move || {
        topological_sort_tasks(&[cycle_a, cycle_b]);
    })
    .is_err());
}

#[tokio::test]
async fn nfr_04_concurrent_safety() {
    let service = setup_service();
    for i in 0..10 {
        let task = CreateTask::new(format!("safe-{i}")).with_command("echo safe");
        let _ = service.create_task(task).await.expect("create task");
    }

    let tasks = service.list_tasks(None, None, None).await.expect("list");
    assert_eq!(tasks.len(), 10);

    let barrier = Arc::new(Barrier::new(tasks.len() + 1));
    let mut handles = Vec::with_capacity(tasks.len());
    for task in tasks {
        let b = Arc::clone(&barrier);
        let svc = Arc::clone(&service);
        let task_id = task.id.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            svc.run_task(&task_id, false).await
        }));
    }

    barrier.wait().await;
    for handle in handles {
        let result = handle.await.expect("join");
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }
}

#[test]
#[ignore = "NFR-5 not implemented: OTEL span and hierarchy assertions require external exporter assertions"]
fn nfr_05_opentelemetry_correctness() {}

#[test]
#[ignore = "NFR-6 not implemented: no plugin fault-isolation harness in this workspace"]
fn nfr_06_plugin_isolation() {}

#[test]
#[ignore = "NFR-7 not implemented: config bootstrap error/exit semantics are not test-covered"]
fn nfr_07_config_graceful_degradation() {}

#[test]
fn nfr_08_watch_responsiveness() {
    let start = Instant::now();
    assert!(assert_file_watch_pulse(Duration::from_millis(500)));
    assert!(start.elapsed() <= Duration::from_secs(3));
}
