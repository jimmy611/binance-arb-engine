import { cva, type VariantProps } from 'class-variance-authority';
import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

// --- Panel ------------------------------------------------------------------

interface PanelProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  title?: ReactNode;
  actions?: ReactNode;
  bodyClassName?: string;
  scroll?: boolean;
}

export function Panel({ title, actions, children, className, bodyClassName, scroll, ...rest }: PanelProps) {
  return (
    <section className={cn('panel flex min-h-0 flex-col', className)} {...rest}>
      {(title || actions) && (
        <header className="panel-head">
          <span className="panel-title">{title}</span>
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </header>
      )}
      <div className={cn('min-h-0 flex-1', scroll && 'overflow-auto', bodyClassName)}>{children}</div>
    </section>
  );
}

// --- Button -----------------------------------------------------------------

const button = cva(
  'focus-ring inline-flex items-center justify-center gap-2 rounded-lg font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-50',
  {
    variants: {
      variant: {
        primary: 'bg-primary/90 text-bg hover:bg-primary',
        accent: 'bg-accent/90 text-bg hover:bg-accent',
        danger: 'bg-down/90 text-white hover:bg-down',
        subtle: 'bg-foreground/5 text-foreground hover:bg-foreground/10 border border-border/60',
        ghost: 'text-muted hover:bg-foreground/5 hover:text-foreground',
      },
      size: {
        sm: 'h-7 px-2.5 text-xs',
        md: 'h-9 px-3.5 text-sm',
        lg: 'h-11 px-5 text-base',
      },
    },
    defaultVariants: { variant: 'subtle', size: 'md' },
  },
);

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof button> {}

export function Button({ className, variant, size, ...rest }: ButtonProps) {
  return <button className={cn(button({ variant, size }), className)} {...rest} />;
}

// --- Badge / chip -----------------------------------------------------------

const badge = cva('chip border', {
  variants: {
    tone: {
      up: 'border-up/30 bg-up/10 text-up',
      down: 'border-down/30 bg-down/10 text-down',
      neutral: 'border-border/60 bg-foreground/5 text-muted',
      accent: 'border-primary/30 bg-primary/10 text-primary',
      warn: 'border-warn/30 bg-warn/10 text-warn',
    },
  },
  defaultVariants: { tone: 'neutral' },
});

interface BadgeProps extends HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badge> {}

export function Badge({ className, tone, ...rest }: BadgeProps) {
  return <span className={cn(badge({ tone }), className)} {...rest} />;
}

// --- Stat tile --------------------------------------------------------------

interface StatProps {
  label: ReactNode;
  value: ReactNode;
  hint?: ReactNode;
  className?: string;
}

export function Stat({ label, value, hint, className }: StatProps) {
  return (
    <div className={cn('flex flex-col gap-0.5', className)}>
      <span className="text-2xs uppercase tracking-wider text-muted">{label}</span>
      <span className="num text-lg font-semibold leading-tight text-foreground">{value}</span>
      {hint && <span className="text-2xs text-muted-foreground">{hint}</span>}
    </div>
  );
}

// --- Empty state ------------------------------------------------------------

export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-h-24 items-center justify-center p-6 text-center text-sm text-muted-foreground">
      {children}
    </div>
  );
}
