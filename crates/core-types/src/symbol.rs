//! Trading pair identity.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::asset::{Asset, RawSymbol};
use crate::error::CoreError;

/// A trading pair: an ordered (base, quote) pair plus the exchange-native raw
/// symbol id. For Binance the raw id is the concatenation `BASE + QUOTE`
/// (`BTCUSDT`), but the raw id is treated as authoritative because a bare
/// concatenation is ambiguous to split without exchange metadata.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Symbol {
    base: Asset,
    quote: Asset,
    raw: RawSymbol,
}

impl Symbol {
    /// Constructs from explicit base, quote, and raw id (the shape exchange
    /// metadata provides: `baseAsset`, `quoteAsset`, `symbol`).
    #[inline]
    #[must_use]
    pub fn new(base: Asset, quote: Asset, raw: RawSymbol) -> Symbol {
        Symbol { base, quote, raw }
    }

    /// Convenience constructor that derives the raw id as `base + quote`.
    /// Useful in tests and for venues whose raw id is exactly the
    /// concatenation.
    pub fn from_parts(base: &str, quote: &str) -> Result<Symbol, CoreError> {
        let base = Asset::new(base)?;
        let quote = Asset::new(quote)?;
        let mut raw_str = String::with_capacity(base.len() + quote.len());
        raw_str.push_str(base.as_str());
        raw_str.push_str(quote.as_str());
        let raw = RawSymbol::new(&raw_str)?;
        Ok(Symbol { base, quote, raw })
    }

    /// The base asset (the asset being priced, sold on a bid).
    #[inline]
    #[must_use]
    pub fn base(&self) -> Asset {
        self.base
    }

    /// The quote asset (the asset the price is denominated in).
    #[inline]
    #[must_use]
    pub fn quote(&self) -> Asset {
        self.quote
    }

    /// The exchange-native raw symbol id (for example `BTCUSDT`).
    #[inline]
    #[must_use]
    pub fn raw(&self) -> RawSymbol {
        self.raw
    }

    /// The raw symbol id as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.raw.as_str()
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.raw.as_str())
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Symbol({} = {}/{})",
            self.raw.as_str(),
            self.base.as_str(),
            self.quote.as_str()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_parts_builds_raw() {
        let s = Symbol::from_parts("btc", "usdt").unwrap();
        assert_eq!(s.base().as_str(), "BTC");
        assert_eq!(s.quote().as_str(), "USDT");
        assert_eq!(s.as_str(), "BTCUSDT");
        assert_eq!(s.to_string(), "BTCUSDT");
    }

    #[test]
    fn explicit_raw_is_kept() {
        let s = Symbol::new(
            Asset::new("BTC").unwrap(),
            Asset::new("USDT").unwrap(),
            RawSymbol::new("BTCUSDT").unwrap(),
        );
        assert_eq!(s.as_str(), "BTCUSDT");
    }

    #[test]
    fn equality_uses_all_fields() {
        let a = Symbol::from_parts("BTC", "USDT").unwrap();
        let b = Symbol::from_parts("BTC", "USDT").unwrap();
        let c = Symbol::from_parts("ETH", "USDT").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn serde_json_roundtrip() {
        let s = Symbol::from_parts("ETH", "BTC").unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let back: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
