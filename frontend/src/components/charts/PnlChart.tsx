import {
  createChart,
  ColorType,
  type IChartApi,
  type ISeriesApi,
  type UTCTimestamp,
} from 'lightweight-charts';
import { useEffect, useRef } from 'react';
import { useTheme } from '@/components/ui/theme';
import { hsl } from '@/lib/cssColor';
import { useStore } from '@/store/useStore';

/** Session PnL (realized + unrealized) as an area series. Recreated on theme
 *  change so the colors track the current theme. */
export function PnlChart() {
  const containerRef = useRef<HTMLDivElement>(null);
  const seriesRef = useRef<ISeriesApi<'Area'> | null>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const pnl = useStore((s) => s.pnl);
  const { theme } = useTheme();

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const chart = createChart(container, {
      autoSize: true,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: hsl('--muted'),
        fontFamily: 'JetBrains Mono Variable, monospace',
      },
      grid: {
        vertLines: { color: hsl('--border', 0.35) },
        horzLines: { color: hsl('--border', 0.35) },
      },
      rightPriceScale: { borderColor: hsl('--border', 0.6) },
      timeScale: { borderColor: hsl('--border', 0.6), timeVisible: true, secondsVisible: false },
      crosshair: { horzLine: { labelBackgroundColor: hsl('--primary') }, vertLine: { labelBackgroundColor: hsl('--primary') } },
    });

    const series = chart.addAreaSeries({
      lineColor: hsl('--primary'),
      topColor: hsl('--primary', 0.28),
      bottomColor: hsl('--primary', 0.02),
      lineWidth: 2,
      priceLineVisible: false,
    });

    chartRef.current = chart;
    seriesRef.current = series;
    return () => {
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, [theme]);

  useEffect(() => {
    const series = seriesRef.current;
    if (!series) return;
    // Dedupe to one point per second, ascending.
    const bysecond = new Map<number, number>();
    for (const p of pnl) {
      bysecond.set(Math.floor(p.ts / 1000), p.realized + p.unrealized);
    }
    const data = [...bysecond.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([time, value]) => ({ time: time as UTCTimestamp, value }));
    series.setData(data);
  }, [pnl]);

  return <div ref={containerRef} className="h-full w-full" />;
}
