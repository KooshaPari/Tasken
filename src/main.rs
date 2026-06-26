// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tasken CLI entry point.

use std::sync::Arc;

use taskkit::adapters::primary::cli::CliAdapter;
use taskkit::adapters::secondary::file::FileStorage;
use taskkit::application::services::TaskService;
use taskkit::config::TaskenConfig;
use taskkit::infrastructure::observability;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let obs = observability::install();
    let config = TaskenConfig::load();

    // Ensure the data directory exists
    let _ = std::fs::create_dir_all(&config.data_dir);

    let store_path = config.store_path();
    let storage = Arc::new(FileStorage::new(&store_path));
    let queue = Arc::new(FileStorage::new(&store_path));
    let service = Arc::new(TaskService::with_config(storage, queue, &config));

    let span = tracing::info_span!(
        "taskkit.process",
        request_id = %obs.request_id(),
        data_dir = %config.data_dir.display(),
        store_path = %store_path.display(),
    );
    let _entered = span.enter();

    let result = CliAdapter::run(service).await;
    let metrics = obs.metrics().snapshot();
    tracing::info!(
        commands_started = metrics.commands_started,
        commands_succeeded = metrics.commands_succeeded,
        commands_failed = metrics.commands_failed,
        health_checks = metrics.health_checks,
        uptime_ms = obs.started_at().elapsed().as_millis() as u64,
        "taskkit process complete"
    );
    result
}
