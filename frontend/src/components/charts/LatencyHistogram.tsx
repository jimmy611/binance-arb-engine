import { useMemo } from 'react';
import { Bar, BarChart, CartesianGrid, Cell, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';
import { useTheme } from '@/components/ui/theme';
import { hsl } from '@/lib/cssColor';
import { useStore } from '@/store/useStore';

const BINS = [
  { label: '0-50', lo: 0, hi: 50 },
  { label: '50-100', lo: 50, hi: 100 },
  { label: '100-200', lo: 100, hi: 200 },
  { label: '200-300', lo: 200, hi: 300 },
  { label: '300+', lo: 300, hi: Infinity },
];

/** Histogram of recent per-trade detection latency (microseconds). */
export function LatencyHistogram() {
  const trades = useStore((s) => s.trades);
  const { theme } = useTheme();

  const { data, primary, grid, muted } = useMemo(() => {
    const counts = BINS.map((b) => ({ label: `${b.label} µs`, count: 0 }));
    for (const t of trades) {
      const us = t.latency.detectUs;
      const idx = BINS.findIndex((b) => us >= b.lo && us < b.hi);
      if (idx >= 0) counts[idx].count += 1;
    }
    return {
      data: counts,
      primary: hsl('--primary'),
      grid: hsl('--border', 0.35),
      muted: hsl('--muted'),
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trades, theme]);

  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={data} margin={{ top: 8, right: 8, bottom: 4, left: -18 }}>
        <CartesianGrid strokeDasharray="3 3" stroke={grid} vertical={false} />
        <XAxis dataKey="label" tick={{ fill: muted, fontSize: 10 }} axisLine={{ stroke: grid }} tickLine={false} />
        <YAxis tick={{ fill: muted, fontSize: 10 }} axisLine={false} tickLine={false} allowDecimals={false} />
        <Tooltip
          cursor={{ fill: hsl('--foreground', 0.05) }}
          contentStyle={{
            background: hsl('--surface'),
            border: `1px solid ${grid}`,
            borderRadius: 8,
            fontSize: 12,
          }}
          labelStyle={{ color: muted }}
        />
        <Bar dataKey="count" radius={[4, 4, 0, 0]}>
          {data.map((_, i) => (
            <Cell key={i} fill={primary} />
          ))}
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  );
}
