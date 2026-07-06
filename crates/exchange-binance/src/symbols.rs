//! The symbol table built from `exchangeInfo`.
//!
//! Binance wire payloads identify a market only by its raw id (`BTCUSDT`), which
//! cannot be split into base and quote without metadata. This table resolves a
//! raw id to a fully-formed [`Symbol`] and its [`SymbolFilters`], and is the
//! single source both the normalizer and the evaluator consult.

use std::collections::HashMap;

use core_types::{Asset, Decimal, RawSymbol, Symbol, SymbolFilters};
use exchange_common::ExchangeError;

use crate::models::{ExchangeInfoRaw, SymbolInfoRaw};

/// One symbol's resolved identity and trading rules.
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    /// The fully-formed symbol.
    pub symbol: Symbol,
    /// Its price/quantity filters.
    pub filters: SymbolFilters,
    /// Whether the symbol is currently `TRADING` (excluded from arbitrage
    /// otherwise).
    pub trading: bool,
}

/// A lookup from raw symbol id to resolved [`SymbolEntry`].
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    by_raw: HashMap<RawSymbol, SymbolEntry>,
}

impl SymbolTable {
    /// Builds a table from an `exchangeInfo` response. Symbols whose identity
    /// cannot be parsed (an over-long asset ticker, an invalid character) are
    /// logged and skipped rather than failing the whole table. Returns an error
    /// only if nothing usable remains.
    pub fn from_exchange_info(info: &ExchangeInfoRaw) -> Result<SymbolTable, ExchangeError> {
        let mut by_raw = HashMap::with_capacity(info.symbols.len());
        for s in &info.symbols {
            match build_entry(s) {
                Ok(entry) => {
                    by_raw.insert(entry.symbol.raw(), entry);
                }
                Err(e) => {
                    tracing::warn!(symbol = %s.symbol, error = %e, "skipping unparseable symbol");
                }
            }
        }
        if by_raw.is_empty() {
            return Err(ExchangeError::Parse(
                "exchangeInfo produced no usable symbols".to_owned(),
            ));
        }
        Ok(SymbolTable { by_raw })
    }

    /// Resolves an uppercase raw symbol id to a [`Symbol`].
    #[must_use]
    pub fn resolve(&self, raw_upper: &str) -> Option<Symbol> {
        let key = RawSymbol::new(raw_upper).ok()?;
        self.by_raw.get(&key).map(|e| e.symbol)
    }

    /// Looks up the full entry for a symbol.
    #[must_use]
    pub fn entry(&self, symbol: &Symbol) -> Option<&SymbolEntry> {
        self.by_raw.get(&symbol.raw())
    }

    /// The filters for a symbol, if known.
    #[must_use]
    pub fn filters(&self, symbol: &Symbol) -> Option<SymbolFilters> {
        self.entry(symbol).map(|e| e.filters)
    }

    /// Number of symbols in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_raw.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_raw.is_empty()
    }

    /// Iterates over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &SymbolEntry> {
        self.by_raw.values()
    }
}

fn build_entry(s: &SymbolInfoRaw) -> Result<SymbolEntry, ExchangeError> {
    let base = Asset::new(&s.base_asset)
        .map_err(|e| ExchangeError::Parse(format!("base {}: {e}", s.base_asset)))?;
    let quote = Asset::new(&s.quote_asset)
        .map_err(|e| ExchangeError::Parse(format!("quote {}: {e}", s.quote_asset)))?;
    let raw = RawSymbol::new(&s.symbol)
        .map_err(|e| ExchangeError::Parse(format!("raw {}: {e}", s.symbol)))?;
    Ok(SymbolEntry {
        symbol: Symbol::new(base, quote, raw),
        filters: extract_filters(&s.filters),
        trading: s.status == "TRADING",
    })
}

/// Extracts the price/quantity filters we care about from Binance's raw filter
/// array. Unknown filter types are ignored; missing fields default to zero,
/// which [`SymbolFilters`] treats as "no constraint".
fn extract_filters(values: &[serde_json::Value]) -> SymbolFilters {
    let mut f = SymbolFilters {
        tick_size: Decimal::ZERO,
        step_size: Decimal::ZERO,
        min_qty: Decimal::ZERO,
        max_qty: Decimal::ZERO,
        min_notional: Decimal::ZERO,
    };
    for v in values {
        let ft = v
            .get("filterType")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        match ft {
            "PRICE_FILTER" => {
                if let Some(t) = dec_field(v, "tickSize") {
                    f.tick_size = t;
                }
            }
            "LOT_SIZE" => {
                if let Some(x) = dec_field(v, "minQty") {
                    f.min_qty = x;
                }
                if let Some(x) = dec_field(v, "maxQty") {
                    f.max_qty = x;
                }
                if let Some(x) = dec_field(v, "stepSize") {
                    f.step_size = x;
                }
            }
            "NOTIONAL" | "MIN_NOTIONAL" => {
                if let Some(x) = dec_field(v, "minNotional") {
                    f.min_notional = x;
                }
            }
            _ => {}
        }
    }
    f
}

fn dec_field(v: &serde_json::Value, key: &str) -> Option<Decimal> {
    Decimal::from_str_exact(v.get(key)?.as_str()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIANGLE: &str = include_str!("../tests/fixtures/exchange_info_triangle.json");

    fn table() -> SymbolTable {
        let info: ExchangeInfoRaw = serde_json::from_str(TRIANGLE).unwrap();
        SymbolTable::from_exchange_info(&info).unwrap()
    }

    #[test]
    fn builds_triangle_from_real_fixture() {
        let t = table();
        assert_eq!(t.len(), 3);
        let btcusdt = t.resolve("BTCUSDT").unwrap();
        assert_eq!(btcusdt.base().as_str(), "BTC");
        assert_eq!(btcusdt.quote().as_str(), "USDT");
        assert!(t.resolve("ETHUSDT").is_some());
        assert!(t.resolve("ETHBTC").is_some());
        assert!(t.resolve("DOGEUSDT").is_none());
    }

    #[test]
    fn parses_real_filters() {
        let t = table();
        let btcusdt = t.resolve("BTCUSDT").unwrap();
        let f = t.filters(&btcusdt).unwrap();
        // Real Binance values: tick and step are positive, min_notional is set.
        assert!(f.tick_size.is_positive(), "tick_size should be positive");
        assert!(f.step_size.is_positive(), "step_size should be positive");
        assert!(f.min_notional.is_positive(), "min_notional should be set");
        let entry = t.entry(&btcusdt).unwrap();
        assert!(entry.trading);
    }
}
