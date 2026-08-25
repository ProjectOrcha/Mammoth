# Logo assets

Canonical source of truth for Mammoth branding. Everything else in the repo is a
copy generated from here by `cargo xtask assets`.

| File | Size | Use |
| --- | --- | --- |
| `mammoth-logo.svg` | 8.3 MB | Original vector export. Used by the repository `README.md`. |
| `mammoth-logo.min.svg` | 2.8 MB | Losslessly optimized drop-in for the above. Copied to `ui/static/logo.svg` and `web/public/logo.svg`. |
| `mammoth-logo.jpg` | 1.2 MB | Raster. Social preview card, slide decks, anywhere SVG is awkward. |
| `mammoth-cli-logo.txt` | 7.7 KB | ASCII art. Copied to `crates/mammoth-cli/assets/banner.txt` and `include_str!`d into the binary for `mammoth quickstart`. |

## A note on the SVG size

`mammoth-logo.svg` is a pixel-by-pixel trace of a raster image: 146,231
`<rect>` elements, one per run of same-coloured pixels. It is a valid SVG and it
renders correctly, but it is not vector art in any useful sense — it does not
scale gracefully and it is large enough to be slow in a browser.

`mammoth-logo.min.svg` is the same image with vertically contiguous runs merged
into single rects: 48,650 elements, 2.8 MB, pixel-identical output. It was
produced losslessly and is a safe drop-in anywhere the original is used.

**Both are much larger than a logo should be.** Before the first public release,
redraw the mammoth as real paths — a proper vector logo of this complexity
should land somewhere between 20 and 80 KB. Until then, prefer
`mammoth-logo.min.svg` in anything that loads over a network, and note that
GitHub's image proxy may decline to render the 8.3 MB original.

## Regenerating the copies

```bash
cargo xtask assets   # not implemented yet — copy by hand until M1
```

Copies `mammoth-logo.min.svg` into `ui/static/logo.svg` and `web/public/logo.svg`,
and `mammoth-cli-logo.txt` into `crates/mammoth-cli/assets/banner.txt`.
