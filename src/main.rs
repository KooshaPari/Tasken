// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tasken CLI entry point.

use std::sync::Arc;

use taskkit::adapters::primary::cli::CliAdapter;
use taskkit::adapters::secondary::file::FileStorage;
use taskkit::application::services::TaskService;
use taskkit::config::TaskenConfig;

#[tokio::main]
async fn main() {
    let result = run().await;
    if let Err(err) = result {
        // Print a structured error message to stderr
        eprintln!("Error: {err:#}");

        // If the error chain contains a TaskError, extract structured info.
        // Fall back to plain error output for non-TaskError failures.
        if let Some(task_err) = err.downcast_ref::<taskkit::domain::errors::TaskError>() {
            match serde_json::to_string(task_err) {
                Ok(json) => eprintln!("Structured error: {json}"),
                Err(_) => { /* best-effort; plain text already emitted above */ }
            }
        }
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let config = TaskenConfig::load();

    // Ensure the data directory exists
    let _ = std::fs::create_dir_all(&config.data_dir);

    let store_path = config.store_path();
    let storage = Arc::new(FileStorage::new(&store_path));
    let queue = Arc::new(FileStorage::new(&store_path));
    let service = Arc::new(TaskService::with_config(storage, queue, &config));

    CliAdapter::run(service).await
}
