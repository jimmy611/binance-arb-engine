// Reads an HSL-channel CSS custom property (e.g. "215 16% 62%") and returns an
// `rgba(...)` string. The charting libraries (lightweight-charts, Recharts) draw
// to canvas/SVG and their color parsers accept rgb/rgba but not `hsl(...)`, so we
// convert here. Returning rgba keeps a single code path for opaque and
// translucent colors.
function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  const sat = s / 100;
  const lum = l / 100;
  const a = sat * Math.min(lum, 1 - lum);
  const f = (n: number): number => {
    const k = (n + h / 30) % 12;
    return lum - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  return [Math.round(255 * f(0)), Math.round(255 * f(8)), Math.round(255 * f(4))];
}

export function hsl(varName: string, alpha = 1): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  if (!raw) return `rgba(0,0,0,${alpha})`;
  const parts = raw.split(/\s+/);
  const h = Number.parseFloat(parts[0] ?? '0');
  const s = Number.parseFloat(parts[1] ?? '0');
  const l = Number.parseFloat(parts[2] ?? '0');
  const [r, g, b] = hslToRgb(h, s, l);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
