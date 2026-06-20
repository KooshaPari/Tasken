// SPDX-License-Identifier: MIT OR Apache-2.0
//! Task entity and related types.

use super::errors::TaskError;
use super::events::TaskEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Unique task identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// Task priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low = 0,
    #[default]
    Normal = 5,
    High = 8,
    Critical = 10,
}

/// Task state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    #[default]
    Pending,
    Scheduled,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

/// Retry policy for failed tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retries.
    pub max_attempts: u32,
    /// Delay between retries (exponential backoff).
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Jitter factor (0.0 to 1.0).
    pub jitter: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            jitter: 0.1,
        }
    }
}

/// Task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier.
    pub id: TaskId,
    /// Human-readable name.
    pub name: String,
    /// Task description.
    pub description: Option<String>,
    /// Current state.
    pub state: TaskState,
    /// Priority level.
    pub priority: Priority,
    /// Timeout duration.
    pub timeout: Option<Duration>,
    /// Retry policy.
    pub retry_policy: Option<RetryPolicy>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
    /// Started timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Completed timestamp.
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Retry count.
    pub retry_count: u32,
    /// Task payload/data.
    pub data: serde_json::Value,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Dependencies — other task IDs that must complete before this task runs.
    pub depends_on: Vec<TaskId>,
}

impl Task {
    /// Create a new task with a name.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: TaskId::new(),
            name: name.into(),
            description: None,
            state: TaskState::Pending,
            priority: Priority::default(),
            timeout: None,
            retry_policy: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            error: None,
            retry_count: 0,
            data: serde_json::Value::Null,
            tags: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    /// Add a description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the retry policy.
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the payload data.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Add a dependency on another task.
    pub fn with_dependency(mut self, task_id: TaskId) -> Self {
        self.depends_on.push(task_id);
        self
    }

    /// Set a shell command as the task payload.
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.data = serde_json::json!({"command": command.into()});
        self
    }

    /// Transition to a new state.
    pub fn transition_to(&mut self, new_state: TaskState) -> Result<TaskEvent, TaskError> {
        let old_state = self.state;

        // Validate state transition
        if !self.can_transition_to(&new_state) {
            return Err(TaskError::InvalidStateTransition {
                from: old_state,
                to: new_state,
            });
        }

        self.state = new_state;
        self.updated_at = Utc::now();

        // Set timestamps based on new state
        match new_state {
            TaskState::Running => self.started_at = Some(Utc::now()),
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled => {
                self.completed_at = Some(Utc::now())
            }
            _ => {}
        }

        Ok(TaskEvent::StateChanged {
            task_id: self.id.clone(),
            from: old_state,
            to: new_state,
            timestamp: Utc::now(),
        })
    }

    /// Check if a state transition is valid.
    pub fn can_transition_to(&self, new_state: &TaskState) -> bool {
        match (&self.state, new_state) {
            // Pending can only go to Scheduled or Running
            (TaskState::Pending, TaskState::Scheduled) => true,
            (TaskState::Pending, TaskState::Running) => true,
            (TaskState::Pending, TaskState::Cancelled) => true,
            // Scheduled can go to Running, Cancelled
            (TaskState::Scheduled, TaskState::Running) => true,
            (TaskState::Scheduled, TaskState::Cancelled) => true,
            // Running can go to Completed, Failed, Retrying, Cancelled
            (TaskState::Running, TaskState::Completed) => true,
            (TaskState::Running, TaskState::Failed) => true,
            (TaskState::Running, TaskState::Retrying) => true,
            (TaskState::Running, TaskState::Cancelled) => true,
            // Retrying goes back to Running
            (TaskState::Retrying, TaskState::Running) => true,
            (TaskState::Retrying, TaskState::Cancelled) => true,
            // Terminal states can't transition
            (TaskState::Completed, _) => false,
            (TaskState::Failed, _) => false,
            (TaskState::Cancelled, _) => false,
            // Any to Pending is invalid
            (_, TaskState::Pending) => false,
            _ => false,
        }
    }

    /// Check if task can be retried.
    pub fn can_retry(&self) -> bool {
        if let Some(ref policy) = self.retry_policy {
            self.retry_count < policy.max_attempts
        } else {
            false
        }
    }

    /// Calculate next retry delay.
    pub fn retry_delay(&self) -> Duration {
        if let Some(ref policy) = self.retry_policy {
            let exp_delay = policy.base_delay * 2u32.pow(self.retry_count);
            exp_delay.min(policy.max_delay)
        } else {
            Duration::from_secs(1)
        }
    }
}

