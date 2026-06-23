// SPDX-License-Identifier: MIT OR Apache-2.0
//! File watcher — monitors a directory for changes and re-runs a callback.
//!
//! Uses the `notify` crate with debounce support to coalesce rapid changes.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Minimum interval between consecutive callback invocations (debounce).
const DEFAULT_DEBOUNCE_MS: u64 = 500;

/// Watches a directory for file-system changes and re-runs a callback.
pub struct FileWatcher {
    /// Whether the watcher is still running.
    running: Arc<AtomicBool>,
    /// Debounce interval.
    debounce_ms: u64,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWatcher {
    /// Create a new `FileWatcher` with default debounce (500 ms).
    pub fn new() -> Self {
        Self { running: Arc::new(AtomicBool::new(false)), debounce_ms: DEFAULT_DEBOUNCE_MS }
    }

    /// Set a custom debounce interval in milliseconds.
    pub fn with_debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Watch `path` (file or directory) and call `callback` each time a
    /// change is detected.  Rapid changes within the debounce window are
    /// coalesced into a single invocation.
    ///
    /// Blocks until an error occurs or the watcher is stopped (e.g. by
    /// dropping the returned handle or calling [`Self::stop`]).
    pub fn watch_and_run<F>(&self, path: &Path, callback: F) -> notify::Result<()>
    where
        F: Fn() + Send + 'static,
    {
        self.running.store(true, Ordering::SeqCst);

        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        let mut watcher: RecommendedWatcher =
            Watcher::new(tx, Config::default().with_poll_interval(Duration::from_millis(200)))?;

        // If path is a file, watch its parent directory instead.
        let watch_path: PathBuf = if path.is_file() {
            path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
        } else {
            path.to_path_buf()
        };

        watcher.watch(&watch_path, RecursiveMode::Recursive)?;

        let debounce = Duration::from_millis(self.debounce_ms);
        let mut last_trigger = Instant::now();
        let mut pending = false;

        eprintln!(
            "Watching {} for changes (debounce: {} ms)...",
            watch_path.display(),
            self.debounce_ms
        );

        while self.running.load(Ordering::SeqCst) {
            match rx.recv_timeout(debounce) {
                Ok(Ok(event)) => {
                    // Ignore metadata-only or temporary-file events to reduce noise.
                    if is_relevant_event(&event) {
                        pending = true;
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[watcher] error: {e}");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Timeout with no new events — fire if we have something pending.
                    if pending && last_trigger.elapsed() >= debounce {
                        callback();
                        pending = false;
                        last_trigger = Instant::now();
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!("[watcher] channel disconnected, stopping");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Signal the watcher loop to stop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Returns `true` if the event describes a change that should trigger a
/// re-run (i.e. content modifications, creations, or deletions).
fn is_relevant_event(event: &Event) -> bool {
    match event.kind {
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => true,
        EventKind::Access(_) | EventKind::Other | EventKind::Any => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use notify::event::EventAttributes;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_is_relevant_event_create() {
        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![PathBuf::from("/tmp/test")],
            attrs: EventAttributes::new(),
        };
        assert!(is_relevant_event(&event));
    }

    #[test]
    fn test_is_relevant_event_remove() {
        let event = Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![PathBuf::from("/tmp/test")],
            attrs: EventAttributes::new(),
        };
        assert!(is_relevant_event(&event));
    }

    #[test]
    fn test_is_relevant_event_modify() {
        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![PathBuf::from("/tmp/test")],
            attrs: EventAttributes::new(),
        };
        assert!(is_relevant_event(&event));
    }

    #[test]
    fn test_is_relevant_event_access() {
        let event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Close(
                notify::event::AccessMode::Any,
            )),
            paths: vec![PathBuf::from("/tmp/test")],
            attrs: EventAttributes::new(),
        };
        assert!(!is_relevant_event(&event));
    }

    #[test]
    fn test_is_relevant_event_other() {
        let event = Event { kind: EventKind::Other, paths: vec![], attrs: EventAttributes::new() };
        assert!(!is_relevant_event(&event));
    }

    /// Integration-style test: create a temp dir, start watching it in a
    /// background thread, touch a file, and verify the callback fires.
    #[test]
    fn test_watch_detects_file_creation() {
        let dir = TempDir::new().unwrap();
        let watch_path = dir.path().to_path_buf();

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let watcher = FileWatcher::new().with_debounce(100);
        let running = watcher.running.clone();
        let watch_path_clone = watch_path.clone();

        // Spawn watcher in background thread.
        std::thread::spawn(move || {
            let cb = move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            };
            let _ = watcher.watch_and_run(&watch_path_clone, cb);
        });

        // Give watcher time to start.
        std::thread::sleep(Duration::from_millis(300));

        // Create a file — should trigger the callback.
        let file_path = watch_path.join("test_file.txt");
        std::fs::write(&file_path, "hello").unwrap();

        // Wait for debounce + processing.
        std::thread::sleep(Duration::from_millis(500));

        // Stop watcher.
        running.store(false, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(200));

        let count = counter.load(Ordering::SeqCst);
        assert!(count >= 1, "callback should have fired at least once, got {count}");
    }
}
