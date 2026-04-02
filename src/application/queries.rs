//! Task query definitions.

use super::super::domain::errors::TaskError;
use super::super::domain::events::TaskEvent;
use super::super::domain::{Task, TaskId, TaskState};
use super::services::TaskService;

/// Query to get a task by ID.
pub struct GetTask {
    pub task_id: TaskId,
}

impl GetTask {
    pub fn new(task_id: TaskId) -> Self {
        Self { task_id }
    }

    /// Execute the query.
    pub async fn execute(self, service: &TaskService) -> Result<Option<Task>, TaskError> {
        service.get_task(&self.task_id).await
    }
}

/// Query to list all tasks.
pub struct ListTasks {
    pub state_filter: Option<TaskState>,
    pub tag_filter: Option<String>,
    pub limit: Option<usize>,
}

impl Default for ListTasks {
    fn default() -> Self {
        Self {
            state_filter: None,
            tag_filter: None,
            limit: Some(100),
        }
    }
}

impl ListTasks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_state(mut self, state: TaskState) -> Self {
        self.state_filter = Some(state);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag_filter = Some(tag.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Execute the query.
    pub async fn execute(self, service: &TaskService) -> Result<Vec<Task>, TaskError> {
        service
            .list_tasks(self.state_filter, self.tag_filter, self.limit)
            .await
    }
}

/// Query to get task event history.
pub struct GetTaskHistory {
    pub task_id: TaskId,
}

impl GetTaskHistory {
    pub fn new(task_id: TaskId) -> Self {
        Self { task_id }
    }

    /// Execute the query.
    pub async fn execute(self, service: &TaskService) -> Result<Vec<TaskEvent>, TaskError> {
        service.get_task_history(&self.task_id).await
    }
}
