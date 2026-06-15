//! Disk-backed persistent cache for task results.
//!
//! The in-memory [`TaskCache`](super::super::infrastructure::cache::TaskCache)
//! is process-local: a server restart loses all cached results. The
//! [`PersistentTaskCache`] solves that by atomically writing entries
//! to a JSON file, with a TTL that survives restarts.
//!
//! # Format
//!
//! ```json
//! {
//!   "version": 1,
//!   "entries": {
//!     "<task_id>": {
//!       "result": { ... TaskResult ... },
//!       "expires_at": "2026-06-15T12:34:56Z"
//!     }
//!   }
//! }
//! ```
//!
//! # Concurrency
//!
//! - All reads/writes are guarded by a `Mutex`.
//! - Writes go to a sibling temp file and are then atomically renamed
//!   into place so a crash mid-write can never leave a torn JSON file.
//!
//! # Failure handling
//!
//! Persistence errors are surfaced as [`CacheError`]. The in-memory
//! mirror continues to work even if a flush fails, so the caller can
//! choose whether to fall back, retry, or bubble up the error.

use crate::domain::tasks::{TaskId, TaskResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// Cache persistence errors.
#[derive(Debug)]
pub enum CacheError {
    /// I/O error.
    Io(io::Error),
    /// JSON parse / serialize error.
    Serde(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::Io(e) => write!(f, "cache I/O error: {e}"),
            CacheError::Serde(s) => write!(f, "cache JSON error: {s}"),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CacheError::Io(e) => Some(e),
            CacheError::Serde(_) => None,
        }
    }
}

impl From<io::Error> for CacheError {
    fn from(e: io::Error) -> Self {
        CacheError::Io(e)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(e: serde_json::Error) -> Self {
        CacheError::Serde(e.to_string())
    }
}

/// On-disk representation of a cache entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEntry {
    result: TaskResult,
    /// Absolute UTC time at which this entry expires.
    expires_at: DateTime<Utc>,
}

/// On-disk representation of the entire cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedCache {
    /// Format version for forward compatibility.
    version: u32,
    entries: HashMap<String, PersistedEntry>,
}

/// A disk-backed TTL cache. Thread-safe.
///
/// Behaves like [`TaskCache`](super::super::infrastructure::cache::TaskCache)
/// but persists to disk. Reads are served from the in-memory mirror
/// (which is hydrated from disk on first access).
pub struct PersistentTaskCache {
    path: PathBuf,
    in_memory: Mutex<HashMap<TaskId, PersistedEntry>>,
    default_ttl: Duration,
}

impl PersistentTaskCache {
    /// Schema version. Bump when changing the on-disk format.
    const VERSION: u32 = 1;

