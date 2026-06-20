// SPDX-License-Identifier: MIT OR Apache-2.0
//! In-memory storage adapter.

use crate::domain::{
    errors::PortError,
    ports::{QueuePort, StoragePort},
    Schedule, Task, Workflow,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory storage implementation.
pub struct MemoryStorage {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
    schedules: Arc<RwLock<HashMap<String, Schedule>>>,
    queue: Arc<RwLock<Vec<Task>>>,
}

impl MemoryStorage {
    /// Create a new memory storage.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            schedules: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StoragePort for MemoryStorage {
    async fn save_task(&self, task: &Task) -> Result<(), PortError> {
        let mut tasks = self.tasks.write().map_err(|e| PortError::Storage(e.to_string()))?;
        tasks.insert(task.id.0.clone(), task.clone());
        Ok(())
    }

    async fn load_task(&self, id: &str) -> Result<Option<Task>, PortError> {
        let tasks = self.tasks.read().map_err(|e| PortError::Storage(e.to_string()))?;
        Ok(tasks.get(id).cloned())
    }

    async fn delete_task(&self, id: &str) -> Result<(), PortError> {
        let mut tasks = self.tasks.write().map_err(|e| PortError::Storage(e.to_string()))?;
        tasks.remove(id);
        Ok(())
    }

    async fn list_tasks(&self) -> Result<Vec<Task>, PortError> {
        let tasks = self.tasks.read().map_err(|e| PortError::Storage(e.to_string()))?;
        Ok(tasks.values().cloned().collect())
    }

    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), PortError> {
        let mut workflows = self.workflows.write().map_err(|e| PortError::Storage(e.to_string()))?;
        workflows.insert(workflow.id.0.clone(), workflow.clone());
        Ok(())
    }

    async fn load_workflow(&self, id: &str) -> Result<Option<Workflow>, PortError> {
        let workflows = self.workflows.read().map_err(|e| PortError::Storage(e.to_string()))?;
        Ok(workflows.get(id).cloned())
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>, PortError> {
        let workflows = self.workflows.read().map_err(|e| PortError::Storage(e.to_string()))?;
        Ok(workflows.values().cloned().collect())
    }

    async fn save_schedule(&self, schedule: &Schedule) -> Result<(), PortError> {
        let mut schedules = self.schedules.write().map_err(|e| PortError::Storage(e.to_string()))?;
        schedules.insert(schedule.id.0.clone(), schedule.clone());
        Ok(())
    }

    async fn load_schedule(&self, id: &str) -> Result<Option<Schedule>, PortError> {
        let schedules = self.schedules.read().map_err(|e| PortError::Storage(e.to_string()))?;
        Ok(schedules.get(id).cloned())
    }

    async fn list_schedules(&self) -> Result<Vec<Schedule>, PortError> {
        let schedules = self.schedules.read().map_err(|e| PortError::Storage(e.to_string()))?;
        Ok(schedules.values().cloned().collect())
    }
}

#[async_trait]
impl QueuePort for MemoryStorage {
    async fn enqueue(&self, task: Task) -> Result<(), PortError> {
        let mut queue = self.queue.write().map_err(|e| PortError::Queue(e.to_string()))?;
        queue.push(task);
        Ok(())
    }

    async fn dequeue(&self) -> Result<Option<Task>, PortError> {
        let mut queue = self.queue.write().map_err(|e| PortError::Queue(e.to_string()))?;
        Ok(queue.pop())
    }

    async fn len(&self) -> Result<usize, PortError> {
        let queue = self.queue.read().map_err(|e| PortError::Queue(e.to_string()))?;
        Ok(queue.len())
    }

    async fn is_empty(&self) -> Result<bool, PortError> {
        let queue = self.queue.read().map_err(|e| PortError::Queue(e.to_string()))?;
        Ok(queue.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_save_and_load_task() {
        let storage = MemoryStorage::new();
        let task = Task::new("test-task");

        storage.save_task(&task).await.unwrap();
        let loaded = storage.load_task(&task.id.0).await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test-task");
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let storage = MemoryStorage::new();

        storage.save_task(&Task::new("task-1")).await.unwrap();
        storage.save_task(&Task::new("task-2")).await.unwrap();

        let tasks = storage.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_queue() {
        let storage = MemoryStorage::new();
        let task = Task::new("queued-task");

        storage.enqueue(task.clone()).await.unwrap();
        assert_eq!(storage.len().await.unwrap(), 1);

        let dequeued = storage.dequeue().await.unwrap();
        assert!(dequeued.is_some());
        assert_eq!(storage.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_save_and_load_workflow() {
        let storage = MemoryStorage::new();
        let workflow = Workflow::new("test-workflow");

        storage.save_workflow(&workflow).await.unwrap();
        let loaded = storage.load_workflow(&workflow.id.0).await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test-workflow");
    }

    #[tokio::test]
    async fn test_save_and_load_schedule() {
        let storage = MemoryStorage::new();
        let schedule = Schedule::once("task-1", chrono::Utc::now() + chrono::Duration::hours(1));

        storage.save_schedule(&schedule).await.unwrap();
        let loaded = storage.load_schedule(&schedule.id.0).await.unwrap();

        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().task_id, "task-1");
    }
}
