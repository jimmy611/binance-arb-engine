interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  className?: string;
  strokeClass?: string;
}

/** A tiny dependency-free inline sparkline. */
export function Sparkline({ data, width = 120, height = 32, className, strokeClass = 'stroke-primary' }: SparklineProps) {
  if (data.length < 2) {
    return <svg width={width} height={height} className={className} aria-hidden />;
  }
  const min = Math.min(...data);
  const max = Math.max(...data);
  const span = max - min || 1;
  const step = width / (data.length - 1);
  const points = data
    .map((v, i) => `${(i * step).toFixed(1)},${(height - ((v - min) / span) * height).toFixed(1)}`)
    .join(' ');
  const last = data[data.length - 1];
  const cx = width;
  const cy = height - ((last - min) / span) * height;

  return (
    <svg width={width} height={height} className={className} aria-hidden>
      <polyline points={points} fill="none" className={strokeClass} strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
      <circle cx={cx - 1} cy={cy} r={1.8} className={strokeClass.replace('stroke', 'fill')} />
    </svg>
  );
}
