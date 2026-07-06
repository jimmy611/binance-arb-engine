//! Core value types shared across the entire engine.
//!
//! This crate is the vocabulary every other crate speaks: the exact fixed-point
//! [`Decimal`], allocation-free [`Asset`]/[`Symbol`] identifiers, the unified
//! [`MarketEvent`] enum, the order-book output contract, ids, time, exchange
//! filters, and the engine [`EngineMode`]. It has no dependency on any other
//! workspace crate, so it can never introduce a cycle and its types are cheap
//! to depend on everywhere.
//!
//! Design rules enforced here:
//!
//! * No `unsafe`. A trading engine has no business with memory-unsafety bugs in
//!   its type layer.
//! * Amounts are exact fixed-point; only rates and log-weights are floating
//!   point, and those live in the detector, not here.
//! * Identifiers are `Copy` and inline so the hot path never allocates to name
//!   a symbol.
#![forbid(unsafe_code)]

pub mod asset;
pub mod book;
pub mod decimal;
pub mod error;
pub mod filters;
pub mod ids;
pub mod market;
pub mod mode;
pub mod symbol;
pub mod time;

pub use asset::{Asset, InlineAscii, RawSymbol};
pub use book::{BookStatus, BookUpdate};
pub use decimal::{Decimal, SCALE, SCALE_FACTOR};
pub use error::{CoreError, Result};
pub use filters::SymbolFilters;
pub use ids::{ClientOrderId, OrderId, SeqNo};
pub use market::{BookTicker, DepthDelta, DepthSnapshot, MarketEvent, PriceLevel, Side, Trade};
pub use mode::EngineMode;
pub use symbol::Symbol;
pub use time::Nanos;

/// The common glob-import surface. `use core_types::prelude::*;` brings the
/// everyday types into scope without pulling in module paths.
pub mod prelude {
    pub use crate::asset::{Asset, RawSymbol};
    pub use crate::book::{BookStatus, BookUpdate};
    pub use crate::decimal::Decimal;
    pub use crate::error::{CoreError, Result};
    pub use crate::filters::SymbolFilters;
    pub use crate::ids::{ClientOrderId, OrderId, SeqNo};
    pub use crate::market::{
        BookTicker, DepthDelta, DepthSnapshot, MarketEvent, PriceLevel, Side, Trade,
    };
    pub use crate::mode::EngineMode;
    pub use crate::symbol::Symbol;
    pub use crate::time::Nanos;
}
