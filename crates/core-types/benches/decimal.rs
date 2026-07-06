//! Microbenchmarks for the fixed-point decimal hot-path operations.
//!
//! These establish the baseline for the CI latency-regression gate. Decimal
//! multiply/divide sit inside the per-leg slippage math, so a regression here
//! ripples into the detection budget. `parse` measures ingest cost (every
//! Binance price/quantity arrives as a string) and `display` measures the API
//! egress cost.

use core_types::Decimal;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn decimal_ops(c: &mut Criterion) {
    let a = Decimal::from_str_exact("60000.12345678").unwrap();
    let b = Decimal::from_str_exact("0.99925000").unwrap();

    c.bench_function("decimal_checked_mul", |bch| {
        bch.iter(|| black_box(a).checked_mul(black_box(b)));
    });

    c.bench_function("decimal_checked_div", |bch| {
        bch.iter(|| black_box(a).checked_div(black_box(b)));
    });

    c.bench_function("decimal_checked_add", |bch| {
        bch.iter(|| black_box(a).checked_add(black_box(b)));
    });

    c.bench_function("decimal_parse", |bch| {
        bch.iter(|| Decimal::from_str_exact(black_box("60000.12345678")));
    });

    c.bench_function("decimal_display", |bch| {
        bch.iter(|| black_box(a).to_string());
    });
}

criterion_group!(benches, decimal_ops);
criterion_main!(benches);
