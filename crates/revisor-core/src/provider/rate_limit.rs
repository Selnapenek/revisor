use super::types::RateLimitInfo;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Thread-safe rate limit tracker updated from HTTP response headers
pub struct RateLimitTracker {
    remaining: AtomicU32,
    limit: AtomicU32,
    reset_at: AtomicU64,
}

impl RateLimitTracker {
    pub fn new() -> Self {
        Self {
            remaining: AtomicU32::new(5000),
            limit: AtomicU32::new(5000),
            reset_at: AtomicU64::new(0),
        }
    }

    /// Update from response headers
    pub fn update_from_headers(&self, headers: &reqwest::header::HeaderMap) {
        if let Some(remaining) = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            self.remaining.store(remaining, Ordering::Relaxed);
        }
        if let Some(limit) = headers
            .get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
        {
            self.limit.store(limit, Ordering::Relaxed);
        }
        if let Some(reset) = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
        {
            self.reset_at.store(reset, Ordering::Relaxed);
        }
    }

    pub fn info(&self) -> RateLimitInfo {
        RateLimitInfo {
            remaining: self.remaining.load(Ordering::Relaxed),
            limit: self.limit.load(Ordering::Relaxed),
            reset_at: self.reset_at.load(Ordering::Relaxed),
        }
    }

    /// Returns true if we should back off (< 10% remaining)
    pub fn should_throttle(&self) -> bool {
        let remaining = self.remaining.load(Ordering::Relaxed);
        let limit = self.limit.load(Ordering::Relaxed);
        if limit == 0 {
            return false;
        }
        remaining < limit / 10
    }
}
