// Registers the `docs` collection with Starlight's schema.
//
// Without this file Astro does not know src/content/docs/ is a collection, and
// the build silently produces only a 404 page. If the site ever builds with no
// pages, check here first.
import { defineCollection } from 'astro:content';
import { docsSchema } from '@astrojs/starlight/schema';

export const collections = {
  docs: defineCollection({ schema: docsSchema() }),
};
