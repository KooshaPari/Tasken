// SPDX-License-Identifier: MIT OR Apache-2.0
// Integration tests for dependency cycle detection in the task DAG.
//
// These tests exercise topological-sort cycle detection at the
// integration level, verifying that cycles are caught across task
// groups and that valid DAGs are sorted correctly.
//
// Run with: `cargo test --test integration`

use taskkit::domain::tasks::{topological_sort_tasks, Task};

// ---------------------------------------------------------------------------
// Cycle detection — topological_sort_tasks panics on cycles
// ---------------------------------------------------------------------------

#[test]
fn test_direct_cycle_detected() {
    // a depends on b, b depends on a → cycle
    let mut a = Task::new("a");
    let b = Task::new("b").with_dependency(a.id.clone());
    a.depends_on.push(b.id.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        topological_sort_tasks(&[a, b]);
    }));
    assert!(result.is_err(), "direct cycle should panic");
}

#[test]
fn test_indirect_cycle_detected() {
    // a → b → c → a (3-node cycle)
    let mut a = Task::new("a");
    let b = Task::new("b").with_dependency(a.id.clone());
    let c = Task::new("c").with_dependency(b.id.clone());
    a.depends_on.push(c.id.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        topological_sort_tasks(&[a, b, c]);
    }));
    assert!(result.is_err(), "indirect 3-node cycle should panic");
}

#[test]
fn test_self_cycle_detected() {
    // A task depending on itself
    let mut a = Task::new("self-cycle");
    a.depends_on.push(a.id.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        topological_sort_tasks(&[a]);
    }));
    assert!(result.is_err(), "self-cycle should panic");
}

// ---------------------------------------------------------------------------
// Valid DAG sorting
// ---------------------------------------------------------------------------

#[test]
fn test_valid_dag_sorted_correctly() {
    let build_lib = Task::new("build-lib");
    let build_bin = Task::new("build-bin").with_dependency(build_lib.id.clone());
    let test = Task::new("test")
        .with_dependency(build_lib.id.clone())
        .with_dependency(build_bin.id.clone());
    let deploy = Task::new("deploy").with_dependency(test.id.clone());

    let sorted = topological_sort_tasks(&[deploy, test, build_bin, build_lib]);
    assert_eq!(sorted.len(), 4);

    // build-lib must appear before build-bin and test
    let pos_lib = sorted.iter().position(|t| t.name == "build-lib").unwrap();
    let pos_bin = sorted.iter().position(|t| t.name == "build-bin").unwrap();
    let pos_test = sorted.iter().position(|t| t.name == "test").unwrap();
    let pos_deploy = sorted.iter().position(|t| t.name == "deploy").unwrap();

    assert!(pos_lib < pos_bin, "build-lib before build-bin");
    assert!(pos_lib < pos_test, "build-lib before test");
    assert!(pos_bin < pos_test, "build-bin before test");
    assert!(pos_test < pos_deploy, "test before deploy");
}

// ---------------------------------------------------------------------------
// Disconnected subgraphs
// ---------------------------------------------------------------------------

#[test]
fn test_disconnected_subgraphs() {
    // Two independent chains that share no dependencies
    let a1 = Task::new("chain1-a");
    let a2 = Task::new("chain1-b").with_dependency(a1.id.clone());

    let b1 = Task::new("chain2-a");
    let b2 = Task::new("chain2-b").with_dependency(b1.id.clone());

    let sorted = topological_sort_tasks(&[a2, b2, a1, b1]);
    assert_eq!(sorted.len(), 4);

    // Each chain must be internally ordered
    let pos_a1 = sorted.iter().position(|t| t.name == "chain1-a").unwrap();
    let pos_a2 = sorted.iter().position(|t| t.name == "chain1-b").unwrap();
    let pos_b1 = sorted.iter().position(|t| t.name == "chain2-a").unwrap();
    let pos_b2 = sorted.iter().position(|t| t.name == "chain2-b").unwrap();

    assert!(pos_a1 < pos_a2, "chain1 internal order");
    assert!(pos_b1 < pos_b2, "chain2 internal order");
}

// ---------------------------------------------------------------------------
// Single task (no deps) — trivially valid
// ---------------------------------------------------------------------------

#[test]
fn test_single_task_no_deps() {
    let t = Task::new("standalone");
    let sorted = topological_sort_tasks(&[t]);
    assert_eq!(sorted.len(), 1);
}

// ---------------------------------------------------------------------------
// Many tasks with fan-out / fan-in diamond
// ---------------------------------------------------------------------------

#[test]
fn test_diamond_dag() {
    //      root
    //     /    \
    //  left   right
    //     \    /
    //      leaf
    let root = Task::new("root");
    let left = Task::new("left").with_dependency(root.id.clone());
    let right = Task::new("right").with_dependency(root.id.clone());
    let leaf = Task::new("leaf")
        .with_dependency(left.id.clone())
        .with_dependency(right.id.clone());

    let sorted = topological_sort_tasks(&[leaf, right, left, root]);
    assert_eq!(sorted.len(), 4);

    let pos_root = sorted.iter().position(|t| t.name == "root").unwrap();
    let pos_leaf = sorted.iter().position(|t| t.name == "leaf").unwrap();
    assert!(pos_root < pos_leaf, "root before leaf");
}
