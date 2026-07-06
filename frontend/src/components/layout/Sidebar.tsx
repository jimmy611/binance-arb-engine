import { Activity, FlaskConical, LayoutDashboard, Radar, Receipt, Settings, ShieldAlert } from 'lucide-react';
import { NavLink } from 'react-router-dom';
import { cn } from '@/lib/cn';

const NAV = [
  { to: '/', label: 'Dashboard', icon: LayoutDashboard, end: true },
  { to: '/opportunities', label: 'Opportunities', icon: Radar, end: false },
  { to: '/trades', label: 'Trades', icon: Receipt, end: false },
  { to: '/risk', label: 'Risk', icon: ShieldAlert, end: false },
  { to: '/backtest', label: 'Backtest', icon: FlaskConical, end: false },
  { to: '/settings', label: 'Settings', icon: Settings, end: false },
];

export function Sidebar() {
  return (
    <aside className="glass z-20 flex w-16 flex-col gap-1 rounded-none border-y-0 border-l-0 px-2 py-3 lg:w-56 lg:px-3">
      <div className="mb-4 flex items-center gap-2.5 px-1.5">
        <div className="grid h-8 w-8 place-items-center rounded-lg bg-primary/15 text-primary shadow-glow">
          <Activity className="h-5 w-5" strokeWidth={2.4} />
        </div>
        <div className="hidden lg:block">
          <div className="text-sm font-semibold tracking-tight text-foreground">ARB</div>
          <div className="text-2xs text-muted-foreground">Triangular Engine</div>
        </div>
      </div>

      <nav className="flex flex-col gap-0.5">
        {NAV.map(({ to, label, icon: Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              cn(
                'focus-ring flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm transition-colors',
                isActive
                  ? 'bg-primary/12 font-medium text-primary'
                  : 'text-muted hover:bg-foreground/5 hover:text-foreground',
              )
            }
          >
            <Icon className="h-5 w-5 shrink-0" />
            <span className="hidden lg:inline">{label}</span>
          </NavLink>
        ))}
      </nav>

      <div className="mt-auto hidden px-2 text-2xs text-muted-foreground lg:block">
        v0.1 · dry-run default
      </div>
    </aside>
  );
}
