/**
 * Rewrites ```mermaid fences into `<pre class="mermaid">` so the client-side
 * renderer in `src/components/Head.astro` can turn them into SVG.
 *
 * This has to be a *remark* plugin rather than a rehype one: Starlight runs
 * Expressive Code over the HTML tree, and it would happily render the diagram
 * source as a syntax-highlighted code block before we ever saw it. Replacing
 * the node at the markdown stage takes the block out of Expressive Code's
 * hands entirely.
 *
 * Markdown only. In `.mdx` pages raw HTML is parsed as JSX, so write diagrams
 * there as literal `<pre class="mermaid">` markup instead of a fence.
 */

const ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;' };

export default function remarkMermaid() {
  return (tree, file) => {
    if (file?.history?.[0]?.endsWith('.mdx')) return;
    replace(tree);
  };
}

function replace(node) {
  if (!Array.isArray(node.children)) return;
  node.children = node.children.map((child) => {
    if (child.type === 'code' && child.lang === 'mermaid') {
      // The browser decodes these back before `textContent` reaches mermaid,
      // so the diagram source survives round-tripping through HTML intact.
      const source = child.value.replace(/[&<>]/g, (c) => ESCAPES[c]);
      return {
        type: 'html',
        value: `<figure class="mermaid-figure not-content"><pre class="mermaid">${source}</pre></figure>`,
      };
    }
    replace(child);
    return child;
  });
}
