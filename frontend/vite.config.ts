import { fileURLToPath, URL } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Single-page trading terminal. `@` aliases `src`. Heavy vendor code is split so
// the shell paints before the charting libraries load.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  build: {
    target: 'es2021',
    sourcemap: false,
    chunkSizeWarningLimit: 1100,
    rollupOptions: {
      output: {
        manualChunks: {
          react: ['react', 'react-dom', 'react-router-dom'],
          charts: ['lightweight-charts', 'recharts'],
          data: ['@tanstack/react-query', '@tanstack/react-virtual', 'zustand'],
        },
      },
    },
  },
  server: { port: 5173, strictPort: false },
});
