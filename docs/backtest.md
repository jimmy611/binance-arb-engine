# Backtesting

How to run backtests and interpret results. Authored in full at milestone M13,
when the `backtest` harness and the `replay` engine exist.

Planned contents:

- Recording market data to WAL and selecting a 24-hour window.
- Running `arb-backtest` with a chosen algorithm and risk configuration.
- Reading the report: equity curve, hit rate, average edge, latency
  distribution, ghost rate.
- The comparison view for two configurations side by side.
- The gate: live mode may only be enabled after a backtest over at least 24
  hours of recorded data shows positive expected value at the default risk
  configuration. A negative result is an expected and acceptable outcome; it
  keeps the engine in dry-run.
