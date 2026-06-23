// SPDX-License-Identifier: MIT OR Apache-2.0
//! Token-bucket rate limiter for dispatch / task execution.
//!
//! Provides a classic token-bucket algorithm:
//!
//! - A fixed **capacity** of tokens (maximum burst size).
//! - A **refill rate** in tokens per second.
//! - A **refill interval** that controls how often tokens are added.
//! - `try_consume(n)` for non-blocking admission checks.
//! - `consume(n)` for async admission that waits until tokens are available.
//!
//! # Example
//!
//! ```ignore
//! use crate::domain::rate_limiter::TokenBucket;
//!
//! let limiter = TokenBucket::new(10, 5.0, None);  // cap 10, refill 5/s
//! assert!(limiter.try_consume(1).await);            // one token available
//! ```

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// A token-bucket rate limiter.
///
/// Thread-safe: interior mutability via `tokio::sync::Mutex`.
///
/// By default the bucket starts **full** (i.e. `capacity` tokens are
/// available immediately).  Use [`with_starting_tokens`](Self::with_starting_tokens)
/// to override this.
#[derive(Debug)]
pub struct TokenBucket {
    inner: Arc<Mutex<TokenBucketInner>>,
    capacity: u64,
    refill_rate: f64,          // tokens per second
    refill_interval: Duration, // how often we add tokens
}

#[derive(Debug)]
struct TokenBucketInner {
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    /// Create a new token bucket.
    ///
    /// * `capacity` – maximum number of tokens (burst size).
    /// * `refill_rate` – tokens added per second (can be fractional).
    /// * `refill_interval` – how often tokens are added.  When `None`,
    ///   defaults to 100 ms (i.e. 10 refills / second).
    ///
    /// The bucket starts **full**.
    pub fn new(capacity: u64, refill_rate: f64, refill_interval: Option<Duration>) -> Self {
        let interval = refill_interval.unwrap_or(Duration::from_millis(100));
        Self {
            inner: Arc::new(Mutex::new(TokenBucketInner {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            })),
            capacity,
            refill_rate,
            refill_interval: interval,
        }
    }

    /// Create a new token bucket that starts with `n` tokens instead of
    /// being full.  Useful for cold-start scenarios.
    pub fn with_starting_tokens(
        capacity: u64,
        refill_rate: f64,
        refill_interval: Option<Duration>,
        starting_tokens: u64,
    ) -> Self {
        let cap = starting_tokens.min(capacity);
        let interval = refill_interval.unwrap_or(Duration::from_millis(100));
        Self {
            inner: Arc::new(Mutex::new(TokenBucketInner {
                tokens: cap as f64,
                last_refill: Instant::now(),
            })),
            capacity,
            refill_rate,
            refill_interval: interval,
        }
    }

    /// Return the bucket capacity (max tokens).
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Refill rate in tokens / second.
    pub fn refill_rate(&self) -> f64 {
        self.refill_rate
    }

    // -----------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------

    /// Refill tokens based on elapsed time since last refill.
    fn refill(inner: &mut TokenBucketInner, capacity: u64, rate: f64, interval: Duration) {
        let now = Instant::now();
        let elapsed = now.duration_since(inner.last_refill);

        // Only refill if at least one interval has passed.
        if elapsed < interval {
            return;
        }

        // Compute how many whole intervals have elapsed and add
        // the corresponding number of tokens.
        let intervals = elapsed.as_secs_f64() / interval.as_secs_f64();
        let added = rate * interval.as_secs_f64() * intervals;
        inner.tokens = (inner.tokens + added).min(capacity as f64);
        inner.last_refill = now;
    }

    // -----------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------

    /// Non-blocking attempt to consume `n` tokens.
    ///
    /// Returns `true` if the tokens were consumed, `false` otherwise.
    /// Token refill happens opportunistically on each call.
    pub async fn try_consume(&self, n: u32) -> bool {
        let nf = n as f64;
        let mut inner = self.inner.lock().await;
        Self::refill(&mut inner, self.capacity, self.refill_rate, self.refill_interval);
        if inner.tokens >= nf {
            inner.tokens -= nf;
            true
        } else {
            false
        }
    }

