//! Slippage-aware profitability: the volume-weighted breakpoint optimizer.
//!
//! Best bid/ask alone overstate a tradeable cycle: executing more than the
//! top-of-book quantity walks the book. For a cycle `A -> B -> C -> A` with
//! starting notional `n_A`, each leg converts its input into its output by
//! consuming order-book levels at worse and worse prices, so the realized cycle
//! output is a concave, piecewise-linear function of `n_A`. Profit
//! `output(n_A) - n_A` is therefore concave, and its maximum sits at a
//! breakpoint (an order-book level boundary), not in the interior.
//!
//! We model each leg as a sequence of constant-rate chunks (one per level, rate
//! decreasing) and compose the three legs by advancing `n_A` to the next
//! boundary among them, which yields the composed marginal rate as a step
//! function. Consuming every slice whose composed marginal rate is at least one
//! gives the exact profit-maximizing size in `O(depth)`.
//!
//! All amounts here are in the cycle's starting asset. Fees are folded into the
//! chunk rates, so the resulting profit is already net of fees and slippage.

use core_types::{PriceLevel, Side};

/// Relative tolerance for deciding a chunk is exhausted, guarding float drift.
const EPS: f64 = 1e-12;

/// One leg's order-book side, ready to be consumed.
///
/// `levels` must be in consumption order: for a `Sell` (of the base, a
/// `BASE -> QUOTE` hop) these are bids in descending price; for a `Buy` (a
/// `QUOTE -> BASE` hop) these are asks in ascending price.
#[derive(Debug, Clone)]
pub struct LegBook {
    /// Whether this leg sells or buys the base asset.
    pub side: Side,
    /// Taker fee fraction applied to this leg (for example `0.001`).
    pub fee: f64,
    /// Book levels for the side being consumed, best first.
    pub levels: Vec<PriceLevel>,
}

/// A constant-rate slice: `rate` units of output per unit of input, for up to
/// `cap` units of input.
#[derive(Debug, Clone, Copy)]
struct Chunk {
    rate: f64,
    cap: f64,
}

/// A constant-rate segment of the composed cycle, in starting-asset units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Length of the segment along `n_A`.
    pub len: f64,
    /// Composed marginal return factor over the segment (output A per input A).
    pub rate: f64,
}

/// The composed profit shape of a cycle as a function of starting notional.
#[derive(Debug, Clone)]
pub struct CycleShape {
    segments: Vec<Segment>,
    total_capacity: f64,
}

impl CycleShape {
    /// The maximum tradeable size (total input the books can absorb before any
    /// leg is exhausted), in starting-asset units.
    #[must_use]
    pub fn total_capacity(&self) -> f64 {
        self.total_capacity
    }

    /// The constant-rate segments, in increasing-size order (rate
    /// non-increasing).
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The profit-maximizing starting size: the sum of the lengths of all
    /// leading segments whose composed marginal rate is at least one.
    #[must_use]
    pub fn optimal_size(&self) -> f64 {
        self.segments
            .iter()
            .take_while(|s| s.rate >= 1.0)
            .map(|s| s.len)
            .sum()
    }

    /// Profit (net of fees and slippage) for a given starting size, in
    /// starting-asset units. Sizes beyond `total_capacity` are clamped.
    #[must_use]
    pub fn profit_at(&self, size: f64) -> f64 {
        let mut remaining = size.max(0.0);
        let mut profit = 0.0;
        for seg in &self.segments {
            if remaining <= 0.0 {
                break;
            }
            let take = remaining.min(seg.len);
            profit += (seg.rate - 1.0) * take;
            remaining -= take;
        }
        profit
    }

    /// The realized net return (edge) at a size: `profit / size`.
    #[must_use]
    pub fn net_return_at(&self, size: f64) -> f64 {
        if size <= 0.0 {
            0.0
        } else {
            self.profit_at(size) / size
        }
    }

