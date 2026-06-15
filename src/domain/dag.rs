//! DAG dependency resolution with parallel execution layers.
//!
//! The base [`Workflow`](crate::domain::Workflow) type stores a DAG and
//! provides basic topological-order execution. This module adds the
//! SOTA patterns:
//!
//! - **Kahn's algorithm** for stable layer-by-layer execution.
//! - **Parallel layers**: groups of steps that have no inter-dependency
//!   and can therefore run concurrently.
//! - **Cycle detection with the actual cycle path** so the user sees
//!   which steps form the cycle, not just a generic error.
//! - **Strict validation** that catches missing dependencies, duplicate
//!   IDs, and self-loops with informative messages.

use super::errors::TaskError;
use super::workflows::{Workflow, WorkflowStep};
use std::collections::{HashMap, HashSet};

/// Errors specific to DAG resolution. They are richer than the
/// generic "Workflow contains cycles" string the base type returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    /// A step depends on a non-existent step.
    MissingDependency { step: String, missing: String },
    /// Two steps share the same id.
    DuplicateId(String),
    /// A step depends on itself directly.
    SelfLoop(String),
    /// The graph contains a cycle; the path traverses the cycle.
    Cycle(Vec<String>),
    /// The graph is empty.
    Empty,
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagError::MissingDependency { step, missing } => {
                write!(f, "step '{}' depends on unknown step '{}'", step, missing)
            }
            DagError::DuplicateId(id) => write!(f, "duplicate step id: '{}'", id),
            DagError::SelfLoop(id) => write!(f, "step '{}' depends on itself", id),
            DagError::Cycle(path) => {
                write!(f, "workflow contains cycle: {}", path.join(" -> "))
            }
            DagError::Empty => write!(f, "workflow has no steps"),
        }
    }
}

impl std::error::Error for DagError {}

impl From<DagError> for TaskError {
    fn from(e: DagError) -> Self {
        TaskError::InvalidOperation(e.to_string())
    }
}

/// A precomputed, validated DAG with parallel execution layers.
#[derive(Debug, Clone)]
pub struct ExecutionDag {
    /// The original workflow steps, in stable insertion order.
    steps: Vec<WorkflowStep>,
    /// Parallel execution layers. `layers[i]` is a set of step IDs
    /// that can be executed concurrently once `layers[i-1]` is done.
    layers: Vec<Vec<String>>,
    /// Map from step id -> its layer index.
    layer_of: HashMap<String, usize>,
    /// Steps ordered by Kahn's algorithm (used as a fallback).
    topo: Vec<String>,
    /// Forward adjacency: for each step id, the list of steps that
    /// directly depend on it (i.e. steps for which it is a `depends_on`).
    /// Used for downstream reachability queries like `must_precede`.
    dependents: HashMap<String, Vec<String>>,
}

