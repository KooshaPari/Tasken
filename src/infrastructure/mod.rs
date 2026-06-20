// SPDX-License-Identifier: MIT OR Apache-2.0
//! Infrastructure layer.

pub mod cache;
pub mod error;
pub mod persistent_cache;

/// OpenTelemetry-compatible span instrumentation. Compiled only when the
/// `otel` feature is enabled (see `Cargo.toml`). The trait + impl live in
/// their own feature-gated module so the zero-cost default build is
/// unaffected (no extra deps, no extra span allocations).
#[cfg(feature = "otel")]
pub mod otel;

pub use cache::TaskCache;
pub use error::TaskKitError;
pub use persistent_cache::PersistentTaskCache;
