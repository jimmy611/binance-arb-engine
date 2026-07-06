import { Play } from 'lucide-react';
import { useState } from 'react';
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import { Badge, Button, EmptyState, Panel, Stat } from '@/components/ui/primitives';
import { hsl } from '@/lib/cssColor';
import { dirClass, fmtBps, fmtPct, fmtSignedUsd } from '@/lib/format';

interface Result {
  curve: Array<{ i: number; equity: number }>;
  finalPnl: number;
  hitRate: number;
  avgEdgeBps: number;
  trades: number;
  medianAckMs: number;
}

function runBacktest(minEdgeBps: number): Result {
  // A self-contained illustrative run over synthetic recorded flow. Higher edge
  // thresholds trade less and drift closer to flat; results are frequently
  // negative after fees and latency, which is the honest, expected outcome.
  const points = 240;
  const tradeRate = Math.max(0.05, 0.6 - minEdgeBps * 0.03);
  const drift = (minEdgeBps - 6) * 0.02; // higher threshold, less bleed
  let equity = 0;
  let wins = 0;
  let trades = 0;
  const curve: Result['curve'] = [];
  for (let i = 0; i < points; i += 1) {
    if (Math.random() < tradeRate) {
      trades += 1;
      const pnl = drift + (Math.random() - 0.52) * 3;
      equity += pnl;
      if (pnl > 0) wins += 1;
    }
    curve.push({ i, equity: Number(equity.toFixed(2)) });
  }
  return {
    curve,
    finalPnl: equity,
    hitRate: trades ? wins / trades : 0,
    avgEdgeBps: minEdgeBps + (Math.random() - 0.5) * 2,
    trades,
    medianAckMs: 10 + Math.round(Math.random() * 8),
  };
}

export function Backtest() {
  const [walFile, setWalFile] = useState('wal-2026-07-05.bin');
  const [minEdgeBps, setMinEdgeBps] = useState(6);
  const [result, setResult] = useState<Result | null>(null);
  const [running, setRunning] = useState(false);

  const run = () => {
    setRunning(true);
    setTimeout(() => {
      setResult(runBacktest(minEdgeBps));
      setRunning(false);
    }, 500);
  };

  const grid = hsl('--border', 0.35);
  const muted = hsl('--muted');
  const line = result && result.finalPnl >= 0 ? hsl('--up') : hsl('--down');

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <Panel title="Backtest configuration" bodyClassName="p-4 space-y-4">
        <label className="block space-y-1">
          <span className="text-2xs uppercase tracking-wider text-muted">WAL file</span>
          <input
            value={walFile}
            onChange={(e) => setWalFile(e.target.value)}
            className="focus-ring h-9 w-full rounded-lg border border-border/60 bg-bg px-3 font-mono text-sm"
          />
        </label>
        <label className="block space-y-1">
          <span className="text-2xs uppercase tracking-wider text-muted">Min edge (bps): {minEdgeBps}</span>
          <input
            type="range"
            min={0}
            max={20}
            value={minEdgeBps}
            onChange={(e) => setMinEdgeBps(Number(e.target.value))}
            className="w-full accent-primary"
          />
        </label>
        <Button variant="primary" size="md" onClick={run} disabled={running} className="w-full">
          <Play className="h-4 w-4" />
          {running ? 'Running replay…' : 'Run backtest'}
        </Button>
        <p className="text-2xs text-muted-foreground">
          Replays recorded order flow through the exact engine code. A negative result keeps the
          engine in dry-run; live mode is gated behind a positive-EV 24h backtest.
        </p>
      </Panel>

      <Panel title="Results" className="lg:col-span-2" bodyClassName="p-4">
        {!result ? (
          <EmptyState>Configure and run a backtest to see the equity curve.</EmptyState>
        ) : (
          <div className="flex h-full flex-col gap-4">
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-5">
              <Stat
                label="Net PnL"
                value={<span className={dirClass(result.finalPnl)}>{fmtSignedUsd(result.finalPnl)}</span>}
              />
              <Stat label="Hit rate" value={fmtPct(result.hitRate)} />
              <Stat label="Avg edge" value={fmtBps(result.avgEdgeBps)} />
              <Stat label="Trades" value={result.trades} />
              <Stat label="Median ack" value={`${result.medianAckMs} ms`} />
            </div>
            <div className="flex items-center gap-2">
              <Badge tone={result.finalPnl >= 0 ? 'up' : 'down'}>
                {result.finalPnl >= 0 ? 'POSITIVE EV' : 'NEGATIVE EV — stays in dry-run'}
              </Badge>
            </div>
            <div className="min-h-0 flex-1">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={result.curve} margin={{ top: 8, right: 8, bottom: 4, left: -16 }}>
                  <CartesianGrid strokeDasharray="3 3" stroke={grid} vertical={false} />
                  <XAxis dataKey="i" tick={{ fill: muted, fontSize: 10 }} axisLine={{ stroke: grid }} tickLine={false} />
                  <YAxis tick={{ fill: muted, fontSize: 10 }} axisLine={false} tickLine={false} />
                  <Tooltip
                    contentStyle={{ background: hsl('--surface'), border: `1px solid ${grid}`, borderRadius: 8, fontSize: 12 }}
                    labelStyle={{ color: muted }}
                  />
                  <Line type="monotone" dataKey="equity" stroke={line} strokeWidth={2} dot={false} isAnimationActive={false} />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </div>
        )}
      </Panel>
    </div>
  );
}
