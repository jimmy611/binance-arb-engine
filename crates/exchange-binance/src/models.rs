//! Serde models for Binance REST and WebSocket payloads.
//!
//! These mirror Binance's wire shapes exactly and hold prices/quantities as
//! their raw strings; conversion to the exact [`core_types::Decimal`] happens in
//! [`crate::normalize`], so parse failures are attributed to the right stage.
//! Filter arrays are kept as raw JSON values rather than a tagged enum so that
//! Binance adding a new filter type or field never breaks deserialization.

use serde::Deserialize;

/// Top-level `/api/v3/exchangeInfo` response (subset we use).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeInfoRaw {
    /// Account/IP rate limits declared by the venue.
    #[serde(default)]
    pub rate_limits: Vec<RateLimitRaw>,
    /// Per-symbol trading rules.
    pub symbols: Vec<SymbolInfoRaw>,
}

/// One declared rate limit.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitRaw {
    /// `REQUEST_WEIGHT`, `ORDERS`, or `RAW_REQUESTS`.
    pub rate_limit_type: String,
    /// `SECOND`, `MINUTE`, or `DAY`.
    pub interval: String,
    /// Multiplier on the interval unit (for example `10` with `SECOND`).
    pub interval_num: u64,
    /// The limit count within the interval.
    pub limit: u64,
}

/// Per-symbol trading rules.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfoRaw {
    /// Raw symbol id, for example `BTCUSDT`.
    pub symbol: String,
    /// `TRADING`, `HALT`, `BREAK`, and so on.
    pub status: String,
    /// Base asset, for example `BTC`.
    pub base_asset: String,
    /// Quote asset, for example `USDT`.
    pub quote_asset: String,
    /// Filter objects, kept raw for forward compatibility.
    #[serde(default)]
    pub filters: Vec<serde_json::Value>,
}

/// A REST depth snapshot from `/api/v3/depth`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepthSnapshotRaw {
    /// The snapshot's last update id.
    pub last_update_id: u64,
    /// Bid levels as `[price, qty]` string pairs.
    pub bids: Vec<[String; 2]>,
    /// Ask levels as `[price, qty]` string pairs.
    pub asks: Vec<[String; 2]>,
}

/// A combined-stream frame: `{"stream": "...", "data": {...}}`.
#[derive(Debug, Clone, Deserialize)]
pub struct CombinedFrame {
    /// The stream name, for example `btcusdt@bookTicker`.
    pub stream: String,
    /// The inner payload, deserialized per stream kind.
    pub data: serde_json::Value,
}

/// `@bookTicker` payload. Note there is no event-time field on this stream.
#[derive(Debug, Clone, Deserialize)]
pub struct BookTickerRaw {
    /// Order book update id.
    #[serde(rename = "u")]
    pub update_id: u64,
    /// Symbol.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Best bid price.
    #[serde(rename = "b")]
    pub bid_price: String,
    /// Best bid quantity.
    #[serde(rename = "B")]
    pub bid_qty: String,
    /// Best ask price.
    #[serde(rename = "a")]
    pub ask_price: String,
    /// Best ask quantity.
    #[serde(rename = "A")]
    pub ask_qty: String,
}

/// `@depth` diff payload.
#[derive(Debug, Clone, Deserialize)]
pub struct DepthDiffRaw {
    /// Event time (ms).
    #[serde(rename = "E")]
    pub event_time: u64,
    /// Symbol.
    #[serde(rename = "s")]
    pub symbol: String,
    /// First update id in the event (`U`).
    #[serde(rename = "U")]
    pub first_update_id: u64,
    /// Final update id in the event (`u`).
    #[serde(rename = "u")]
    pub final_update_id: u64,
    /// Changed bid levels (`[price, qty]`, qty 0 removes).
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    /// Changed ask levels.
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

/// `@trade` payload.
#[derive(Debug, Clone, Deserialize)]
pub struct TradeRaw {
    /// Event time (ms).
    #[serde(rename = "E")]
    pub event_time: u64,
    /// Symbol.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Trade id.
    #[serde(rename = "t")]
    pub trade_id: u64,
    /// Price.
    #[serde(rename = "p")]
    pub price: String,
    /// Quantity.
    #[serde(rename = "q")]
    pub qty: String,
    /// `true` if the buyer was the maker.
    #[serde(rename = "m")]
    pub buyer_is_maker: bool,
}
