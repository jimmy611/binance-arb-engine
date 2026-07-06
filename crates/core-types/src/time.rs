//! Time types.
//!
//! Two clocks matter and they are kept separate on purpose:
//!
//! * [`Nanos`] is a wall-clock instant (nanoseconds since the Unix epoch, UTC),
//!   used to stamp events for the WAL, the UI, and cross-host correlation. Wall
//!   clocks can jump (NTP steps), so this is never used to measure durations.
//! * Latency is measured with a monotonic clock (`std::time::Instant`) at the
//!   call site and reported as a duration. Monotonic instants are not stored in
//!   these value types because they are process-local and meaningless off-host.

use core::fmt;
use core::time::Duration;

use serde::{Deserialize, Serialize};

/// A wall-clock instant in nanoseconds since the Unix epoch (UTC).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Nanos(pub u64);

impl Nanos {
    /// The Unix epoch.
    pub const EPOCH: Nanos = Nanos(0);

    /// The current wall-clock time. Returns [`Nanos::EPOCH`] if the system
    /// clock is set before the Unix epoch, which does not happen on a
    /// chrony-synced host.
    #[must_use]
    pub fn now() -> Nanos {
        use std::time::{SystemTime, UNIX_EPOCH};
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => Nanos(d.as_nanos() as u64),
            Err(_) => Nanos::EPOCH,
        }
    }

    /// The raw nanosecond value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Milliseconds since the epoch (truncated), the unit Binance uses for
    /// order timestamps and `recvWindow`.
    #[inline]
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0 / 1_000_000
    }

    /// Constructs from milliseconds since the epoch.
    #[inline]
    #[must_use]
    pub const fn from_millis(ms: u64) -> Nanos {
        Nanos(ms.saturating_mul(1_000_000))
    }

    /// The signed duration from `self` to `later`, saturating at zero if
    /// `later` precedes `self`. Because both are wall-clock stamps this is only
    /// meaningful for coarse age checks, never for latency measurement.
    #[inline]
    #[must_use]
    pub const fn elapsed_until(self, later: Nanos) -> Duration {
        Duration::from_nanos(later.0.saturating_sub(self.0))
    }
}

impl fmt::Display for Nanos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ns", self.0)
    }
}

impl fmt::Debug for Nanos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Nanos({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millis_roundtrip() {
        let n = Nanos::from_millis(1_700_000_000_000);
        assert_eq!(n.as_millis(), 1_700_000_000_000);
    }

    #[test]
    fn elapsed_saturates() {
        let a = Nanos(1000);
        let b = Nanos(3000);
        assert_eq!(a.elapsed_until(b).as_nanos(), 2000);
        assert_eq!(b.elapsed_until(a).as_nanos(), 0);
    }

    #[test]
    fn now_is_after_epoch() {
        assert!(Nanos::now() > Nanos::EPOCH);
    }
}
