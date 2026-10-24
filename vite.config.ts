import { defineConfig } from 'vite';
export default defineConfig({ build: { modulePreload: false, rollupOptions: { input: ['index.html', 'developer.html'] } } });
