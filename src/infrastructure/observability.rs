// SPDX-License-Identifier: MIT OR Apache-2.0
//! Process-level observability helpers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use tracing_subscriber::EnvFilter;

static METRICS: OnceLock<TaskenMetrics> = OnceLock::new();

/// Lightweight process metrics hook.
///
/// This is intentionally dependency-free so it can run in every build
/// without a dedicated metrics backend. The counters are exported via
/// logs or snapshots for embedding applications.
#[derive(Debug)]
pub struct TaskenMetrics {
    commands_started: AtomicU64,
    commands_succeeded: AtomicU64,
    commands_failed: AtomicU64,
    health_checks: AtomicU64,
}

impl TaskenMetrics {
    fn global() -> &'static Self {
        METRICS.get_or_init(|| Self {
            commands_started: AtomicU64::new(0),
            commands_succeeded: AtomicU64::new(0),
            commands_failed: AtomicU64::new(0),
            health_checks: AtomicU64::new(0),
        })
    }

    pub fn record_command_started(&self) {
        self.commands_started.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_command_finished(&self, success: bool) {
        if success {
            self.commands_succeeded.fetch_add(1, Ordering::Relaxed);
        } else {
            self.commands_failed.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_health_check(&self) {
        self.health_checks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TaskenMetricsSnapshot {
        TaskenMetricsSnapshot {
            commands_started: self.commands_started.load(Ordering::Relaxed),
            commands_succeeded: self.commands_succeeded.load(Ordering::Relaxed),
            commands_failed: self.commands_failed.load(Ordering::Relaxed),
            health_checks: self.health_checks.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskenMetricsSnapshot {
    pub commands_started: u64,
    pub commands_succeeded: u64,
    pub commands_failed: u64,
    pub health_checks: u64,
}

/// Guard that keeps process observability state alive for the lifetime of the command.
#[derive(Debug)]
pub struct ObservabilityGuard {
    request_id: String,
    started_at: Instant,
}

impl ObservabilityGuard {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    pub fn metrics(&self) -> &'static TaskenMetrics {
        TaskenMetrics::global()
    }
}

/// Install structured logging and return a guard carrying the request id.
pub fn install() -> ObservabilityGuard {
    let request_id = uuid::Uuid::new_v4().to_string();
    let filter = std::env::var("TASKEN_LOG_LEVEL")
        .ok()
        .or_else(|| std::env::var("RUST_LOG").ok())
        .and_then(|spec| EnvFilter::try_new(spec).ok())
        .unwrap_or_else(|| EnvFilter::new("taskkit=info"));

    let json_logs =
        std::env::var("TASKEN_LOG_FORMAT").map(|v| v.eq_ignore_ascii_case("json")).unwrap_or(true);

    if json_logs {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .with_target(true)
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .with_target(true)
            .try_init();
    }

    ObservabilityGuard { request_id, started_at: Instant::now() }
}

pub fn metrics() -> &'static TaskenMetrics {
    TaskenMetrics::global()
}
