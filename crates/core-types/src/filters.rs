//! Exchange trading rules for a symbol.
//!
//! Every venue constrains order price and quantity: a minimum increment (tick)
//! for price, a minimum increment (step) for quantity, absolute min/max
//! quantity, and a minimum notional (price * quantity). Orders that violate
//! these are rejected by the exchange, so the evaluator and execution router
//! must snap sizes and prices to these rules before submitting. Populated from
//! Binance `exchangeInfo` (`PRICE_FILTER`, `LOT_SIZE`, `MIN_NOTIONAL`) at M2.

use serde::{Deserialize, Serialize};

use crate::decimal::Decimal;

/// Price and quantity constraints for a single symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolFilters {
    /// Minimum price increment. Valid prices are multiples of this.
    pub tick_size: Decimal,
    /// Minimum quantity increment. Valid quantities are multiples of this.
    pub step_size: Decimal,
    /// Minimum order quantity.
    pub min_qty: Decimal,
    /// Maximum order quantity.
    pub max_qty: Decimal,
    /// Minimum order notional (price * quantity).
    pub min_notional: Decimal,
}

impl SymbolFilters {
    /// Rounds a quantity down to the nearest valid multiple of `step_size`.
    /// Rounding down (never up) guarantees the result never exceeds available
    /// size or the requested amount. Returns the input unchanged if
    /// `step_size` is zero.
    #[must_use]
    pub fn snap_qty_down(&self, qty: Decimal) -> Decimal {
        snap_down(qty, self.step_size)
    }

    /// Rounds a price down to the nearest valid multiple of `tick_size`.
    /// Returns the input unchanged if `tick_size` is zero.
    #[must_use]
    pub fn snap_price_down(&self, price: Decimal) -> Decimal {
        snap_down(price, self.tick_size)
    }

    /// Returns `true` if `qty` at `price` satisfies quantity bounds and the
    /// minimum notional.
    #[must_use]
    pub fn is_tradeable(&self, price: Decimal, qty: Decimal) -> bool {
        qty >= self.min_qty
            && qty <= self.max_qty
            && price
                .checked_mul(qty)
                .is_some_and(|n| n >= self.min_notional)
    }
}

/// Rounds `value` down to the nearest multiple of `increment` using the raw
/// integer representation, which is exact. A zero or negative increment is
/// treated as "no constraint" and returns `value` unchanged.
fn snap_down(value: Decimal, increment: Decimal) -> Decimal {
    let inc = increment.raw();
    if inc <= 0 {
        return value;
    }
    let v = value.raw();
    // Euclidean flooring so negative values also round toward negative infinity,
    // though prices and quantities are non-negative in practice.
    let floored = v.div_euclid(inc) * inc;
    Decimal::from_raw(floored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn filters() -> SymbolFilters {
        SymbolFilters {
            tick_size: d("0.01"),
            step_size: d("0.001"),
            min_qty: d("0.001"),
            max_qty: d("1000"),
            min_notional: d("10"),
        }
    }

    #[test]
    fn snaps_quantity_down_to_step() {
        let f = filters();
        assert_eq!(f.snap_qty_down(d("1.2345")), d("1.234"));
        assert_eq!(f.snap_qty_down(d("1.234")), d("1.234"));
    }

    #[test]
    fn snaps_price_down_to_tick() {
        let f = filters();
        assert_eq!(f.snap_price_down(d("100.019")), d("100.01"));
    }

    #[test]
    fn zero_increment_is_no_constraint() {
        let f = SymbolFilters {
            tick_size: Decimal::ZERO,
            step_size: Decimal::ZERO,
            min_qty: Decimal::ZERO,
            max_qty: d("1000"),
            min_notional: Decimal::ZERO,
        };
        assert_eq!(f.snap_qty_down(d("1.2345")), d("1.2345"));
    }

    #[test]
    fn tradeable_checks_bounds_and_notional() {
        let f = filters();
        // 0.2 * 100 = 20 >= 10 notional, qty within [0.001, 1000].
        assert!(f.is_tradeable(d("100"), d("0.2")));
        // Below min notional: 0.001 * 100 = 0.1 < 10.
        assert!(!f.is_tradeable(d("100"), d("0.001")));
        // Below min qty.
        assert!(!f.is_tradeable(d("100"), d("0.0001")));
    }
}
