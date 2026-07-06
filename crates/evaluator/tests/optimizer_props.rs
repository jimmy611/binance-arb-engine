//! Property-based tests for the VWAP cycle optimizer: profit is concave in size
//! (composed marginal rate never increases) and the reported optimum is the true
//! maximum over the feasible range.

use core_types::{Decimal, PriceLevel, Side};
use evaluator::{optimize_cycle, LegBook};
use proptest::prelude::*;

fn dec(x: f64) -> Decimal {
    Decimal::from_f64(x).expect("finite positive")
}

/// A sell leg has bids in descending price (rate decreasing).
fn sell_leg() -> impl Strategy<Value = LegBook> {
    (
        1.0f64..100.0,
        prop::collection::vec((0.80f64..0.999, 0.1f64..100.0), 1..6),
        0.0f64..0.002,
    )
        .prop_map(|(p0, steps, fee)| {
            let mut price = p0;
            let mut levels = Vec::new();
            for (ratio, qty) in steps {
                levels.push(PriceLevel::new(dec(price), dec(qty)));
                price *= ratio;
            }
            LegBook {
                side: Side::Sell,
                fee,
                levels,
            }
        })
}

/// A buy leg has asks in ascending price (rate decreasing).
fn buy_leg() -> impl Strategy<Value = LegBook> {
    (
        0.001f64..1.0,
        prop::collection::vec((1.01f64..1.25, 1.0f64..1000.0), 1..6),
        0.0f64..0.002,
    )
        .prop_map(|(p0, steps, fee)| {
            let mut price = p0;
            let mut levels = Vec::new();
            for (ratio, qty) in steps {
                levels.push(PriceLevel::new(dec(price), dec(qty)));
                price *= ratio;
            }
            LegBook {
                side: Side::Buy,
                fee,
                levels,
            }
        })
}

proptest! {
    #[test]
    fn reported_optimum_is_the_maximum(l1 in sell_leg(), l2 in buy_leg(), l3 in sell_leg()) {
        let shape = optimize_cycle(&l1, &l2, &l3);
        let cap = shape.total_capacity();
        let optimum = shape.optimal_profit();
        for i in 0..=20 {
            let size = cap * f64::from(i) / 20.0;
            prop_assert!(
                optimum + 1e-6 >= shape.profit_at(size),
                "profit_at({size}) = {} beat optimum {optimum}",
                shape.profit_at(size)
            );
        }
    }

    #[test]
    fn composed_rate_is_non_increasing(l1 in sell_leg(), l2 in buy_leg(), l3 in sell_leg()) {
        let shape = optimize_cycle(&l1, &l2, &l3);
        for w in shape.segments().windows(2) {
            prop_assert!(w[1].rate <= w[0].rate + 1e-9, "rate increased");
        }
    }

    #[test]
    fn profit_is_zero_at_zero_size(l1 in sell_leg(), l2 in buy_leg(), l3 in sell_leg()) {
        let shape = optimize_cycle(&l1, &l2, &l3);
        prop_assert_eq!(shape.profit_at(0.0), 0.0);
    }
}