    /// The profit at [`Self::optimal_size`].
    #[must_use]
    pub fn optimal_profit(&self) -> f64 {
        self.profit_at(self.optimal_size())
    }
}

/// Builds the chunk list for one leg. Zero-quantity levels are skipped.
fn leg_chunks(leg: &LegBook) -> Vec<Chunk> {
    let net = 1.0 - leg.fee;
    leg.levels
        .iter()
        .filter_map(|lvl| {
            let price = lvl.price.to_f64();
            let qty = lvl.qty.to_f64();
            if price <= 0.0 || qty <= 0.0 {
                return None;
            }
            let chunk = match leg.side {
                // Sell base into a bid: input is base (qty), output is quote,
                // rate = price * net (quote per base).
                Side::Sell => Chunk {
                    rate: price * net,
                    cap: qty,
                },
                // Buy base from an ask: input is quote (price*qty), output is
                // base, rate = net / price (base per quote).
                Side::Buy => Chunk {
                    rate: net / price,
                    cap: price * qty,
                },
            };
            Some(chunk)
        })
        .collect()
}

/// Advances through one leg's chunks, tracking remaining capacity in the leg's
/// own input units.
struct Cursor {
    chunks: Vec<Chunk>,
    idx: usize,
    rem: f64,
}

impl Cursor {
    fn new(chunks: Vec<Chunk>) -> Cursor {
        let rem = chunks.first().map_or(0.0, |c| c.cap);
        Cursor {
            chunks,
            idx: 0,
            rem,
        }
    }

    fn rate(&self) -> Option<f64> {
        self.chunks.get(self.idx).map(|c| c.rate)
    }

    /// Consumes `amount` of this leg's input, advancing to the next chunk when
    /// the current one is spent.
    fn consume(&mut self, amount: f64) {
        self.rem -= amount;
        let cap = self.chunks.get(self.idx).map_or(0.0, |c| c.cap);
        if self.rem <= cap * EPS {
            self.idx += 1;
            self.rem = self.chunks.get(self.idx).map_or(0.0, |c| c.cap);
        }
    }
}

