import { useState } from 'react';
import { TradeDetail } from '@/components/trades/TradeDetail';
import { Badge, EmptyState, Panel } from '@/components/ui/primitives';
import { cn } from '@/lib/cn';
import { dirClass, fmtSignedUsd, fmtTime, fmtUsd } from '@/lib/format';
import { useStore } from '@/store/useStore';
import type { Trade, TradeOutcome } from '@/types/api';

const GRID = 'grid grid-cols-[72px_minmax(140px,1fr)_92px_96px_88px_92px_64px_88px] items-center gap-2 px-3';

const OUTCOME_TONE: Record<TradeOutcome, 'up' | 'warn' | 'down'> = {
  filled: 'up',
  partial: 'warn',
  aborted: 'down',
};

export function Trades() {
  const trades = useStore((s) => s.trades);
  const [selected, setSelected] = useState<Trade | null>(null);

  return (
    <Panel
      title="Trades"
      className="h-[calc(100vh-8.5rem)]"
      actions={<Badge tone="neutral">{trades.length} this session</Badge>}
      bodyClassName="flex flex-col"
    >
      <div className={cn(GRID, 'h-8 shrink-0 border-b border-border/60 bg-bg-elev/40')}>
        <span className="th px-0">Time</span>
        <span className="th px-0">Cycle</span>
        <span className="th px-0">Mode</span>
        <span className="th px-0 text-right">Gross</span>
        <span className="th px-0 text-right">Fees</span>
        <span className="th px-0 text-right">Net</span>
        <span className="th px-0 text-right">Ack</span>
        <span className="th px-0 text-center">Outcome</span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {trades.length === 0 ? (
          <EmptyState>No trades yet. In dry-run, executed cycles are logged here.</EmptyState>
        ) : (
          trades.map((t) => (
            <div
              key={t.cycleId}
              onClick={() => setSelected(t)}
              className={cn(GRID, 'data-row h-9 cursor-pointer text-sm')}
            >
              <span className="num text-2xs text-muted-foreground">{fmtTime(t.ts)}</span>
              <span className="num truncate">{t.path.join(' → ')}</span>
              <span className="text-2xs text-muted">{t.mode}</span>
              <span className="num text-right text-muted">{fmtUsd(t.grossQuote)}</span>
              <span className="num text-right text-muted">{fmtUsd(t.feesQuote)}</span>
              <span className={cn('num text-right', dirClass(t.netQuote))}>{fmtSignedUsd(t.netQuote)}</span>
              <span className="num text-right text-muted">{t.latency.ackMs}ms</span>
              <span className="flex justify-center">
                <Badge tone={OUTCOME_TONE[t.outcome]} className="uppercase">
                  {t.outcome}
                </Badge>
              </span>
            </div>
          ))
        )}
      </div>
      <TradeDetail trade={selected} onClose={() => setSelected(null)} />
    </Panel>
  );
}
