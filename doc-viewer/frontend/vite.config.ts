import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

export default defineConfig({
  plugins: [preact()],
  build: {
    outDir: '../static',
    emptyOutDir: true,
  },
  resolve: {
    // Ensure only one copy of preact is used (prevents hooks issues with shared components)
    dedupe: ['preact', 'preact/hooks', '@preact/signals'],
  },
  optimizeDeps: {
    exclude: ['@context-engine/viewer-api-frontend'],
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3001'
    }
  }
});
