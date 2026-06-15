//! In-memory cache for task results.

use crate::domain::tasks::{TaskId, TaskResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Cache entry with expiration.
struct CacheEntry {
    result: TaskResult,
    inserted_at: Instant,
    ttl: Duration,
}

/// In-memory result cache with TTL support.
#[derive(Clone)]
pub struct TaskCache {
    inner: Arc<Mutex<HashMap<TaskId, CacheEntry>>>,
    default_ttl: Duration,
}

impl TaskCache {
    /// Create a new cache with default TTL.
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            default_ttl,
        }
    }

    /// Get a cached result if present and not expired.
    pub fn get(&self, task_id: &TaskId) -> Option<TaskResult> {
        let mut map = self.inner.lock().unwrap();
        let entry = map.get(task_id)?;
        if entry.inserted_at.elapsed() > entry.ttl {
            map.remove(task_id);
            return None;
        }
        Some(entry.result.clone())
    }

    /// Insert a result into the cache.
    pub fn insert(&self, task_id: TaskId, result: TaskResult) {
        let mut map = self.inner.lock().unwrap();
        map.insert(
            task_id,
            CacheEntry {
                result,
                inserted_at: Instant::now(),
                ttl: self.default_ttl,
            },
        );
    }

    /// Insert with a custom TTL.
    pub fn insert_with_ttl(&self, task_id: TaskId, result: TaskResult, ttl: Duration) {
        let mut map = self.inner.lock().unwrap();
        map.insert(
            task_id,
            CacheEntry {
                result,
                inserted_at: Instant::now(),
                ttl,
            },
        );
    }

    /// Invalidate a cached entry.
    pub fn invalidate(&self, task_id: &TaskId) {
        let mut map = self.inner.lock().unwrap();
        map.remove(task_id);
    }

    /// Clear all entries.
    pub fn clear(&self) {
        let mut map = self.inner.lock().unwrap();
        map.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for TaskCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(300))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_cache_insert_and_get() {
        let cache = TaskCache::new(Duration::from_secs(60));
        let task_id = TaskId::from_string("t1");
        let result = TaskResult {
            task_id: task_id.clone(),
            success: true,
            output: Some(serde_json::json!({"status": "ok"})),
            error: None,
            duration: Duration::from_secs(1),
            timestamp: chrono::Utc::now(),
        };
        cache.insert(task_id.clone(), result.clone());
        assert_eq!(cache.len(), 1);
        let got = cache.get(&task_id).unwrap();
        assert!(got.success);
    }

    #[test]
    fn test_cache_ttl_expiration() {
        let cache = TaskCache::new(Duration::from_millis(1));
        let task_id = TaskId::from_string("t1");
        let result = TaskResult {
            task_id: task_id.clone(),
            success: true,
            output: None,
            error: None,
            duration: Duration::from_secs(1),
            timestamp: chrono::Utc::now(),
        };
        cache.insert(task_id.clone(), result);
        std::thread::sleep(Duration::from_millis(20));
        assert!(cache.get(&task_id).is_none());
    }

    #[test]
    fn test_cache_invalidate() {
        let cache = TaskCache::new(Duration::from_secs(60));
        let task_id = TaskId::from_string("t1");
        let result = TaskResult {
            task_id: task_id.clone(),
            success: true,
            output: None,
            error: None,
            duration: Duration::from_secs(1),
            timestamp: chrono::Utc::now(),
        };
        cache.insert(task_id.clone(), result);
        cache.invalidate(&task_id);
        assert!(cache.is_empty());
    }
}
