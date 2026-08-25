import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// Custom domain? Put it in web/public/CNAME and delete `base` below.
export default defineConfig({
  site: 'https://sakib-dalal.github.io',
  base: '/mammoth',
  integrations: [
    starlight({
      title: 'Mammoth',
      // The logo lives in public/, so it is referenced by URL rather than
      // imported — Starlight's `logo.src` resolves through Astro's asset
      // pipeline and only accepts paths under src/.
      favicon: '/logo.svg',
      description: 'A Hadoop-class distributed storage engine in Rust.',
      social: { github: 'https://github.com/Sakib-Dalal/mammoth' },
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
