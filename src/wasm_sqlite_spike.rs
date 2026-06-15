//! WASM SQLite driver spike (W3a).
//!
//! This module is a SPIKE for the WASM-target SQLite driver described in
//! the V5 plan (W3 wave, plan ID W3a). The real implementation will be
//! built on top of `wasm-sqlite` or `rusqlite-wasm-bindgen` and is owned
//! by the W3a follow-up. This file only captures the trait surface so
//! downstream W3 sub-waves (b, c, d, e) can plan against it.
//!
//! **Do not use this in production.** All method bodies on the
//! reference stub return [`WasmSqliteError::NotCompiledIn`].
//!
//! Refs:
//! - V5 plan: plans/2026-06-15-CONSOLIDATED-DAG-V5.md (W3 wave, plan ID W3a)
//! - ADR-009: docs/adr/2026-06-15/ADR-009-tasken-architecture-wasm-dag.md

use std::fmt;

/// One row of query results.
///
/// In the real implementation this will wrap the column values
/// produced by the underlying WASM SQLite binding. For the spike it
/// is a string-only placeholder so the trait signature is stable
/// without committing to a column type system.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    /// Column values in declaration order.
    pub values: Vec<String>,
}

/// Errors that can be produced by the WASM SQLite driver spike.
#[derive(Debug)]
pub enum WasmSqliteError {
    /// The WASM SQLite backend has not been compiled in.
    /// This is the default result of every call in the spike.
    NotCompiledIn,
    /// I/O error opening or reading a file.
    IoError(String),
    /// SQL parse / execution error returned by SQLite.
    SqlError(String),
}

impl fmt::Display for WasmSqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WasmSqliteError::NotCompiledIn => {
                write!(f, "WASM SQLite backend not compiled in (W3a spike only)")
            }
            WasmSqliteError::IoError(s) => write!(f, "WASM SQLite I/O error: {s}"),
            WasmSqliteError::SqlError(s) => write!(f, "WASM SQLite SQL error: {s}"),
        }
    }
}

impl std::error::Error for WasmSqliteError {}

/// The spike-shape trait for a WASM-target SQLite driver.
///
/// Implementors will wrap a real WASM SQLite binding
/// (`wasm-sqlite`, `rusqlite-wasm-bindgen`, or similar). The trait
/// is intentionally minimal so the surface is easy to mock and to
/// port to/from the native `PersistentTaskCache` shape.
pub trait WasmSqliteDriver: Sized {
    /// Open a database at the given virtual path.
    ///
    /// Note: in WASM the `path` argument is effectively meaningless —
    /// the browser sandbox has no real filesystem, so the underlying
    /// binding will route the path through whatever virtual FS the
    /// embedder (Vite, webpack, etc.) exposes. The argument is kept
    /// in the signature so the trait matches the native
    /// `PersistentTaskCache::open` shape and downstream code can stay
    /// portable.
    fn open(path: &str) -> Result<Self, WasmSqliteError>;

    /// Execute a SQL statement that produces no rows
    /// (DDL, INSERT, UPDATE, DELETE). Returns the number of rows
    /// affected.
    fn execute(&self, sql: &str) -> Result<usize, WasmSqliteError>;

    /// Execute a SQL query and return the resulting rows.
    fn query(&self, sql: &str) -> Result<Vec<Row>, WasmSqliteError>;
}

#[cfg(test)]
mod tests {
    //! Compile-time shape tests for the spike trait. These are NOT
    //! run today because the module is intentionally not registered
    //! in `lib.rs` (per the W3a no-modify constraint). They will
    //! activate the moment the W3a follow-up adds
    //! `pub mod wasm_sqlite_spike;` to `lib.rs`.

    use super::*;

    /// A trivial stub that satisfies the trait without doing any real
    /// work. The point is to lock in the trait shape so the spike
    /// cannot drift silently.
    struct StubDriver;

    impl WasmSqliteDriver for StubDriver {
        fn open(_path: &str) -> Result<Self, WasmSqliteError> {
            Err(WasmSqliteError::NotCompiledIn)
        }
        fn execute(&self, _sql: &str) -> Result<usize, WasmSqliteError> {
            Err(WasmSqliteError::NotCompiledIn)
        }
        fn query(&self, _sql: &str) -> Result<Vec<Row>, WasmSqliteError> {
            Err(WasmSqliteError::NotCompiledIn)
        }
    }

    #[test]
    fn spike_open_returns_not_compiled_in() {
        let r = StubDriver::open(":memory:");
        assert!(matches!(r, Err(WasmSqliteError::NotCompiledIn)));
    }

    #[test]
    fn spike_query_returns_not_compiled_in() {
        let d = StubDriver;
        let r = d.query("SELECT 1");
        assert!(matches!(r, Err(WasmSqliteError::NotCompiledIn)));
    }
}