impl ExecutionDag {
    /// Validate and pre-compute the execution DAG from a workflow.
    ///
    /// The workflow's `build_dag` must be called first; this function
    /// reads the resulting `dag`/`node_map`/`steps` fields. If the
    /// workflow is empty, returns `DagError::Empty`. If a cycle is
    /// detected, returns the cycle path.
    pub fn from_workflow(workflow: &Workflow) -> Result<Self, DagError> {
        if workflow.steps.is_empty() {
            return Err(DagError::Empty);
        }

        // 1. Detect duplicate IDs.
        let mut seen: HashSet<String> = HashSet::new();
        for step in &workflow.steps {
            if !seen.insert(step.id.clone()) {
                return Err(DagError::DuplicateId(step.id.clone()));
            }
        }

        // 2. Detect missing dependencies and self-loops.
        for step in &workflow.steps {
            for dep in &step.depends_on {
                if dep == &step.id {
                    return Err(DagError::SelfLoop(step.id.clone()));
                }
                if !seen.contains(dep) {
                    return Err(DagError::MissingDependency {
                        step: step.id.clone(),
                        missing: dep.clone(),
                    });
                }
            }
        }

        // 3. Build adjacency in both directions. `outgoing` records
        //    what depends on a step (used by cycle detection and
        //    Kahn's algorithm). `dependents` is the same set in
        //    map form for O(1) lookups from `must_precede`.
        let mut in_degree: HashMap<String, usize> =
            workflow.steps.iter().map(|s| (s.id.clone(), 0)).collect();
        let mut outgoing: HashMap<String, Vec<String>> = workflow
            .steps
            .iter()
            .map(|s| (s.id.clone(), Vec::new()))
            .collect();
        let mut dependents: HashMap<String, Vec<String>> = workflow
            .steps
            .iter()
            .map(|s| (s.id.clone(), Vec::new()))
            .collect();
        for step in &workflow.steps {
            for dep in &step.depends_on {
                *in_degree.get_mut(&step.id).unwrap() += 1;
                outgoing.get_mut(dep).unwrap().push(step.id.clone());
                dependents.get_mut(dep).unwrap().push(step.id.clone());
            }
        }

        // 4. Kahn's algorithm: repeatedly peel off nodes with
        //    in-degree zero, recording their layer index.
        let mut topo: Vec<String> = Vec::with_capacity(workflow.steps.len());
        let mut layer_of: HashMap<String, usize> = HashMap::new();
        let mut layers: Vec<Vec<String>> = Vec::new();
        let mut current: Vec<String> = in_degree
            .iter()
            .filter_map(|(id, deg)| if *deg == 0 { Some(id.clone()) } else { None })
            .collect();
        current.sort();

        while !current.is_empty() {
            let layer_idx = layers.len();
            for id in &current {
                layer_of.insert(id.clone(), layer_idx);
            }
            layers.push(current.clone());
            topo.extend(current.iter().cloned());
            let mut next: Vec<String> = Vec::new();
            for id in &current {
                if let Some(children) = outgoing.get(id) {
                    for child in children {
                        if let Some(deg) = in_degree.get_mut(child) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                next.push(child.clone());
                            }
                        }
                    }
                }
            }
            next.sort();
            // Deduplicate in case two parents point to the same child
            // (one child shouldn't appear twice in a single layer, but
            // Kahn's invariant is preserved by the dedup).
            next.dedup();
            current = next;
        }

        // 5. If topo doesn't cover all steps, there is a cycle.
        if topo.len() != workflow.steps.len() {
            let remaining: Vec<String> = workflow
                .steps
                .iter()
                .map(|s| s.id.clone())
                .filter(|id| !topo.contains(id))
                .collect();
            let cycle = find_cycle_path(&remaining, &outgoing);
            return Err(DagError::Cycle(cycle));
        }

        // 6. Sort each layer by insertion order for determinism.
        let order_index: HashMap<&str, usize> = workflow
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.as_str(), i))
            .collect();
        for layer in &mut layers {
            layer.sort_by_key(|id| order_index.get(id.as_str()).copied().unwrap_or(usize::MAX));
        }

        Ok(ExecutionDag {
            steps: workflow.steps.clone(),
            layers,
            layer_of,
            topo,
            dependents,
        })
    }

    /// Number of parallel layers.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Steps in layer `i` (0-indexed). Steps within a layer can be
    /// executed in parallel.
    pub fn layer(&self, i: usize) -> Option<&[String]> {
        self.layers.get(i).map(|v| v.as_slice())
    }

    /// All parallel layers.
    pub fn layers(&self) -> &[Vec<String>] {
        &self.layers
    }

    /// The layer index of a given step, if present.
    pub fn layer_index_of(&self, step_id: &str) -> Option<usize> {
        self.layer_of.get(step_id).copied()
    }

    /// All step ids in stable topological order.
    pub fn topological_order(&self) -> &[String] {
        &self.topo
    }

    /// Look up a step by id.
    pub fn step(&self, id: &str) -> Option<&WorkflowStep> {
        self.steps.iter().find(|s| s.id == id)
    }

    /// Total step count.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// True when the DAG has no steps.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Returns `true` if step `a` must complete before step `b`
    /// (i.e. `b` is a transitive dependent of `a`).
    pub fn must_precede(&self, a: &str, b: &str) -> bool {
        if a == b {
            return false;
        }
        // Walk downstream from `a` following the "who depends on me"
        // relation. If we reach `b`, then `a` must precede `b`.
        let mut visited: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = vec![a.to_string()];
        while let Some(node) = stack.pop() {
            if !visited.insert(node.clone()) {
                continue;
            }
            if let Some(children) = self.dependents.get(&node) {
                for child in children {
                    if child == b {
                        return true;
                    }
                    stack.push(child.clone());
                }
            }
        }
        false
    }

    /// Steps that can start as soon as the given set is complete.
    /// This is the "ready wave" of the next layer.
    pub fn ready_after(&self, completed: &[String]) -> Vec<String> {
        let completed: HashSet<&str> = completed.iter().map(String::as_str).collect();
        let mut ready: Vec<String> = Vec::new();
        for step in &self.steps {
            if completed.contains(step.id.as_str()) {
                continue;
            }
            let all_deps_done = step
                .depends_on
                .iter()
                .all(|d| completed.contains(d.as_str()));
            if all_deps_done {
                ready.push(step.id.clone());
            }
        }
        ready.sort();
        ready
    }
}

