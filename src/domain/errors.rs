//! Domain errors.

use super::TaskState;

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
}

impl serde::Serialize for TaskError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
