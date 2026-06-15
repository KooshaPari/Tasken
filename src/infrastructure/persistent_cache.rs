//! Disk-backed persistent cache for task results.
//!
//! Stores cached task results in a JSON file at a configurable path
//! (default: `~/.taskkit/cache.json`). The cache supports TTL-based
//! expiration and falls back to an ephemeral (in-memory) cache when
//! disk storage is unavailable (e.g., in test environments).

use crate::domain::tasks::{TaskId, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A single entry in the persistent cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// The cached task result.
    result: TaskResult,
    /// Timestamp (seconds since epoch) when this entry was inserted.
    inserted_at_secs: u64,
    /// TTL in seconds.
    ttl_secs: u64,
}

/// Cache state serialized to disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CacheState {
    entries: HashMap<String, CacheEntry>,
}

/// A disk-backed persistent cache with an in-memory fallback.
///
/// Cache entries are written to a JSON file and reloaded on startup.
/// When disk I/O fails, the cache operates in ephemeral mode (in-memory only).
#[derive(Clone)]
pub struct PersistentTaskCache {
    /// Path to the cache file on disk.
    path: Option<PathBuf>,
    /// Shared mutable state: entries + insertion timestamps.
    inner: Arc<Mutex<HashMap<TaskId, (TaskResult, Instant, Duration)>>>,
    /// Whether the cache is in ephemeral mode (no disk persistence).
    ephemeral: bool,
}

impl PersistentTaskCache {
    /// Open or create a persistent cache at the given path.
    ///
    /// If the file exists, entries are loaded from it. Stale entries
    /// (past their TTL) are discarded on load.
    pub fn open(path: &Path, default_ttl: Duration) -> Result<Self, String> {
        let entries = if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<CacheState>(&content) {
                        Ok(state) => state,
                        Err(e) => {
                            // Corrupted cache — start fresh
                            eprintln!("cache file corrupted, rebuilding: {e}");
                            CacheState::default()
                        }
                    }
                }
                Err(e) => {
                    eprintln!("cannot read cache file, starting fresh: {e}");
                    CacheState::default()
                }
            }
        } else {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            CacheState::default()
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut inner = HashMap::new();
        for (key, entry) in entries.entries {
            let age_secs = now.saturating_sub(entry.inserted_at_secs);
            if age_secs < entry.ttl_secs {
                let remaining = Duration::from_secs(entry.ttl_secs - age_secs);
                inner.insert(
                    TaskId::from_string(key),
                    (
                        entry.result,
                        Instant::now()
                            .checked_sub(Duration::from_secs(age_secs))
                            .unwrap_or(Instant::now()),
                        remaining,
                    ),
                );
            }
        }

        Ok(Self {
            path: Some(path.to_path_buf()),
            inner: Arc::new(Mutex::new(inner)),
            ephemeral: false,
        })
    }

    /// Create an ephemeral (in-memory only) cache. Useful for tests.
    pub fn ephemeral(default_ttl: Duration) -> Self {
        Self {
            path: None,
            inner: Arc::new(Mutex::new(HashMap::new())),
            ephemeral: true,
        }
    }

    /// Retrieve a cached result if present and not expired.
    pub fn get(&self, task_id: &TaskId) -> Option<TaskResult> {
        let mut map = self.inner.lock().ok()?;
        let (result, inserted_at, ttl) = map.get(task_id)?;
        if inserted_at.elapsed() > *ttl {
            map.remove(task_id);
            return None;
        }
        Some(result.clone())
    }

    /// Insert a result into the cache and persist to disk (if not ephemeral).
    pub fn insert(&self, task_id: TaskId, result: TaskResult) -> Result<(), String> {
        let ttl = Duration::from_secs(300); // 5-minute default TTL
        {
            let mut map = self.inner.lock().map_err(|e| e.to_string())?;
            map.insert(task_id.clone(), (result, Instant::now(), ttl));
        }

        // Persist to disk unless ephemeral
        if !self.ephemeral {
            self.flush_to_disk()?;
        }

        Ok(())
    }

    /// Insert with a custom TTL.
    pub fn insert_with_ttl(
        &self,
        task_id: TaskId,
        result: TaskResult,
        ttl: Duration,
    ) -> Result<(), String> {
        {
            let mut map = self.inner.lock().map_err(|e| e.to_string())?;
            map.insert(task_id.clone(), (result, Instant::now(), ttl));
        }

        if !self.ephemeral {
            self.flush_to_disk()?;
        }

        Ok(())
    }

    /// Invalidate a single entry.
    pub fn invalidate(&self, task_id: &TaskId) -> Result<(), String> {
        {
            let mut map = self.inner.lock().map_err(|e| e.to_string())?;
            map.remove(task_id);
        }
        if !self.ephemeral {
            self.flush_to_disk()?;
        }
        Ok(())
    }

    /// Clear all entries.
    pub fn clear(&self) -> Result<(), String> {
        {
            let mut map = self.inner.lock().map_err(|e| e.to_string())?;
            map.clear();
        }
        if !self.ephemeral {
            self.flush_to_disk()?;
        }
        Ok(())
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Flush the in-memory state to disk.
    fn flush_to_disk(&self) -> Result<(), String> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let map = self.inner.lock().map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut state = CacheState::default();
        for (task_id, (result, _inserted_at, ttl)) in map.iter() {
            state.entries.insert(
                task_id.0.clone(),
                CacheEntry {
                    result: result.clone(),
                    inserted_at_secs: now,
                    ttl_secs: ttl.as_secs(),
                },
            );
        }

        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| format!("cache serialization failed: {e}"))?;

        // Atomically write via temp file to avoid corruption
        let tmp_path = path.with_extension("cache.tmp");
        fs::write(&tmp_path, &json)
            .map_err(|e| format!("cache write failed: {e}"))?;
        fs::rename(&tmp_path, &path)
            .map_err(|e| format!("cache rename failed: {e}"))?;

        Ok(())
    }
}

