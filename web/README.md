# Mammoth website

The Astro/Starlight site presents Mammoth as durable context memory for coding
agents. Main product routes are `/intro/`, `/memory/`, and `/contributing/`.
The homepage and memory documentation are shared by main and AI_coded. Older
storage pages remain accessible as an archive and are absent from primary navigation.

Requires Node 22.12+ (22.x):

```bash
npm ci
npm run dev
npm run build
SITE_URL=https://projectorcha.github.io BASE_PATH=/Mammoth npm run build
```

Vercel serves the domain root. GitHub Pages publishes main under `/Mammoth/`.
Use `withBase` for component links and the existing Markdown link plugin for docs.
The CLI reference is generated per branch with `cargo xtask docs`.
