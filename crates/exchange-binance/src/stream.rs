//! Combined-stream WebSocket client that emits normalized [`MarketEvent`]s.
//!
//! A background task owns the socket: it connects, reads frames, normalizes
//! them, and forwards them over a bounded channel, reconnecting with jittered
//! exponential backoff on any drop. Backpressure is intentional: if the consumer
//! falls behind, the channel fills and the reader slows, rather than growing an
//! unbounded queue.
//!
//! Subscription model: Binance combined streams are addressed by URL, so this
//! adapter takes its subscriptions before connecting. Call [`MarketFeed::subscribe`]
//! (or construct with [`BinanceMarketStream::start`]) and then
//! [`MarketFeed::connect`]. Dynamic resubscription after connect is not yet
//! implemented and is refused explicitly rather than silently ignored.

use std::sync::Arc;

use core_types::{MarketEvent, Nanos};
use exchange_common::{ExchangeError, MarketFeed, StreamKind, Subscription};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::models::CombinedFrame;
use crate::normalize::normalize_frame;
use crate::symbols::SymbolTable;

type FeedResult<T> = core::result::Result<T, ExchangeError>;

/// Default public combined-stream host.
pub const MAINNET_WS: &str = "wss://stream.binance.com:9443";
/// Public market-data mirror host.
pub const DATA_WS: &str = "wss://data-stream.binance.vision";

/// Channel capacity between the reader task and the consumer.
const CHANNEL_CAPACITY: usize = 4096;

/// A Binance combined-stream market feed.
pub struct BinanceMarketStream {
    ws_base: String,
    subs: Vec<Subscription>,
    table: Arc<SymbolTable>,
    rx: Option<mpsc::Receiver<FeedResult<MarketEvent>>>,
    handle: Option<JoinHandle<()>>,
}

impl BinanceMarketStream {
    /// Creates a feed against `ws_base` (for example [`MAINNET_WS`]) with no
    /// subscriptions yet.
    #[must_use]
    pub fn new(ws_base: impl Into<String>, table: Arc<SymbolTable>) -> BinanceMarketStream {
        BinanceMarketStream {
            ws_base: ws_base.into(),
            subs: Vec::new(),
            table,
            rx: None,
            handle: None,
        }
    }

    /// Convenience: build with subscriptions and connect immediately.
    pub fn start(
        ws_base: impl Into<String>,
        subs: Vec<Subscription>,
        table: Arc<SymbolTable>,
    ) -> FeedResult<BinanceMarketStream> {
        let mut s = BinanceMarketStream::new(ws_base, table);
        s.subs = subs;
        s.spawn()?;
        Ok(s)
    }

    /// Whether the reader task has been spawned.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    /// The combined-stream names for the current subscriptions, for example
    /// `["btcusdt@bookTicker", "btcusdt@depth@100ms"]`.
    #[must_use]
    pub fn stream_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for sub in &self.subs {
            let lower = sub.symbol.as_str().to_ascii_lowercase();
            for kind in &sub.kinds {
                names.push(format!("{lower}{}", stream_suffix(*kind)));
            }
        }
        names
    }

    fn combined_url(&self) -> FeedResult<String> {
        let names = self.stream_names();
        if names.is_empty() {
            return Err(ExchangeError::Protocol(
                "no subscriptions; call subscribe before connect".to_owned(),
            ));
        }
        Ok(format!(
            "{}/stream?streams={}",
            self.ws_base,
            names.join("/")
        ))
    }

    fn spawn(&mut self) -> FeedResult<()> {
        if self.handle.is_some() {
            return Ok(());
        }
        let url = self.combined_url()?;
        let table = Arc::clone(&self.table);
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let handle = tokio::spawn(run(url, table, tx));
        self.rx = Some(rx);
        self.handle = Some(handle);
        Ok(())
    }
}

impl Drop for BinanceMarketStream {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

impl MarketFeed for BinanceMarketStream {
    async fn connect(&mut self) -> FeedResult<()> {
        self.spawn()
    }

    async fn subscribe(&mut self, subs: &[Subscription]) -> FeedResult<()> {
        if self.is_running() {
            return Err(ExchangeError::Unsupported(
                "dynamic resubscribe after connect; subscribe before connect",
            ));
        }
        self.subs.extend(subs.iter().cloned());
        Ok(())
    }

    async fn next_event(&mut self) -> Option<FeedResult<MarketEvent>> {
        match &mut self.rx {
            Some(rx) => rx.recv().await,
            None => None,
        }
    }
}

/// The suffix appended to a lowercase symbol for a stream kind.
fn stream_suffix(kind: StreamKind) -> &'static str {
    match kind {
        StreamKind::BookTicker => "@bookTicker",
        StreamKind::Depth => "@depth@100ms",
        StreamKind::Trades => "@trade",
    }
}

/// The reader task: connect, read, normalize, forward, reconnect on drop.
async fn run(url: String, table: Arc<SymbolTable>, tx: mpsc::Sender<FeedResult<MarketEvent>>) {
    let mut backoff = crate::backoff::Backoff::connector_default();
    loop {
        match connect_async(url.as_str()).await {
            Ok((ws, _resp)) => {
                backoff.reset();
                tracing::info!("binance combined stream connected");
                let (mut write, mut read) = ws.split();
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let recv_ts = Nanos::now();
                            let event = match serde_json::from_str::<CombinedFrame>(text.as_str()) {
                                Ok(frame) => normalize_frame(&frame, &table, recv_ts),
                                Err(e) => Err(ExchangeError::Parse(e.to_string())),
                            };
                            if tx.send(event).await.is_err() {
                                // Consumer dropped; shut the task down.
                                return;
                            }
                        }
                        Ok(Message::Ping(payload)) => {
                            let _ = write.send(Message::Pong(payload)).await;
                        }
                        Ok(Message::Close(_)) => {
                            tracing::warn!("binance stream closed by server");
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            let _ = tx.send(Err(ExchangeError::Connection(e.to_string()))).await;
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                if tx
                    .send(Err(ExchangeError::Connection(e.to_string())))
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }

        let delay = backoff.next_delay();
        tracing::info!(
            delay_ms = delay.as_millis() as u64,
            "reconnecting after backoff"
        );
        tokio::time::sleep(delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::Symbol;

    fn table() -> Arc<SymbolTable> {
        Arc::new(SymbolTable::default())
    }

    #[test]
    fn builds_stream_names() {
        let mut s = BinanceMarketStream::new(MAINNET_WS, table());
        let sym = Symbol::from_parts("BTC", "USDT").unwrap();
        s.subs.push(Subscription {
            symbol: sym,
            kinds: vec![StreamKind::BookTicker, StreamKind::Depth],
        });
        assert_eq!(
            s.stream_names(),
            vec![
                "btcusdt@bookTicker".to_owned(),
                "btcusdt@depth@100ms".to_owned()
            ]
        );
    }

    #[test]
    fn combined_url_requires_subscriptions() {
        let s = BinanceMarketStream::new(MAINNET_WS, table());
        assert!(s.combined_url().is_err());
    }

    #[test]
    fn combined_url_format() {
        let mut s = BinanceMarketStream::new(MAINNET_WS, table());
        s.subs.push(Subscription::single(
            Symbol::from_parts("ETH", "BTC").unwrap(),
            StreamKind::Trades,
        ));
        assert_eq!(
            s.combined_url().unwrap(),
            "wss://stream.binance.com:9443/stream?streams=ethbtc@trade"
        );
    }
}
