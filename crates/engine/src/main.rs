//! The engine binary: wires the dry-run pipeline and owns the process lifecycle.
//!
//! Fail-safe by default: the engine boots in [`EngineMode::DryRun`] and never
//! submits an order. With `RUN_SECONDS=N` set it connects to Binance's public,
//! read-only market data for N seconds, maintains order books, updates the
//! currency graph, detects triangular arbitrage, sizes it with the VWAP
//! optimizer, checks risk, and logs and streams opportunities. Live trading
//! (real orders) is a separate, gated path and is not enabled here.
#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;

use control_api::{ControlState, Hub};
use core_types::{EngineMode, MarketEvent, Nanos, Symbol};
use exchange_binance::{BinanceMarketStream, BinanceRest, SymbolTable, MAINNET_WS};
use exchange_common::{MarketFeed, StreamKind, Subscription};
use graph::AssetGraph;
use persistence::WalWriter;
use risk::{RiskConfig, RiskManager};
use tokio::sync::mpsc;

mod pipeline;
use pipeline::Pipeline;

/// The symbol universe: a set of markets that form triangles across USDT and
/// crosses. In production this is the full Binance universe filtered by config.
const UNIVERSE: &[&str] = &[
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "ETHBTC", "BNBBTC", "SOLBTC", "BNBETH",
];
const TAKER_FEE: f64 = 0.001;

#[tokio::main]
async fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let mode = EngineMode::default();
    assert_eq!(mode, EngineMode::DryRun, "engine must boot in dry-run");

    let run_secs: u64 = std::env::var("RUN_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if run_secs == 0 {
        println!(
            "arb-engine v{} initialized in {mode} mode. \
             Set RUN_SECONDS=N to run the read-only pipeline against Binance public data \
             for N seconds. No orders are ever sent in this build.",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    if let Err(e) = run(run_secs).await {
        tracing::error!(error = %e, "engine run failed");
        std::process::exit(1);
    }
}

async fn run(run_secs: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Symbol metadata and the currency graph.
    let rest = Arc::new(BinanceRest::mainnet()?);
    let info = rest.exchange_info(Some(UNIVERSE)).await?;
    let table = Arc::new(SymbolTable::from_exchange_info(&info)?);
    tracing::info!(symbols = table.len(), "symbol table built");

    let specs: Vec<(Symbol, f64)> = table.iter().map(|e| (e.symbol, TAKER_FEE)).collect();
    let graph = AssetGraph::from_specs(&specs);
    tracing::info!(
        assets = graph.num_assets(),
        edges = graph.num_edges(),
        triangles = graph.num_triangles(),
        "currency graph built"
    );

    // Components.
    let hub = Arc::new(Hub::new(4096));
    let state = Arc::new(ControlState::new());
    let risk = RiskManager::new(RiskConfig::default());
    let wal_dir = std::env::var("WAL_DIR").unwrap_or_else(|_| "wal".to_owned());
    let wal = WalWriter::open(&wal_dir)?;
    let mut pipeline = Pipeline::new(
        graph,
        risk,
        wal,
        Arc::clone(&hub),
        Arc::clone(&state),
        TAKER_FEE,
    );

    // Market-data feed pumped into a single-consumer channel.
    let (tx, mut rx) = mpsc::channel::<MarketEvent>(8192);
    let subs: Vec<Subscription> = table
        .iter()
        .map(|e| Subscription {
            symbol: e.symbol,
            kinds: vec![StreamKind::Depth, StreamKind::BookTicker],
        })
        .collect();
    let mut feed = BinanceMarketStream::new(MAINNET_WS, Arc::clone(&table));
    feed.subscribe(&subs).await?;
    feed.connect().await?;
    let feed_tx = tx.clone();
    tokio::spawn(async move {
        let mut feed = feed;
        while let Some(item) = feed.next_event().await {
            match item {
                Ok(event) => {
                    if feed_tx.send(event).await.is_err() {
                        break;
                    }
                }
                Err(e) => tracing::debug!(error = %e, "feed event error"),
            }
        }
    });

    // Prime each book with a REST snapshot. Per the Binance protocol the stream
    // is opened first (above) and buffers diffs; the snapshot then seeds the
    // book and the buffered diffs replay on top. Any residual gap triggers a
    // fresh snapshot via the resync path.
    for symbol in table.iter().map(|e| e.symbol).collect::<Vec<_>>() {
        let rest = Arc::clone(&rest);
        let tx = tx.clone();
        tokio::spawn(async move { fetch_snapshot(&rest, &tx, symbol).await });
    }

    // Optionally serve the control API (gRPC + WebSocket). Off by default so a
    // short demo run does not bind ports.
    if std::env::var("SERVE").as_deref() == Ok("1") {
        serve_control_api(Arc::clone(&state), Arc::clone(&hub));
    }

    // Main loop with a deadline.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(run_secs);
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => break,
            maybe = rx.recv() => {
                let Some(event) = maybe else { break };
                if let Some(symbol) = pipeline.handle(event) {
                    let rest = Arc::clone(&rest);
                    let tx = tx.clone();
                    tokio::spawn(async move { fetch_snapshot(&rest, &tx, symbol).await });
                }
            }
        }
    }

    tracing::info!(
        book_updates = pipeline.book_updates(),
        opportunities = pipeline.opportunities(),
        "run complete"
    );
    println!(
        "run complete: {} book updates, {} opportunities detected (dry-run, no orders sent)",
        pipeline.book_updates(),
        pipeline.opportunities()
    );
    Ok(())
}

fn serve_control_api(state: Arc<ControlState>, hub: Arc<Hub>) {
    tokio::spawn(async move {
        let addr = "127.0.0.1:50051".parse().expect("valid addr");
        if let Err(e) = control_api::serve_grpc(addr, state).await {
            tracing::warn!(error = %e, "grpc server ended");
        }
    });
    tokio::spawn(async move {
        let addr = "127.0.0.1:8080".parse().expect("valid addr");
        if let Err(e) = control_api::serve_ws(addr, hub).await {
            tracing::warn!(error = %e, "ws server ended");
        }
    });
}

/// Fetches a REST depth snapshot for a stale symbol and feeds it back in.
async fn fetch_snapshot(rest: &BinanceRest, tx: &mpsc::Sender<MarketEvent>, symbol: Symbol) {
    match rest.depth_snapshot(symbol.as_str(), 100).await {
        Ok(raw) => {
            match exchange_binance::normalize::normalize_depth_snapshot(&raw, symbol, Nanos::now())
            {
                Ok(snap) => {
                    let _ = tx.send(MarketEvent::DepthSnapshot(snap)).await;
                }
                Err(e) => tracing::debug!(error = %e, "snapshot normalize failed"),
            }
        }
        Err(e) => tracing::debug!(error = %e, symbol = %symbol, "snapshot fetch failed"),
    }
}