    /// Open or create a persistent cache at `path`. If the file does
    /// not exist, an empty cache is initialized in memory. If the
    /// file is corrupt, it is moved aside as `<path>.corrupt` and an
    /// empty cache is returned.
    pub fn open(path: impl AsRef<Path>, default_ttl: Duration) -> Result<Self, CacheError> {
        let path = path.as_ref().to_path_buf();
        let in_memory = if path.exists() {
            match Self::read_from_disk(&path) {
                Ok(cache) => Self::drop_expired(cache),
                Err(_e) => {
                    // Quarantine the corrupt file but keep going with
                    // an empty cache. The caller can still operate.
                    let backup = path.with_extension("json.corrupt");
                    let _ = fs::rename(&path, &backup);
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        };

        Ok(Self {
            path,
            in_memory: Mutex::new(in_memory),
            default_ttl,
        })
    }

    /// Construct a fresh in-memory cache without touching disk.
    /// Useful for tests.
    pub fn ephemeral(default_ttl: Duration) -> Self {
        Self {
            path: PathBuf::new(),
            in_memory: Mutex::new(HashMap::new()),
            default_ttl,
        }
    }

    /// Read a value from the cache. Returns `None` when missing or
    /// expired; expired entries are evicted lazily.
    pub fn get(&self, task_id: &TaskId) -> Option<TaskResult> {
        let mut map = self.in_memory.lock().ok()?;
        let entry = map.get(task_id)?;
        if Utc::now() >= entry.expires_at {
            map.remove(task_id);
            return None;
        }
        Some(entry.result.clone())
    }

    /// Insert a value with the default TTL.
    pub fn insert(&self, task_id: TaskId, result: TaskResult) -> Result<(), CacheError> {
        let ttl = self.default_ttl;
        self.insert_with_ttl(task_id, result, ttl)
    }

    /// Insert a value with a custom TTL.
    pub fn insert_with_ttl(
        &self,
        task_id: TaskId,
        result: TaskResult,
        ttl: Duration,
    ) -> Result<(), CacheError> {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::seconds(60));
        let entry = PersistedEntry { result, expires_at };
        {
            let mut map = self
                .in_memory
                .lock()
                .map_err(|e| CacheError::Io(io::Error::other(e.to_string())))?;
            map.insert(task_id, entry);
        }
        if !self.path.as_os_str().is_empty() {
            self.flush()?;
        }
        Ok(())
    }

    /// Invalidate a single entry.
    pub fn invalidate(&self, task_id: &TaskId) -> Result<(), CacheError> {
        {
            let mut map = self
                .in_memory
                .lock()
                .map_err(|e| CacheError::Io(io::Error::other(e.to_string())))?;
            map.remove(task_id);
        }
        if !self.path.as_os_str().is_empty() {
            self.flush()?;
        }
        Ok(())
    }

    /// Clear the cache entirely.
    pub fn clear(&self) -> Result<(), CacheError> {
        {
            let mut map = self
                .in_memory
                .lock()
                .map_err(|e| CacheError::Io(io::Error::other(e.to_string())))?;
            map.clear();
        }
        if !self.path.as_os_str().is_empty() {
            self.flush()?;
        }
        Ok(())
    }

    /// Number of entries currently in memory.
    pub fn len(&self) -> usize {
        self.in_memory.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// True when the cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Path to the backing file (empty for ephemeral caches).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Drop expired entries and persist the trimmed view.
    pub fn prune_expired(&self) -> Result<usize, CacheError> {
        let removed;
        {
            let mut map = self
                .in_memory
                .lock()
                .map_err(|e| CacheError::Io(io::Error::other(e.to_string())))?;
            let before = map.len();
            let now = Utc::now();
            map.retain(|_, e| e.expires_at > now);
            removed = before - map.len();
        }
        if !self.path.as_os_str().is_empty() && removed > 0 {
            self.flush()?;
        }
        Ok(removed)
    }

    /// Write the in-memory state to disk atomically.
    fn flush(&self) -> Result<(), CacheError> {
        let map = self
            .in_memory
            .lock()
            .map_err(|e| CacheError::Io(io::Error::other(e.to_string())))?;
        let cache = PersistedCache {
            version: Self::VERSION,
            entries: map
                .iter()
                .map(|(k, v)| (k.0.clone(), v.clone()))
                .collect(),
        };
        let serialized = serde_json::to_string_pretty(&cache)?;
        // Write to a sibling temp file then atomically rename.
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let tmp = parent.join(format!(
            ".{}.tmp",
            self.path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "cache.json".to_string())
        ));
        fs::write(&tmp, serialized)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    fn read_from_disk(path: &Path) -> Result<HashMap<TaskId, PersistedEntry>, CacheError> {
        let content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }
        let cache: PersistedCache = serde_json::from_str(&content)?;
        Ok(cache
            .entries
            .into_iter()
            .map(|(k, v)| (TaskId::from_string(k), v))
            .collect())
    }

    fn drop_expired(
        entries: HashMap<TaskId, PersistedEntry>,
    ) -> HashMap<TaskId, PersistedEntry> {
        let now = Utc::now();
        entries
            .into_iter()
            .filter(|(_, e)| e.expires_at > now)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fake_result(id: &str, success: bool) -> (TaskId, TaskResult) {
        let task_id = TaskId::from_string(id);
        let result = TaskResult {
            task_id: task_id.clone(),
            success,
            output: Some(serde_json::json!({"id": id})),
            error: None,
            duration: Duration::from_millis(1),
            timestamp: Utc::now(),
        };
        (task_id, result)
    }

    #[test]
    fn test_ephemeral_insert_get() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        let (id, r) = fake_result("t1", true);
        cache.insert(id.clone(), r.clone()).unwrap();
        let got = cache.get(&id).unwrap();
        assert_eq!(got.task_id, id);
        assert!(got.success);
    }

    #[test]
    fn test_ephemeral_invalidate() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        let (id, r) = fake_result("t1", true);
        cache.insert(id.clone(), r).unwrap();
        cache.invalidate(&id).unwrap();
        assert!(cache.get(&id).is_none());
    }

