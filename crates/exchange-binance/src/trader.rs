//! The Binance order-execution adapter (live path).
//!
//! Implements [`OrderExecution`] by building a signed order request and POSTing
//! it to Binance's REST order endpoint. This is the real live path, but it is
//! not exercised in tests here: doing so submits real orders and requires
//! registered Ed25519 API keys. The engine constructs this only in live mode,
//! behind the signed flag and after a risk approval; the execution state machine
//! in the `execution` crate is what is unit-tested (against a mock).
//!
//! The order parameters are signed exactly as sent: the signature covers the
//! canonical query string, and the base64 signature is URL-encoded before being
//! appended.

use core_types::{Decimal, OrderId, Symbol};
use exchange_common::{
    ExchangeError, OrderExecution, OrderRequest, OrderResponse, OrderStatus, OrderType,
    Result as ExResult,
};
use serde::Deserialize;

use crate::auth::Ed25519Signer;

const ORDER_PATH: &str = "/api/v3/order";

/// A live Binance trading client.
pub struct BinanceTrader {
    http: reqwest::Client,
    base: String,
    api_key: String,
    signer: Ed25519Signer,
    recv_window_ms: u64,
}

impl BinanceTrader {
    /// Builds a trader against `base_url` with the given API key and Ed25519
    /// signer.
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        signer: Ed25519Signer,
    ) -> ExResult<BinanceTrader> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| ExchangeError::Connection(e.to_string()))?;
        Ok(BinanceTrader {
            http,
            base: base_url.into(),
            api_key: api_key.into(),
            signer,
            recv_window_ms: 5000,
        })
    }

    fn now_ms() -> u64 {
        core_types::Nanos::now().as_millis()
    }

    /// Builds the signed query string for a set of parameters.
    fn signed_query(&self, params: &[(&str, String)]) -> String {
        let query = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        let signature = url_encode(&self.signer.sign_query(&query));
        format!("{query}&signature={signature}")
    }

    async fn signed_request(&self, method: reqwest::Method, query: String) -> ExResult<String> {
        let url = format!("{}{ORDER_PATH}?{query}", self.base);
        let resp = self
            .http
            .request(method, &url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ExchangeError::Timeout
                } else {
                    ExchangeError::Connection(e.to_string())
                }
            })?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            let code = status.as_u16();
            if code == 429 || code == 418 {
                return Err(ExchangeError::RateLimited {
                    retry_after_ms: None,
                });
            }
            return Err(ExchangeError::Http {
                status: code,
                body: body.chars().take(256).collect(),
            });
        }
        Ok(body)
    }
}

impl OrderExecution for BinanceTrader {
    async fn submit_order(&self, request: &OrderRequest) -> ExResult<OrderResponse> {
        let mut params: Vec<(&str, String)> = vec![
            ("symbol", request.symbol.as_str().to_owned()),
            ("side", side_str(request.side).to_owned()),
        ];
        match request.order_type {
            OrderType::LimitIoc => {
                params.push(("type", "LIMIT".to_owned()));
                params.push(("timeInForce", "IOC".to_owned()));
                if let Some(price) = request.price {
                    params.push(("price", price.to_string()));
                }
            }
            OrderType::Market => params.push(("type", "MARKET".to_owned())),
        }
        params.push(("quantity", request.qty.to_string()));
        params.push(("newClientOrderId", request.client_order_id.0.clone()));
        params.push(("newOrderRespType", "RESULT".to_owned()));
        params.push(("recvWindow", self.recv_window_ms.to_string()));
        params.push(("timestamp", Self::now_ms().to_string()));

        let query = self.signed_query(&params);
        let body = self.signed_request(reqwest::Method::POST, query).await?;
        let raw: OrderResponseRaw =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Parse(e.to_string()))?;
        Ok(raw.into_response())
    }

    async fn cancel_order(&self, symbol: Symbol, order_id: OrderId) -> ExResult<()> {
        let params: Vec<(&str, String)> = vec![
            ("symbol", symbol.as_str().to_owned()),
            ("orderId", order_id.0.to_string()),
            ("recvWindow", self.recv_window_ms.to_string()),
            ("timestamp", Self::now_ms().to_string()),
        ];
        let query = self.signed_query(&params);
        self.signed_request(reqwest::Method::DELETE, query).await?;
        Ok(())
    }
}

fn side_str(side: core_types::Side) -> &'static str {
    match side {
        core_types::Side::Buy => "BUY",
        core_types::Side::Sell => "SELL",
    }
}

/// URL-encodes the characters that appear in base64 output.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '+' => out.push_str("%2B"),
            '/' => out.push_str("%2F"),
            '=' => out.push_str("%3D"),
            other => out.push(other),
        }
    }
    out
}

/// Binance order response (subset).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponseRaw {
    order_id: i64,
    client_order_id: String,
    status: String,
    #[serde(default)]
    executed_qty: String,
    #[serde(default)]
    cummulative_quote_qty: String,
}

impl OrderResponseRaw {
    fn into_response(self) -> OrderResponse {
        let filled_qty = Decimal::from_str_exact(&self.executed_qty).unwrap_or(Decimal::ZERO);
        let cumulative_quote =
            Decimal::from_str_exact(&self.cummulative_quote_qty).unwrap_or(Decimal::ZERO);
        let avg_price = if filled_qty.is_positive() {
            cumulative_quote.checked_div(filled_qty)
        } else {
            None
        };
        OrderResponse {
            order_id: OrderId(self.order_id),
            client_order_id: core_types::ClientOrderId(self.client_order_id),
            status: map_status(&self.status),
            filled_qty,
            cumulative_quote,
            avg_price,
        }
    }
}

fn map_status(s: &str) -> OrderStatus {
    match s {
        "NEW" => OrderStatus::New,
        "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
        "FILLED" => OrderStatus::Filled,
        "CANCELED" | "PENDING_CANCEL" => OrderStatus::Canceled,
        "EXPIRED" | "EXPIRED_IN_MATCH" => OrderStatus::Expired,
        _ => OrderStatus::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encodes_base64_chars() {
        assert_eq!(url_encode("ab+/c="), "ab%2B%2Fc%3D");
        assert_eq!(url_encode("plain"), "plain");
    }

    #[test]
    fn maps_status_strings() {
        assert_eq!(map_status("FILLED"), OrderStatus::Filled);
        assert_eq!(map_status("PARTIALLY_FILLED"), OrderStatus::PartiallyFilled);
        assert_eq!(map_status("EXPIRED_IN_MATCH"), OrderStatus::Expired);
        assert_eq!(map_status("SOMETHING_ELSE"), OrderStatus::Rejected);
    }

    #[test]
    fn parses_order_response() {
        let json = r#"{"symbol":"BTCUSDT","orderId":123,"clientOrderId":"c-1","status":"FILLED","executedQty":"0.01000000","cummulativeQuoteQty":"600.00000000"}"#;
        let raw: OrderResponseRaw = serde_json::from_str(json).unwrap();
        let resp = raw.into_response();
        assert_eq!(resp.order_id, OrderId(123));
        assert_eq!(resp.status, OrderStatus::Filled);
        assert_eq!(resp.filled_qty, Decimal::from_str_exact("0.01").unwrap());
        assert_eq!(resp.avg_price, Some(Decimal::from_int(60000)));
    }
}
