-- ARB engine TimescaleDB schema.
--
-- Mounted into the TimescaleDB container's init directory by docker-compose and
-- applied on first start. Stores the analytics-oriented time series the control
-- API serves: ticks, order-book tops, opportunities, executed trades, PnL, and
-- latency. The write-ahead log (see the `wal` module) remains the source of
-- truth for exact replay; these tables are for aggregation and queries.

CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Best bid/ask samples per symbol.
CREATE TABLE IF NOT EXISTS book_top (
    ts          TIMESTAMPTZ      NOT NULL,
    symbol      TEXT             NOT NULL,
    seq         BIGINT           NOT NULL,
    bid_price   NUMERIC,
    bid_qty     NUMERIC,
    ask_price   NUMERIC,
    ask_qty     NUMERIC,
    status      TEXT             NOT NULL
);
SELECT create_hypertable('book_top', 'ts', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS book_top_symbol_ts ON book_top (symbol, ts DESC);

-- Detected opportunities (post-fee, pre-execution).
CREATE TABLE IF NOT EXISTS opportunities (
    ts             TIMESTAMPTZ   NOT NULL,
    cycle_id       TEXT          NOT NULL,
    path           TEXT          NOT NULL,   -- e.g. USDT>BTC>ETH>USDT
    gross_product  DOUBLE PRECISION,
    edge_bps       DOUBLE PRECISION,
    ev_quote       DOUBLE PRECISION,
    size_quote     DOUBLE PRECISION,
    confidence     DOUBLE PRECISION,
    accepted       BOOLEAN       NOT NULL
);
SELECT create_hypertable('opportunities', 'ts', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS opportunities_cycle_ts ON opportunities (cycle_id, ts DESC);

-- Executed trades (one row per cycle attempt), with the latency breakdown.
CREATE TABLE IF NOT EXISTS trades (
    ts             TIMESTAMPTZ   NOT NULL,
    cycle_id       TEXT          NOT NULL,
    path           TEXT          NOT NULL,
    mode           TEXT          NOT NULL,   -- parallel_ioc | sequential | dry_run
    outcome        TEXT          NOT NULL,   -- filled | partial | aborted
    gross_quote    DOUBLE PRECISION,
    fees_quote     DOUBLE PRECISION,
    net_quote      DOUBLE PRECISION,
    detect_us      INTEGER,
    eval_us        INTEGER,
    sign_us        INTEGER,
    ack_ms         INTEGER
);
SELECT create_hypertable('trades', 'ts', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS trades_cycle_ts ON trades (cycle_id, ts DESC);

-- Individual fills per leg (joined to trades by cycle_id).
CREATE TABLE IF NOT EXISTS fills (
    ts            TIMESTAMPTZ    NOT NULL,
    cycle_id      TEXT           NOT NULL,
    leg           SMALLINT       NOT NULL,
    symbol        TEXT           NOT NULL,
    side          TEXT           NOT NULL,
    price         NUMERIC,
    qty           NUMERIC,
    fee_quote     DOUBLE PRECISION
);
SELECT create_hypertable('fills', 'ts', if_not_exists => TRUE);

-- Realized and unrealized PnL series.
CREATE TABLE IF NOT EXISTS pnl (
    ts               TIMESTAMPTZ  NOT NULL,
    realized_quote   DOUBLE PRECISION,
    unrealized_quote DOUBLE PRECISION
);
SELECT create_hypertable('pnl', 'ts', if_not_exists => TRUE);

-- Per-stage latency samples (microseconds), for the histograms.
CREATE TABLE IF NOT EXISTS latency (
    ts       TIMESTAMPTZ  NOT NULL,
    stage    TEXT         NOT NULL,   -- parse | book | detect | eval | sign | ack
    micros   INTEGER      NOT NULL
);
SELECT create_hypertable('latency', 'ts', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS latency_stage_ts ON latency (stage, ts DESC);

-- Retention: keep raw high-frequency series for 14 days by default. Adjust in
-- the runbook per storage budget.
SELECT add_retention_policy('book_top', INTERVAL '14 days', if_not_exists => TRUE);
SELECT add_retention_policy('latency', INTERVAL '14 days', if_not_exists => TRUE);
