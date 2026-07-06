//! Order-submission types and the execution capability.
//!
//! This is the second slice of the `ExchangeAdapter` surface: everything needed
//! to place and cancel orders. It lives behind a separate trait from
//! [`crate::MarketFeed`] so a read-only build never links order code.

use core::future::Future;

use core_types::{ClientOrderId, Decimal, OrderId, Side, Symbol};
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// The order type used for a leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Immediate-or-cancel limit order (the arbitrage default: fills what it can
    /// at the limit price now, cancels the rest).
    LimitIoc,
    /// Market order (guaranteed fill, accepts slippage). Used for lock-in legs
    /// and inventory unwind.
    Market,
}

/// A request to place one order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderRequest {
    /// The market.
    pub symbol: Symbol,
    /// Buy or sell the base asset.
    pub side: Side,
    /// Order type.
    pub order_type: OrderType,
    /// Limit price. Required for [`OrderType::LimitIoc`], ignored for market.
    pub price: Option<Decimal>,
    /// Base quantity, already snapped to the symbol's lot step.
    pub qty: Decimal,
    /// Client-assigned id for correlating the response and fills.
    pub client_order_id: ClientOrderId,
}

/// The lifecycle status of an order as reported by the exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    /// Accepted, not yet filled.
    New,
    /// Some quantity filled, more outstanding.
    PartiallyFilled,
    /// Fully filled.
    Filled,
    /// Canceled (an IOC remainder, or an explicit cancel).
    Canceled,
    /// Rejected by the exchange (rate limit, balance, filter violation).
    Rejected,
    /// Expired without filling.
    Expired,
}

impl OrderStatus {
    /// Whether any quantity filled.
    #[must_use]
    pub fn is_any_fill(self) -> bool {
        matches!(self, OrderStatus::PartiallyFilled | OrderStatus::Filled)
    }

    /// Whether the order is fully done (no more fills will occur).
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            OrderStatus::Filled
                | OrderStatus::Canceled
                | OrderStatus::Rejected
                | OrderStatus::Expired
        )
    }
}

/// The exchange's response to an order submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderResponse {
    /// Exchange-assigned order id.
    pub order_id: OrderId,
    /// Echoed client order id.
    pub client_order_id: ClientOrderId,
    /// Current status.
    pub status: OrderStatus,
    /// Cumulative base quantity filled.
    pub filled_qty: Decimal,
    /// Cumulative quote spent/received.
    pub cumulative_quote: Decimal,
    /// Volume-weighted average fill price, if any quantity filled.
    pub avg_price: Option<Decimal>,
}

/// The order-submission capability of an exchange adapter. Implemented by the
/// Binance adapter at the live-execution milestone.
pub trait OrderExecution: Send + Sync {
    /// Submits an order.
    fn submit_order(
        &self,
        request: &OrderRequest,
    ) -> impl Future<Output = Result<OrderResponse>> + Send;

    /// Cancels a resting order.
    fn cancel_order(
        &self,
        symbol: Symbol,
        order_id: OrderId,
    ) -> impl Future<Output = Result<()>> + Send;
}
