import { useVirtualizer } from '@tanstack/react-virtual';
import { useMemo, useRef, useState } from 'react';
import { OppHeader, OppRow } from '@/components/opportunities/OppRow';
import { OpportunityDetail } from '@/components/opportunities/OpportunityDetail';
import { Badge, Panel } from '@/components/ui/primitives';
import { useStore } from '@/store/useStore';
import type { Opportunity } from '@/types/api';

export function Opportunities() {
  const opportunities = useStore((s) => s.opportunities);
  const [minEv, setMinEv] = useState(0);
  const [minSize, setMinSize] = useState(0);
  const [symbol, setSymbol] = useState('');
  const [selected, setSelected] = useState<Opportunity | null>(null);

  const rows = useMemo(() => {
    const q = symbol.trim().toUpperCase();
    return opportunities
      .filter((o) => o.evQuote >= minEv && o.sizeQuote >= minSize && (q === '' || o.path.join('').includes(q) || o.legs.some((l) => l.symbol.includes(q))))
      .sort((a, b) => b.evQuote - a.evQuote);
  }, [opportunities, minEv, minSize, symbol]);

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 36,
    overscan: 14,
  });

  return (
    <Panel
      title="Opportunities"
      className="h-[calc(100vh-8.5rem)]"
      actions={
        <div className="flex flex-wrap items-center gap-2">
          <FilterNum label="min EV $" value={minEv} onChange={setMinEv} />
          <FilterNum label="min size $" value={minSize} onChange={setMinSize} />
          <input
            value={symbol}
            onChange={(e) => setSymbol(e.target.value)}
            placeholder="symbol"
            className="focus-ring h-7 w-28 rounded-md border border-border/60 bg-bg px-2 text-xs uppercase"
          />
          <Badge tone="neutral">{rows.length} rows</Badge>
        </div>
      }
      bodyClassName="flex flex-col"
    >
      <OppHeader />
      <div ref={parentRef} className="min-h-0 flex-1 overflow-auto">
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative', width: '100%' }}>
          {virtualizer.getVirtualItems().map((vi) => (
            <OppRow
              key={rows[vi.index].id}
              opp={rows[vi.index]}
              onSelect={setSelected}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                transform: `translateY(${vi.start}px)`,
              }}
            />
          ))}
        </div>
      </div>
      <OpportunityDetail opp={selected} onClose={() => setSelected(null)} />
    </Panel>
  );
}

function FilterNum({ label, value, onChange }: { label: string; value: number; onChange: (v: number) => void }) {
  return (
    <label className="flex items-center gap-1.5 text-2xs text-muted">
      {label}
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value) || 0)}
        className="focus-ring h-7 w-16 rounded-md border border-border/60 bg-bg px-2 text-xs"
      />
    </label>
  );
}
