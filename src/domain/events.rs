//! Domain events for event sourcing.

use super::{TaskId, TaskState};
use chrono::{DateTime, Utc};

/// A task-related event.
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// Task state changed.
    StateChanged {
        task_id: TaskId,
        from: TaskState,
        to: TaskState,
        timestamp: DateTime<Utc>,
    },

    /// Task execution started.
    ExecutionStarted {
        task_id: TaskId,
        timestamp: DateTime<Utc>,
    },

    /// Task execution completed.
    ExecutionCompleted {
        task_id: TaskId,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Task execution failed.
    ExecutionFailed {
        task_id: TaskId,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// Task was scheduled.
    Scheduled {
        task_id: TaskId,
        scheduled_for: DateTime<Utc>,
        timestamp: DateTime<Utc>,
    },

    /// Task was retried.
    Retried {
        task_id: TaskId,
        attempt: u32,
        timestamp: DateTime<Utc>,
    },

    /// Task was cancelled.
    Cancelled {
        task_id: TaskId,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

/// Event kind for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEventKind {
    Created,
    StateChanged,
    ExecutionStarted,
    ExecutionCompleted,
    ExecutionFailed,
    Scheduled,
    Retried,
    Cancelled,
}

impl TaskEvent {
    /// Get the event kind.
    pub fn kind(&self) -> TaskEventKind {
        match self {
            TaskEvent::StateChanged { .. } => TaskEventKind::StateChanged,
            TaskEvent::ExecutionStarted { .. } => TaskEventKind::ExecutionStarted,
            TaskEvent::ExecutionCompleted { .. } => TaskEventKind::ExecutionCompleted,
            TaskEvent::ExecutionFailed { .. } => TaskEventKind::ExecutionFailed,
            TaskEvent::Scheduled { .. } => TaskEventKind::Scheduled,
            TaskEvent::Retried { .. } => TaskEventKind::Retried,
            TaskEvent::Cancelled { .. } => TaskEventKind::Cancelled,
        }
    }

    /// Get the task ID.
    pub fn task_id(&self) -> &TaskId {
        match self {
            TaskEvent::StateChanged { task_id, .. } => task_id,
            TaskEvent::ExecutionStarted { task_id, .. } => task_id,
            TaskEvent::ExecutionCompleted { task_id, .. } => task_id,
            TaskEvent::ExecutionFailed { task_id, .. } => task_id,
            TaskEvent::Scheduled { task_id, .. } => task_id,
            TaskEvent::Retried { task_id, .. } => task_id,
            TaskEvent::Cancelled { task_id, .. } => task_id,
        }
    }
}

impl serde::Serialize for TaskEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(serde::Serialize)]
        struct EventEnvelope<'a> {
            kind: &'static str,
            task_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            from_state: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            to_state: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            duration_ms: Option<u64>,
            timestamp: &'a DateTime<Utc>,
        }

        let envelope = match self {
            TaskEvent::StateChanged {
                task_id,
                from,
                to,
                timestamp,
            } => EventEnvelope {
                kind: "state_changed",
                task_id: &task_id.0,
                from_state: Some(
                    format!("{:?}", from)
                        .to_lowercase()
                        .trim_start_matches("taskstate::")
                        .into(),
                ),
                to_state: Some(
                    format!("{:?}", to)
                        .to_lowercase()
                        .trim_start_matches("taskstate::")
                        .into(),
                ),
                error: None,
                duration_ms: None,
                timestamp,
            },
            TaskEvent::ExecutionStarted { task_id, timestamp } => EventEnvelope {
                kind: "execution_started",
                task_id: &task_id.0,
                from_state: None,
                to_state: None,
                error: None,
                duration_ms: None,
                timestamp,
            },
            TaskEvent::ExecutionCompleted {
                task_id,
                duration_ms,
                timestamp,
            } => EventEnvelope {
                kind: "execution_completed",
                task_id: &task_id.0,
                from_state: None,
                to_state: None,
                error: None,
                duration_ms: Some(*duration_ms),
                timestamp,
            },
            TaskEvent::ExecutionFailed {
                task_id,
                error,
                timestamp,
            } => EventEnvelope {
                kind: "execution_failed",
                task_id: &task_id.0,
                from_state: None,
                to_state: None,
                error: Some(error),
                duration_ms: None,
                timestamp,
            },
            TaskEvent::Scheduled {
                task_id,
                scheduled_for,
                timestamp,
            } => EventEnvelope {
                kind: "scheduled",
                task_id: &task_id.0,
                from_state: None,
                to_state: None,
                error: None,
                duration_ms: None,
                timestamp,
            },
            TaskEvent::Retried {
                task_id,
                attempt,
                timestamp,
            } => EventEnvelope {
                kind: "retried",
                task_id: &task_id.0,
                from_state: None,
                to_state: None,
                error: None,
                duration_ms: Some(*attempt as u64),
                timestamp,
            },
            TaskEvent::Cancelled {
                task_id,
                reason,
                timestamp,
            } => EventEnvelope {
                kind: "cancelled",
                task_id: &task_id.0,
                from_state: None,
                to_state: None,
                error: reason.as_deref(),
                duration_ms: None,
                timestamp,
            },
        };

        envelope.serialize(serializer)
    }
}
