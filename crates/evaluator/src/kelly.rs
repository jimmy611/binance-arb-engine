//! Position sizing via a fractional Kelly criterion.
//!
//! For an opportunity with estimated edge `e` (expected fractional return) and
//! variance `v` (from historical fills of similar cycles), the full-Kelly
//! fraction is `e / v`. In high-frequency contexts parameter estimation error is
//! severe, so we scale by a `fractional_multiplier` (quarter Kelly by default)
//! for robustness, then clamp to hard notional limits. The result is further
//! capped by the joint order-book capacity computed by the VWAP optimizer.

/// Parameters for fractional Kelly sizing.
#[derive(Debug, Clone, Copy)]
pub struct KellyParams {
    /// Scale on the full-Kelly fraction (0.25 = quarter Kelly).
    pub fractional_multiplier: f64,
    /// Hard minimum notional per trade.
    pub min_notional: f64,
    /// Hard maximum notional per trade.
    pub max_notional: f64,
}

impl Default for KellyParams {
    fn default() -> KellyParams {
        KellyParams {
            fractional_multiplier: 0.25,
            min_notional: 0.0,
            max_notional: f64::INFINITY,
        }
    }
}

/// The fractional-Kelly suggested size, clamped to `[min_notional,
/// max_notional]`.
///
/// Returns `0.0` when there is no edge, non-positive variance, or no capital,
/// which means "do not trade". Otherwise the raw suggestion
/// `(e / v) * multiplier * capital` is clamped to the configured bounds.
#[must_use]
pub fn fractional_kelly_size(edge: f64, variance: f64, capital: f64, params: &KellyParams) -> f64 {
    if edge <= 0.0 || variance <= 0.0 || capital <= 0.0 {
        return 0.0;
    }
    let kelly_fraction = edge / variance;
    let raw = kelly_fraction * params.fractional_multiplier * capital;
    let (lo, hi) = if params.min_notional <= params.max_notional {
        (params.min_notional, params.max_notional)
    } else {
        (params.max_notional, params.min_notional)
    };
    raw.clamp(lo, hi)
}

/// The final tradeable size: the Kelly suggestion capped by the order-book
/// capacity. Returns `0.0` if the capacity cannot meet the minimum notional.
#[must_use]
pub fn size_with_capacity(
    edge: f64,
    variance: f64,
    capital: f64,
    capacity: f64,
    params: &KellyParams,
) -> f64 {
    let suggested = fractional_kelly_size(edge, variance, capital, params);
    let capped = suggested.min(capacity);
    if capped < params.min_notional {
        0.0
    } else {
        capped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_kelly_scales_the_fraction() {
        let params = KellyParams {
            fractional_multiplier: 0.25,
            min_notional: 0.0,
            max_notional: f64::INFINITY,
        };
        // edge 0.02, variance 0.04 => full kelly 0.5; quarter => 0.125 of 10000.
        let size = fractional_kelly_size(0.02, 0.04, 10_000.0, &params);
        assert!((size - 1250.0).abs() < 1e-6);
    }

    #[test]
    fn no_edge_means_no_trade() {
        let params = KellyParams::default();
        assert_eq!(fractional_kelly_size(0.0, 0.04, 10_000.0, &params), 0.0);
        assert_eq!(fractional_kelly_size(-0.01, 0.04, 10_000.0, &params), 0.0);
        assert_eq!(fractional_kelly_size(0.02, 0.0, 10_000.0, &params), 0.0);
    }

    #[test]
    fn clamps_to_limits() {
        let params = KellyParams {
            fractional_multiplier: 1.0,
            min_notional: 100.0,
            max_notional: 500.0,
        };
        // Raw suggestion huge => capped to max.
        assert_eq!(fractional_kelly_size(0.5, 0.01, 100_000.0, &params), 500.0);
        // Raw suggestion tiny => raised to min.
        assert_eq!(fractional_kelly_size(0.0001, 1.0, 100.0, &params), 100.0);
    }

    #[test]
    fn capacity_caps_and_rejects_below_min() {
        let params = KellyParams {
            fractional_multiplier: 1.0,
            min_notional: 100.0,
            max_notional: 10_000.0,
        };
        // Kelly wants 5000 but the book only holds 800.
        assert_eq!(
            size_with_capacity(0.05, 0.01, 100_000.0, 800.0, &params),
            800.0
        );
        // Book cannot meet the minimum notional => no trade.
        assert_eq!(
            size_with_capacity(0.05, 0.01, 100_000.0, 50.0, &params),
            0.0
        );
    }
}
