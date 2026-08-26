/**
 * Prefixes root-relative links and image sources with Astro's `base`.
 *
 * Pages are written with plain absolute paths — `/concepts/fast-paths/` — so
 * the source reads the same whichever host it is built for. On Vercel the base
 * is `/` and this plugin does nothing at all. On GitHub Pages the site lives
 * under /Mammoth/, and every one of those paths would 404 without the prefix.
 *
 * rehype rather than remark: by the HTML stage there is one shape to handle —
 * `href`/`src` on an element — instead of markdown links, images, reference
 * links and link definitions each in their own node type.
 *
 * Frontmatter never reaches this pipeline, so the landing page's hero links are
 * prefixed in src/components/Hero.astro instead.
 */

// The attribute carrying a URL, per element we care about. Anything else with
// a root-relative URL (a bare <a> written in raw HTML, say) is not something
// these pages do.
const URL_ATTR = { a: 'href', area: 'href', img: 'src' };

export default function rehypeBaseLinks(base) {
  const prefix = base.replace(/\/+$/, '');
  if (!prefix) return () => () => {};
  return () => (tree) => walk(tree, prefix);
}

function walk(node, prefix) {
  const attr = URL_ATTR[node.tagName];
  const value = attr && node.properties?.[attr];

  // Root-relative only: `//cdn.example.com/x` is protocol-relative and absolute
  // already, and a path that starts with the prefix has been handled.
  if (
    typeof value === 'string' &&
    value.startsWith('/') &&
    !value.startsWith('//') &&
    value !== prefix &&
    !value.startsWith(`${prefix}/`)
  ) {
    node.properties[attr] = prefix + value;
  }

  if (Array.isArray(node.children)) {
    for (const child of node.children) walk(child, prefix);
  }
}