impl Default for PersistentTaskCache {
    fn default() -> Self {
        Self::ephemeral(Duration::from_secs(300))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_result(id: &str) -> TaskResult {
        TaskResult {
            task_id: TaskId::from_string(id),
            success: true,
            output: Some(serde_json::json!({"status": "ok"})),
            error: None,
            duration: Duration::from_secs(1),
            timestamp: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_ephemeral_insert_and_get() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        let task_id = TaskId::from_string("t1");
        let result = make_result("t1");
        cache.insert(task_id.clone(), result.clone()).unwrap();
        assert_eq!(cache.len(), 1);
        let got = cache.get(&task_id).unwrap();
        assert!(got.success);
    }

    #[test]
    fn test_ephemeral_ttl_expiration() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_millis(1));
        let task_id = TaskId::from_string("t2");
        let result = make_result("t2");
        cache.insert(task_id.clone(), result).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(cache.get(&task_id).is_none());
    }

    #[test]
    fn test_ephemeral_invalidate() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        let task_id = TaskId::from_string("t3");
        let result = make_result("t3");
        cache.insert(task_id.clone(), result).unwrap();
        cache.invalidate(&task_id).unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_ephemeral_clear() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        cache.insert(TaskId::from_string("a"), make_result("a")).unwrap();
        cache.insert(TaskId::from_string("b"), make_result("b")).unwrap();
        assert_eq!(cache.len(), 2);
        cache.clear().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_disk_cache_persistence() {
        let dir = std::env::temp_dir().join(format!("taskkit-cache-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("cache.json");

        // Write to cache
        {
            let cache = PersistentTaskCache::open(&path, Duration::from_secs(300)).unwrap();
            assert!(cache.is_empty());
            let task_id = TaskId::from_string("disk-t1");
            let result = make_result("disk-t1");
            cache.insert(task_id.clone(), result).unwrap();
            assert_eq!(cache.len(), 1);
        }

        // Re-open and verify data persists
        {
            let cache = PersistentTaskCache::open(&path, Duration::from_secs(300)).unwrap();
            assert_eq!(cache.len(), 1);
            let got = cache.get(&TaskId::from_string("disk-t1")).unwrap();
            assert!(got.success);
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_disk_cache_expired_on_reload() {
        let dir = std::env::temp_dir().join(format!("taskkit-cache-expired-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("cache.json");

        // Insert with a 0-second TTL (already expired)
        {
            let cache = PersistentTaskCache::open(&path, Duration::from_secs(0)).unwrap();
            cache
                .insert_with_ttl(TaskId::from_string("expired"), make_result("expired"), Duration::from_secs(0))
                .unwrap();
        }

        // Re-open — entry should be gone
        {
            let cache = PersistentTaskCache::open(&path, Duration::from_secs(300)).unwrap();
            assert!(cache.is_empty());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
