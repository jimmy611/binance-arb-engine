import { cn } from '@/lib/cn';
import { dirClass } from '@/lib/format';

interface ValueProps {
  value: number;
  format: (v: number) => string;
  colorBySign?: boolean;
  className?: string;
}

/** A right-aligned, tabular-numeric value, optionally colored by sign. */
export function Value({ value, format, colorBySign, className }: ValueProps) {
  return (
    <span className={cn('num tabular-nums', colorBySign && dirClass(value), className)}>
      {format(value)}
    </span>
  );
}
