//! Pure normalization: Binance wire payloads into [`core_types::MarketEvent`].
//!
//! Everything here is synchronous and IO-free, which is what makes it fully
//! testable against recorded fixtures. The WebSocket task and REST client are
//! thin wrappers that feed these functions.

use core_types::{
    BookTicker, Decimal, DepthDelta, DepthSnapshot, MarketEvent, Nanos, PriceLevel, Symbol, Trade,
};
use exchange_common::ExchangeError;

use crate::models::{BookTickerRaw, CombinedFrame, DepthDiffRaw, DepthSnapshotRaw, TradeRaw};
use crate::symbols::SymbolTable;

type Result<T> = core::result::Result<T, ExchangeError>;

/// Normalizes a combined-stream frame into a [`MarketEvent`], stamping it with
/// the local receive time `recv_ts`.
pub fn normalize_frame(
    frame: &CombinedFrame,
    table: &SymbolTable,
    recv_ts: Nanos,
) -> Result<MarketEvent> {
    match stream_kind(&frame.stream)? {
        Kind::BookTicker => {
            let raw: BookTickerRaw = serde_json::from_value(frame.data.clone()).map_err(parse)?;
            normalize_book_ticker(&raw, table, recv_ts)
        }
        Kind::Depth => {
            let raw: DepthDiffRaw = serde_json::from_value(frame.data.clone()).map_err(parse)?;
            normalize_depth_diff(&raw, table, recv_ts)
        }
        Kind::Trade => {
            let raw: TradeRaw = serde_json::from_value(frame.data.clone()).map_err(parse)?;
            normalize_trade(&raw, table, recv_ts)
        }
    }
}

enum Kind {
    BookTicker,
    Depth,
    Trade,
}

/// Determines the stream kind from the combined-stream name. Names look like
/// `btcusdt@bookTicker`, `btcusdt@depth@100ms`, or `btcusdt@trade`; the kind is
/// the token immediately after the first `@`.
fn stream_kind(stream: &str) -> Result<Kind> {
    match stream.split('@').nth(1).unwrap_or("") {
        "bookTicker" => Ok(Kind::BookTicker),
        "depth" => Ok(Kind::Depth),
        "trade" => Ok(Kind::Trade),
        other => Err(ExchangeError::Protocol(format!(
            "unknown stream kind {other:?} in {stream:?}"
        ))),
    }
}

/// Normalizes a `@bookTicker` payload.
pub fn normalize_book_ticker(
    raw: &BookTickerRaw,
    table: &SymbolTable,
    ts: Nanos,
) -> Result<MarketEvent> {
    let symbol = resolve(table, &raw.symbol)?;
    Ok(MarketEvent::BookTicker(BookTicker {
        symbol,
        update_id: raw.update_id,
        bid_price: dec(&raw.bid_price)?,
        bid_qty: dec(&raw.bid_qty)?,
        ask_price: dec(&raw.ask_price)?,
        ask_qty: dec(&raw.ask_qty)?,
        ts,
    }))
}

/// Normalizes a `@depth` diff payload.
pub fn normalize_depth_diff(
    raw: &DepthDiffRaw,
    table: &SymbolTable,
    ts: Nanos,
) -> Result<MarketEvent> {
    let symbol = resolve(table, &raw.symbol)?;
    Ok(MarketEvent::DepthDelta(DepthDelta {
        symbol,
        first_update_id: raw.first_update_id,
        final_update_id: raw.final_update_id,
        bids: levels(&raw.bids)?,
        asks: levels(&raw.asks)?,
        ts,
    }))
}

/// Normalizes a `@trade` payload.
pub fn normalize_trade(raw: &TradeRaw, table: &SymbolTable, ts: Nanos) -> Result<MarketEvent> {
    let symbol = resolve(table, &raw.symbol)?;
    Ok(MarketEvent::Trade(Trade {
        symbol,
        trade_id: raw.trade_id,
        price: dec(&raw.price)?,
        qty: dec(&raw.qty)?,
        buyer_is_maker: raw.buyer_is_maker,
        ts,
    }))
}

/// Normalizes a REST depth snapshot for a known symbol.
pub fn normalize_depth_snapshot(
    raw: &DepthSnapshotRaw,
    symbol: Symbol,
    ts: Nanos,
) -> Result<DepthSnapshot> {
    Ok(DepthSnapshot {
        symbol,
        last_update_id: raw.last_update_id,
        bids: levels(&raw.bids)?,
        asks: levels(&raw.asks)?,
        ts,
    })
}

fn resolve(table: &SymbolTable, raw_symbol: &str) -> Result<Symbol> {
    table
        .resolve(raw_symbol)
        .ok_or_else(|| ExchangeError::Protocol(format!("unknown symbol {raw_symbol:?}")))
}

fn dec(s: &str) -> Result<Decimal> {
    Decimal::from_str_exact(s).map_err(|e| ExchangeError::Parse(format!("decimal {s:?}: {e}")))
}

