import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import remarkMermaid from './plugins/remark-mermaid.mjs';

// Custom domain? Put it in web/public/CNAME and delete `base` below.
export default defineConfig({
  site: 'https://projectorcha.github.io',
  base: '/Mammoth',
  // Diagrams are ```mermaid fences everywhere — in these pages, in the
  // README and in docs/guide/. The plugin hands them to the client-side
  // renderer wired up in src/components/Head.astro.
  markdown: { remarkPlugins: [remarkMermaid] },
  integrations: [
    starlight({
      title: 'Mammoth',
      // The logo lives in public/, so it is referenced by URL rather than
      // imported — Starlight's `logo.src` resolves through Astro's asset
      // pipeline and only accepts paths under src/.
      favicon: '/logo.svg',
      description: 'A Hadoop-class distributed storage engine in Rust.',
      social: { github: 'https://github.com/ProjectOrcha/Mammoth' },
      components: { Head: './src/components/Head.astro' },
      // Cinzel for Roman capitals, EB Garamond for the text face, Courier
      // Prime for the letterspaced micro-labels. Every stack in mammoth.css
      // names a system fallback, so the site still reads if these never load.
      head: [
        {
          tag: 'link',
          attrs: { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
        },
        {
          tag: 'link',
          attrs: {
            rel: 'preconnect',
            href: 'https://fonts.gstatic.com',
            crossorigin: '',
          },
        },
        {
          tag: 'link',
          attrs: {
            rel: 'stylesheet',
            href:
              'https://fonts.googleapis.com/css2?family=Cinzel:wght@400;500;600&family=EB+Garamond:ital,wght@0,400;0,500;0,600;1,400&family=Courier+Prime:wght@400;700&display=swap',
          },
        },
      ],
      customCss: ['./src/styles/mammoth.css'],
      sidebar: [
        {
          label: 'Start Here',
          items: [
            { label: 'What is Mammoth?', link: '/intro/what/' },
            { label: 'Hadoop in 10 minutes', link: '/intro/hadoop-primer/' },
            { label: 'Install', link: '/intro/install/' },
            { label: '5-minute cluster', link: '/intro/quickstart/' },
          ],
        },
        { label: 'Concepts', autogenerate: { directory: 'concepts' } },
        { label: 'CLI', autogenerate: { directory: 'cli' } },
        { label: 'Data Guide', autogenerate: { directory: 'data' } },
        { label: 'Operations', autogenerate: { directory: 'ops' } },
        { label: 'Migration', autogenerate: { directory: 'migration' } },
        { label: 'API', autogenerate: { directory: 'api' } },
      ],
    }),
  ],
});