/// Task result type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
}

impl Task {
    /// Create a successful result.
    pub fn success_result(&self, output: serde_json::Value, duration: Duration) -> TaskResult {
        TaskResult {
            task_id: self.id.clone(),
            success: true,
            output: Some(output),
            error: None,
            duration,
            timestamp: Utc::now(),
        }
    }

    /// Create a failure result.
    pub fn failure_result(&self, error: String, duration: Duration) -> TaskResult {
        TaskResult {
            task_id: self.id.clone(),
            success: false,
            output: None,
            error: Some(error),
            duration,
            timestamp: Utc::now(),
        }
    }
}


/// Topologically sort tasks based on their `depends_on` field.
///
/// Uses Kahn's algorithm. Tasks with no dependencies come first,
/// followed by tasks whose dependencies are satisfied.
///
/// # Panics
///
/// Panics if a cycle is detected in the dependency graph.
pub fn topological_sort_tasks(tasks: &[Task]) -> Vec<Task> {
    if tasks.is_empty() {
        return Vec::new();
    }

    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut task_map: HashMap<String, &Task> = HashMap::new();

    for task in tasks {
        in_degree.entry(task.id.0.clone()).or_insert(0);
        adjacency.entry(task.id.0.clone()).or_default();
        task_map.insert(task.id.0.clone(), task);
    }

    for task in tasks {
        for dep in &task.depends_on {
            adjacency
                .entry(dep.0.clone())
                .or_default()
                .push(task.id.0.clone());
            *in_degree.entry(task.id.0.clone()).or_insert(0) += 1;
        }
    }

    let mut queue: Vec<String> = in_degree
        .iter()
        .filter_map(|(id, deg)| if *deg == 0 { Some(id.clone()) } else { None })
        .collect::<Vec<_>>();
    queue.sort();

    let mut sorted: Vec<Task> = Vec::with_capacity(tasks.len());
    let mut visited = HashSet::new();

    while let Some(id) = queue.pop() {
        visited.insert(id.clone());
        if let Some(task) = task_map.get(&id) {
            sorted.push((*task).clone());
        }
        if let Some(children) = adjacency.get(&id) {
            for child in children {
                if let Some(deg) = in_degree.get_mut(child) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 && !visited.contains(child.as_str()) {
                        queue.push(child.clone());
                    }
                }
            }
        }
        queue.sort();
    }

    assert_eq!(
        visited.len(),
        tasks.len(),
        "cycle detected in task dependency graph"
    );

    sorted
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("test-task");
        assert_eq!(task.name, "test-task");
        assert_eq!(task.state, TaskState::Pending);
    }

    #[test]
    fn test_state_transitions() {
        let mut task = Task::new("test");
        assert!(task.transition_to(TaskState::Running).is_ok());
        assert_eq!(task.state, TaskState::Running);
        assert!(task.started_at.is_some());

        assert!(task.transition_to(TaskState::Completed).is_ok());
        assert_eq!(task.state, TaskState::Completed);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_invalid_transition() {
        let mut task = Task::new("test");
        // Can't go directly from Pending to Completed
        assert!(task.transition_to(TaskState::Completed).is_err());
    }

    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
    }

    #[test]
    fn test_can_retry() {
        let mut task = Task::new("retry-test").with_retry_policy(RetryPolicy::default());
        assert!(task.can_retry());
        task.retry_count = 3;
        assert!(!task.can_retry());
    }

    #[test]
    fn test_can_retry_no_policy() {
        let task = Task::new("no-retry");
        assert!(!task.can_retry());
    }

    #[test]
    fn test_retry_delay() {
        let mut task = Task::new("delay-test").with_retry_policy(RetryPolicy::default());
        assert_eq!(task.retry_delay(), Duration::from_secs(1));
        task.retry_count = 1;
        assert_eq!(task.retry_delay(), Duration::from_secs(2));
        task.retry_count = 2;
        assert_eq!(task.retry_delay(), Duration::from_secs(4));
    }

    #[test]
    fn test_retry_delay_max_cap() {
        let mut task = Task::new("delay-cap").with_retry_policy(RetryPolicy {
            max_attempts: 10,
            base_delay: Duration::from_secs(30),
            max_delay: Duration::from_secs(60),
            jitter: 0.0,
        });
        task.retry_count = 5;
        // 30 * 2^5 = 960, but capped at 60
        assert_eq!(task.retry_delay(), Duration::from_secs(60));
    }

    #[test]
    fn test_with_command() {
        let task = Task::new("cmd-test").with_command("echo hello");
        assert_eq!(task.data, serde_json::json!({"command": "echo hello"}));
    }

    #[test]
    fn test_with_tag() {
        let task = Task::new("tag-test").with_tag("dev").with_tag("rust");
        assert_eq!(task.tags, vec!["dev", "rust"]);
    }

    #[test]
    fn test_success_result() {
        let task = Task::new("result-test");
        let result = task.success_result(serde_json::json!({"status": "ok"}), Duration::from_secs(1));
        assert!(result.success);
        assert_eq!(result.task_id, task.id);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_failure_result() {
        let task = Task::new("result-test");
        let result = task.failure_result("it broke".to_string(), Duration::from_secs(1));
        assert!(!result.success);
        assert_eq!(result.error, Some("it broke".to_string()));
    }

    #[test]
    fn test_terminal_state_no_transition() {
        let mut task = Task::new("terminal");
        task.transition_to(TaskState::Running).unwrap();
        task.transition_to(TaskState::Completed).unwrap();
        assert!(task.transition_to(TaskState::Failed).is_err());
    }

    #[test]
    fn test_depends_on() {
        let task = Task::new("dep-test")
            .with_dependency(TaskId::from_string("pre-req"))
            .with_dependency(TaskId::from_string("another-req"));
        assert_eq!(task.depends_on.len(), 2);
        assert_eq!(task.depends_on[0].0, "pre-req");
    }

    #[test]
    fn test_topological_sort_empty() {
        let sorted = topological_sort_tasks(&[]);
        assert!(sorted.is_empty());
    }

    #[test]
    fn test_topological_sort_no_deps() {
        let t1 = Task::new("a");
        let t2 = Task::new("b");
        let sorted = topological_sort_tasks(&[t2.clone(), t1.clone()]);
        // Without deps, preserved in original order
        assert_eq!(sorted.len(), 2);
    }

    #[test]
    fn test_topological_sort_with_deps() {
        let t1 = Task::new("build-lib");
        let t2 = Task::new("run-tests")
            .with_dependency(t1.id.clone());
        let t3 = Task::new("deploy")
            .with_dependency(t2.id.clone());

        let sorted = topological_sort_tasks(&[t3.clone(), t2.clone(), t1.clone()]);
        assert_eq!(sorted.len(), 3);
        // build-lib must be first, run-tests second, deploy third
        assert_eq!(sorted[0].name, "build-lib");
        assert_eq!(sorted[1].name, "run-tests");
        assert_eq!(sorted[2].name, "deploy");
    }

    #[test]
    fn test_topological_sort_detects_cycle() {
        let mut t1 = Task::new("a");
        let mut t2 = Task::new("b").with_dependency(t1.id.clone());
        t1.depends_on.push(t2.id.clone()); // a -> b -> a: cycle

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            topological_sort_tasks(&[t1, t2]);
        }));
        assert!(result.is_err());
    }
}
