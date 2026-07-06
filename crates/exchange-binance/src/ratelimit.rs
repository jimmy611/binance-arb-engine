//! Token-bucket rate limiting for REST weight.
//!
//! Binance rejects requests that exceed a declared weight-per-interval budget,
//! and a rejection is expensive (it can trigger an IP ban). The engine therefore
//! stays under a self-imposed fraction (default 80%) of the declared limit. The
//! bucket is pure and clock-injected so it is deterministically testable; the
//! REST client supplies monotonic seconds from a process-start `Instant`.

/// A classic token bucket. Tokens refill continuously at `refill_per_sec` up to
/// `capacity`; taking `n` tokens succeeds only if at least `n` are available.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last_ts: f64,
}

impl TokenBucket {
    /// Creates a full bucket with the given capacity and refill rate.
    #[must_use]
    pub fn new(capacity: f64, refill_per_sec: f64) -> TokenBucket {
        TokenBucket {
            capacity,
            tokens: capacity,
            refill_per_sec,
            last_ts: 0.0,
        }
    }

    /// Currently available tokens after refilling to `now_secs` (does not take).
    pub fn available(&mut self, now_secs: f64) -> f64 {
        self.refill(now_secs);
        self.tokens
    }

    /// Attempts to take `amount` tokens as of `now_secs`. Returns `true` and
    /// deducts on success; returns `false` and leaves the bucket unchanged on
    /// insufficient tokens.
    pub fn try_take(&mut self, now_secs: f64, amount: f64) -> bool {
        self.refill(now_secs);
        if self.tokens + f64::EPSILON >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    fn refill(&mut self, now_secs: f64) {
        let elapsed = (now_secs - self.last_ts).max(0.0);
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        self.last_ts = now_secs;
    }
}

/// REST weight limiter, sized to a fraction of Binance's declared
/// weight-per-minute limit.
#[derive(Debug, Clone)]
pub struct RestRateLimiter {
    weight: TokenBucket,
}

impl RestRateLimiter {
    /// Builds a limiter for `weight_per_minute` at the given `utilisation`
    /// fraction (for example `0.8` for the 80% ceiling). The bucket capacity is
    /// the full per-minute budget scaled by utilisation, refilling at the same
    /// average rate, so short bursts are allowed but the sustained rate stays
    /// under the fraction.
    #[must_use]
    pub fn new(weight_per_minute: u64, utilisation: f64) -> RestRateLimiter {
        let budget = (weight_per_minute as f64) * utilisation;
        RestRateLimiter {
            weight: TokenBucket::new(budget, budget / 60.0),
        }
    }

    /// Binance's common default: 6000 weight per minute at 80% utilisation.
    #[must_use]
    pub fn binance_default() -> RestRateLimiter {
        RestRateLimiter::new(6000, 0.8)
    }

    /// Tries to reserve `weight` units as of `now_secs`.
    pub fn try_consume(&mut self, now_secs: f64, weight: u32) -> bool {
        self.weight.try_take(now_secs, f64::from(weight))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_until_empty_then_refills() {
        let mut b = TokenBucket::new(10.0, 1.0);
        // Ten unit takes at t=0 succeed, the eleventh fails.
        for _ in 0..10 {
            assert!(b.try_take(0.0, 1.0));
        }
        assert!(!b.try_take(0.0, 1.0));
        // After 5 seconds, 5 tokens have refilled.
        assert!(b.try_take(5.0, 5.0));
        assert!(!b.try_take(5.0, 0.5));
    }

    #[test]
    fn refill_caps_at_capacity() {
        let mut b = TokenBucket::new(10.0, 100.0);
        assert!(b.try_take(0.0, 10.0));
        // Even after a long idle, tokens cap at capacity, not more.
        assert_eq!(b.available(1000.0), 10.0);
    }

    #[test]
    fn rest_limiter_stays_under_budget() {
        // 6000/min at 80% => 4800 capacity, 80/sec refill.
        let mut lim = RestRateLimiter::binance_default();
        // A weight-20 exchangeInfo call succeeds from full.
        assert!(lim.try_consume(0.0, 20));
        // Draining the whole 4800 budget at t=0 then failing.
        let mut drained = 20;
        while lim.try_consume(0.0, 40) {
            drained += 40;
        }
        assert!(drained <= 4800 + 40, "should not exceed the 80% budget");
    }
}
