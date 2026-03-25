//! Adapters layer - infrastructure implementations.

pub mod primary;
pub mod secondary;
pub mod plugins;

// Re-exports
pub use primary::cli::CliAdapter;
pub use secondary::memory::MemoryStorage;
