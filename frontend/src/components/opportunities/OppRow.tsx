import { memo, useEffect, useRef, useState } from 'react';
import { Badge } from '@/components/ui/primitives';
import { cn } from '@/lib/cn';
import { dirClass, fmtAgo, fmtBps, fmtNum, fmtPct, fmtUsd } from '@/lib/format';
import type { OppStatus, Opportunity } from '@/types/api';

export const OPP_GRID = 'grid grid-cols-[minmax(150px,1fr)_80px_96px_84px_64px_92px_56px] items-center gap-2 px-3';

const STATUS_TONE: Record<OppStatus, 'accent' | 'up' | 'neutral'> = {
  live: 'accent',
  executing: 'up',
  ghost: 'neutral',
  filtered: 'neutral',
};

export function OppHeader() {
  return (
    <div className={cn(OPP_GRID, 'h-8 border-b border-border/60 bg-bg-elev/40')}>
      <span className="th px-0">Cycle</span>
      <span className="th px-0 text-right">Edge</span>
      <span className="th px-0 text-right">EV</span>
      <span className="th px-0 text-right">Size</span>
      <span className="th px-0 text-right">Conf</span>
      <span className="th px-0 text-center">Status</span>
      <span className="th px-0 text-right">Age</span>
    </div>
  );
}

interface OppRowProps {
  opp: Opportunity;
  onSelect?: (opp: Opportunity) => void;
  style?: React.CSSProperties;
}

function OppRowInner({ opp, onSelect, style }: OppRowProps) {
  const prev = useRef(opp.evQuote);
  const [flash, setFlash] = useState<'' | 'up' | 'down'>('');

  useEffect(() => {
    if (opp.evQuote > prev.current) setFlash('up');
    else if (opp.evQuote < prev.current) setFlash('down');
    prev.current = opp.evQuote;
    const t = setTimeout(() => setFlash(''), 500);
    return () => clearTimeout(t);
  }, [opp.evQuote]);

  return (
    <div
      role="row"
      style={style}
      onClick={() => onSelect?.(opp)}
      className={cn(
        OPP_GRID,
        'data-row h-9 cursor-pointer text-sm',
        flash === 'up' && 'animate-flash-up',
        flash === 'down' && 'animate-flash-down',
      )}
    >
      <span className="num truncate text-foreground">{opp.path.join(' → ')}</span>
      <span className={cn('num text-right', dirClass(opp.edgeBps))}>{fmtBps(opp.edgeBps)}</span>
      <span className={cn('num text-right', dirClass(opp.evQuote))}>{fmtUsd(opp.evQuote)}</span>
      <span className="num text-right text-muted">{fmtNum(opp.sizeQuote, 0)}</span>
      <span className="num text-right text-muted">{fmtPct(opp.confidence, 0)}</span>
      <span className="flex justify-center">
        <Badge tone={STATUS_TONE[opp.status]} className="uppercase">
          {opp.status}
        </Badge>
      </span>
      <span className="num text-right text-2xs text-muted-foreground">{fmtAgo(opp.ts)}</span>
    </div>
  );
}

export const OppRow = memo(OppRowInner);