fn levels(pairs: &[[String; 2]]) -> Result<Vec<PriceLevel>> {
    pairs
        .iter()
        .map(|[p, q]| Ok(PriceLevel::new(dec(p)?, dec(q)?)))
        .collect()
}

fn parse(e: serde_json::Error) -> ExchangeError {
    ExchangeError::Parse(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ExchangeInfoRaw;

    const TRIANGLE: &str = include_str!("../tests/fixtures/exchange_info_triangle.json");

    fn table() -> SymbolTable {
        let info: ExchangeInfoRaw = serde_json::from_str(TRIANGLE).unwrap();
        SymbolTable::from_exchange_info(&info).unwrap()
    }

    #[test]
    fn normalizes_book_ticker_frame() {
        let t = table();
        let txt = r#"{"stream":"btcusdt@bookTicker","data":{"u":1,"s":"BTCUSDT","b":"63180.01000000","B":"1.53208000","a":"63180.02000000","A":"2.07532000"}}"#;
        let frame: CombinedFrame = serde_json::from_str(txt).unwrap();
        let ev = normalize_frame(&frame, &t, Nanos(42)).unwrap();
        match ev {
            MarketEvent::BookTicker(b) => {
                assert_eq!(b.symbol.as_str(), "BTCUSDT");
                assert_eq!(b.bid_price, Decimal::from_str_exact("63180.01").unwrap());
                assert_eq!(b.ask_qty, Decimal::from_str_exact("2.07532").unwrap());
                assert_eq!(b.ts, Nanos(42));
            }
            other => panic!("expected book ticker, got {other:?}"),
        }
    }

    #[test]
    fn normalizes_depth_diff_frame() {
        let t = table();
        let txt = r#"{"stream":"ethbtc@depth@100ms","data":{"e":"depthUpdate","E":1783315197300,"s":"ETHBTC","U":9062179581,"u":9062179596,"b":[["0.02810000","54.55880000"],["0.02809000","0.00000000"]],"a":[["0.02811000","10.0"]]}}"#;
        let frame: CombinedFrame = serde_json::from_str(txt).unwrap();
        match normalize_frame(&frame, &t, Nanos(1)).unwrap() {
            MarketEvent::DepthDelta(d) => {
                assert_eq!(d.symbol.as_str(), "ETHBTC");
                assert_eq!(d.first_update_id, 9_062_179_581);
                assert_eq!(d.final_update_id, 9_062_179_596);
                assert_eq!(d.bids.len(), 2);
                // A zero-quantity level is preserved as a removal instruction.
                assert!(d.bids[1].qty.is_zero());
                assert_eq!(d.asks.len(), 1);
            }
            other => panic!("expected depth delta, got {other:?}"),
        }
    }

    #[test]
    fn normalizes_trade_frame() {
        let t = table();
        let txt = r#"{"stream":"btcusdt@trade","data":{"e":"trade","E":1783315197300,"s":"BTCUSDT","t":12345,"p":"63180.05000000","q":"0.01000000","T":1783315197300,"m":true}}"#;
        let frame: CombinedFrame = serde_json::from_str(txt).unwrap();
        match normalize_frame(&frame, &t, Nanos(7)).unwrap() {
            MarketEvent::Trade(tr) => {
                assert_eq!(tr.trade_id, 12345);
                assert!(tr.buyer_is_maker);
                assert_eq!(tr.price, Decimal::from_str_exact("63180.05").unwrap());
            }
            other => panic!("expected trade, got {other:?}"),
        }
    }

    #[test]
    fn unknown_symbol_is_protocol_error() {
        let t = table();
        let txt = r#"{"stream":"dogeusdt@bookTicker","data":{"u":1,"s":"DOGEUSDT","b":"0.1","B":"1","a":"0.2","A":"1"}}"#;
        let frame: CombinedFrame = serde_json::from_str(txt).unwrap();
        let err = normalize_frame(&frame, &t, Nanos(0)).unwrap_err();
        assert!(matches!(err, ExchangeError::Protocol(_)));
    }

    #[test]
    fn unknown_stream_kind_is_protocol_error() {
        let t = table();
        let txt = r#"{"stream":"btcusdt@kline_1m","data":{}}"#;
        let frame: CombinedFrame = serde_json::from_str(txt).unwrap();
        assert!(matches!(
            normalize_frame(&frame, &t, Nanos(0)).unwrap_err(),
            ExchangeError::Protocol(_)
        ));
    }

    #[test]
    fn normalizes_real_depth_snapshot() {
        let t = table();
        let snap_txt = include_str!("../tests/fixtures/depth_btcusdt_20.json");
        let raw: DepthSnapshotRaw = serde_json::from_str(snap_txt).unwrap();
        let btcusdt = t.resolve("BTCUSDT").unwrap();
        let snap = normalize_depth_snapshot(&raw, btcusdt, Nanos(0)).unwrap();
        assert_eq!(snap.bids.len(), 20);
        assert_eq!(snap.asks.len(), 20);
        // Best bid strictly below best ask (real book is not crossed).
        assert!(snap.bids[0].price < snap.asks[0].price);
    }
}
