// The whole GUI is embedded into the binary with rust-embed and served with an
// SPA fallback, so there is no server to render on and nothing to prerender:
// every page's content comes from the gateway API at runtime.
export const ssr = false;
export const prerender = false;
