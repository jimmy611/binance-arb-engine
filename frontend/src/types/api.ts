// The frontend view model. These mirror the streaming payloads defined in
// proto/arbengine/v1/arb.proto, in a browser-friendly shape (numbers instead of
// Decimal strings, camelCase). The simulation transport is the reference
// producer of this contract; a real backend adapter maps the protobuf
// StreamEvent JSON onto these types.

export type EngineMode = 'dry_run' | 'live';
export type ConnectionState = 'connected' | 'connecting' | 'disconnected' | 'degraded';
export type Side = 'buy' | 'sell';
export type ExecMode = 'parallel_ioc' | 'sequential' | 'dry_run';
export type OppStatus = 'live' | 'ghost' | 'executing' | 'filtered';
export type TradeOutcome = 'filled' | 'partial' | 'aborted';
export type AlertLevel = 'info' | 'warn' | 'error';
export type Severity = 'ok' | 'warn' | 'breach';

export interface BookTop {
  symbol: string;
  bid: number;
  ask: number;
  bidQty: number;
  askQty: number;
  status: 'live' | 'stale';
  ts: number;
}

export interface OppLeg {
  symbol: string;
  side: Side;
  price: number;
  qty: number;
}

export interface Opportunity {
  id: string;
  path: string[];
  legs: OppLeg[];
  grossProduct: number;
  edgeBps: number;
  evQuote: number;
  sizeQuote: number;
  confidence: number;
  mode: ExecMode;
  status: OppStatus;
  ts: number;
}

export interface Fill {
  leg: number;
  symbol: string;
  side: Side;
  price: number;
  qty: number;
  feeQuote: number;
}

export interface LatencyBreakdown {
  detectUs: number;
  evalUs: number;
  signUs: number;
  ackMs: number;
}

export interface Trade {
  cycleId: string;
  path: string[];
  fills: Fill[];
  grossQuote: number;
  feesQuote: number;
  netQuote: number;
  mode: ExecMode;
  outcome: TradeOutcome;
  latency: LatencyBreakdown;
  ts: number;
}

export interface Position {
  asset: string;
  symbol: string;
  side: Side;
  size: number;
  entry: number;
  mark: number;
  uPnl: number;
}

export interface ExchangeStatus {
  name: string;
  state: ConnectionState;
  msgRate: number;
}

export interface SystemMetrics {
  mode: EngineMode;
  exchanges: ExchangeStatus[];
  msgRate: number;
  detectionRate: number;
  executionRate: number;
  detectP50Us: number;
  detectP99Us: number;
  ackP50Ms: number;
  ackP99Ms: number;
  ghostRate: number;
  rejectRate: number;
  realizedQuote: number;
  unrealizedQuote: number;
  ts: number;
}

export interface Alert {
  id: string;
  level: AlertLevel;
  source: string;
  message: string;
  ts: number;
}

export interface RiskLimit {
  key: string;
  label: string;
  used: number;
  limit: number;
  unit: string;
  severity: Severity;
}

export interface PnlPoint {
  ts: number;
  realized: number;
  unrealized: number;
}

export interface LatencyBucket {
  bucket: string;
  count: number;
}

export type StreamEvent =
  | { topic: 'ticker'; data: BookTop }
  | { topic: 'opportunity'; data: Opportunity }
  | { topic: 'trade'; data: Trade }
  | { topic: 'metrics'; data: SystemMetrics }
  | { topic: 'alert'; data: Alert }
  | { topic: 'positions'; data: Position[] };

export interface Ack {
  ok: boolean;
  message: string;
}
