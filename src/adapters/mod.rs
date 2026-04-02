//! Adapters layer - infrastructure implementations.

pub mod plugins;
pub mod primary;
pub mod secondary;

// Re-exports
pub use primary::cli::CliAdapter;
pub use secondary::memory::MemoryStorage;
