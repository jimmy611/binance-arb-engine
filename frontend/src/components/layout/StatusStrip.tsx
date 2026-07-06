import type { ReactNode } from 'react';
import { fmtMicros, fmtNum, fmtPct } from '@/lib/format';
import { useStore } from '@/store/useStore';

function Cell({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex shrink-0 flex-col">
      <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</span>
      <span className="num text-xs font-medium text-foreground">{children}</span>
    </div>
  );
}

export function StatusStrip() {
  const m = useStore((s) => s.metrics);

  return (
    <div className="flex h-9 shrink-0 items-center gap-5 overflow-x-auto border-b border-border/60 bg-bg-elev/50 px-4">
      {m ? (
        <>
          <Cell label="Msg/s">{fmtNum(m.msgRate, 0)}</Cell>
          <Cell label="Detect/s">{fmtNum(m.detectionRate, 0)}</Cell>
          <Cell label="Exec/s">{fmtNum(m.executionRate, 2)}</Cell>
          <Cell label="Detect p50">{fmtMicros(m.detectP50Us)}</Cell>
          <Cell label="Detect p99">{fmtMicros(m.detectP99Us)}</Cell>
          <Cell label="Ack p99">{`${m.ackP99Ms} ms`}</Cell>
          <Cell label="Ghost">{fmtPct(m.ghostRate)}</Cell>
          <Cell label="Reject">{fmtPct(m.rejectRate)}</Cell>
        </>
      ) : (
        <span className="text-xs text-muted-foreground">connecting to engine…</span>
      )}
    </div>
  );
}
