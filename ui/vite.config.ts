// `sveltekit()` comes from @sveltejs/kit/vite, not from the Svelte plugin —
// it wraps @sveltejs/vite-plugin-svelte and adds the routing, so importing it
// from the wrong package fails at config load with a confusing message.
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 5173,
    // `npm run dev` talks to a real gateway if one is running:
    //   mammoth serve --role gateway
    // If nothing answers, the client falls back to the simulated cluster in
    // src/lib/demo.ts and says so in the header — see src/lib/api.ts.
    // With no gateway running, vite logs an ECONNREFUSED for the first probe
    // and the client switches to the simulated cluster. That one log line is
    // the honest signal that nothing is listening — leave it.
    proxy: { '/api': { target: 'http://localhost:8080', changeOrigin: true } },
  },
});