    /// Async wait until `n` tokens are available, then consume them.
    ///
    /// This will **spin-wait** with short sleeps, yielding to the tokio
    /// runtime between attempts.  For production workloads with very high
    /// QPS, consider a dedicated wake-up channel instead.
    pub async fn consume(&self, n: u32) {
        loop {
            // Fast path – try once.
            {
                let mut inner = self.inner.lock().await;
                Self::refill(&mut inner, self.capacity, self.refill_rate, self.refill_interval);
                let nf = n as f64;
                if inner.tokens >= nf {
                    inner.tokens -= nf;
                    return;
                }
            }
            // Wait for at least one refill interval before retrying.
            tokio::time::sleep(self.refill_interval).await;
        }
    }

    /// Return an estimate of currently available tokens (may be slightly
    /// stale by the time the caller acts on it).
    pub async fn available(&self) -> f64 {
        let mut inner = self.inner.lock().await;
        Self::refill(&mut inner, self.capacity, self.refill_rate, self.refill_interval);
        inner.tokens
    }

    /// Clone the handle (shares the same underlying state).
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            capacity: self.capacity,
            refill_rate: self.refill_rate,
            refill_interval: self.refill_interval,
        }
    }
}

// Manual Clone impl – we only clone the Arc handle, keeping shared state.
impl Clone for TokenBucket {
    fn clone(&self) -> Self {
        self.clone_handle()
    }
}

// ---------------------------------------------------------------------------
// Rate-limit string parser
// ---------------------------------------------------------------------------

