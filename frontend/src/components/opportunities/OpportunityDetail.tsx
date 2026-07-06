import { Modal } from '@/components/ui/Modal';
import { Badge, Stat } from '@/components/ui/primitives';
import { fmtBps, fmtNum, fmtPct, fmtPrice, fmtUsd } from '@/lib/format';
import type { Opportunity } from '@/types/api';

interface Props {
  opp: Opportunity | null;
  onClose: () => void;
}

/** Detail view: the triangle path, per-leg breakdown, and the EV/Kelly math. */
export function OpportunityDetail({ opp, onClose }: Props) {
  return (
    <Modal open={opp !== null} title="Opportunity detail" onClose={onClose} className="max-w-lg">
      {opp && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <span className="num text-base font-semibold text-foreground">{opp.path.join(' → ')}</span>
            <Badge tone={opp.status === 'live' ? 'accent' : 'neutral'} className="uppercase">
              {opp.status}
            </Badge>
          </div>

          <div className="grid grid-cols-4 gap-3">
            <Stat label="Edge" value={<span className="text-up">{fmtBps(opp.edgeBps)}</span>} />
            <Stat label="EV" value={fmtUsd(opp.evQuote)} />
            <Stat label="Size" value={fmtUsd(opp.sizeQuote)} />
            <Stat label="Confidence" value={fmtPct(opp.confidence)} />
          </div>

          <div>
            <div className="mb-1 text-2xs uppercase tracking-wider text-muted">Legs (VWAP)</div>
            <div className="overflow-hidden rounded-lg border border-border/60">
              <div className="grid grid-cols-[1fr_60px_1fr_1fr] gap-2 border-b border-border/60 bg-bg-elev/40 px-3 py-1.5 text-2xs uppercase tracking-wider text-muted">
                <span>Symbol</span>
                <span>Side</span>
                <span className="text-right">Price</span>
                <span className="text-right">Qty</span>
              </div>
              {opp.legs.map((leg, i) => (
                <div key={i} className="grid grid-cols-[1fr_60px_1fr_1fr] gap-2 px-3 py-1.5 text-sm">
                  <span className="num">{leg.symbol}</span>
                  <span className={leg.side === 'buy' ? 'text-up' : 'text-down'}>{leg.side}</span>
                  <span className="num text-right">{fmtPrice(leg.price)}</span>
                  <span className="num text-right text-muted">{fmtNum(leg.qty, 4)}</span>
                </div>
              ))}
            </div>
          </div>

          <div className="rounded-lg border border-border/60 bg-bg-elev/30 p-3 text-xs text-muted">
            <div className="flex justify-between">
              <span>Gross product</span>
              <span className="num text-foreground">{opp.grossProduct.toFixed(6)}</span>
            </div>
            <div className="flex justify-between">
              <span>Execution mode</span>
              <span className="num text-foreground">{opp.mode}</span>
            </div>
            <div className="mt-2 border-t border-border/50 pt-2 text-2xs text-muted-foreground">
              Sizing is fractional-Kelly capped by the joint order-book capacity; EV is
              latency-adjusted by the survival probability.
            </div>
          </div>
        </div>
      )}
    </Modal>
  );
}
