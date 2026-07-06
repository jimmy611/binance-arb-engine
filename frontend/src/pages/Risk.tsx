import { KillSwitch } from '@/components/risk/KillSwitch';
import { Badge, EmptyState, Panel } from '@/components/ui/primitives';
import { cn } from '@/lib/cn';
import { fmtAgo, fmtNum } from '@/lib/format';
import { useStore } from '@/store/useStore';
import type { AlertLevel, Severity } from '@/types/api';

const SEV_BAR: Record<Severity, string> = {
  ok: 'bg-up',
  warn: 'bg-warn',
  breach: 'bg-down',
};
const ALERT_TONE: Record<AlertLevel, 'neutral' | 'warn' | 'down'> = {
  info: 'neutral',
  warn: 'warn',
  error: 'down',
};

export function Risk() {
  const risk = useStore((s) => s.risk);
  const alerts = useStore((s) => s.alerts);
  const emergency = useStore((s) => s.emergencyStopped);

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <Panel title="Risk limits" className="lg:col-span-2" bodyClassName="p-4 space-y-3">
        {risk.length === 0 ? (
          <EmptyState>Waiting for metrics…</EmptyState>
        ) : (
          risk.map((r) => {
            const ratio = r.limit > 0 ? Math.min(1.2, r.used / r.limit) : 0;
            return (
              <div key={r.key} className="space-y-1">
                <div className="flex items-baseline justify-between">
                  <span className="text-sm text-foreground">{r.label}</span>
                  <span className="num text-xs text-muted">
                    {fmtNum(r.used, r.unit === '%' ? 1 : 0)}
                    {r.unit && r.unit !== '%' ? ` ${r.unit}` : r.unit} / {fmtNum(r.limit, 0)}
                    {r.unit === '%' ? '%' : ''}
                  </span>
                </div>
                <div className="h-1.5 overflow-hidden rounded-full bg-foreground/10">
                  <div
                    className={cn('h-full rounded-full transition-all', SEV_BAR[r.severity])}
                    style={{ width: `${Math.min(100, ratio * 100)}%` }}
                  />
                </div>
              </div>
            );
          })
        )}
      </Panel>

      <div className="flex flex-col gap-4">
        <Panel title="Emergency" bodyClassName="p-4 space-y-3">
          <p className="text-xs text-muted">
            Reachable from three independent channels (UI, CLI, HTTP). This is the UI channel; it
            halts new orders and unwinds to USDT.
          </p>
          {emergency && <Badge tone="down">EMERGENCY STOP ENGAGED</Badge>}
          <KillSwitch />
        </Panel>

        <Panel title="Alerts" className="flex-1" bodyClassName="overflow-auto">
          {alerts.length === 0 ? (
            <EmptyState>No alerts.</EmptyState>
          ) : (
            alerts.map((a) => (
              <div key={a.id} className="data-row flex items-start gap-2 px-3 py-2">
                <Badge tone={ALERT_TONE[a.level]} className="mt-0.5 shrink-0 uppercase">
                  {a.level}
                </Badge>
                <div className="min-w-0 flex-1">
                  <div className="text-sm text-foreground">{a.message}</div>
                  <div className="text-2xs text-muted-foreground">
                    {a.source} · {fmtAgo(a.ts)} ago
                  </div>
                </div>
              </div>
            ))
          )}
        </Panel>
      </div>
    </div>
  );
}
