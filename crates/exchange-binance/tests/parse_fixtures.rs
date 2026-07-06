//! Integration tests that replay recorded Binance payloads through the public
//! parsing and normalization surface. These are fully offline and
//! deterministic: they read committed fixtures captured from live Binance, so
//! the adapter's parser is exercised against real wire shapes without any
//! network dependency in CI.

use core_types::MarketEvent;
use core_types::Nanos;
use exchange_binance::models::{CombinedFrame, ExchangeInfoRaw};
use exchange_binance::normalize::normalize_frame;
use exchange_binance::symbols::SymbolTable;

const TRIANGLE: &str = include_str!("fixtures/exchange_info_triangle.json");
const WS_FRAMES: &str = include_str!("fixtures/ws_frames.jsonl");

fn table() -> SymbolTable {
    let info: ExchangeInfoRaw = serde_json::from_str(TRIANGLE).expect("exchangeInfo parses");
    SymbolTable::from_exchange_info(&info).expect("table builds")
}

#[test]
fn every_recorded_frame_normalizes() {
    let t = table();
    let mut book_tickers = 0usize;
    let mut depth_diffs = 0usize;
    let mut trades = 0usize;
    let mut total = 0usize;

    for (i, line) in WS_FRAMES.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let frame: CombinedFrame =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("line {i} is not a frame: {e}"));
        let event = normalize_frame(&frame, &t, Nanos::now())
            .unwrap_or_else(|e| panic!("line {i} failed to normalize: {e}"));
        total += 1;
        match event {
            MarketEvent::BookTicker(_) => book_tickers += 1,
            MarketEvent::DepthDelta(_) => depth_diffs += 1,
            MarketEvent::Trade(_) => trades += 1,
            MarketEvent::DepthSnapshot(_) => {}
        }
    }

    // The captured window was book-ticker heavy with some depth diffs.
    assert!(total >= 50, "expected a substantial capture, got {total}");
    assert!(book_tickers > 0, "expected book tickers in the capture");
    assert!(depth_diffs > 0, "expected at least one depth diff");
    let _ = trades; // trades may not appear in a short window; parsing is unit-tested.
}

#[test]
fn book_ticker_values_are_coherent() {
    let t = table();
    for line in WS_FRAMES.lines() {
        if !line.contains("@bookTicker") {
            continue;
        }
        let frame: CombinedFrame = serde_json::from_str(line).unwrap();
        if let MarketEvent::BookTicker(b) = normalize_frame(&frame, &t, Nanos::now()).unwrap() {
            // A real top-of-book is never crossed and quantities are positive.
            assert!(b.bid_price < b.ask_price, "book ticker crossed: {b:?}");
            assert!(b.bid_qty.is_positive() && b.ask_qty.is_positive());
        }
    }
}

/// Live smoke test against Binance public endpoints. Ignored by default so CI
/// stays offline; run manually with `cargo test -p exchange-binance -- --ignored`.
#[tokio::test]
#[ignore = "hits live Binance public endpoints"]
async fn live_smoke() {
    use exchange_binance::{BinanceMarketStream, BinanceRest, MAINNET_WS};
    use exchange_common::{MarketFeed, StreamKind, Subscription};
    use std::sync::Arc;
    use std::time::Duration;

    let rest = BinanceRest::mainnet().unwrap();
    let info = rest
        .exchange_info(Some(&["BTCUSDT", "ETHUSDT", "ETHBTC"]))
        .await
        .expect("exchangeInfo");
    let table = Arc::new(SymbolTable::from_exchange_info(&info).unwrap());
    let sym = table.resolve("BTCUSDT").unwrap();

    let mut feed = BinanceMarketStream::new(MAINNET_WS, table);
    feed.subscribe(&[Subscription::single(sym, StreamKind::BookTicker)])
        .await
        .unwrap();
    feed.connect().await.unwrap();

    let ev = tokio::time::timeout(Duration::from_secs(10), feed.next_event())
        .await
        .expect("received an event within 10s");
    assert!(ev.is_some(), "feed yielded an event");
    assert!(
        ev.unwrap().is_ok(),
        "first event should be a valid market event"
    );
}
