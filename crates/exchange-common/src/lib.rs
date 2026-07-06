//! Exchange-agnostic abstractions shared by every venue adapter.
//!
//! Hosts the common [`ExchangeError`], the read-only [`MarketFeed`] trait, and
//! subscription descriptors. Order-submission capabilities are added at the
//! execution milestones as a separate trait so a read-only build never links
//! order code. Each concrete adapter (starting with `exchange-binance`)
//! normalizes its venue payloads into the shared [`core_types::MarketEvent`].
#![forbid(unsafe_code)]

pub mod error;
pub mod feed;

pub use error::{ExchangeError, Result};
pub use feed::{MarketFeed, StreamKind, Subscription};
