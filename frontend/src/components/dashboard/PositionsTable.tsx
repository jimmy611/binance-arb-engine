import { EmptyState } from '@/components/ui/primitives';
import { cn } from '@/lib/cn';
import { dirClass, fmtNum, fmtPrice, fmtSignedUsd } from '@/lib/format';
import { useStore } from '@/store/useStore';

const GRID = 'grid grid-cols-[1fr_56px_88px_96px_96px_88px] items-center gap-2 px-3';

export function PositionsTable() {
  const positions = useStore((s) => s.positions);

  if (positions.length === 0) {
    return <EmptyState>No open positions. Inventory is flat.</EmptyState>;
  }

  return (
    <div className="flex flex-col">
      <div className={cn(GRID, 'h-8 border-b border-border/60 bg-bg-elev/40')}>
        <span className="th px-0">Symbol</span>
        <span className="th px-0">Side</span>
        <span className="th px-0 text-right">Size</span>
        <span className="th px-0 text-right">Entry</span>
        <span className="th px-0 text-right">Mark</span>
        <span className="th px-0 text-right">uPnL</span>
      </div>
      {positions.map((p) => (
        <div key={p.symbol} className={cn(GRID, 'data-row h-9 text-sm')}>
          <span className="num text-foreground">{p.symbol}</span>
          <span className={p.side === 'buy' ? 'text-up' : 'text-down'}>{p.side}</span>
          <span className="num text-right text-muted">{fmtNum(p.size, 4)}</span>
          <span className="num text-right text-muted">{fmtPrice(p.entry)}</span>
          <span className="num text-right">{fmtPrice(p.mark)}</span>
          <span className={cn('num text-right', dirClass(p.uPnl))}>{fmtSignedUsd(p.uPnl)}</span>
        </div>
      ))}
    </div>
  );
}
