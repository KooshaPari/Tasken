// SPDX-License-Identifier: MIT OR Apache-2.0
//! Domain errors.

use super::TaskState;

/// Port-related errors for storage, queue, and notification adapters.
#[derive(Debug, thiserror::Error)]
pub enum PortError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Queue error: {0}")]
    Queue(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Not found: {0}")]
    NotFound(String),
}

/// Task-related errors.
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("Task not found: {0}")]
    NotFound(String),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: TaskState, to: TaskState },

    #[error("Task execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Task timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Task cancelled")]
    Cancelled,

    #[error("Retry limit exceeded ({0} attempts)")]
    RetryLimitExceeded(u32),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Port error: {0}")]
    Port(#[from] PortError),
}

impl serde::Serialize for TaskError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_port_error_display() {
        let err = PortError::Storage("disk full".to_string());
        assert_eq!(err.to_string(), "Storage error: disk full");
    }

    #[test]
    fn test_task_error_display() {
        let err = TaskError::NotFound("task-1".to_string());
        assert_eq!(err.to_string(), "Task not found: task-1");
    }

    #[test]
    fn test_task_error_serialize() {
        let err = TaskError::Cancelled;
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Task cancelled\"");
    }

    #[test]
    fn test_port_error_from() {
        let port = PortError::Queue("full".to_string());
        let task_err = TaskError::from(port);
        assert!(matches!(task_err, TaskError::Port(_)));
    }

    #[test]
    fn test_task_error_variants() {
        let variants: Vec<TaskError> = vec![
            TaskError::NotFound("x".to_string()),
            TaskError::InvalidStateTransition { from: TaskState::Pending, to: TaskState::Running },
            TaskError::ExecutionFailed("fail".to_string()),
            TaskError::Timeout(Duration::from_secs(1)),
            TaskError::Cancelled,
            TaskError::RetryLimitExceeded(3),
            TaskError::InvalidOperation("bad".to_string()),
            TaskError::StorageError("io".to_string()),
            TaskError::ValidationError("bad".to_string()),
            TaskError::SerializationError("json".to_string()),
        ];
        for v in variants {
            let s = serde_json::to_string(&v).unwrap();
            assert!(s.starts_with("\"") && s.ends_with("\""));
        }
    }
}
