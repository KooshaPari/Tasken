// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for dependency cycle detection and DAG orchestration.
//!
//! These tests verify that:
//!   - Linear dependency chains (a → b → c) execute in the correct order.
//!   - Diamond DAGs (a → {b, c} → d) execute all branches successfully.
//!   - Circular dependencies are detected and rejected at every API level.
//!   - Self-referencing cycles and complex multi-node cycles are caught.

use std::sync::Arc;

use taskkit::adapters::secondary::memory::MemoryStorage;
use taskkit::application::services::TaskService;
use taskkit::application::CreateTask;
use taskkit::domain::tasks::{TaskId, topological_sort_tasks};
use taskkit::domain::workflows::{Workflow, WorkflowStep};
use taskkit::infrastructure::PersistentTaskCache;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn setup_service() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    Arc::new(TaskService::new(storage, queue))
}

fn setup_service_with_cache() -> Arc<TaskService> {
    let storage = Arc::new(MemoryStorage::new());
    let queue = Arc::new(MemoryStorage::new());
    let cache = Arc::new(PersistentTaskCache::ephemeral(
        std::time::Duration::from_secs(300),
    ));
    Arc::new(TaskService::with_cache(storage, queue, cache))
}

// ---------------------------------------------------------------------------
// Linear DAG  (a → b → c)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_linear_dag_workflow() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("a").with_command("echo a"))
        .await
        .unwrap();
    let task_b = service
        .create_task(CreateTask::new("b").with_command("echo b"))
        .await
        .unwrap();
    let task_c = service
        .create_task(CreateTask::new("c").with_command("echo c"))
        .await
        .unwrap();

    let workflow = Workflow::new("linear")
        .with_step(WorkflowStep::new("step-a").with_task(task_a.id.clone()))
        .with_step(
            WorkflowStep::new("step-b")
                .with_task(task_b.id.clone())
                .with_dependency("step-a"),
        )
        .with_step(
            WorkflowStep::new("step-c")
                .with_task(task_c.id.clone())
                .with_dependency("step-b"),
        );

    let created = service.create_workflow(workflow).await.unwrap();
    let results = service.execute_workflow(&created.id, false).await.unwrap();

    assert_eq!(results.len(), 3, "all three steps must produce a result");
    assert!(results.iter().all(|r| r.success), "every step must succeed");
}

#[tokio::test]
async fn test_linear_dag_topological_sort() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("build").with_command("echo build"))
        .await
        .unwrap();
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
    // build must appear before test, and test before deploy
    let pos_build = names.iter().position(|n| *n == "build").unwrap();
    let pos_test = names.iter().position(|n| *n == "test").unwrap();
    let pos_deploy = names.iter().position(|n| *n == "deploy").unwrap();
    assert!(
        pos_build < pos_test,
        "build must be sorted before test"
    );
    assert!(
        pos_test < pos_deploy,
        "test must be sorted before deploy"
    );
}

// ---------------------------------------------------------------------------
// Diamond DAG  (a → {b, c} → d)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_diamond_dag_workflow() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("root").with_command("echo root"))
        .await
        .unwrap();
    let task_b = service
        .create_task(CreateTask::new("left").with_command("echo left"))
        .await
        .unwrap();
    let task_c = service
        .create_task(CreateTask::new("right").with_command("echo right"))
        .await
        .unwrap();
    let task_d = service
        .create_task(CreateTask::new("leaf").with_command("echo leaf"))
        .await
        .unwrap();

    // Diamond: root → left, root → right, (left ∧ right) → leaf
    let workflow = Workflow::new("diamond")
        .with_step(WorkflowStep::new("root").with_task(task_a.id.clone()))
        .with_step(
            WorkflowStep::new("left")
                .with_task(task_b.id.clone())
                .with_dependency("root"),
        )
        .with_step(
            WorkflowStep::new("right")
                .with_task(task_c.id.clone())
                .with_dependency("root"),
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

// ---------------------------------------------------------------------------
// Circular dependency detection  (a → b → a)
// ---------------------------------------------------------------------------

#[test]
fn test_circular_dependency_detected_via_topological_sort() {
    // a → b → a  is a cycle that must be detected (via panic in
    // topological_sort_tasks, which uses assert_eq!).
    let mut t1 = taskkit::domain::tasks::Task::new("a");
    let mut t2 = taskkit::domain::tasks::Task::new("b").with_dependency(t1.id.clone());
    t1.depends_on.push(t2.id.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        topological_sort_tasks(&[t1, t2]);
    }));
    assert!(
        result.is_err(),
        "topological_sort_tasks must panic on a circular dependency"
    );
}

#[tokio::test]
async fn test_circular_dependency_detected_via_workflow() {
    let service = setup_service();

    // Two steps: b depends on a, a depends on b  →  cycle
    let mut step_a = WorkflowStep::new("a");
    step_a.depends_on = vec!["b".to_string()];
    let mut step_b = WorkflowStep::new("b");
    step_b.depends_on = vec!["a".to_string()];

    let mut workflow = Workflow::new("cycle-test")
        .with_step(step_a)
        .with_step(step_b);

    let err = workflow.build_dag().unwrap_err();
    assert!(
        err.contains("cycle"),
        "build_dag error must mention 'cycle', got: {err}"
    );
}

