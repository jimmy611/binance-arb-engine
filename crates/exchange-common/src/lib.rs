//! Exchange-agnostic abstractions shared by every venue adapter.
//!
//! This crate will host the `ExchangeAdapter` trait (connect, subscribe,
//! next_event, submit_order, cancel_order, fee_schedule, symbol_filters) and
//! the shared normalizers that turn venue-specific payloads into the common
//! [`core_types::MarketEvent`] enum.
//!
//! Deliberately empty at milestone M0. The trait is introduced at M2 alongside
//! the first concrete adapter (`exchange-binance`), so its shape is driven by a
//! real implementation rather than guessed in the abstract.
#![forbid(unsafe_code)]
