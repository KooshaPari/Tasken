//! Contextual error messages.
//!
//! A bare `TaskError::NotFound("abc-123")` tells the user what
//! happened but not *where* or *why*. A SOTA error message includes
//! enough context for a human to act: which task, which workflow,
//! which step, which file path, which operation was in progress.
//!
//! [`ContextualError`] wraps any `std::error::Error + Send + Sync` and
//! adds a context string. The wrapper is itself an `Error`, and the
//! `Display` impl produces a multi-line message that reads top-down:
//!
//! ```text
//! while executing workflow 'nightly-build':
//!   while running step 'compile':
//!     Storage error: backend read failed
//! ```
//!
//! A list of `ContextualError`s can be combined into a [`ContextChain`]
//! to represent nested operations without losing any layer.

use std::error::Error as StdError;
use std::fmt;

/// Wrapper that pairs a source error with a context string.
pub struct ContextualError {
    context: String,
    source: Box<dyn StdError + Send + Sync + 'static>,
}

impl ContextualError {
    /// Wrap `source` with a context label.
    pub fn new<C, E>(context: C, source: E) -> Self
    where
        C: Into<String>,
        E: StdError + Send + Sync + 'static,
    {
        Self {
            context: context.into(),
            source: Box::new(source),
        }
    }

    /// Borrow the context label.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Borrow the source error.
    pub fn source(&self) -> &(dyn StdError + Send + Sync + 'static) {
        &*self.source
    }
}

impl fmt::Debug for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextualError")
            .field("context", &self.context)
            .field("source", &self.source)
            .finish()
    }
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.context)
    }
}

impl StdError for ContextualError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&*self.source)
    }
}

/// Trait for adding context to `Result`s, inspired by `anyhow::Context`.
///
/// ```ignore
/// use taskkit::infrastructure::context::ResultContext;
/// let result: std::io::Result<()> = ...;
/// let with_ctx = result.context("loading config")?;
/// ```
pub trait ResultContext<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    /// Wrap the error with `context`. The returned `Result`'s error
    /// type is `ContextualError`, so the original error is still
    /// accessible via `source()`.
    fn context<C: Into<String>>(self, context: C) -> Result<T, ContextualError>;
    /// Like `context`, but lazily evaluated.
    fn with_context<C, F>(self, f: F) -> Result<T, ContextualError>
    where
        C: Into<String>,
        F: FnOnce() -> C;
}

impl<T, E> ResultContext<T, E> for Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn context<C: Into<String>>(self, context: C) -> Result<T, ContextualError> {
        self.map_err(|e| ContextualError::new(context, e))
    }

    fn with_context<C, F>(self, f: F) -> Result<T, ContextualError>
    where
        C: Into<String>,
        F: FnOnce() -> C,
    {
        self.map_err(|e| ContextualError::new(f(), e))
    }
}

/// A chain of errors, suitable for representing a sequence of failed
/// operations (e.g. a workflow with multiple failing steps).
#[derive(Debug)]
pub struct ContextChain {
    /// Top-level operation label.
    label: String,
    /// Errors in order of occurrence.
    errors: Vec<ContextualError>,
}

impl ContextChain {
    /// Create an empty chain labelled with `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            errors: Vec::new(),
        }
    }

    /// Add an error with a context label.
    pub fn add_error<C, E>(&mut self, context: C, error: E) -> &mut Self
    where
        C: Into<String>,
        E: StdError + Send + Sync + 'static,
    {
        self.errors.push(ContextualError::new(context, error));
        self
    }

    /// True if no errors have been recorded.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Number of errors in the chain.
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Top-level label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Iterate over the recorded errors.
    pub fn iter(&self) -> impl Iterator<Item = &ContextualError> {
        self.errors.iter()
    }

    /// Convert into the recorded error vector.
    pub fn into_errors(self) -> Vec<ContextualError> {
        self.errors
    }

    /// Convert into a `Result<()>` that yields the first error as
    /// a `ContextualError` chain if any.
    pub fn into_result(self) -> Result<(), ContextualError> {
        if let Some(e) = self.errors.into_iter().next() {
            Err(e)
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for ContextChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} ({} error(s)):", self.label, self.errors.len())?;
        for (i, e) in self.errors.iter().enumerate() {
            writeln!(f, "  [{i}] {e}: {}", e.source())?;
        }
        Ok(())
    }
}