    #[test]
    fn test_ephemeral_clear() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        let (id1, r1) = fake_result("t1", true);
        let (id2, r2) = fake_result("t2", true);
        cache.insert(id1, r1).unwrap();
        cache.insert(id2, r2).unwrap();
        assert_eq!(cache.len(), 2);
        cache.clear().unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_ttl_expiration_removes_entry() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_millis(1));
        let (id, r) = fake_result("t1", true);
        cache.insert(id.clone(), r).unwrap();
        // Wait long enough for the entry to expire
        std::thread::sleep(Duration::from_millis(20));
        assert!(cache.get(&id).is_none());
    }

    #[test]
    fn test_persistence_survives_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.json");

        // First session: insert
        {
            let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
            let (id, r) = fake_result("p1", true);
            cache.insert(id.clone(), r).unwrap();
            assert_eq!(cache.len(), 1);
        }

        // Second session: re-open and verify
        {
            let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
            assert_eq!(cache.len(), 1);
            let id = TaskId::from_string("p1");
            let got = cache.get(&id).expect("entry should be loaded");
            assert!(got.success);
            assert_eq!(got.task_id, id);
        }
    }

    #[test]
    fn test_corrupt_file_is_quarantined() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, "{ not valid json").unwrap();

        let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
        assert!(cache.is_empty());

        // The corrupt file should be moved aside
        let backup = path.with_extension("json.corrupt");
        assert!(backup.exists());
    }

    #[test]
    fn test_empty_file_loads_as_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.json");
        std::fs::write(&path, "").unwrap();

        let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_missing_file_loads_as_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_atomic_write_does_not_leave_temp() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.json");
        let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
        let (id, r) = fake_result("t1", true);
        cache.insert(id, r).unwrap();

        // Temp file should not exist after a successful insert
        let parent = path.parent().unwrap();
        let temps: Vec<_> = std::fs::read_dir(parent)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(temps.is_empty(), "no temp files should remain");
    }

    #[test]
    fn test_prune_expired() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        let (id1, r1) = fake_result("p1", true);
        let (id2, r2) = fake_result("p2", true);
        // Insert with a 1ms TTL then a long TTL
        cache.insert_with_ttl(id1.clone(), r1, Duration::from_millis(1)).unwrap();
        cache.insert(id2, r2).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let removed = cache.prune_expired().unwrap();
        assert_eq!(removed, 1);
        assert_eq!(cache.len(), 1);
        // The long-lived entry should still be there
        assert!(cache.get(&TaskId::from_string("p1")).is_none());
    }

    #[test]
    fn test_path_returns_backing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.json");
        let cache = PersistentTaskCache::open(&path, Duration::from_secs(60)).unwrap();
        assert_eq!(cache.path(), path);
    }

    #[test]
    fn test_ephemeral_path_is_empty() {
        let cache = PersistentTaskCache::ephemeral(Duration::from_secs(60));
        assert_eq!(cache.path().as_os_str().to_string_lossy(), "");
    }

    #[test]
    fn test_cache_error_display() {
        let e = CacheError::Serde("bad".into());
        assert!(e.to_string().contains("bad"));
        let e2: CacheError = io::Error::new(io::ErrorKind::NotFound, "missing").into();
        assert!(e2.to_string().contains("missing"));
    }
}
