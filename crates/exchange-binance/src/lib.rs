//! Binance adapter: read-only combined-stream market data and REST snapshots.
//!
//! Layering (from pure to IO):
//!   * [`models`]: serde wire structs, exact Binance shapes.
//!   * [`symbols`]: the `exchangeInfo`-derived symbol table.
//!   * [`normalize`]: pure conversion of wire payloads into
//!     [`core_types::MarketEvent`]. Fully fixture-tested.
//!   * [`ratelimit`] and [`backoff`]: pure policy helpers.
//!   * [`rest`] and [`stream`]: thin IO wrappers around the pure core.
//!
//! Order submission is intentionally absent; it arrives at the execution
//! milestones as a separate capability so a read-only build never links it.
#![forbid(unsafe_code)]

pub mod auth;
pub mod backoff;
pub mod models;
pub mod normalize;
pub mod ratelimit;
pub mod rest;
pub mod stream;
pub mod symbols;

pub use auth::Ed25519Signer;
pub use rest::{BinanceRest, DATA_REST, MAINNET_REST};
pub use stream::{BinanceMarketStream, DATA_WS, MAINNET_WS};
pub use symbols::{SymbolEntry, SymbolTable};
