import { sveltekit } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    // `npm run dev` talks to a real gateway: mammoth serve --role gateway
    proxy: { '/api': 'http://localhost:8080' },
  },
});
