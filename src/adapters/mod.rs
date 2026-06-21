// SPDX-License-Identifier: MIT OR Apache-2.0
//! Adapters layer - infrastructure implementations.

pub mod plugins;
pub mod primary;
pub mod secondary;

// Re-exports
pub use primary::cli::CliAdapter;
pub use secondary::memory::MemoryStorage;
