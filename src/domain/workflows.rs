//! Workflow definitions and DAG orchestration.

use serde::{Deserialize, Serialize};
use petgraph::graph::{DiGraph, NodeIndex};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use super::TaskId;

/// Workflow identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

/// Workflow state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    Draft,
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl Default for WorkflowState {
    fn default() -> Self {
        WorkflowState::Draft
    }
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
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: None,
            name: name.into(),
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
            let target_idx = self.node_map.get(&step.id)
                .ok_or_else(|| format!("Step not found: {}", step.id))?;

            for dep in &step.depends_on {
                if let Some(source_idx) = self.node_map.get(dep) {
                    self.dag.add_edge(*source_idx, *target_idx, ());
                }
            }
        }

        // Check for cycles
        if let Some(cycle) = petgraph::algo::toposort(&self.dag, None) {
            if cycle.len() != self.steps.len() {
                return Err("Workflow contains cycles".to_string());
            }
        }

        Ok(())
    }

    /// Get execution order (topological sort).
    pub fn execution_order(&self) -> Result<Vec<&WorkflowStep>, String> {
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for step in &self.steps {
            self.visit_step(step, &mut visited, &mut result)?;
        }

        Ok(result)
    }

    fn visit_step(
        &self,
        step: &WorkflowStep,
        visited: &mut std::collections::HashSet<&str>,
        result: &mut Vec<&WorkflowStep>,
    ) -> Result<(), String> {
        if visited.contains(step.id.as_str()) {
            return Ok(());
        }

        visited.insert(step.id.as_str());

        // Visit dependencies first
        for dep in &step.depends_on {
            if let Some(dep_step) = self.steps.iter().find(|s| &s.id == dep) {
                self.visit_step(dep_step, visited, result)?;
            }
        }

        result.push(step);
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
            .with_step(
                WorkflowStep::new("step-1")
                    .with_dependency("step-0")
            )
            .with_step(WorkflowStep::new("step-0"));

        assert_eq!(workflow.steps.len(), 2);
    }

    #[test]
    fn test_execution_order() {
        let mut workflow = Workflow::new("test")
            .with_step(WorkflowStep::new("step-1").with_dependency("step-0"))
            .with_step(WorkflowStep::new("step-0"));

        workflow.build_dag().unwrap();
        let order = workflow.execution_order().unwrap();

        assert_eq!(order[0].name, "step-0");
        assert_eq!(order[1].name, "step-1");
    }
}
