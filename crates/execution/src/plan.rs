//! Building an executable order plan from a sized cycle.
//!
//! Given the ordered legs of a cycle (symbol, side, the price that produced the
//! opportunity, and the intended base quantity) plus each symbol's filters, this
//! snaps quantities to the lot step and prices to the tick, chooses each leg's
//! order type from the execution mode, and assembles the [`OrderRequest`]s. It
//! sends nothing; the router decides what to do with the plan.

use core_types::{ClientOrderId, Decimal, Side, Symbol, SymbolFilters};
use exchange_common::{OrderRequest, OrderType};
use serde::{Deserialize, Serialize};

use crate::modes::ExecutionMode;

/// One leg of a sized cycle, ready to be turned into an order.
#[derive(Debug, Clone)]
pub struct PlannedLeg {
    /// The market to trade.
    pub symbol: Symbol,
    /// Buy or sell the base.
    pub side: Side,
    /// The price that produced the opportunity (used as the IOC limit).
    pub price: Decimal,
    /// Intended base quantity (pre-snap).
    pub qty: Decimal,
    /// The symbol's trading filters.
    pub filters: SymbolFilters,
}

/// A fully-assembled, filter-compliant order plan for one cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Correlates the plan across logs, WAL, and fills.
    pub cycle_id: String,
    /// The chosen execution mode.
    pub mode: ExecutionMode,
    /// The orders, in leg order.
    pub orders: Vec<OrderRequest>,
    /// Approximate starting notional (leg-0 price times quantity).
    pub notional: Decimal,
    /// Whether every leg satisfies its symbol filters. A false here means the
    /// plan must not be submitted.
    pub tradeable: bool,
}

/// Builds an [`ExecutionPlan`] from ordered legs and a mode. In sequential mode,
/// leg 0 is an IOC limit and the remaining legs are market orders; in parallel
/// mode every leg is an IOC limit.
#[must_use]
pub fn build_plan(cycle_id: &str, legs: &[PlannedLeg], mode: ExecutionMode) -> ExecutionPlan {
    let mut orders = Vec::with_capacity(legs.len());
    let mut tradeable = !legs.is_empty();
    let mut notional = Decimal::ZERO;

    for (i, leg) in legs.iter().enumerate() {
        let qty = leg.filters.snap_qty_down(leg.qty);
        let price = leg.filters.snap_price_down(leg.price);

        let use_market = matches!(mode, ExecutionMode::SequentialLockIn) && i > 0;
        let order_type = if use_market {
            OrderType::Market
        } else {
            OrderType::LimitIoc
        };
        let order_price = if order_type == OrderType::Market {
            None
        } else {
            Some(price)
        };

        if !leg.filters.is_tradeable(price, qty) {
            tradeable = false;
        }
        if i == 0 {
            notional = price.checked_mul(qty).unwrap_or(Decimal::ZERO);
        }

        orders.push(OrderRequest {
            symbol: leg.symbol,
            side: leg.side,
            order_type,
            price: order_price,
            qty,
            client_order_id: ClientOrderId(format!("{cycle_id}-L{i}")),
        });
    }

    ExecutionPlan {
        cycle_id: cycle_id.to_owned(),
        mode,
        orders,
        notional,
        tradeable,
    }
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

    fn legs() -> Vec<PlannedLeg> {
        vec![
            PlannedLeg {
                symbol: Symbol::from_parts("BTC", "USDT").unwrap(),
                side: Side::Sell,
                price: d("60000.019"),
                qty: d("0.0234"),
                filters: filters(),
            },
            PlannedLeg {
                symbol: Symbol::from_parts("ETH", "BTC").unwrap(),
                side: Side::Buy,
                price: d("0.052"),
                qty: d("1.2345"),
                filters: filters(),
            },
            PlannedLeg {
                symbol: Symbol::from_parts("ETH", "USDT").unwrap(),
                side: Side::Sell,
                price: d("3000.5"),
                qty: d("1.2"),
                filters: filters(),
            },
        ]
    }

    #[test]
    fn parallel_uses_ioc_limits_and_snaps() {
        let plan = build_plan("cyc-1", &legs(), ExecutionMode::ParallelIoc);
        assert_eq!(plan.orders.len(), 3);
        for o in &plan.orders {
            assert_eq!(o.order_type, OrderType::LimitIoc);
            assert!(o.price.is_some());
        }
        // Price and qty snapped to tick/step.
        assert_eq!(plan.orders[0].price.unwrap(), d("60000.01"));
        assert_eq!(plan.orders[0].qty, d("0.023"));
        assert_eq!(plan.orders[0].client_order_id.0, "cyc-1-L0");
    }

    #[test]
    fn sequential_locks_in_with_market_legs() {
        let plan = build_plan("cyc-2", &legs(), ExecutionMode::SequentialLockIn);
        assert_eq!(plan.orders[0].order_type, OrderType::LimitIoc);
        assert_eq!(plan.orders[1].order_type, OrderType::Market);
        assert_eq!(plan.orders[2].order_type, OrderType::Market);
        assert!(plan.orders[1].price.is_none());
    }

    #[test]
    fn below_min_notional_is_not_tradeable() {
        let mut legs = legs();
        legs[0].qty = d("0.001"); // 0.001 * 60000 = 60 >= 10, still ok
        legs[2].qty = d("0.001"); // 0.001 * 3000 = 3 < 10 min notional
        let plan = build_plan("cyc-3", &legs, ExecutionMode::ParallelIoc);
        assert!(!plan.tradeable);
    }

    #[test]
    fn notional_from_first_leg() {
        let plan = build_plan("cyc-4", &legs(), ExecutionMode::ParallelIoc);
        // 60000.01 * 0.023 = 1380.00023
        assert_eq!(plan.notional, d("1380.00023"));
    }
}
