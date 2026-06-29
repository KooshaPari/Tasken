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
        #[derive(serde::Serialize)]
        struct ErrorEnvelope {
            #[serde(rename = "type")]
            type_: String,
            message: String,
        }

        let envelope = match self {
            TaskError::NotFound(id) => ErrorEnvelope {
                type_: "NotFound".to_string(),
                message: format!("Task not found: {id}"),
            },
            TaskError::InvalidStateTransition { from, to } => ErrorEnvelope {
                type_: "InvalidStateTransition".to_string(),
                message: format!("Invalid state transition from {from:?} to {to:?}"),
            },
            TaskError::ExecutionFailed(msg) => {
                ErrorEnvelope { type_: "ExecutionFailed".to_string(), message: msg.clone() }
            }
            TaskError::Timeout(d) => ErrorEnvelope {
                type_: "Timeout".to_string(),
                message: format!("Task timed out after {d:?}"),
            },
            TaskError::Cancelled => ErrorEnvelope {
                type_: "Cancelled".to_string(),
                message: "Task cancelled".to_string(),
            },
            TaskError::RetryLimitExceeded(n) => ErrorEnvelope {
                type_: "RetryLimitExceeded".to_string(),
                message: format!("Retry limit exceeded ({n} attempts)"),
            },
            TaskError::InvalidOperation(msg) => {
                ErrorEnvelope { type_: "InvalidOperation".to_string(), message: msg.clone() }
            }
            TaskError::StorageError(msg) => {
                ErrorEnvelope { type_: "StorageError".to_string(), message: msg.clone() }
            }
            TaskError::ValidationError(msg) => {
                ErrorEnvelope { type_: "ValidationError".to_string(), message: msg.clone() }
            }
            TaskError::SerializationError(msg) => {
                ErrorEnvelope { type_: "SerializationError".to_string(), message: msg.clone() }
            }
            TaskError::Port(inner) => {
                ErrorEnvelope { type_: "PortError".to_string(), message: inner.to_string() }
            }
        };
        serde::Serialize::serialize(&envelope, serializer)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

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
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "Cancelled");
        assert_eq!(v["message"], "Task cancelled");
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
            let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
            assert!(
                parsed["type"].is_string(),
                "Missing 'type' field in serialized error: {parsed}"
            );
            assert!(
                parsed["message"].is_string(),
                "Missing 'message' field in serialized error: {parsed}"
            );
        }
    }

    #[test]
    fn test_port_error_wrapper_serialize() {
        let port = PortError::Storage("disk full".to_string());
        let task_err = TaskError::Port(port);
        let json = serde_json::to_string(&task_err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "PortError");
        assert_eq!(v["message"], "Storage error: disk full");
    }

    #[test]
    fn test_structured_error_json_roundtrip() {
        let err = TaskError::ExecutionFailed("something broke".to_string());
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "ExecutionFailed");
        assert_eq!(v["message"], "something broke");
        // Verify the object has exactly 2 fields
        assert_eq!(v.as_object().map(|o| o.len()), Some(2));
    }
}
