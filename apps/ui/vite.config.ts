import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri expects a fixed port in development and relative asset paths in builds.
export default defineConfig({
  plugins: [react()],
  base: './',
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: process.env.TAURI_DEV_HOST || 'localhost' },
  build: { target: 'es2021', outDir: 'dist' },
  test: { environment: 'node' },
});
