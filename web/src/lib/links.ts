/** Astro component links bypass the Markdown link plugin. Apply the hosting base here. */
export function withBase(path: string): string {
  const base = import.meta.env.BASE_URL.replace(/\/+$/, '');
  return path.startsWith('/') && !path.startsWith('//')
    ? `${base}${path}`
    : path;
}
