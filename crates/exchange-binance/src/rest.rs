//! Binance REST client for `exchangeInfo` and depth snapshots.
//!
//! Only the read-only endpoints needed for market data live here. Every request
//! passes the REST-weight token bucket first, so the client cannot exceed the
//! self-imposed 80% weight ceiling. The signed order endpoints are added at the
//! execution milestone in a separate module.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use exchange_common::ExchangeError;
use serde::de::DeserializeOwned;

use crate::models::{DepthSnapshotRaw, ExchangeInfoRaw};
use crate::ratelimit::RestRateLimiter;

type Result<T> = core::result::Result<T, ExchangeError>;

/// Binance's public REST base URL.
pub const MAINNET_REST: &str = "https://api.binance.com";
/// The public market-data mirror, useful where the main host is restricted.
pub const DATA_REST: &str = "https://data-api.binance.vision";

/// A read-only Binance REST client.
#[derive(Debug)]
pub struct BinanceRest {
    http: reqwest::Client,
    base: String,
    limiter: Mutex<RestRateLimiter>,
    start: Instant,
}

impl BinanceRest {
    /// Builds a client against `base_url` (for example [`MAINNET_REST`]).
    pub fn new(base_url: impl Into<String>) -> Result<BinanceRest> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("arb-engine/0.1 (read-only market data)")
            .build()
            .map_err(|e| ExchangeError::Connection(e.to_string()))?;
        Ok(BinanceRest {
            http,
            base: base_url.into(),
            limiter: Mutex::new(RestRateLimiter::binance_default()),
            start: Instant::now(),
        })
    }

    /// Client against the main public host.
    pub fn mainnet() -> Result<BinanceRest> {
        BinanceRest::new(MAINNET_REST)
    }

    /// Fetches `exchangeInfo`, optionally restricted to specific raw symbols.
    pub async fn exchange_info(&self, symbols: Option<&[&str]>) -> Result<ExchangeInfoRaw> {
        let path = "/api/v3/exchangeInfo";
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(syms) = symbols {
            // Binance expects a JSON array string, e.g. ["BTCUSDT","ETHUSDT"].
            let arr = syms
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(",");
            query.push(("symbols", format!("[{arr}]")));
        }
        self.get_json(path, &query, 20).await
    }

    /// Fetches a REST depth snapshot for a symbol. `limit` is clamped to
    /// Binance's accepted set at the call site by the caller; weight scales with
    /// depth.
    pub async fn depth_snapshot(&self, symbol: &str, limit: u16) -> Result<DepthSnapshotRaw> {
        let weight = depth_weight(limit);
        let limit_s = limit.to_string();
        let query = [("symbol", symbol.to_owned()), ("limit", limit_s)];
        self.get_json("/api/v3/depth", &query, weight).await
    }

    /// Server time in milliseconds (weight 1). Useful for clock-drift checks.
    pub async fn server_time(&self) -> Result<u64> {
        #[derive(serde::Deserialize)]
        struct ServerTime {
            #[serde(rename = "serverTime")]
            server_time: u64,
        }
        let t: ServerTime = self.get_json("/api/v3/time", &[], 1).await?;
        Ok(t.server_time)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        weight: u32,
    ) -> Result<T> {
        // Reserve weight before sending; drop the guard before awaiting so the
        // std mutex is never held across an await point.
        {
            let now = self.start.elapsed().as_secs_f64();
            let mut lim = self
                .limiter
                .lock()
                .map_err(|_| ExchangeError::Connection("rate limiter poisoned".to_owned()))?;
            if !lim.try_consume(now, weight) {
                return Err(ExchangeError::RateLimited {
                    retry_after_ms: None,
                });
            }
        }

        let url = format!("{}{}", self.base, path);
        let resp = self.http.get(&url).query(query).send().await.map_err(|e| {
            if e.is_timeout() {
                ExchangeError::Timeout
            } else {
                ExchangeError::Connection(e.to_string())
            }
        })?;

        let status = resp.status();
        if !status.is_success() {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            let truncated: String = body.chars().take(256).collect();
            // 429 (rate limit) and 418 (IP ban) are surfaced as RateLimited.
            if code == 429 || code == 418 {
                return Err(ExchangeError::RateLimited {
                    retry_after_ms: None,
                });
            }
            return Err(ExchangeError::Http {
                status: code,
                body: truncated,
            });
        }

        resp.json::<T>()
            .await
            .map_err(|e| ExchangeError::Parse(e.to_string()))
    }
}

/// REST weight for a depth request of the given limit (Binance's tiered
/// schedule).
fn depth_weight(limit: u16) -> u32 {
    match limit {
        0..=100 => 5,
        101..=500 => 25,
        501..=1000 => 50,
        _ => 250,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_weight_tiers() {
        assert_eq!(depth_weight(20), 5);
        assert_eq!(depth_weight(100), 5);
        assert_eq!(depth_weight(500), 25);
        assert_eq!(depth_weight(1000), 50);
        assert_eq!(depth_weight(5000), 250);
    }

    #[test]
    fn constructs_client() {
        assert!(BinanceRest::mainnet().is_ok());
        assert!(BinanceRest::new(DATA_REST).is_ok());
    }
}
