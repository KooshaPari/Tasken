//! Tasken CLI entry point.

use std::path::PathBuf;
use std::sync::Arc;
use taskkit::adapters::primary::cli::CliAdapter;
use taskkit::adapters::secondary::file::FileStorage;
use taskkit::application::services::TaskService;

#[tokio::main]
async fn main() {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("taskkit");
    let _ = std::fs::create_dir_all(&data_dir);
    let store_path = data_dir.join("store.json");

    let storage = Arc::new(FileStorage::new(&store_path));
    let queue = Arc::new(FileStorage::new(&store_path));
    let service = Arc::new(TaskService::new(storage, queue));

    CliAdapter::run(service).await;
}
