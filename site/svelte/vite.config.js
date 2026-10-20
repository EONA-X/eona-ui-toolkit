import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

// Same reasons as site/react/vite.config.ts: relative base, one Svelte runtime.
export default defineConfig({
  base: './',
  plugins: [svelte()],
  resolve: { dedupe: ['svelte'] },
});
