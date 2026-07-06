import type { Config } from 'tailwindcss';

// Colors are CSS custom properties (see src/index.css) so light/dark swap by
// changing variables rather than Tailwind `dark:` variants. Directional colors
// (up/down) and the accents (cyan/amber) are first-class tokens because they
// carry meaning on the data surfaces.
const config: Config = {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        bg: 'hsl(var(--bg) / <alpha-value>)',
        'bg-elev': 'hsl(var(--bg-elev) / <alpha-value>)',
        surface: 'hsl(var(--surface) / <alpha-value>)',
        panel: 'hsl(var(--panel) / <alpha-value>)',
        foreground: 'hsl(var(--foreground) / <alpha-value>)',
        muted: 'hsl(var(--muted) / <alpha-value>)',
        'muted-foreground': 'hsl(var(--muted-foreground) / <alpha-value>)',
        border: 'hsl(var(--border) / <alpha-value>)',
        ring: 'hsl(var(--ring) / <alpha-value>)',
        primary: 'hsl(var(--primary) / <alpha-value>)',
        accent: 'hsl(var(--accent) / <alpha-value>)',
        up: 'hsl(var(--up) / <alpha-value>)',
        down: 'hsl(var(--down) / <alpha-value>)',
        warn: 'hsl(var(--warn) / <alpha-value>)',
      },
      fontFamily: {
        sans: ['Inter Variable', 'Inter', 'system-ui', 'sans-serif'],
        mono: [
          'JetBrains Mono Variable',
          'JetBrains Mono',
          'ui-monospace',
          'SFMono-Regular',
          'Menlo',
          'monospace',
        ],
      },
      fontSize: { '2xs': ['0.6875rem', { lineHeight: '0.9rem' }] },
      borderRadius: { xl: '0.9rem', '2xl': '1.15rem' },
      boxShadow: {
        glass: '0 1px 0 0 hsl(var(--border) / 0.5) inset, 0 16px 48px -18px rgba(0,0,0,0.6)',
        glow: '0 0 0 1px hsl(var(--primary) / 0.35), 0 0 26px -6px hsl(var(--primary) / 0.55)',
      },
      keyframes: {
        'flash-up': {
          '0%': { backgroundColor: 'hsl(var(--up) / 0.22)' },
          '100%': { backgroundColor: 'transparent' },
        },
        'flash-down': {
          '0%': { backgroundColor: 'hsl(var(--down) / 0.22)' },
          '100%': { backgroundColor: 'transparent' },
        },
        blink: { '0%,100%': { opacity: '1' }, '50%': { opacity: '0.3' } },
      },
      animation: {
        'flash-up': 'flash-up 0.55s ease-out',
        'flash-down': 'flash-down 0.55s ease-out',
        blink: 'blink 1.5s ease-in-out infinite',
      },
    },
  },
  plugins: [],
};

export default config;
