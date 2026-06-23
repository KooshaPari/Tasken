// SPDX-License-Identifier: MIT OR Apache-2.0
//! Application configuration.
//!
//! Configuration is loaded from environment variables (with optional `.env` file
//! support via `dotenvy`). All keys have sensible defaults so the application
//! runs out of the box with zero configuration.
//!
//! # Environment variables
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `TASKEN_DATA_DIR` | (platform data dir) `/taskkit` | Base directory for task data |
//! | `TASKEN_STORE_FILE` | `store.json` | Storage file name |
//! | `TASKEN_CACHE_DIR` | (inherits `data_dir`) | Cache directory (overrides `${data_dir}`) |
//! | `TASKEN_CACHE_FILE` | `cache.json` | Cache file name |
//! | `TASKEN_CACHE_TTL_SECONDS` | `300` | Default cache TTL in seconds |
//! | `TASKEN_DEFAULT_LIST_LIMIT` | `100` | Default limit for listing tasks |

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Configuration struct
// ---------------------------------------------------------------------------

/// Consolidated application configuration.
///
/// All fields have sensible defaults. Create one with [`TaskenConfig::load`]
/// which reads environment variables (and a `.env` file if present).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TaskenConfig {
    /// Base directory for persistent task data.
    ///
    /// Defaults to the platform data directory (e.g.
    /// `~/Library/Application Support` on macOS, `~/.local/share` on Linux)
    /// joined with `taskkit`.
    pub data_dir: PathBuf,

    /// File name (relative to `data_dir`) for the task/workflow/schedule store.
    pub store_file: String,

    /// Optional override for the cache directory.
    ///
    /// When `None`, the cache lives inside `data_dir`.
    pub cache_dir: Option<PathBuf>,

    /// File name (relative to `cache_dir` or `data_dir`) for the result cache.
    pub cache_file: String,

    /// Default TTL (in seconds) for cached task results.
    pub cache_ttl_seconds: u64,

    /// Default maximum number of tasks returned by list queries.
    pub default_list_limit: usize,
}

impl Default for TaskenConfig {
    fn default() -> Self {
        let default_data_dir =
            dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("taskkit");

        Self {
            data_dir: default_data_dir,
            store_file: "store.json".to_string(),
            cache_dir: None,
            cache_file: "cache.json".to_string(),
            cache_ttl_seconds: 300,
            default_list_limit: 100,
        }
    }
}

impl TaskenConfig {
    /// Load configuration from environment variables.
    ///
    /// If a `.env` file exists in the current directory or its parents, it is
    /// loaded first (idempotent — safe to call multiple times).
    pub fn load() -> Self {
        // Attempt to load .env file; ignore errors (no .env is not a failure).
        let _ = dotenvy::dotenv();

        envy::prefixed("TASKEN_").from_env::<Self>().unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // Derived accessors
    // ------------------------------------------------------------------

    /// Absolute path to the store file.
    pub fn store_path(&self) -> PathBuf {
        self.data_dir.join(&self.store_file)
    }

    /// Absolute path to the cache directory.
    pub fn resolved_cache_dir(&self) -> PathBuf {
        self.cache_dir.clone().unwrap_or_else(|| self.data_dir.clone())
    }

    /// Absolute path to the cache file.
    pub fn cache_path(&self) -> PathBuf {
        self.resolved_cache_dir().join(&self.cache_file)
    }

    /// Default TTL as a [`Duration`].
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache_ttl_seconds)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let cfg = TaskenConfig::default();
        assert!(!cfg.store_file.is_empty());
        assert!(!cfg.cache_file.is_empty());
        assert_eq!(cfg.cache_ttl_seconds, 300);
        assert_eq!(cfg.default_list_limit, 100);
        assert!(cfg.store_path().ends_with("store.json"));
    }

    #[test]
    fn test_cache_path_without_override() {
        let cfg = TaskenConfig::default();
        // When cache_dir is None, cache_path should be data_dir / cache_file
        assert!(cfg.cache_dir.is_none());
        assert_eq!(cfg.resolved_cache_dir(), cfg.data_dir);
        assert_eq!(cfg.cache_path(), cfg.data_dir.join("cache.json"));
    }

    #[test]
    fn test_cache_path_with_override() {
        let custom = PathBuf::from("/tmp/tasken-cache");
        let cfg = TaskenConfig { cache_dir: Some(custom.clone()), ..Default::default() };
        assert_eq!(cfg.resolved_cache_dir(), custom);
        assert_eq!(cfg.cache_path(), custom.join("cache.json"));
    }

    #[test]
    fn test_store_path() {
        let cfg = TaskenConfig::default();
        assert_eq!(cfg.store_path(), cfg.data_dir.join("store.json"));
    }

    #[test]
    fn test_cache_ttl_duration() {
        let mut cfg = TaskenConfig::default();
        cfg.cache_ttl_seconds = 60;
        assert_eq!(cfg.cache_ttl(), Duration::from_secs(60));
    }

    #[test]
    fn test_deserialize_from_env() {
        // Simulate environment variable overrides
        temp_env::with_vars(
            vec![
                ("TASKEN_STORE_FILE", Some("custom_store.json")),
                ("TASKEN_CACHE_TTL_SECONDS", Some("600")),
                ("TASKEN_DEFAULT_LIST_LIMIT", Some("50")),
            ],
            || {
                let cfg = TaskenConfig::load();
                assert_eq!(cfg.store_file, "custom_store.json");
                assert_eq!(cfg.cache_ttl_seconds, 600);
                assert_eq!(cfg.default_list_limit, 50);
            },
        );
    }

    #[test]
    fn test_load_empty_env_falls_back_to_defaults() {
        // When no env vars are set, load() should produce a config that is
        // structurally equivalent to the default (data_dir may differ because
        // it reads the real platform dir).
        // Use temp_env to isolate from other tests that may set env vars.
        temp_env::with_vars(
            vec![
                ("TASKEN_STORE_FILE", None::<&str>),
                ("TASKEN_CACHE_FILE", None::<&str>),
                ("TASKEN_CACHE_TTL_SECONDS", None::<&str>),
                ("TASKEN_DEFAULT_LIST_LIMIT", None::<&str>),
                ("TASKEN_DATA_DIR", None::<&str>),
                ("TASKEN_CACHE_DIR", None::<&str>),
            ],
            || {
                let cfg = TaskenConfig::load();
                assert_eq!(cfg.store_file, "store.json");
                assert_eq!(cfg.cache_file, "cache.json");
                assert_eq!(cfg.cache_ttl_seconds, 300);
                assert_eq!(cfg.default_list_limit, 100);
            },
        );
    }
}
