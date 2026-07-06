//! Inline, allocation-free ASCII identifiers.
//!
//! Asset tickers ("BTC", "USDT", "1000SATS") and raw exchange symbols
//! ("BTCUSDT") are tiny, fixed, and touched on the hot path as graph-vertex and
//! order-book keys. Heap-allocating a `String` for each would add pointer
//! chasing and allocator traffic to the microsecond budget, so we store them
//! inline in a fixed byte array. The type is `Copy`, so it moves by value with
//! no indirection, and equality/hashing operate on the raw bytes.
//!
//! Unused trailing bytes are always zero, which makes the derived `Eq`, `Hash`,
//! and `Ord` correct: two identifiers are equal exactly when their byte arrays
//! match, and lexicographic array ordering matches string ordering because the
//! stored charset (uppercase ASCII alphanumerics) never contains a zero byte.

use core::fmt;
use core::str::FromStr;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::CoreError;

/// A fixed-capacity, `Copy` ASCII identifier holding up to `N` uppercase
/// alphanumeric bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InlineAscii<const N: usize> {
    bytes: [u8; N],
    len: u8,
}

/// An asset ticker such as `BTC` or `USDT`.
pub type Asset = InlineAscii<16>;

/// A raw exchange symbol id such as `BTCUSDT`.
pub type RawSymbol = InlineAscii<32>;

impl<const N: usize> InlineAscii<N> {
    /// Maximum number of bytes this identifier can hold.
    pub const CAPACITY: usize = N;

    /// Parses an identifier, uppercasing ASCII letters and validating that
    /// every character is an uppercase letter or a digit. Returns
    /// [`CoreError::InvalidAsset`] on an empty string, an over-long string, or a
    /// disallowed character.
    pub fn new(s: &str) -> Result<Self, CoreError> {
        // N must fit in the u8 length field. Checked at compile time.
        const {
            assert!(
                N <= u8::MAX as usize,
                "InlineAscii capacity exceeds u8 length"
            );
        }
        let raw = s.as_bytes();
        if raw.is_empty() {
            return Err(CoreError::InvalidAsset {
                input: s.to_owned(),
                reason: "empty",
            });
        }
        if raw.len() > N {
            return Err(CoreError::InvalidAsset {
                input: s.to_owned(),
                reason: "too long for capacity",
            });
        }
        let mut bytes = [0u8; N];
        for (i, &c) in raw.iter().enumerate() {
            let up = c.to_ascii_uppercase();
            if !(up.is_ascii_uppercase() || up.is_ascii_digit()) {
                return Err(CoreError::InvalidAsset {
                    input: s.to_owned(),
                    reason: "non-alphanumeric character",
                });
            }
            bytes[i] = up;
        }
        Ok(Self {
            bytes,
            len: raw.len() as u8,
        })
    }

    /// Borrows the identifier as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Invariant: bytes[..len] were validated as ASCII on construction.
        core::str::from_utf8(&self.bytes[..self.len as usize])
            .expect("InlineAscii invariant: contents are valid ASCII")
    }

    /// Number of bytes in the identifier.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Always `false`: an [`InlineAscii`] can never be constructed empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<const N: usize> fmt::Display for InlineAscii<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize> fmt::Debug for InlineAscii<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl<const N: usize> FromStr for InlineAscii<N> {
    type Err = CoreError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InlineAscii::new(s)
    }
}

impl<const N: usize> TryFrom<&str> for InlineAscii<N> {
    type Error = CoreError;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        InlineAscii::new(s)
    }
}

impl<const N: usize> Serialize for InlineAscii<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de, const N: usize> Deserialize<'de> for InlineAscii<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        InlineAscii::new(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_and_borrows() {
        let a = Asset::new("btc").unwrap();
        assert_eq!(a.as_str(), "BTC");
        assert_eq!(a.len(), 3);
        assert!(!a.is_empty());
    }

    #[test]
    fn uppercases_and_keeps_digits() {
        assert_eq!(Asset::new("1000sats").unwrap().as_str(), "1000SATS");
    }

    #[test]
    fn equality_and_ordering_match_contents() {
        assert_eq!(Asset::new("BTC").unwrap(), Asset::new("btc").unwrap());
        assert!(Asset::new("BTC").unwrap() < Asset::new("BTCB").unwrap());
        assert!(Asset::new("BT").unwrap() < Asset::new("BTC").unwrap());
        assert_ne!(Asset::new("USDT").unwrap(), Asset::new("USDC").unwrap());
    }

    #[test]
    fn rejects_invalid() {
        assert!(Asset::new("").is_err());
        assert!(Asset::new("BTC-USD").is_err());
        assert!(Asset::new("has space").is_err());
        // 17 chars exceeds the 16-byte Asset capacity.
        assert!(Asset::new("ABCDEFGHIJKLMNOPQ").is_err());
    }

    #[test]
    fn raw_symbol_has_larger_capacity() {
        let r = RawSymbol::new("BTCUSDT").unwrap();
        assert_eq!(r.as_str(), "BTCUSDT");
        assert_eq!(RawSymbol::CAPACITY, 32);
    }

    #[test]
    fn serde_json_roundtrip() {
        let a = Asset::new("ETH").unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, "\"ETH\"");
        let back: Asset = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);
    }
}
