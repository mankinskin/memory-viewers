import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

export default defineConfig({
  plugins: [preact()],
  build: {
    outDir: '../static',
    emptyOutDir: true,
  },
  resolve: {
    dedupe: ['preact', 'preact/hooks', '@preact/signals'],
  },
  server: {
    // In dev mode, proxy API calls to the ticket serve backend.
    // Change TICKET_SERVE_URL to wherever `ticket serve` is running.
    proxy: {
      '/api': process.env['TICKET_SERVE_URL'] ?? 'http://localhost:4000',
    },
  },
});