impl StdError for ContextChain {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.errors.first().map(|e| {
            let ptr: &(dyn StdError + 'static) = e;
            ptr
        })
    }
}

/// Render a contextual error and all of its sources as a multi-line
/// string, one line per layer of context. Useful for log output.
pub fn render_chain(err: &dyn StdError) -> String {
    let mut out = String::new();
    out.push_str(&err.to_string());
    let mut current = err.source();
    let mut depth = 1;
    while let Some(src) = current {
        out.push('\n');
        out.push_str(&"  ".repeat(depth));
        out.push_str("caused by: ");
        out.push_str(&src.to_string());
        current = src.source();
        depth += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::errors::TaskError;
    use std::io;

    #[test]
    fn test_contextual_error_new_and_display() {
        let inner = TaskError::NotFound("abc".into());
        let wrapped = ContextualError::new("loading task", inner);
        assert_eq!(wrapped.context(), "loading task");
        assert!(wrapped.source().downcast_ref::<TaskError>().is_some());
        assert_eq!(wrapped.to_string(), "loading task");
    }

    #[test]
    fn test_contextual_error_debug() {
        let wrapped = ContextualError::new("ctx", TaskError::Cancelled);
        let dbg = format!("{wrapped:?}");
        assert!(dbg.contains("ContextualError"));
        assert!(dbg.contains("ctx"));
    }

    #[test]
    fn test_result_context_trait() {
        let r: io::Result<()> = Err(io::Error::new(io::ErrorKind::Other, "boom"));
        let with_ctx = r.context("opening file");
        let err = with_ctx.err().unwrap();
        assert_eq!(err.context(), "opening file");
        // The source should still be a std::io::Error
        let src = err.source();
        assert!(src.downcast_ref::<io::Error>().is_some());
    }

    #[test]
    fn test_result_context_lazily_evaluated() {
        let r: io::Result<()> = Err(io::Error::new(io::ErrorKind::Other, "x"));
        let mut counter = 0;
        let _ = r.with_context(|| {
            counter += 1;
            "lazy-ctx"
        });
        assert_eq!(counter, 1);
    }

    #[test]
    fn test_result_context_lazily_not_evaluated_on_ok() {
        let r: io::Result<()> = Ok(());
        let mut counter = 0;
        let _ = r.with_context(|| {
            counter += 1;
            "lazy"
        });
        assert_eq!(counter, 0);
    }

    #[test]
    fn test_context_chain_collects() {
        let mut chain = ContextChain::new("workflow 'x'");
        chain.add_error("step a", TaskError::Cancelled);
        chain.add_error("step b", TaskError::Timeout(std::time::Duration::from_secs(1)));
        assert_eq!(chain.len(), 2);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_context_chain_display_includes_all() {
        let mut chain = ContextChain::new("workflow 'build'");
        chain.add_error("step compile", TaskError::ExecutionFailed("gcc".into()));
        let s = chain.to_string();
        assert!(s.contains("workflow 'build'"));
        assert!(s.contains("step compile"));
        assert!(s.contains("gcc"));
    }

    #[test]
    fn test_context_chain_into_result_empty_ok() {
        let chain = ContextChain::new("nothing");
        assert!(chain.into_result().is_ok());
    }

    #[test]
    fn test_context_chain_into_result_with_error() {
        let mut chain = ContextChain::new("wf");
        chain.add_error("c", TaskError::Cancelled);
        assert!(chain.into_result().is_err());
    }

    #[test]
    fn test_render_chain_multi_layer() {
        let inner = TaskError::ExecutionFailed("kaboom".into());
        let middle = ContextualError::new("step compile", inner);
        let outer = ContextualError::new("workflow nightly", middle);
        let rendered = render_chain(&outer);
        assert!(rendered.contains("workflow nightly"));
        assert!(rendered.contains("step compile"));
        assert!(rendered.contains("kaboom"));
        assert!(rendered.contains("caused by:"));
    }

    #[test]
    fn test_render_chain_single_layer() {
        let e = TaskError::Cancelled;
        let s = render_chain(&e);
        assert_eq!(s, "Task cancelled");
    }

    #[test]
    fn test_context_iter() {
        let mut chain = ContextChain::new("c");
        chain.add_error("a", TaskError::Cancelled);
        chain.add_error("b", TaskError::Cancelled);
        let count = chain.iter().count();
        assert_eq!(count, 2);
    }
}