/// Attempt to find an actual cycle path through the remaining nodes
/// by following outgoing edges. The result is a vector that begins
/// and ends with the same node, illustrating the cycle.
fn find_cycle_path(remaining: &[String], outgoing: &HashMap<String, Vec<String>>) -> Vec<String> {
    if remaining.is_empty() {
        return Vec::new();
    }
    let remaining_set: HashSet<&str> = remaining.iter().map(String::as_str).collect();
    let start = remaining[0].clone();
    let mut path: Vec<String> = vec![start.clone()];
    let mut visited: HashSet<String> = HashSet::new();
    let mut current = start.clone();
    // Walk until we revisit the start node (which closes the cycle)
    // or run out of forward edges.
    loop {
        if current != start {
            visited.insert(current.clone());
        }
        let next = outgoing
            .get(&current)
            .and_then(|outs| {
                // Prefer the start node if it appears here, to close
                // the cycle. Otherwise pick the first child that's
                // still in the cycle set and not yet visited.
                if let Some(closing) = outs
                    .iter()
                    .find(|o| o.as_str() == start.as_str() && remaining_set.contains(o.as_str()))
                {
                    Some(closing.clone())
                } else {
                    outs.iter()
                        .find(|o| {
                            remaining_set.contains(o.as_str())
                                && !visited.contains(o.as_str())
                        })
                        .cloned()
                }
            });
        match next {
            Some(n) => {
                path.push(n.clone());
                current = n;
                if current == start {
                    break;
                }
            }
            None => break,
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::workflows::WorkflowStep;

    fn three_step_chain() -> Workflow {
        Workflow::new("chain")
            .with_step(WorkflowStep::new("a"))
            .with_step(WorkflowStep::new("b").with_dependency("a"))
            .with_step(WorkflowStep::new("c").with_dependency("b"))
    }

    fn diamond() -> Workflow {
        Workflow::new("diamond")
            .with_step(WorkflowStep::new("a"))
            .with_step(WorkflowStep::new("b").with_dependency("a"))
            .with_step(WorkflowStep::new("c").with_dependency("a"))
            .with_step(WorkflowStep::new("d").with_dependency("b").with_dependency("c"))
    }

    #[test]
    fn test_simple_chain_layers() {
        let mut wf = three_step_chain();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert_eq!(dag.layer_count(), 3);
        assert_eq!(dag.layer(0).unwrap(), &["a".to_string()]);
        assert_eq!(dag.layer(1).unwrap(), &["b".to_string()]);
        assert_eq!(dag.layer(2).unwrap(), &["c".to_string()]);
    }

    #[test]
    fn test_diamond_parallel_layers() {
        let mut wf = diamond();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        // a | b,c | d
        assert_eq!(dag.layer_count(), 3);
        assert_eq!(dag.layer(0).unwrap(), &["a".to_string()]);
        let middle = dag.layer(1).unwrap();
        assert!(middle.contains(&"b".to_string()));
        assert!(middle.contains(&"c".to_string()));
        assert_eq!(middle.len(), 2);
        assert_eq!(dag.layer(2).unwrap(), &["d".to_string()]);
    }

    #[test]
    fn test_empty_workflow_is_error() {
        let wf = Workflow::new("empty");
        let err = ExecutionDag::from_workflow(&wf).unwrap_err();
        assert_eq!(err, DagError::Empty);
    }

    #[test]
    fn test_duplicate_id_detected() {
        let wf = Workflow::new("dup")
            .with_step(WorkflowStep::new("a"))
            .with_step(WorkflowStep::new("a"));
        let err = ExecutionDag::from_workflow(&wf).unwrap_err();
        assert!(matches!(err, DagError::DuplicateId(_)));
    }

    #[test]
    fn test_self_loop_detected() {
        let wf = Workflow::new("self")
            .with_step(WorkflowStep::new("a").with_dependency("a"));
        let err = ExecutionDag::from_workflow(&wf).unwrap_err();
        assert!(matches!(err, DagError::SelfLoop(_)));
    }

    #[test]
    fn test_missing_dependency_detected() {
        let wf = Workflow::new("missing")
            .with_step(WorkflowStep::new("a").with_dependency("z"));
        let err = ExecutionDag::from_workflow(&wf).unwrap_err();
        match err {
            DagError::MissingDependency { step, missing } => {
                assert_eq!(step, "a");
                assert_eq!(missing, "z");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_cycle_path_reported() {
        // a -> b -> c -> a
        let wf = Workflow::new("cycle")
            .with_step(WorkflowStep::new("a").with_dependency("c"))
            .with_step(WorkflowStep::new("b").with_dependency("a"))
            .with_step(WorkflowStep::new("c").with_dependency("b"));
        let err = ExecutionDag::from_workflow(&wf).unwrap_err();
        match err {
            DagError::Cycle(path) => {
                assert!(!path.is_empty());
                assert_eq!(path[0], path[path.len() - 1]); // closes the cycle
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn test_topological_order_chain() {
        let mut wf = three_step_chain();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert_eq!(
            dag.topological_order(),
            &["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_layer_index_of() {
        let mut wf = diamond();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert_eq!(dag.layer_index_of("a"), Some(0));
        assert_eq!(dag.layer_index_of("d"), Some(2));
        assert_eq!(dag.layer_index_of("z"), None);
    }

    #[test]
    fn test_must_precede() {
        let mut wf = diamond();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert!(dag.must_precede("a", "d"));
        assert!(dag.must_precede("a", "b"));
        assert!(dag.must_precede("b", "d"));
        assert!(!dag.must_precede("b", "c"));
        assert!(!dag.must_precede("a", "a"));
    }

    #[test]
    fn test_ready_after_initial() {
        let mut wf = diamond();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        let ready = dag.ready_after(&[]);
        assert_eq!(ready, vec!["a".to_string()]);
    }

    #[test]
    fn test_ready_after_partial() {
        let mut wf = diamond();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        let ready = dag.ready_after(&["a".to_string()]);
        assert!(ready.contains(&"b".to_string()));
        assert!(ready.contains(&"c".to_string()));
        assert!(!ready.contains(&"d".to_string()));
    }

    #[test]
    fn test_ready_after_complete() {
        let mut wf = three_step_chain();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        let ready = dag.ready_after(&["a".to_string(), "b".to_string(), "c".to_string()]);
        assert!(ready.is_empty());
    }

    #[test]
    fn test_step_lookup() {
        let mut wf = three_step_chain();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert!(dag.step("a").is_some());
        assert!(dag.step("missing").is_none());
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut wf = three_step_chain();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert_eq!(dag.len(), 3);
        assert!(!dag.is_empty());
    }

    #[test]
    fn test_layer_out_of_bounds() {
        let mut wf = three_step_chain();
        wf.build_dag().unwrap();
        let dag = ExecutionDag::from_workflow(&wf).unwrap();
        assert!(dag.layer(99).is_none());
    }

    #[test]
    fn test_dag_error_display() {
        let e = DagError::Cycle(vec!["a".into(), "b".into(), "a".into()]);
        let s = e.to_string();
        assert!(s.contains("cycle"));
        assert!(s.contains("a -> b -> a"));
    }

    #[test]
    fn test_dag_error_into_task_error() {
        let e: TaskError = DagError::Empty.into();
        let s = e.to_string();
        assert!(s.contains("no steps"));
    }
}
