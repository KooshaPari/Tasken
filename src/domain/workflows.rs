// SPDX-License-Identifier: MIT OR Apache-2.0
//! Workflow definitions and DAG orchestration.

use chrono::{DateTime, Utc};
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};

use super::tasks::TaskId;

/// Workflow identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

/// Workflow state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    #[default]
    Draft,
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Step identifier.
    pub id: String,
    /// Task ID to execute (optional if inline action).
    pub task_id: Option<TaskId>,
    /// Step name.
    pub name: String,
    /// Dependencies (other step IDs that must complete first).
    pub depends_on: Vec<String>,
    /// Step configuration.
    pub config: serde_json::Value,
    /// Timeout for this step.
    pub timeout_seconds: Option<u64>,
    /// Retry configuration.
    pub retry_on_failure: bool,
}

impl WorkflowStep {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: name.clone(), // Use name as id for easier testing
            task_id: None,
            name,
            depends_on: Vec::new(),
            config: serde_json::Value::Null,
            timeout_seconds: None,
            retry_on_failure: false,
        }
    }

    pub fn with_task(mut self, task_id: TaskId) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.depends_on.push(dep.into());
        self
    }
}

/// Workflow definition with DAG structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique identifier.
    pub id: WorkflowId,
    /// Workflow name.
    pub name: String,
    /// Description.
    pub description: Option<String>,
    /// Current state.
    pub state: WorkflowState,
    /// Workflow steps (nodes in the DAG).
    pub steps: Vec<WorkflowStep>,
    /// DAG representation for execution order.
    #[serde(skip)]
    dag: DiGraph<String, ()>,
    /// Node index mapping.
    #[serde(skip)]
    node_map: std::collections::HashMap<String, NodeIndex>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
    /// Started timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Completed timestamp.
    pub completed_at: Option<DateTime<Utc>>,
}

impl Workflow {
    /// Create a new empty workflow.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: WorkflowId::new(),
            name: name.into(),
            description: None,
            state: WorkflowState::Draft,
            steps: Vec::new(),
            dag: DiGraph::new(),
            node_map: std::collections::HashMap::new(),
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
        }
    }

    /// Add a step to the workflow.
    pub fn with_step(mut self, step: WorkflowStep) -> Self {
        let idx = self.dag.add_node(step.id.clone());
        self.node_map.insert(step.id.clone(), idx);
        self.steps.push(step);
        self
    }

    /// Build the DAG from steps.
    pub fn build_dag(&mut self) -> Result<(), String> {
        // Clear and rebuild
        self.dag = DiGraph::new();
        self.node_map.clear();

        // Add all nodes
        for step in &self.steps {
            let idx = self.dag.add_node(step.id.clone());
            self.node_map.insert(step.id.clone(), idx);
        }

        // Add edges based on dependencies
        for step in &self.steps {
            let target_idx = self
                .node_map
                .get(&step.id)
                .ok_or_else(|| format!("Step not found: {}", step.id))?;

            for dep in &step.depends_on {
                if let Some(source_idx) = self.node_map.get(dep) {
                    self.dag.add_edge(*source_idx, *target_idx, ());
                }
            }
        }

        // Check for cycles
        match petgraph::algo::toposort(&self.dag, None) {
            Ok(cycle) => {
                if cycle.len() != self.steps.len() {
                    return Err("Workflow contains cycles".to_string());
                }
            }
            Err(_) => {
                return Err("Workflow contains cycles".to_string());
            }
        }

        Ok(())
    }

    /// Get execution order (topological sort).
    pub fn execution_order(&self) -> Result<Vec<WorkflowStep>, String> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for step in &self.steps {
            self.visit_step(&step.id, &mut visited, &mut result)?;
        }

        Ok(result)
    }

    fn visit_step(
        &self,
        step_id: &str,
        visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<WorkflowStep>,
    ) -> Result<(), String> {
        if visited.contains(step_id) {
            return Ok(());
        }

        visited.insert(step_id.to_string());

        // Find the step
        let step = self
            .steps
            .iter()
            .find(|s| s.id == step_id)
            .ok_or_else(|| format!("Step not found: {step_id}"))?;

        // Visit dependencies first
        for dep in &step.depends_on {
            self.visit_step(dep, visited, result)?;
        }

        result.push(step.clone());
        Ok(())
    }

    /// Get steps that can run in parallel at a given point.
    pub fn ready_steps(&self, completed: &[String]) -> Vec<&WorkflowStep> {
        self.steps
            .iter()
            .filter(|step| {
                !completed.contains(&step.id)
                    && step.depends_on.iter().all(|dep| completed.contains(dep))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_creation() {
        let workflow = Workflow::new("test-workflow");
        assert_eq!(workflow.name, "test-workflow");
        assert_eq!(workflow.state, WorkflowState::Draft);
    }

    #[test]
    fn test_workflow_with_steps() {
        let workflow = Workflow::new("test")
            .with_step(WorkflowStep::new("step-1").with_dependency("step-0"))
            .with_step(WorkflowStep::new("step-0"));

        assert_eq!(workflow.steps.len(), 2);
    }

    #[test]
    fn test_execution_order() {
        let mut workflow = Workflow::new("test")
            .with_step(WorkflowStep::new("step-0")) // Add step-0 first
            .with_step(WorkflowStep::new("step-1").with_dependency("step-0")); // Then step-1 depends on step-0

        workflow.build_dag().unwrap();
        let order = workflow.execution_order().unwrap();

        assert_eq!(order[0].name, "step-0");
        assert_eq!(order[1].name, "step-1");
    }
}
