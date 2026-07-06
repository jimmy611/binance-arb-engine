// Number and time formatting for the terminal. Prices and quantities are
// right-aligned and decimal-consistent; large values use compact notation.

const usd = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

const usdCompact = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  notation: 'compact',
  maximumFractionDigits: 2,
});

/** Signed USD, compact for large magnitudes. */
export function fmtUsd(value: number, compact = false): string {
  if (!Number.isFinite(value)) return '—';
  return compact && Math.abs(value) >= 100_000 ? usdCompact.format(value) : usd.format(value);
}

/** A signed USD with an explicit leading + for positive values. */
export function fmtSignedUsd(value: number): string {
  const s = fmtUsd(value);
  return value > 0 ? `+${s}` : s;
}

/** Fixed-decimal number. */
export function fmtNum(value: number, dp = 2): string {
  if (!Number.isFinite(value)) return '—';
  return value.toLocaleString('en-US', { minimumFractionDigits: dp, maximumFractionDigits: dp });
}

/** A price with adaptive precision (more decimals for small prices). */
export function fmtPrice(value: number): string {
  if (!Number.isFinite(value)) return '—';
  const dp = value >= 1000 ? 2 : value >= 1 ? 4 : 8;
  return value.toLocaleString('en-US', { minimumFractionDigits: dp, maximumFractionDigits: dp });
}

/** Basis points, signed. */
export function fmtBps(value: number): string {
  if (!Number.isFinite(value)) return '—';
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(1)} bps`;
}

/** A 0..1 fraction as a percentage. */
export function fmtPct(fraction: number, dp = 1): string {
  if (!Number.isFinite(fraction)) return '—';
  return `${(fraction * 100).toFixed(dp)}%`;
}

/** Microseconds, compact (shows as us or ms). */
export function fmtMicros(us: number): string {
  if (!Number.isFinite(us)) return '—';
  return us >= 1000 ? `${(us / 1000).toFixed(2)} ms` : `${Math.round(us)} µs`;
}

/** A wall-clock time (ms epoch) as HH:MM:SS. */
export function fmtTime(ms: number): string {
  return new Date(ms).toLocaleTimeString('en-US', { hour12: false });
}

/** A short relative age like "3s" or "2m". */
export function fmtAgo(ms: number, now = Date.now()): string {
  const delta = Math.max(0, now - ms);
  if (delta < 1000) return `${delta}ms`;
  if (delta < 60_000) return `${Math.floor(delta / 1000)}s`;
  if (delta < 3_600_000) return `${Math.floor(delta / 60_000)}m`;
  return `${Math.floor(delta / 3_600_000)}h`;
}

/** The direction class for a signed value. */
export function dirClass(value: number): string {
  if (value > 0) return 'text-up';
  if (value < 0) return 'text-down';
  return 'text-muted';
}
