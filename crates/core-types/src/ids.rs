//! Identifier and sequencing types.

use core::fmt;

use serde::{Deserialize, Serialize};

/// A monotonically increasing sequence number assigned by the engine to every
/// internal event (book update, opportunity, order request, and so on).
/// Monotonicity is what makes deterministic replay possible: replaying events
/// in sequence-number order reproduces the exact original ordering.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct SeqNo(pub u64);

impl SeqNo {
    /// The starting sequence number.
    pub const ZERO: SeqNo = SeqNo(0);

    /// Returns the next sequence number. Wraps only after 2^64 events, which at
    /// any realistic event rate does not occur within the life of a process.
    #[inline]
    #[must_use]
    pub const fn next(self) -> SeqNo {
        SeqNo(self.0.wrapping_add(1))
    }

    /// The raw integer value.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SeqNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for SeqNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqNo({})", self.0)
    }
}

/// An exchange-assigned order id. Binance order ids are positive 64-bit
/// integers.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OrderId(pub i64);

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OrderId({})", self.0)
    }
}

/// A client-assigned order id echoed back by the exchange, used to correlate a
/// submission with its acknowledgement and fills. Binance permits a wider
/// character set than an asset ticker (letters, digits, and `.:/_-`), so this
/// is a heap string rather than an inline ASCII identifier. It is set on the
/// order-submission path, not on the market-data hot path.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClientOrderId(pub String);

impl fmt::Display for ClientOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ClientOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClientOrderId({:?})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seqno_increments_and_orders() {
        let a = SeqNo::ZERO;
        let b = a.next();
        assert_eq!(b.get(), 1);
        assert!(a < b);
        assert_eq!(SeqNo::ZERO.next().next(), SeqNo(2));
    }

    #[test]
    fn ids_display() {
        assert_eq!(OrderId(42).to_string(), "42");
        assert_eq!(ClientOrderId("abc-1".to_owned()).to_string(), "abc-1");
    }
}
