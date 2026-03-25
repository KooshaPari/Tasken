//! Infrastructure error handling.

use std::fmt;

/// TaskKit-specific errors.
#[derive(Debug)]
pub enum TaskKitError {
    /// Configuration error.
    Config(String),
    /// Initialization error.
    Init(String),
    /// Runtime error.
    Runtime(String),
    /// Shutdown error.
    Shutdown(String),
}

impl fmt::Display for TaskKitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskKitError::Config(msg) => write!(f, "Configuration error: {}", msg),
            TaskKitError::Init(msg) => write!(f, "Initialization error: {}", msg),
            TaskKitError::Runtime(msg) => write!(f, "Runtime error: {}", msg),
            TaskKitError::Shutdown(msg) => write!(f, "Shutdown error: {}", msg),
        }
    }
}

impl std::error::Error for TaskKitError {}
