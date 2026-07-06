//! Exponential backoff with full jitter for reconnection.
//!
//! On a dropped WebSocket the connector must not hot-loop reconnects (which
//! would hammer Binance and risk a ban) nor reconnect in lockstep with other
//! clients (the thundering-herd problem). Full jitter, sleeping a uniform random
//! duration in `[0, cap]` where `cap` grows exponentially, addresses both. The
//! jitter uses a small deterministic PRNG seeded from the wall clock so it needs
//! no external dependency.

use core::time::Duration;

use core_types::Nanos;

/// Exponential backoff state with full jitter.
#[derive(Debug, Clone)]
pub struct Backoff {
    base_ms: u64,
    max_ms: u64,
    attempt: u32,
    rng: u64,
}

impl Backoff {
    /// Creates a backoff with the given base and maximum delay.
    #[must_use]
    pub fn new(base_ms: u64, max_ms: u64) -> Backoff {
        // Seed the PRNG from the wall clock; any nonzero value works.
        let seed = Nanos::now().get() | 1;
        Backoff {
            base_ms,
            max_ms,
            attempt: 0,
            rng: seed,
        }
    }

    /// The connector default: 250ms base, 30s cap.
    #[must_use]
    pub fn connector_default() -> Backoff {
        Backoff::new(250, 30_000)
    }

    /// Resets to the first attempt after a successful connection.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// The exponential ceiling for the current attempt, `min(max, base * 2^n)`.
    fn cap_ms(&self) -> u64 {
        let shifted = self.base_ms.checked_shl(self.attempt).unwrap_or(u64::MAX);
        shifted.min(self.max_ms).max(self.base_ms.min(self.max_ms))
    }

    /// Returns the next delay (uniform in `[0, cap]`) and advances the attempt
    /// counter.
    pub fn next_delay(&mut self) -> Duration {
        let cap = self.cap_ms();
        let r = self.next_rand();
        let delay = if cap == 0 { 0 } else { r % (cap + 1) };
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_millis(delay)
    }

    /// xorshift64* step.
    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_never_exceeds_cap() {
        let mut b = Backoff::new(100, 5_000);
        for _ in 0..1000 {
            let cap_before = b.cap_ms();
            let d = b.next_delay().as_millis() as u64;
            assert!(d <= cap_before, "delay {d} exceeded cap {cap_before}");
        }
    }

    #[test]
    fn cap_grows_then_saturates() {
        let mut b = Backoff::new(100, 1_000);
        // attempt 0 -> 100, 1 -> 200, 2 -> 400, 3 -> 800, 4 -> 1000 (capped)
        let caps: Vec<u64> = (0..6)
            .map(|_| {
                let c = b.cap_ms();
                b.next_delay();
                c
            })
            .collect();
        assert_eq!(caps, vec![100, 200, 400, 800, 1000, 1000]);
    }

    #[test]
    fn reset_returns_to_base() {
        let mut b = Backoff::new(100, 10_000);
        for _ in 0..5 {
            b.next_delay();
        }
        b.reset();
        assert_eq!(b.cap_ms(), 100);
    }

    #[test]
    fn huge_attempt_does_not_overflow() {
        let mut b = Backoff::new(250, 30_000);
        for _ in 0..200 {
            let d = b.next_delay().as_millis() as u64;
            assert!(d <= 30_000);
        }
    }
}