/// Parse a rate-limit string like `"10/s"`, `"60/m"`, `"100/h"`, or `"5"` into
/// a tokens-per-second float.
///
/// Returns `None` on invalid input.
///
/// # Examples
///
/// ```
/// # use taskkit::domain::rate_limiter::parse_rate_limit;
/// assert_eq!(parse_rate_limit("10/s"), Some(10.0));
/// assert_eq!(parse_rate_limit("60/m"), Some(1.0));
/// assert_eq!(parse_rate_limit("3600/h"), Some(1.0));
/// assert_eq!(parse_rate_limit("5"), Some(5.0));
/// assert_eq!(parse_rate_limit(""), None);
/// assert_eq!(parse_rate_limit("abc"), None);
/// ```
pub fn parse_rate_limit(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Split on '/'
    if let Some(slash_pos) = s.find('/') {
        let num_part = &s[..slash_pos];
        let unit_part = &s[slash_pos + 1..];

        let rate: f64 = num_part.parse().ok()?;
        if rate < 0.0 {
            return None;
        }

        let multiplier = match unit_part {
            "s" | "sec" | "second" => 1.0,
            "m" | "min" | "minute" => 1.0 / 60.0,
            "h" | "hr" | "hour" => 1.0 / 3600.0,
            _ => return None,
        };

        Some(rate * multiplier)
    } else {
        // No unit — assume per-second
        let rate: f64 = s.parse().ok()?;
        if rate < 0.0 {
            return None;
        }
        Some(rate)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    // ------------------------------------------------------------------
    // TokenBucket unit tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_basic_consume() {
        let bucket = TokenBucket::new(10, 10.0, None);
        assert!(bucket.try_consume(1).await);
        assert_eq!(bucket.available().await, 9.0);
    }

    #[tokio::test]
    async fn test_consume_exact_capacity() {
        let bucket = TokenBucket::new(5, 5.0, None);
        assert!(bucket.try_consume(5).await);
        assert_eq!(bucket.available().await, 0.0);
        // Should fail now
        assert!(!bucket.try_consume(1).await);
    }

    #[tokio::test]
    async fn test_consume_over_capacity_fails() {
        let bucket = TokenBucket::new(3, 5.0, None);
        assert!(!bucket.try_consume(4).await);
        // Tokens should be unchanged
        assert_eq!(bucket.available().await, 3.0);
    }

    #[tokio::test]
    async fn test_refill_after_time() {
        let bucket = TokenBucket::new(10, 100.0, Some(Duration::from_millis(50)));

        // Drain all tokens
        assert!(bucket.try_consume(10).await);
        assert_eq!(bucket.available().await, 0.0);

        // Wait for refill (~100ms should give us ~10 tokens at 100/s)
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should have refilled some tokens
        let avail = bucket.available().await;
        assert!(avail > 0.0, "expected tokens after refill, got {avail}");
        assert!(avail <= 10.0);
    }

    #[tokio::test]
    async fn test_burst_within_capacity() {
        let bucket = TokenBucket::new(100, 10.0, None);

        // Instant burst within capacity
        assert!(bucket.try_consume(50).await);
        assert!(bucket.try_consume(50).await);
        assert!(!bucket.try_consume(1).await);
    }

    #[tokio::test]
    async fn test_bucket_cannot_exceed_capacity() {
        let bucket = TokenBucket::new(5, 1000.0, Some(Duration::from_millis(10)));

        // Drain
        assert!(bucket.try_consume(5).await);
        assert_eq!(bucket.available().await, 0.0);

        // Wait well past a full refill period
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should be at capacity, not above
        let avail = bucket.available().await;
        assert!(avail <= 5.0, "expected at most 5, got {avail}");
    }

    #[tokio::test]
    async fn test_consume_async_waits_for_tokens() {
        let bucket = Arc::new(TokenBucket::new(2, 10.0, Some(Duration::from_millis(50))));

        // Drain tokens in the main task
        assert!(bucket.try_consume(2).await);
        assert_eq!(bucket.available().await, 0.0);

        // Spawn a consumer that will need to wait
        let bucket_clone = Arc::clone(&bucket);
        let handle = tokio::spawn(async move {
            bucket_clone.consume(1).await;
        });

        // Give it a little time — consume will block until refill happens
        tokio::time::sleep(Duration::from_millis(200)).await;

        // The consumer should have completed by now (refill at 10/s,
        // so ~100ms for 1 token)
        assert!(handle.await.is_ok(), "consume should have completed after refill");
    }

    #[tokio::test]
    async fn test_starting_tokens() {
        let bucket = TokenBucket::with_starting_tokens(10, 5.0, None, 0);
        assert_eq!(bucket.available().await, 0.0);
        assert!(!bucket.try_consume(1).await);
    }

    #[test]
    fn test_parse_rate_limit_per_second() {
        assert_eq!(parse_rate_limit("10/s"), Some(10.0));
        assert_eq!(parse_rate_limit("0.5/s"), Some(0.5));
        assert_eq!(parse_rate_limit("100/sec"), Some(100.0));
        assert_eq!(parse_rate_limit("50/second"), Some(50.0));
    }

    #[test]
    fn test_parse_rate_limit_per_minute() {
        assert_eq!(parse_rate_limit("60/m"), Some(1.0));
        assert_eq!(parse_rate_limit("120/min"), Some(2.0));
        assert_eq!(parse_rate_limit("30/minute"), Some(0.5));
    }

    #[test]
    fn test_parse_rate_limit_per_hour() {
        assert_eq!(parse_rate_limit("3600/h"), Some(1.0));
        assert_eq!(parse_rate_limit("7200/hr"), Some(2.0));
        assert_eq!(parse_rate_limit("3600/hour"), Some(1.0));
    }

    #[test]
    fn test_parse_rate_limit_no_unit() {
        assert_eq!(parse_rate_limit("5"), Some(5.0));
        assert_eq!(parse_rate_limit("0"), Some(0.0));
    }

    #[test]
    fn test_parse_rate_limit_invalid() {
        assert_eq!(parse_rate_limit(""), None);
        assert_eq!(parse_rate_limit("abc"), None);
        assert_eq!(parse_rate_limit("10/xyz"), None);
        assert_eq!(parse_rate_limit("-5/s"), None);
    }

    #[test]
    fn test_token_bucket_capacity_and_rate() {
        let bucket = TokenBucket::new(42, 3.5, None);
        assert_eq!(bucket.capacity(), 42);
        assert_eq!(bucket.refill_rate(), 3.5);
    }

    #[tokio::test]
    async fn test_clone_shares_state() {
        let bucket = TokenBucket::new(5, 10.0, None);
        let cloned = bucket.clone();

        // Consume from original
        assert!(bucket.try_consume(3).await);

        // Cloned should see reduced tokens
        assert_eq!(cloned.available().await, 2.0);
    }
}
