// SPDX-License-Identifier: MIT OR Apache-2.0
//! SPIKE — W3b — real impl is owned by W3b follow-up.
//!
//! Spike for the cron expression parser that will let Tasken schedule
//! DAG runs. The trait surface below is what downstream W3 sub-waves
//! plan against; the body is intentionally a stub that only handles
//! `* * * * *` (every minute) and rejects everything else with
//! [`CronError::NotImplemented`]. The real implementation will use the
//! `cron` crate (Rust) and is owned by the W3b follow-up.
//!
//! References:
//! - V5 plan: plans/2026-06-15-CONSOLIDATED-DAG-V5.md (W3 wave, plan ID W3b)
//! - ADR-009: docs/adr/2026-06-15/ADR-009-tasken-architecture-wasm-dag.md

use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

/// Errors produced by [`CronParser`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CronError {
    /// The expression is recognised syntactically but the SPIKE
    /// stub does not implement it yet. The W3b follow-up will
    /// replace these with proper `cron` crate errors.
    #[error("cron expression not yet implemented in this spike: {0}")]
    NotImplemented(String),

    /// The expression is malformed.
    #[error("invalid cron expression: {0}")]
    Invalid(String),
}

/// A parsed cron expression. In the spike, this is just the original
/// string plus a couple of predicates. The real impl will hold
/// normalised `cron` crate fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronExpr {
    /// The original expression string.
    pub expression: String,
}

impl CronExpr {
    /// `true` if this expression represents the canonical
    /// "every minute" schedule (`* * * * *`).
    pub fn is_every_minute(&self) -> bool {
        self.expression == "* * * * *"
    }
}

/// Stub cron parser.
///
/// Holds an optional parsed [`CronExpr`]. Construct one with
/// [`CronParser::default`], feed it an expression via
/// [`CronParser::parse`], and then ask it for the next fire time or
/// whether a given instant matches.
#[derive(Debug, Default, Clone)]
pub struct CronParser {
    inner: Option<CronExpr>,
}

impl CronParser {
    /// Parse a cron expression and store it in this parser.
    ///
    /// In the spike, only `"* * * * *"` is accepted; every other
    /// expression (including well-known shortcuts like `@hourly`)
    /// returns [`CronError::NotImplemented`].
    pub fn parse(&mut self, expr: &str) -> Result<CronExpr, CronError> {
        if expr == "* * * * *" {
            let cron_expr = CronExpr {
                expression: expr.to_string(),
            };
            self.inner = Some(cron_expr.clone());
            Ok(cron_expr)
        } else {
            Err(CronError::NotImplemented(expr.to_string()))
        }
    }

    /// Compute the next fire time strictly after `base`.
    ///
    /// In the spike this is simply `base + 1 minute` when a valid
    /// expression has been parsed, and `None` otherwise.
    pub fn next_after(&self, base: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.inner.as_ref().map(|_| base + Duration::minutes(1))
    }

    /// `true` if `instant` matches the stored expression.
    ///
    /// In the spike, every parsed expression matches every instant.
    /// The real impl will compare the expression's normalised
    /// minute / hour / dom / month / dow fields against `instant`.
    pub fn matches(&self, _instant: DateTime<Utc>) -> bool {
        self.inner.is_some()
    }
}
