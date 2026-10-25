import { defineConfig } from 'vite';
export default defineConfig({ build: { modulePreload: false, rollupOptions: { input: ['index.html', 'developer.html', 'repair.html', 'repair-developer.html', 'trial-a.html', 'trial-b.html', 'convoy-control-developer.html'] } } });
