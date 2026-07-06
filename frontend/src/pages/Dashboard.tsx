import { useMemo, useState } from 'react';
import { LatencyHistogram } from '@/components/charts/LatencyHistogram';
import { PnlChart } from '@/components/charts/PnlChart';
import { PositionsTable } from '@/components/dashboard/PositionsTable';
import { OppHeader, OppRow } from '@/components/opportunities/OppRow';
import { OpportunityDetail } from '@/components/opportunities/OpportunityDetail';
import { Badge, Panel, Stat } from '@/components/ui/primitives';
import { dirClass, fmtSignedUsd, fmtUsd } from '@/lib/format';
import { useStore } from '@/store/useStore';
import type { Opportunity } from '@/types/api';

export function Dashboard() {
  const opportunities = useStore((s) => s.opportunities);
  const metrics = useStore((s) => s.metrics);
  const [selected, setSelected] = useState<Opportunity | null>(null);

  const top = useMemo(
    () => [...opportunities].sort((a, b) => b.evQuote - a.evQuote).slice(0, 20),
    [opportunities],
  );

  const realized = metrics?.realizedQuote ?? 0;
  const unrealized = metrics?.unrealizedQuote ?? 0;

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Panel className="p-3">
          <Stat label="Realized PnL" value={<span className={dirClass(realized)}>{fmtSignedUsd(realized)}</span>} />
        </Panel>
        <Panel className="p-3">
          <Stat label="Unrealized PnL" value={<span className={dirClass(unrealized)}>{fmtSignedUsd(unrealized)}</span>} />
        </Panel>
        <Panel className="p-3">
          <Stat label="Net session" value={<span className={dirClass(realized + unrealized)}>{fmtUsd(realized + unrealized)}</span>} />
        </Panel>
        <Panel className="p-3">
          <Stat label="Live opportunities" value={opportunities.filter((o) => o.status === 'live').length} />
        </Panel>
      </div>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <Panel
          title="Live opportunity feed"
          className="h-[360px]"
          actions={<Badge tone="accent">top {top.length}</Badge>}
          bodyClassName="overflow-auto"
        >
          <OppHeader />
          {top.map((o) => (
            <OppRow key={o.id} opp={o} onSelect={setSelected} />
          ))}
        </Panel>

        <Panel title="Open positions" className="h-[360px]" bodyClassName="overflow-auto">
          <PositionsTable />
        </Panel>

        <Panel title="Session PnL (realized + unrealized)" className="h-[360px]" bodyClassName="p-2">
          <PnlChart />
        </Panel>

        <Panel title="Detection latency (per trade)" className="h-[360px]" bodyClassName="p-2">
          <LatencyHistogram />
        </Panel>
      </div>

      <OpportunityDetail opp={selected} onClose={() => setSelected(null)} />
    </div>
  );
}
