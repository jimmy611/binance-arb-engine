import { Bell } from 'lucide-react';
import { KillSwitch } from '@/components/risk/KillSwitch';
import { Badge, Button } from '@/components/ui/primitives';
import { ThemeToggle } from '@/components/ui/ThemeToggle';
import { useControl } from '@/data/ControlContext';
import { cn } from '@/lib/cn';
import { useStore } from '@/store/useStore';
import type { ConnectionState } from '@/types/api';

const DOT: Record<ConnectionState, string> = {
  connected: 'bg-up',
  connecting: 'bg-warn animate-blink',
  degraded: 'bg-warn',
  disconnected: 'bg-down',
};

function ConnDot({ connection }: { connection: ConnectionState }) {
  return (
    <span className="flex items-center gap-2 text-xs text-muted">
      <span className={cn('h-2 w-2 rounded-full', DOT[connection])} />
      <span className="capitalize">{connection}</span>
    </span>
  );
}

export function TopBar() {
  const mode = useStore((s) => s.mode);
  const connection = useStore((s) => s.connection);
  const alertCount = useStore((s) => s.alerts.length);
  const emergency = useStore((s) => s.emergencyStopped);
  const { transportKind } = useControl();

  return (
    <header className="glass z-10 flex h-14 shrink-0 items-center justify-between gap-3 rounded-none border-x-0 border-t-0 px-4">
      <div className="flex items-center gap-3">
        <ConnDot connection={connection} />
        <Badge tone={transportKind === 'simulation' ? 'warn' : 'accent'}>
          {transportKind === 'simulation' ? 'SIMULATION' : 'LIVE FEED'}
        </Badge>
      </div>

      <div className="flex items-center gap-2">
        <Badge tone={mode === 'live' ? 'down' : 'neutral'}>
          {mode === 'live' ? 'LIVE TRADING' : 'DRY-RUN'}
        </Badge>
        {emergency && <Badge tone="down">EMERGENCY STOP</Badge>}
        <div className="relative">
          <Button variant="ghost" size="sm" aria-label="Alerts">
            <Bell className="h-4 w-4" />
          </Button>
          {alertCount > 0 && (
            <span className="absolute right-0 top-0 grid h-4 min-w-4 place-items-center rounded-full bg-down px-1 text-[10px] font-semibold text-white">
              {alertCount}
            </span>
          )}
        </div>
        <ThemeToggle />
        <KillSwitch compact />
      </div>
    </header>
  );
}
