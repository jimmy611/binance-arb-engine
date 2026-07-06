// Reads an HSL-channel CSS custom property (e.g. "210 22% 92%") and returns a
// canvas/SVG-usable color string. Used by the charting libraries, which need
// concrete color strings rather than Tailwind classes.
export function hsl(varName: string, alpha = 1): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  if (!raw) return alpha < 1 ? `rgba(0,0,0,${alpha})` : '#000';
  const [h, s, l] = raw.split(/\s+/);
  return alpha < 1 ? `hsla(${h}, ${s}, ${l}, ${alpha})` : `hsl(${h}, ${s}, ${l})`;
}
