// SPDX-License-Identifier: MIT OR Apache-2.0
//! Group entity — logical collection of tasks.

use super::tasks::TaskId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique group identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub String);

impl GroupId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl Default for GroupId {
    fn default() -> Self {
        Self::new()
    }
}

/// A group is a named collection of task IDs.
///
/// Groups allow users to organize related tasks and operate on them
/// as a unit (listing, running all members, inspecting group metadata).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Unique identifier.
    pub id: GroupId,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Task IDs that belong to this group.
    pub task_ids: Vec<TaskId>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Group {
    /// Create a new group with a given name.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: GroupId::new(),
            name: name.into(),
            description: None,
            task_ids: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a task to the group.
    pub fn with_task(mut self, task_id: TaskId) -> Self {
        self.task_ids.push(task_id);
        self
    }

    /// Return true if the group has no tasks.
    pub fn is_empty(&self) -> bool {
        self.task_ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_creation() {
        let group = Group::new("build-group");
        assert_eq!(group.name, "build-group");
        assert!(group.description.is_none());
        assert!(group.task_ids.is_empty());
        assert!(group.is_empty());
    }

    #[test]
    fn test_group_with_description() {
        let group = Group::new("test-group")
            .with_description("All test tasks");
        assert_eq!(group.description.as_deref(), Some("All test tasks"));
    }

    #[test]
    fn test_group_with_tasks() {
        let t1 = TaskId::new();
        let t2 = TaskId::new();
        let group = Group::new("my-group")
            .with_task(t1.clone())
            .with_task(t2.clone());
        assert_eq!(group.task_ids.len(), 2);
        assert!(!group.is_empty());
    }

    #[test]
    fn test_group_id_new() {
        let id1 = GroupId::new();
        let id2 = GroupId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_group_id_from_string() {
        let id = GroupId::from_string("my-group-id");
        assert_eq!(id.0, "my-group-id");
    }
}
