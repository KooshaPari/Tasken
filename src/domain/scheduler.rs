//! Scheduling logic for task execution.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

/// Schedule identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScheduleId(pub String);

impl ScheduleId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ScheduleId {
    fn default() -> Self {
        Self::new()
    }
}

/// Kind of schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScheduleKind {
    /// One-time execution at a specific time.
    Once { at: DateTime<Utc> },
    /// Repeated at a fixed interval.
    Interval { every: i64 },
    /// Cron-based schedule.
    Cron { expression: String },
    /// Daily at a specific time.
    Daily { at: String },
    /// Weekly on specific days.
    Weekly { days: Vec<String>, at: String },
}

impl ScheduleKind {
    /// Calculate the next execution time.
    pub fn next_run(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            ScheduleKind::Once { at } => {
                if *at > from { Some(*at) } else { None }
            }
            ScheduleKind::Interval { every } => {
                Some(from + Duration::seconds(*every))
            }
            ScheduleKind::Cron { expression } => {
                // Simplified cron parsing
                // In production, use the `cron` crate
                cron_parser::next_run(expression, from)
                    .ok()
                    .map(|dt| DateTime::from_timestamp(dt, 0))
                    .flatten()
            }
            ScheduleKind::Daily { at } => {
                // Simplified daily parsing
                Some(from + Duration::days(1))
            }
            ScheduleKind::Weekly { .. } => {
                Some(from + Duration::weeks(1))
            }
        }
    }
}

/// A schedule attached to a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// Unique identifier.
    pub id: ScheduleId,
    /// Associated task ID.
    pub task_id: String,
    /// Schedule kind.
    pub kind: ScheduleKind,
    /// Whether the schedule is active.
    pub active: bool,
    /// Last execution time.
    pub last_run: Option<DateTime<Utc>>,
    /// Next execution time.
    pub next_run: Option<DateTime<Utc>>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

impl Schedule {
    /// Create a new one-time schedule.
    pub fn once(task_id: impl Into<String>, at: DateTime<Utc>) -> Self {
        Self {
            id: ScheduleId::new(),
            task_id: task_id.into(),
            kind: ScheduleKind::Once { at },
            active: true,
            last_run: None,
            next_run: Some(at),
            created_at: Utc::now(),
        }
    }

    /// Create a new interval schedule.
    pub fn interval(task_id: impl Into<String>, every_seconds: i64) -> Self {
        let now = Utc::now();
        Self {
            id: ScheduleId::new(),
            task_id: task_id.into(),
            kind: ScheduleKind::Interval { every: every_seconds },
            active: true,
            last_run: None,
            next_run: Some(now + Duration::seconds(every_seconds)),
            created_at: now,
        }
    }

    /// Create a new cron schedule.
    pub fn cron(task_id: impl Into<String>, expression: impl Into<String>) -> Self {
        let now = Utc::now();
        let kind = ScheduleKind::Cron { expression: expression.into() };
        Self {
            id: ScheduleId::new(),
            task_id: task_id.into(),
            kind: kind.clone(),
            active: true,
            last_run: None,
            next_run: kind.next_run(now),
            created_at: now,
        }
    }

    /// Update the next run time after execution.
    pub fn tick(&mut self) {
        self.last_run = self.next_run;
        if let Some(next) = self.kind.next_run(self.last_run.unwrap_or(Utc::now())) {
            self.next_run = Some(next);
        }
    }

    /// Pause the schedule.
    pub fn pause(&mut self) {
        self.active = false;
    }

    /// Resume the schedule.
    pub fn resume(&mut self) {
        self.active = true;
    }
}

/// Scheduler trait for managing scheduled tasks.
pub trait Scheduler: Send + Sync {
    /// Add a schedule.
    fn add_schedule(&self, schedule: Schedule) -> Result<(), String>;

    /// Remove a schedule.
    fn remove_schedule(&self, id: &ScheduleId) -> Result<(), String>;

    /// Get all due schedules.
    fn get_due_schedules(&self, at: DateTime<Utc>) -> Vec<Schedule>;

    /// Check if a schedule exists.
    fn has_schedule(&self, id: &ScheduleId) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_once_schedule() {
        let future = Utc::now() + Duration::hours(1);
        let schedule = Schedule::once("task-1", future);

        assert!(schedule.active);
        assert_eq!(schedule.task_id, "task-1");
    }

    #[test]
    fn test_interval_schedule() {
        let schedule = Schedule::interval("task-1", 3600);

        assert!(schedule.active);
        assert!(schedule.next_run.is_some());
    }

    #[test]
    fn test_schedule_tick() {
        let future = Utc::now() + Duration::hours(1);
        let mut schedule = Schedule::once("task-1", future);

        schedule.tick();
        assert!(schedule.last_run.is_some());
    }
}
