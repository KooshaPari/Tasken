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

impl TaskError {
    /// Return a unique machine-readable error code for this variant.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "task_not_found",
            Self::InvalidStateTransition { .. } => "invalid_state_transition",
            Self::ExecutionFailed(_) => "execution_failed",
            Self::Timeout(_) => "task_timeout",
            Self::Cancelled => "task_cancelled",
            Self::RetryLimitExceeded(_) => "retry_limit_exceeded",
            Self::InvalidOperation(_) => "invalid_operation",
            Self::StorageError(_) => "storage_error",
            Self::ValidationError(_) => "validation_error",
            Self::SerializationError(_) => "serialization_error",
            Self::Port(_) => "port_error",
        }
    }

    /// Return the Rust variant name as a string.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "NotFound",
            Self::InvalidStateTransition { .. } => "InvalidStateTransition",
            Self::ExecutionFailed(_) => "ExecutionFailed",
            Self::Timeout(_) => "Timeout",
            Self::Cancelled => "Cancelled",
            Self::RetryLimitExceeded(_) => "RetryLimitExceeded",
            Self::InvalidOperation(_) => "InvalidOperation",
            Self::StorageError(_) => "StorageError",
            Self::ValidationError(_) => "ValidationError",
            Self::SerializationError(_) => "SerializationError",
            Self::Port(_) => "Port",
        }
    }

    /// Return a JSON-friendly structured representation.
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "error_code": self.error_code(),
            "message": self.to_string(),
            "type": self.variant_name(),
        })
    }
}

impl serde::Serialize for TaskError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("error_code", self.error_code())?;
        map.serialize_entry("message", &self.to_string())?;
        map.serialize_entry("type", self.variant_name())?;
        map.end()
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
    fn test_task_error_serialize_structured() {
        let err = TaskError::Cancelled;
        let json = serde_json::to_string(&err).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["error_code"], "task_cancelled");
        assert_eq!(v["message"], "Task cancelled");
        assert_eq!(v["type"], "Cancelled");
    }

    #[test]
    fn test_task_error_error_code() {
        assert_eq!(TaskError::NotFound("x".to_string()).error_code(), "task_not_found");
        assert_eq!(
            TaskError::InvalidStateTransition { from: TaskState::Pending, to: TaskState::Running }
                .error_code(),
            "invalid_state_transition"
        );
        assert_eq!(TaskError::ExecutionFailed("fail".to_string()).error_code(), "execution_failed");
        assert_eq!(TaskError::Timeout(Duration::from_secs(1)).error_code(), "task_timeout");
        assert_eq!(TaskError::Cancelled.error_code(), "task_cancelled");
        assert_eq!(TaskError::RetryLimitExceeded(3).error_code(), "retry_limit_exceeded");
        assert_eq!(
            TaskError::InvalidOperation("bad".to_string()).error_code(),
            "invalid_operation"
        );
        assert_eq!(TaskError::StorageError("io".to_string()).error_code(), "storage_error");
        assert_eq!(TaskError::ValidationError("bad".to_string()).error_code(), "validation_error");
        assert_eq!(
            TaskError::SerializationError("json".to_string()).error_code(),
            "serialization_error"
        );
        let port = PortError::Queue("full".to_string());
        assert_eq!(TaskError::Port(port).error_code(), "port_error");
    }

    #[test]
    fn test_task_error_variant_name() {
        assert_eq!(TaskError::NotFound("x".to_string()).variant_name(), "NotFound");
        assert_eq!(TaskError::Cancelled.variant_name(), "Cancelled");
    }

    #[test]
    fn test_task_error_to_json_value() {
        let err = TaskError::RetryLimitExceeded(5);
        let v = err.to_json_value();
        assert_eq!(v["error_code"], "retry_limit_exceeded");
        assert_eq!(v["message"], "Retry limit exceeded (5 attempts)");
        assert_eq!(v["type"], "RetryLimitExceeded");
    }

    #[test]
    fn test_port_error_from() {
        let port = PortError::Queue("full".to_string());
        let task_err = TaskError::from(port);
        assert!(matches!(task_err, TaskError::Port(_)));
    }

    #[test]
    fn test_task_error_variants_all_have_structured_output() {
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
        // Just check that Port is constructable
        let _port_variant = TaskError::Port(PortError::NotFound("missing".to_string()));

        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(
                parsed.get("error_code").and_then(|c| c.as_str()).is_some(),
                "variant {:?} must have error_code in JSON, got: {json}",
                v.variant_name()
            );
            assert!(
                parsed.get("message").and_then(|m| m.as_str()).is_some(),
                "variant {:?} must have message in JSON",
                v.variant_name()
            );
            assert!(
                parsed.get("type").and_then(|t| t.as_str()).is_some(),
                "variant {:?} must have type in JSON",
                v.variant_name()
            );
        }
    }
}
