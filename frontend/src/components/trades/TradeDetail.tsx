import { Modal } from '@/components/ui/Modal';
import { Badge, Stat } from '@/components/ui/primitives';
import { dirClass, fmtMicros, fmtNum, fmtPrice, fmtSignedUsd, fmtUsd } from '@/lib/format';
import type { Trade, TradeOutcome } from '@/types/api';

const OUTCOME_TONE: Record<TradeOutcome, 'up' | 'warn' | 'down'> = {
  filled: 'up',
  partial: 'warn',
  aborted: 'down',
};

export function TradeDetail({ trade, onClose }: { trade: Trade | null; onClose: () => void }) {
  return (
    <Modal open={trade !== null} title="Trade audit trail" onClose={onClose} className="max-w-lg">
      {trade && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <span className="num text-base font-semibold">{trade.path.join(' → ')}</span>
            <Badge tone={OUTCOME_TONE[trade.outcome]} className="uppercase">
              {trade.outcome}
            </Badge>
          </div>

          <div className="grid grid-cols-4 gap-3">
            <Stat label="Gross" value={fmtUsd(trade.grossQuote)} />
            <Stat label="Fees" value={fmtUsd(trade.feesQuote)} />
            <Stat
              label="Net"
              value={<span className={dirClass(trade.netQuote)}>{fmtSignedUsd(trade.netQuote)}</span>}
            />
            <Stat label="Mode" value={<span className="text-sm">{trade.mode}</span>} />
          </div>

          <div>
            <div className="mb-1 text-2xs uppercase tracking-wider text-muted">Fills</div>
            <div className="overflow-hidden rounded-lg border border-border/60">
              <div className="grid grid-cols-[36px_1fr_56px_1fr_1fr] gap-2 border-b border-border/60 bg-bg-elev/40 px-3 py-1.5 text-2xs uppercase tracking-wider text-muted">
                <span>Leg</span>
                <span>Symbol</span>
                <span>Side</span>
                <span className="text-right">Price</span>
                <span className="text-right">Qty</span>
              </div>
              {trade.fills.map((f) => (
                <div key={f.leg} className="grid grid-cols-[36px_1fr_56px_1fr_1fr] gap-2 px-3 py-1.5 text-sm">
                  <span className="num text-muted">{f.leg + 1}</span>
                  <span className="num">{f.symbol}</span>
                  <span className={f.side === 'buy' ? 'text-up' : 'text-down'}>{f.side}</span>
                  <span className="num text-right">{fmtPrice(f.price)}</span>
                  <span className="num text-right text-muted">{fmtNum(f.qty, 4)}</span>
                </div>
              ))}
            </div>
          </div>

          <div>
            <div className="mb-1 text-2xs uppercase tracking-wider text-muted">Latency breakdown</div>
            <div className="grid grid-cols-4 gap-3">
              <Stat label="Detect" value={fmtMicros(trade.latency.detectUs)} />
              <Stat label="Eval" value={fmtMicros(trade.latency.evalUs)} />
              <Stat label="Sign" value={fmtMicros(trade.latency.signUs)} />
              <Stat label="Ack" value={`${trade.latency.ackMs} ms`} />
            </div>
          </div>
        </div>
      )}
    </Modal>
  );
}
