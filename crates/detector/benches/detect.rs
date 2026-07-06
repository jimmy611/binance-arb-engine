//! Latency benchmarks for the detection hot path.
//!
//! These establish the CI regression baseline. `scan_affected` is the true hot
//! path (a single book update touches a handful of triangles), `scan_all` is the
//! cold-start cost, and `bellman_ford` is the periodic full-graph sanity check.
//! The graph is a hub-and-spoke universe (every asset pairs against USDT and
//! BTC, which pair with each other), which is the shape that dominates real
//! triangular arbitrage.

use core_types::{BookStatus, BookUpdate, Decimal, Nanos, PriceLevel, SeqNo, Symbol};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use detector::{bellman, triangle};
use graph::AssetGraph;

fn sym(base: &str, quote: &str) -> Symbol {
    Symbol::from_parts(base, quote).unwrap()
}

fn book(symbol: Symbol, bid: &str, ask: &str) -> BookUpdate {
    BookUpdate {
        symbol,
        seq: SeqNo(0),
        update_id: 1,
        status: BookStatus::Live,
        best_bid: Some(PriceLevel::new(
            Decimal::from_str_exact(bid).unwrap(),
            Decimal::from_int(10),
        )),
        best_ask: Some(PriceLevel::new(
            Decimal::from_str_exact(ask).unwrap(),
            Decimal::from_int(10),
        )),
        ts: Nanos(0),
    }
}

/// Builds a hub-and-spoke graph with `k` spoke assets and seeds fair prices.
fn build(k: usize) -> AssetGraph {
    let mut specs: Vec<(Symbol, f64)> = Vec::with_capacity(2 * k + 1);
    for i in 0..k {
        let asset = format!("C{i}");
        specs.push((sym(&asset, "USDT"), 0.001));
        specs.push((sym(&asset, "BTC"), 0.001));
    }
    specs.push((sym("BTC", "USDT"), 0.001));

    let mut g = AssetGraph::from_specs(&specs);
    g.update_from_book(&book(sym("BTC", "USDT"), "60000", "60000"));
    for i in 0..k {
        let asset = format!("C{i}");
        // Ci priced at 100 USDT and the fair 100/60000 in BTC (no arbitrage).
        g.update_from_book(&book(sym(&asset, "USDT"), "100", "100"));
        g.update_from_book(&book(sym(&asset, "BTC"), "0.00166667", "0.00166667"));
    }
    g
}

fn detection(c: &mut Criterion) {
    let g = build(200);
    let spoke = sym("C0", "USDT");

    c.bench_function("scan_affected_single_symbol", |b| {
        b.iter(|| triangle::scan_affected(black_box(&g), black_box(&spoke), 1.0));
    });

    c.bench_function("scan_all_triangles", |b| {
        b.iter(|| triangle::scan_all(black_box(&g), 1.0));
    });

    c.bench_function("bellman_ford_full_graph", |b| {
        b.iter(|| bellman::find_negative_cycle(black_box(&g)));
    });
}

criterion_group!(benches, detection);
criterion_main!(benches);
