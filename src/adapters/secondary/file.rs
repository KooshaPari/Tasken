// SPDX-License-Identifier: MIT OR Apache-2.0
//! File-based storage adapter for persistence.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;

use crate::domain::{
    errors::PortError,
    ports::{QueuePort, StoragePort},
    Group, Schedule, Task, Workflow,
};

/// Data store structure for serialization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct DataStore {
    tasks: HashMap<String, Task>,
    workflows: HashMap<String, Workflow>,
    schedules: HashMap<String, Schedule>,
    groups: HashMap<String, Group>,
    queue: Vec<Task>,
}

/// File-based storage implementation using JSON.
pub struct FileStorage {
    path: std::path::PathBuf,
}

impl FileStorage {
    /// Create a new file storage at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self { path: path.as_ref().to_path_buf() }
    }

    /// Load the data store from disk.
    fn load_store(&self) -> Result<DataStore, PortError> {
        if !self.path.exists() {
            return Ok(DataStore::default());
        }
        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| PortError::Io(format!("Failed to read store: {e}")))?;
        if content.trim().is_empty() {
            return Ok(DataStore::default());
        }
        serde_json::from_str(&content)
            .map_err(|e| PortError::Serialization(format!("Failed to parse store: {e}")))
    }

    /// Save the data store to disk.
    fn save_store(&self, store: &DataStore) -> Result<(), PortError> {
        let content = serde_json::to_string_pretty(store)
            .map_err(|e| PortError::Serialization(format!("Failed to serialize store: {e}")))?;
        std::fs::write(&self.path, content)
            .map_err(|e| PortError::Io(format!("Failed to write store: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl StoragePort for FileStorage {
    async fn save_task(&self, task: &Task) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.tasks.insert(task.id.0.clone(), task.clone());
        self.save_store(&store)
    }

    async fn load_task(&self, id: &str) -> Result<Option<Task>, PortError> {
        let store = self.load_store()?;
        Ok(store.tasks.get(id).cloned())
    }

    async fn delete_task(&self, id: &str) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.tasks.remove(id);
        self.save_store(&store)
    }

    async fn list_tasks(&self) -> Result<Vec<Task>, PortError> {
        let store = self.load_store()?;
        Ok(store.tasks.values().cloned().collect())
    }

    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.workflows.insert(workflow.id.0.clone(), workflow.clone());
        self.save_store(&store)
    }

    async fn load_workflow(&self, id: &str) -> Result<Option<Workflow>, PortError> {
        let store = self.load_store()?;
        Ok(store.workflows.get(id).cloned())
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>, PortError> {
        let store = self.load_store()?;
        Ok(store.workflows.values().cloned().collect())
    }

    async fn save_schedule(&self, schedule: &Schedule) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.schedules.insert(schedule.id.0.clone(), schedule.clone());
        self.save_store(&store)
    }

    async fn load_schedule(&self, id: &str) -> Result<Option<Schedule>, PortError> {
        let store = self.load_store()?;
        Ok(store.schedules.get(id).cloned())
    }

    async fn list_schedules(&self) -> Result<Vec<Schedule>, PortError> {
        let store = self.load_store()?;
        Ok(store.schedules.values().cloned().collect())
    }

    async fn save_group(&self, group: &Group) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.groups.insert(group.id.0.clone(), group.clone());
        self.save_store(&store)
    }

    async fn load_group(&self, id: &str) -> Result<Option<Group>, PortError> {
        let store = self.load_store()?;
        Ok(store.groups.get(id).cloned())
    }

    async fn list_groups(&self) -> Result<Vec<Group>, PortError> {
        let store = self.load_store()?;
        Ok(store.groups.values().cloned().collect())
    }

    async fn delete_group(&self, id: &str) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.groups.remove(id);
        self.save_store(&store)
    }
}

#[async_trait]
impl QueuePort for FileStorage {
    async fn enqueue(&self, task: Task) -> Result<(), PortError> {
        let mut store = self.load_store()?;
        store.queue.push(task);
        self.save_store(&store)
    }

    async fn dequeue(&self) -> Result<Option<Task>, PortError> {
        let mut store = self.load_store()?;
        let task = store.queue.pop();
        self.save_store(&store)?;
        Ok(task)
    }

    async fn len(&self) -> Result<usize, PortError> {
        let store = self.load_store()?;
        Ok(store.queue.len())
    }

    async fn is_empty(&self) -> Result<bool, PortError> {
        let store = self.load_store()?;
        Ok(store.queue.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[tokio::test]
    async fn test_save_and_load_task() {
        let file = NamedTempFile::new().unwrap();
        let storage = FileStorage::new(file.path());
        let task = Task::new("test-task");

        storage.save_task(&task).await.unwrap();
        let loaded = storage.load_task(&task.id.0).await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test-task");
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let file = NamedTempFile::new().unwrap();
        let storage = FileStorage::new(file.path());

        storage.save_task(&Task::new("task-1")).await.unwrap();
        storage.save_task(&Task::new("task-2")).await.unwrap();

        let tasks = storage.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_queue() {
        let file = NamedTempFile::new().unwrap();
        let storage = FileStorage::new(file.path());
        let task = Task::new("queued-task");

        storage.enqueue(task.clone()).await.unwrap();
        assert_eq!(storage.len().await.unwrap(), 1);

        let dequeued = storage.dequeue().await.unwrap();
        assert!(dequeued.is_some());
        assert_eq!(storage.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_persistence() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let task = Task::new("persisted");

        {
            let storage = FileStorage::new(&path);
            storage.save_task(&task).await.unwrap();
        }

        {
            let storage = FileStorage::new(&path);
            let loaded = storage.load_task(&task.id.0).await.unwrap();
            assert!(loaded.is_some());
            assert_eq!(loaded.unwrap().name, "persisted");
        }
    }
}
