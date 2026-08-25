import adapter from '@sveltejs/adapter-static';

/** The whole GUI is embedded into the binary with rust-embed, so it must build
 *  to plain static files with an SPA fallback. */
export default {
  kit: {
    adapter: adapter({ fallback: 'index.html', strict: false }),
  },
};
