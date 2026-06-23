// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tasken CLI entry point.

use std::sync::Arc;

use taskkit::adapters::primary::cli::CliAdapter;
use taskkit::adapters::secondary::file::FileStorage;
use taskkit::application::services::TaskService;
use taskkit::config::TaskenConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = TaskenConfig::load();

    // Ensure the data directory exists
    let _ = std::fs::create_dir_all(&config.data_dir);

    let store_path = config.store_path();
    let storage = Arc::new(FileStorage::new(&store_path));
    let queue = Arc::new(FileStorage::new(&store_path));
    let service = Arc::new(TaskService::with_config(storage, queue, &config));

    CliAdapter::run(service).await
}
