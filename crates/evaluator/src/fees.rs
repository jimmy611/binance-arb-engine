//! Fee model.
//!
//! Binance spot fees are tier-based (VIP level) and reduced when paid in BNB.
//! The engine queries the current tier at startup and periodically; this type is
//! the in-memory representation. Arbitrage uses taker fees by default because it
//! requires immediate execution, but maker fees are tracked for pairs with
//! zero-maker promotions, and per-symbol overrides support special cases.

use std::collections::HashMap;

use core_types::Symbol;

/// Maker and taker fee fractions for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fee {
    /// Maker fee fraction (for example `0.001`).
    pub maker: f64,
    /// Taker fee fraction.
    pub taker: f64,
}

/// A fee schedule with a default tier and optional per-symbol overrides.
#[derive(Debug, Clone, Default)]
pub struct FeeSchedule {
    default: Fee,
    overrides: HashMap<Symbol, Fee>,
}

impl FeeSchedule {
    /// Builds a schedule with a uniform default fee.
    #[must_use]
    pub fn new(maker: f64, taker: f64) -> FeeSchedule {
        FeeSchedule {
            default: Fee { maker, taker },
            overrides: HashMap::new(),
        }
    }

    /// The common Binance spot default: 0.1% maker and taker.
    #[must_use]
    pub fn binance_default() -> FeeSchedule {
        FeeSchedule::new(0.001, 0.001)
    }

    /// Sets a per-symbol override.
    pub fn set_override(&mut self, symbol: Symbol, fee: Fee) {
        self.overrides.insert(symbol, fee);
    }

    /// The fee for a symbol (override if present, else the default).
    #[must_use]
    pub fn fee(&self, symbol: &Symbol) -> Fee {
        self.overrides.get(symbol).copied().unwrap_or(self.default)
    }

    /// The taker fee fraction for a symbol (the arbitrage default).
    #[must_use]
    pub fn taker(&self, symbol: &Symbol) -> f64 {
        self.fee(symbol).taker
    }

    /// The maker fee fraction for a symbol.
    #[must_use]
    pub fn maker(&self, symbol: &Symbol) -> f64 {
        self.fee(symbol).maker
    }

    /// The net-of-taker-fee multiplier for a symbol, `1 - taker`.
    #[must_use]
    pub fn net_taker_multiplier(&self, symbol: &Symbol) -> f64 {
        1.0 - self.taker(symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(base: &str, quote: &str) -> Symbol {
        Symbol::from_parts(base, quote).unwrap()
    }

    #[test]
    fn default_and_override() {
        let mut s = FeeSchedule::binance_default();
        assert_eq!(s.taker(&sym("BTC", "USDT")), 0.001);
        s.set_override(
            sym("BTC", "USDT"),
            Fee {
                maker: 0.0,
                taker: 0.0005,
            },
        );
        assert_eq!(s.taker(&sym("BTC", "USDT")), 0.0005);
        assert_eq!(s.maker(&sym("BTC", "USDT")), 0.0);
        // Unrelated symbol still uses the default.
        assert_eq!(s.taker(&sym("ETH", "USDT")), 0.001);
    }

    #[test]
    fn net_multiplier() {
        let s = FeeSchedule::new(0.001, 0.002);
        assert!((s.net_taker_multiplier(&sym("BTC", "USDT")) - 0.998).abs() < 1e-12);
    }
}
