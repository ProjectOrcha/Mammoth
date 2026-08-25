# Chapter 10 — Publishing the docs to GitHub Pages

**What you'll build:** the Mammoth docs site, live at
`https://projectorcha.github.io/Mammoth/`, rebuilding itself on every push.

**Time:** about 45 minutes, most of it waiting for the first deploy.

---

**Do this early.** You do not need any of chapters 4–9 finished. A live docs
site makes the project feel real, gives you somewhere to point people, and the
Hadoop primer alone will bring you more visitors than a feature list ever will.

## What you already have

The `web/` directory is an [Astro Starlight](https://starlight.astro.build/)
site, and it is not empty:

```
web/
├── astro.config.mjs           site config, sidebar, base path
├── package.json
├── public/                    copied to the site root as-is
│   ├── .nojekyll              stops GitHub running Jekyll over the output
│   ├── install.sh             so the curl one-liner resolves
│   └── logo.svg
└── src/
    ├── content/
    │   ├── config.ts          registers the docs collection  ← important
    │   └── docs/              15 pages of real content
    │       ├── index.mdx              landing page
    │       ├── intro/                 what, hadoop-primer, install, quickstart
    │       ├── concepts/              architecture, performance, visualization
    │       ├── cli/                   overview + generated reference
    │       ├── data/                  block size, formats, partitioning, skew
    │       ├── ops/                   configuration, operations
    │       ├── migration/
    │       └── api/
```

## Step 1 · Run it locally first

Never debug a deploy you have not seen work on your own machine.

```bash
cd web && npm install
```

```bash
npm run dev
```

Open <http://localhost:4321/Mammoth/>. **Note the `/Mammoth/` on the end** —
that is the `base` setting, and forgetting it is the most common "my site is
blank" moment.

Click around. Every page in the sidebar should load.

## Step 2 · Build it, and check the output

```bash
npm run build
```

```
[Building search indexes]
  Indexed 1 language
  Indexed 15 pages
  Indexed 1822 words

[@astrojs/sitemap] `sitemap-index.xml` created at `dist`
[build] 16 page(s) built in 2.49s
[build] Complete!
```

**"16 page(s) built" is the number that matters.** Verify it for real:

```bash
find dist -name '*.html' | sort
```

```
dist/404.html
dist/api/index.html
dist/cli/index.html
dist/cli/reference/index.html
dist/concepts/architecture/index.html
dist/concepts/performance/index.html
dist/concepts/visualization/index.html
dist/data/index.html
dist/index.html
dist/intro/hadoop-primer/index.html
dist/intro/install/index.html
dist/intro/quickstart/index.html
dist/intro/what/index.html
dist/migration/index.html
dist/ops/configuration/index.html
dist/ops/index.html
```

Check the base path made it into the links, because this is what breaks in
production and not locally:

```bash
grep -o 'href="/Mammoth/[a-z/-]*"' dist/index.html | sort -u | head
```

```
href="/Mammoth/"
href="/Mammoth/intro/hadoop-primer/"
href="/Mammoth/intro/quickstart/"
```

And that the static assets came across:

```bash
ls -la dist/logo.svg dist/install.sh dist/.nojekyll
```

Preview the real built output:

```bash
npm run preview
```

> **If `find` shows only `dist/404.html`**, the docs collection is not
> registered. Check `web/src/content/config.ts` exists and reads:
>
> ```ts
> import { defineCollection } from 'astro:content';
> import { docsSchema } from '@astrojs/starlight/schema';
>
> export const collections = {
>   docs: defineCollection({ schema: docsSchema() }),
> };
> ```
>
> Without it Astro does not know `src/content/docs/` is a collection, and the
> build succeeds while producing nothing. It is a silent failure, so it is
> worth knowing about.

## Step 3 · Commit the lockfile

CI runs `npm ci`, which **requires** `package-lock.json` and fails without it.

```bash
git status --short web/
```

```
?? web/package-lock.json
```

```bash
git add web/package-lock.json && git commit -m "chore(web): commit the lockfile"
```

`web/dist/`, `web/.astro/` and `node_modules/` are already in `.gitignore` —
build output does not belong in Git.

## Step 4 · Turn on GitHub Pages

This is a settings change, not a code change, and you only do it once. You need
admin rights on the repository.

1. Go to <https://github.com/ProjectOrcha/Mammoth/settings/pages>
2. Under **Build and deployment** → **Source**, choose **GitHub Actions**

**Not** "Deploy from a branch". That is the old `gh-pages` approach and it does
not work with the workflow below.

There is nothing else to configure. No branch, no folder.

## Step 5 · The workflow

`.github/workflows/pages.yml` already exists in the repository. Read it once so
you know what it does:

```yaml
name: Deploy site

on:
  push:
    branches: [main]
    paths: ['web/**', 'crates/**', '.github/workflows/pages.yml']
  workflow_dispatch:

permissions: { contents: read, pages: write, id-token: write }
concurrency: { group: pages, cancel-in-progress: true }

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Generate CLI reference
        run: cargo xtask docs

      - name: Build rustdoc
        run: |
          cargo doc --no-deps --workspace --all-features
          mkdir -p web/public/rustdoc && cp -r target/doc/* web/public/rustdoc/

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
          cache-dependency-path: web/package-lock.json

      - name: Build site
        working-directory: web
        run: npm ci && npm run build && touch dist/.nojekyll

      - uses: actions/upload-pages-artifact@v3
        with: { path: web/dist }

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

### Line by line, the parts that matter

| Line | Why |
| --- | --- |
| `paths: ['web/**', 'crates/**', ...]` | only rebuild when docs or code change — a README-only commit does not burn CI minutes |
| `permissions: pages: write, id-token: write` | without these, `deploy-pages` fails with a permissions error. This is the single most common cause of a failed first deploy |
| `concurrency: { group: pages }` | two pushes in a row will not race each other into a half-deployed site |
| `cargo xtask docs` | regenerates the CLI reference from the `clap` tree, so the docs cannot drift from the binary |
| `cargo doc` → `web/public/rustdoc/` | publishes the Rust API docs alongside the site |
| `touch dist/.nojekyll` | belt and braces. Without it GitHub's Jekyll would ignore any file starting with `_`, and Astro emits `_astro/` — meaning **no CSS and no JavaScript** |
| `upload-pages-artifact` → `deploy-pages` | the two-job handshake Pages requires |

> **`cargo xtask docs` is a `todo!()` until you implement it** (chapter 2's
> pattern, `clap_markdown` is the crate). Until then it will panic and fail the
> build. Comment out that step, and the `cargo doc` step too, to get your first
> deploy working — then add them back as you implement them:
>
> ```yaml
>       # - name: Generate CLI reference
>       #   run: cargo xtask docs
> ```

## Step 6 · Deploy

```bash
git push origin main
```

Then watch it: <https://github.com/ProjectOrcha/Mammoth/actions>

The first run takes 3–5 minutes (no caches yet). Later runs take about 90
seconds. You are looking for two green ticks — `build`, then `deploy`.

When it is green: **<https://projectorcha.github.io/Mammoth/>**

> **The very first deploy can 404 for a few minutes** even after the workflow
> goes green, while GitHub provisions the site. Wait five minutes and hard
> refresh (`ctrl-shift-R` / `cmd-shift-R`) before you start debugging.

## Step 7 · Deploy without pushing

Useful when you want to re-run after changing a Pages setting:

1. <https://github.com/ProjectOrcha/Mammoth/actions/workflows/pages.yml>
2. **Run workflow** → **Run workflow**

That button exists because of `workflow_dispatch:` in the trigger list.

## Adding a page

Create a markdown file under `web/src/content/docs/`. The frontmatter is the
only requirement:

```markdown
---
title: Troubleshooting
description: What to do when the cluster will not leave safe mode.
---

Your content here.
```

Save it as `web/src/content/docs/ops/troubleshooting.md` and it appears
automatically under **Operations** in the sidebar — that section uses
`autogenerate`, so you do not touch the config.

The four **Start Here** pages are listed explicitly in `astro.config.mjs`
because their order is deliberate. To add one there:

```js
{
  label: 'Start Here',
  items: [
    { label: 'What is Mammoth?', link: '/intro/what/' },
    { label: 'Hadoop in 10 minutes', link: '/intro/hadoop-primer/' },
    { label: 'Install', link: '/intro/install/' },
    { label: '5-minute cluster', link: '/intro/quickstart/' },
    { label: 'Troubleshooting', link: '/ops/troubleshooting/' },   // ← new
  ],
},
```

**Links inside content need the base path.** Write
`[install](/Mammoth/intro/install/)`, not `[install](/intro/install/)`. Get this
wrong and the link works locally and 404s in production.

## Using a custom domain

If you buy `mammoth.dev` or similar:

1. Put the bare domain in a file — no protocol, no trailing slash:

   ```bash
   echo "mammoth.dev" > web/public/CNAME
   ```

2. **Delete the `base` line** from `astro.config.mjs` and set `site`:

   ```js
   export default defineConfig({
     site: 'https://mammoth.dev',
     // base: '/Mammoth',   ← delete this
   ```

3. Then remove `/Mammoth` from every internal link. Search for them:

   ```bash
   grep -rn '/Mammoth/' web/src/
   ```

4. At your DNS provider, add four `A` records for the apex pointing at
   `185.199.108.153`, `185.199.109.153`, `185.199.110.153`, `185.199.111.153`,
   and a `CNAME` for `www` pointing at `projectorcha.github.io`.

5. Back in **Settings → Pages**, enter the domain and tick **Enforce HTTPS**
   once the certificate is issued (can take an hour).

## Check it works

Everything on this list should pass before you call the chapter done.

```bash
cd web && npm run build && find dist -name '*.html' | wc -l
```

Should print `16`.

Then, on the live site:

- <https://projectorcha.github.io/Mammoth/> loads with styling — if the text
  appears unstyled, `.nojekyll` is missing and `_astro/` is being ignored
- The sidebar shows every section
- <https://projectorcha.github.io/Mammoth/intro/hadoop-primer/> renders with its
  diagrams and tables
- The search box (top right) returns results — that is Pagefind, built at deploy
- <https://projectorcha.github.io/Mammoth/install.sh> returns the shell script,
  so `curl -fsSL .../install.sh | sh` resolves
- The logo appears on the landing page

Then prove the loop: change one word in
`web/src/content/docs/intro/what.md`, push, and watch the site update in about
90 seconds.

## If it went wrong

**The workflow fails at `deploy-pages` with a permissions error** — the
`permissions:` block is missing or Pages source is still "Deploy from a branch".
Fix the source setting first, then re-run the workflow.

**The site loads but has no CSS or JavaScript** — `.nojekyll` is missing from
the deployed output. GitHub ran Jekyll, which ignores `_astro/`. The workflow's
`touch dist/.nojekyll` handles it; check that step ran.

**Every link 404s but the home page works** — `base` and the real repository
name disagree. The repo is `Mammoth` with a capital M, so `base` must be
`/Mammoth`. It is case-sensitive.

**`npm ci` fails: "lock file not found"** — you did not commit
`web/package-lock.json`. See step 3.

**`Cannot read properties of undefined (reading 'reduce')` from
`@astrojs/sitemap`** — you have a sitemap version built for Astro 5 while the
site is on Astro 4. `web/package.json` pins it:

```json
  "dependencies": { "@astrojs/sitemap": "3.2.1" },
  "overrides":    { "@astrojs/sitemap": "3.2.1" }
```

Keep both — the `overrides` block is what stops Starlight pulling a newer one in
transitively. If you change it, delete `node_modules` and `package-lock.json`
and reinstall.

**The build succeeds but only produces `404.html`** — missing
`web/src/content/config.ts`. See the callout in step 2.

**A page is missing from the sidebar** — no `title` in its frontmatter, or it is
in a directory with no matching `autogenerate` entry in `astro.config.mjs`.

**The site does not rebuild after a push** — you changed a file outside
`web/**` and `crates/**`. Either add the path to the trigger, or use **Run
workflow**.

---

**Next:** [Chapter 11 — Where to go next](11-what-next.md)
