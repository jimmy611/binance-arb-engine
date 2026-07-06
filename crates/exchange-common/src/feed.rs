//! The read-only market-data abstraction.
//!
//! This is the first slice of the `ExchangeAdapter` surface from the
//! specification: everything needed to receive normalized market data. Order
//! submission (`submit_order`, `cancel_order`) is a separate capability added at
//! the execution milestones so that a read-only build never even links order
//! code. Keeping the trait shaped by a real implementation (`exchange-binance`)
//! is why it is introduced here rather than guessed at M0.
//!
//! The methods return `impl Future + Send` explicitly rather than using
//! `async fn` in the trait, both to keep the futures `Send` (they are spawned
//! onto a multi-threaded runtime) and to avoid the `async_fn_in_trait` lint.

use core::future::Future;

use core_types::{MarketEvent, Symbol};

use crate::error::Result;

/// Which Binance-style stream a subscription wants for a symbol. An adapter maps
/// these to its venue's concrete channel names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StreamKind {
    /// Best bid/ask on every change (lowest latency top-of-book).
    BookTicker,
    /// Incremental depth diffs at the venue's fastest cadence.
    Depth,
    /// Public trade prints.
    Trades,
}

/// A request to receive a set of stream kinds for a symbol.
#[derive(Clone, Debug)]
pub struct Subscription {
    /// The symbol to subscribe.
    pub symbol: Symbol,
    /// The stream kinds wanted for that symbol.
    pub kinds: Vec<StreamKind>,
}

impl Subscription {
    /// A subscription for a single symbol and stream kind.
    #[must_use]
    pub fn single(symbol: Symbol, kind: StreamKind) -> Subscription {
        Subscription {
            symbol,
            kinds: vec![kind],
        }
    }
}

/// A source of normalized [`MarketEvent`]s from one venue.
///
/// The typical lifecycle is `connect`, then `subscribe`, then repeated
/// `next_event` until it returns `None` (the feed has terminated). An adapter is
/// expected to handle reconnection internally; a returned `Err` from
/// `next_event` is a reportable event (a gap, a parse failure) rather than
/// necessarily the end of the stream.
pub trait MarketFeed: Send {
    /// Establishes the underlying transport.
    fn connect(&mut self) -> impl Future<Output = Result<()>> + Send;

    /// Subscribes to the given streams. May be called after `connect` and again
    /// later to add symbols.
    fn subscribe(&mut self, subs: &[Subscription]) -> impl Future<Output = Result<()>> + Send;

    /// Yields the next normalized event, `None` when the feed has ended.
    fn next_event(&mut self) -> impl Future<Output = Option<Result<MarketEvent>>> + Send;
}