/// Composes three legs into the cycle's profit shape.
///
/// `l1` converts A into B, `l2` converts B into C, `l3` converts C into A. Each
/// leg's chunks must already reflect its fee. The result is expressed in units
/// of A.
#[must_use]
pub fn optimize_cycle(l1: &LegBook, l2: &LegBook, l3: &LegBook) -> CycleShape {
    let mut c1 = Cursor::new(leg_chunks(l1));
    let mut c2 = Cursor::new(leg_chunks(l2));
    let mut c3 = Cursor::new(leg_chunks(l3));

    let mut segments = Vec::new();
    let mut total = 0.0;

    // Each iteration advances at least one cursor to its next chunk, so the loop
    // runs at most (levels1 + levels2 + levels3) times. It ends when any leg is
    // exhausted (a `None` rate).
    while let (Some(m1), Some(m2), Some(m3)) = (c1.rate(), c2.rate(), c3.rate()) {
        // Remaining capacity of each leg expressed in units of A.
        // dn_B = m1 * dn_A, dn_C = m1 * m2 * dn_A.
        let cap1 = c1.rem;
        let cap2 = c2.rem / m1;
        let cap3 = c3.rem / (m1 * m2);
        let dn = cap1.min(cap2).min(cap3);
        if dn <= 0.0 || !dn.is_finite() {
            break;
        }

        segments.push(Segment {
            len: dn,
            rate: m1 * m2 * m3,
        });
        total += dn;

        c1.consume(dn);
        c2.consume(dn * m1);
        c3.consume(dn * m1 * m2);
    }

    CycleShape {
        segments,
        total_capacity: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::Decimal;

    fn lvl(price: &str, qty: &str) -> PriceLevel {
        PriceLevel::new(
            Decimal::from_str_exact(price).unwrap(),
            Decimal::from_str_exact(qty).unwrap(),
        )
    }

    fn leg(side: Side, fee: f64, levels: &[(&str, &str)]) -> LegBook {
        LegBook {
            side,
            fee,
            levels: levels.iter().map(|(p, q)| lvl(p, q)).collect(),
        }
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6 * (1.0 + b.abs())
    }

    #[test]
    fn single_level_cycle_is_linear() {
        // Composed rate 2 * 2 * 0.3 = 1.2. Leg 1 caps input A at 100.
        let l1 = leg(Side::Sell, 0.0, &[("2", "100")]); // rate 2, cap 100 A
        let l2 = leg(Side::Buy, 0.0, &[("0.5", "100000")]); // rate 2, cap 50000 B
        let l3 = leg(Side::Sell, 0.0, &[("0.3", "100000")]); // rate 0.3, cap 100000 C
        let shape = optimize_cycle(&l1, &l2, &l3);
        assert!(
            approx(shape.total_capacity(), 100.0),
            "cap {}",
            shape.total_capacity()
        );
        assert!(approx(shape.optimal_size(), 100.0));
        assert!(approx(shape.optimal_profit(), 20.0)); // 0.2 * 100
        assert!(approx(shape.net_return_at(100.0), 0.2));
    }

    #[test]
    fn slippage_stops_at_optimum() {
        // Leg 1 has a rich first level then a deep, terrible one. The composed
        // rate drops below 1 after the first 50 units of A, so the optimum is 50.
        let l1 = leg(Side::Sell, 0.0, &[("2", "50"), ("0.3", "1000")]);
        let l2 = leg(Side::Buy, 0.0, &[("0.5", "1000000")]);
        let l3 = leg(Side::Sell, 0.0, &[("0.3", "1000000")]);
        let shape = optimize_cycle(&l1, &l2, &l3);
        assert!(
            approx(shape.optimal_size(), 50.0),
            "opt {}",
            shape.optimal_size()
        );
        assert!(approx(shape.optimal_profit(), 10.0)); // 0.2 * 50
                                                       // Trading past the optimum destroys profit.
        assert!(shape.profit_at(shape.total_capacity()) < shape.optimal_profit());
    }

    #[test]
    fn fees_reduce_rate() {
        let no_fee = optimize_cycle(
            &leg(Side::Sell, 0.0, &[("2", "100")]),
            &leg(Side::Buy, 0.0, &[("0.5", "100000")]),
            &leg(Side::Sell, 0.0, &[("0.3", "100000")]),
        );
        let with_fee = optimize_cycle(
            &leg(Side::Sell, 0.001, &[("2", "100")]),
            &leg(Side::Buy, 0.001, &[("0.5", "100000")]),
            &leg(Side::Sell, 0.001, &[("0.3", "100000")]),
        );
        assert!(with_fee.optimal_profit() < no_fee.optimal_profit());
    }

    #[test]
    fn segment_rates_are_non_increasing() {
        // Concavity: composed marginal rate never increases as size grows.
        let l1 = leg(
            Side::Sell,
            0.0,
            &[("2", "10"), ("1.9", "10"), ("1.5", "10")],
        );
        let l2 = leg(Side::Buy, 0.0, &[("0.5", "1000"), ("0.55", "1000")]);
        let l3 = leg(Side::Sell, 0.0, &[("0.3", "1000")]);
        let shape = optimize_cycle(&l1, &l2, &l3);
        for w in shape.segments().windows(2) {
            assert!(w[1].rate <= w[0].rate + 1e-12, "rates must not increase");
        }
    }

    #[test]
    fn empty_book_has_no_capacity() {
        let l1 = leg(Side::Sell, 0.0, &[]);
        let l2 = leg(Side::Buy, 0.0, &[("0.5", "100")]);
        let l3 = leg(Side::Sell, 0.0, &[("0.3", "100")]);
        let shape = optimize_cycle(&l1, &l2, &l3);
        assert_eq!(shape.total_capacity(), 0.0);
        assert_eq!(shape.optimal_profit(), 0.0);
    }
}