#[tokio::test]
async fn test_circular_dependency_detected_via_execute() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("a").with_command("echo a"))
        .await
        .unwrap();
    let task_b = service
        .create_task(CreateTask::new("b").with_command("echo b"))
        .await
        .unwrap();

    // a → b, b → a  (via depends_on on the workflow steps)
    let mut workflow = Workflow::new("cycle-exec")
        .with_step(
            WorkflowStep::new("a")
                .with_task(task_a.id.clone())
                .with_dependency("b"),
        )
        .with_step(
            WorkflowStep::new("b")
                .with_task(task_b.id.clone())
                .with_dependency("a"),
        );

    // build_dag must detect the cycle
    let err = workflow.build_dag().unwrap_err();
    assert!(
        err.contains("cycle"),
        "execute_workflow must reject cycle, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Self-referencing cycle  (a → a)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_self_referencing_cycle_detected() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("a").with_command("echo a"))
        .await
        .unwrap();

    let mut workflow = Workflow::new("self-cycle")
        .with_step(
            WorkflowStep::new("a")
                .with_task(task_a.id.clone())
                .with_dependency("a"), // depends on itself
        );

    let err = workflow.build_dag().unwrap_err();
    assert!(
        err.contains("cycle"),
        "self-referencing step must be detected as a cycle, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Complex multi-node cycle  (a → b → c → a)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_complex_cycle_detected() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("a").with_command("echo a"))
        .await
        .unwrap();
    let task_b = service
        .create_task(CreateTask::new("b").with_command("echo b"))
        .await
        .unwrap();
    let task_c = service
        .create_task(CreateTask::new("c").with_command("echo c"))
        .await
        .unwrap();

    let mut workflow = Workflow::new("complex-cycle")
        .with_step(
            WorkflowStep::new("a")
                .with_task(task_a.id.clone())
                .with_dependency("c"),
        )
        .with_step(
            WorkflowStep::new("b")
                .with_task(task_b.id.clone())
                .with_dependency("a"),
        )
        .with_step(
            WorkflowStep::new("c")
                .with_task(task_c.id.clone())
                .with_dependency("b"),
        );

    let err = workflow.build_dag().unwrap_err();
    assert!(
        err.contains("cycle"),
        "a → b → c → a must be detected as a cycle, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// No dependencies  (empty dep list, independent tasks)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_independent_tasks_topological_sort() {
    let service = setup_service();

    let task_a = service
        .create_task(CreateTask::new("alpha").with_command("echo alpha"))
        .await
        .unwrap();
    let task_b = service
        .create_task(CreateTask::new("beta").with_command("echo beta"))
        .await
        .unwrap();
    let task_c = service
        .create_task(CreateTask::new("gamma").with_command("echo gamma"))
        .await
        .unwrap();

    let _ = (task_a, task_b, task_c);

    let sorted = service.list_tasks_sorted(None, None).await.unwrap();
    assert_eq!(sorted.len(), 3, "all three independent tasks must appear");
}

// ---------------------------------------------------------------------------
// Fork DAG  (a → {b, c, d})
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fork_dag_workflow() {
    let service = setup_service_with_cache();

    let task_a = service
        .create_task(CreateTask::new("root").with_command("echo root"))
        .await
        .unwrap();
    let task_b = service
        .create_task(CreateTask::new("b1").with_command("echo b1"))
        .await
        .unwrap();
    let task_c = service
        .create_task(CreateTask::new("b2").with_command("echo b2"))
        .await
        .unwrap();
    let task_d = service
        .create_task(CreateTask::new("b3").with_command("echo b3"))
        .await
        .unwrap();

    let workflow = Workflow::new("fork")
        .with_step(WorkflowStep::new("root").with_task(task_a.id.clone()))
        .with_step(
            WorkflowStep::new("b1")
                .with_task(task_b.id.clone())
                .with_dependency("root"),
        )
        .with_step(
            WorkflowStep::new("b2")
                .with_task(task_c.id.clone())
                .with_dependency("root"),
        )
        .with_step(
            WorkflowStep::new("b3")
                .with_task(task_d.id.clone())
                .with_dependency("root"),
        );

    let created = service.create_workflow(workflow).await.unwrap();
    let results = service.execute_workflow(&created.id, false).await.unwrap();

    assert_eq!(results.len(), 4, "all four fork steps must succeed");
    assert!(results.iter().all(|r| r.success));
}

// ---------------------------------------------------------------------------
// Empty workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_empty_workflow() {
    let service = setup_service();

    let workflow = Workflow::new("empty");
    let created = service.create_workflow(workflow).await.unwrap();
    let results = service.execute_workflow(&created.id, false).await.unwrap();

    assert!(
        results.is_empty(),
        "empty workflow must produce zero results"
    );
}

// ---------------------------------------------------------------------------
// Single-step workflow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_step_workflow() {
    let service = setup_service();

    let task = service
        .create_task(CreateTask::new("solo").with_command("echo solo"))
        .await
        .unwrap();

    let workflow = Workflow::new("single")
        .with_step(WorkflowStep::new("only").with_task(task.id.clone()));

    let created = service.create_workflow(workflow).await.unwrap();
    let results = service.execute_workflow(&created.id, false).await.unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].success);
}
