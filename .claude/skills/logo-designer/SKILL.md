---
name: logo-designer
description: Build, adjust, regenerate and verify the papelería's logos as paintable, transparent SVG wordmarks (one-line, circular with ring, two-line without ring) from an open-license font, and build the "logo lab" page to compare versions and repaint them. Use this skill whenever the user wants a new logo version or variant, changes letter spacing, font weight, colors, the slab-serif I or the ring width, wants the logo without the ring, asks to regenerate or check the official logos in static/img/logo, or wants a page to compare and repaint logo versions. Trigger even when they only say "hazme otra versión del logo", "cambia el aro", "el logo en otro tipo de letra" or "quiero ver los logos juntos".
---

# Logo designer

Turns a word plus an open font into logo SVGs that are transparent, resizable and repaintable letter by letter. It is the tooling behind the three official papelería logos. Everything here runs with Node and produces plain SVG files; nothing needs an image generator.

Use the `poster-designer` skill to *use* the logos in posters (its `brands/papeleria.md` holds the usage rules). Use this one to *change or create* them.

## What is here

| Path | What it is |
|---|---|
| `scripts/build-logo.js` | Config in, three SVGs out: `<slug>-linea.svg`, `<slug>-circular.svg`, `<slug>-dos-lineas.svg`. |
| `scripts/build-rasters.js` | The official SVGs in, PNG (transparent) and JPG (on the dark brand ground) out, 3000 px on the longest side, next to each SVG. |
| `scripts/build-og.js` | `og.json` in, the site's social preview images (`og_*.jpeg`) out, in the Swiss style with the v3 logo: headline plus a ruled list, no photos, no contact data. |
| `scripts/build-icons.js` | Badge in, the site's icon set out: `favicon.svg`, `favicon.ico`, `favicon-96x96.png`, `apple-touch-icon.png`, the two manifest icons and `site.webmanifest`. |
| `scripts/verify.js` | Checks the SVGs (well-formed, no NaN, transparent corners) and renders `preview.png` on white, paper and dark. |
| `scripts/build-lab.js` | Assembles the comparison/repaint page from the SVGs on disk. |
| `scripts/test-lab.js` | Regression test that drives the page's controls in jsdom. Run it after touching the template. |
| `assets/logo-lab.template.html` | The lab page template (data-driven, no brand hard-coded). |
| `assets/papeleria/` | `v1.json`, `v2.json`, `v3.json` (one config per official version), `lab.json` (the lab manifest), `icons.json` (the icon set) and `og.json` (the social images: text, and where each one is used). |
| `assets/fonts/` | Fredoka SemiBold (600) and Bold (700), and Archivo Medium (500), Bold (700) and Black (900), with their SIL OFL licenses. |
| `references/design-notes.md` | Why things are built this way, tuning ratios and every pitfall we hit. Read it before changing the algorithm or adding a font. |

Official assets live in `static/img/logo/{v1,v2,v3}/`, three files each. The configs in `assets/papeleria/` reproduce them exactly (verified byte for byte when this skill was written, except the header comment).

## Setup (once per session)

The repo has no `node_modules`, so install the dependencies in a scratch directory outside it and point `NODE_PATH` there:

```bash
mkdir -p /tmp/logo-work && cd /tmp/logo-work && npm init -y >/dev/null && npm install opentype.js sharp jsdom
export NODE_PATH=/tmp/logo-work/node_modules
SK=/home/ro/code/xplaya/.claude/skills/logo-designer
```

`jsdom` is only for `test-lab.js`. If the session has a scratchpad directory, use it instead of `/tmp`.

## Workflows

### Regenerate or verify the official logos

```bash
for v in v1 v2 v3; do
  node $SK/scripts/build-logo.js $SK/assets/papeleria/$v.json /tmp/logo-work/out/$v
  node $SK/scripts/verify.js /tmp/logo-work/out/$v papeleria
done
```

Compare with `static/img/logo/$v/` (ignore line 2, the header comment). Look at each `preview.png` with the Read tool; a passing check does not mean it looks right.

### Change a version or add a new one

1. Copy the closest config in `assets/papeleria/` to a new name and edit it (fields below). Change one thing at a time.
2. Build into a scratch folder, run `verify.js`, and **look** at `preview.png`. Spacing and proportions are judged by eye, so build two or three variants (for example different `minGap`) and compare them in one image before picking.
3. Ask the user which one they prefer when it is a taste call (weight, case, ring width). Show them side by side; the lab page is the best way.
4. Install the winner in `static/img/logo/<id>/` (use `git mv` when moving tracked files), add it to `assets/papeleria/lab.json`, then rebuild the site page (see "Keeping the lab") and run `test-lab.js`.
5. If the set of official logos changed, update the logo section of `.claude/skills/poster-designer/brands/papeleria.md`.

### Social preview images (`og_*.jpeg`)

The images WhatsApp and other apps show when a link is shared. `assets/papeleria/og.json` holds, for each file, its headline, the short list of what the page offers (all taken from the page itself, no contact data) and the pages that use it (`usedIn`). The generator draws them in the Swiss style: white ground, the v3 logo in its light-ground colors, an Archivo Black headline sized to fit, a thin-ruled list at the bottom. Text is converted to outlines, so the output does not depend on system fonts and can be rendered and looked at here.

```bash
cd /home/ro/code/xplaya
node $SK/scripts/build-og.js $SK/assets/papeleria/og.json /tmp/logo-work/og --root . --png     # preview in a scratch folder
node $SK/scripts/build-og.js $SK/assets/papeleria/og.json static/img --root .                   # replace the site's images
```

- **Layouts.** Every image uses the Swiss layout (headline plus ruled list) unless its entry in `og.json` has `"layout": "ticket"`; that one is drawn as a big illustrated receipt on a teal panel (the v3 logo on the ticket, abstract rows, a QR-like icon, a check badge) with only a two-line headline and a `caption`. `og_recibo.jpeg` is the only one that uses it. The panel color and its shadow are `panel` / `panelShadow` in `og.json`.
- **Size:** the site's files are **1024 × 541**, not the 1200 × 630 that several templates declare in `og:image:width` / `og:image:height`. Keep the file size; correcting the declared numbers is a separate decision.
- **The list is secondary.** WhatsApp shows the image about 300 px wide, so only the headline and the logo survive at that size. Keep headlines short and the list to 6 items at most (one column up to 3 items, two columns from 4).
- **Cache.** Apps cache each image by URL, and `/static` is `immutable` for a year: when the images are replaced, add `?v=` to every `og:image` and `twitter:image` URL in the templates.
- `og_producto.jpeg` is used only when a product has no photo. `og_catalogo.jpeg` and `og_futbol.jpeg` (the World Cup one) were used by no page and were deleted.
- **Status: applied.** `static/img` holds exactly the eight generated `og_*.jpeg`. Every `og:image` and `twitter:image` in the templates carries `?v=2` and declares 1024 × 541 (a product's own photo, which is external, keeps its 1200 × 630). When the images change, bump that `?v=` in all the templates and regenerate the logo page.

### PNG and JPG exports

Each logo folder also holds a PNG and a JPG of every shape (`papeleria-linea`, `-circular`, `-dos-lineas`), made from the SVGs so that people can use the logo where a vector is not accepted. PNG keeps the transparency; **JPG cannot be transparent, so it sits on the brand's dark brown `#231916`**, the ground the logo's colors are made for (on white the teal and mustard fail contrast). The public page offers all three formats for every version.

```bash
cd /home/ro/code/xplaya
for v in v1 v2 v3; do node $SK/scripts/build-rasters.js static/img/logo/$v papeleria; done
```

Whenever an SVG in `static/img/logo/` changes, regenerate its PNG and JPG **and** the site page (next section), in that order: the page's download links carry a hash of each file.

### Site icons and the in-page logo

The site's favicons, home-screen icons and in-page logo come from the **badge**: `static/img/logo/v3/papeleria-insignia.svg`, the v3 circular logo on its dark disc (`#231916`). It is generated with the other v3 files because `v3.json` has `"badge": {"disc": "#231916"}`; the disc carries the dark ground, so the badge reads on white and on dark pages alike. Do not use the plain transparent circular logo for icons: its default colors need a dark ground.

```bash
cd /home/ro/code/xplaya
node $SK/scripts/build-logo.js $SK/assets/papeleria/v3.json static/img/logo/v3      # includes the badge
node $SK/scripts/build-icons.js $SK/assets/papeleria/icons.json --root .            # writes static/img/favicon/*
```

`icons.json` holds the badge path, the output folder, the dark color, the theme color and the app names. The manifest icons keep the badge inside the central 80 % circle (maskable safe zone); `apple-touch-icon` is an opaque square because iOS paints transparency black.

**Cache.** `/static` is served with a one-year `immutable` cache, so a changed file at the same URL is not refreshed for returning visitors. Every template reference to these files carries `?v=3`; **bump that number in all of them when the icons or the badge change** (`grep -rn "?v=3" templates`).

### Build the lab page

```bash
cd /home/ro/code/xplaya
node $SK/scripts/build-lab.js $SK/assets/papeleria/lab.json /tmp/logo-work/lab.html --root .
node $SK/scripts/test-lab.js /tmp/logo-work/lab.html
```

The page shows every version stacked, with controls for background (including none), colors by role, one letter at a time, ring on/off, ring width, size, real-size strips and a copyable CSS snippet. It is a single self-contained HTML file. Publish it as an Artifact for a quick private look; the permanent copy is served by the site, see "Keeping the lab" below.

### A new brand or font

Read `references/design-notes.md` first. In short: pick an open-license font, render candidates next to the reference and discard any whose glyphs come out broken, measure it (the script reads cap height and stroke from the `E` and `I` glyphs), write a config, and iterate as above. Convert glyphs to outlines only when the font license allows it (OFL does).

## Config fields (`assets/papeleria/vN.json`)

All lengths are font units (1000 per em). Rules of thumb are in stem-thickness ratios; `references/design-notes.md` lists them.

| Field | Meaning |
|---|---|
| `slug`, `title`, `label`, `note` | File prefix, the `<title>` text, and the header comment strings. |
| `font` | woff file name, looked up next to the config and then in `assets/fonts/`. |
| `letters`, `lines` | The word, and how it splits into two lines for the circular/stacked forms (`["PAPE","LERIA"]`). |
| `cssPrefix`, `classPrefix` | CSS variable prefix (`--papeleria-teal`) and class prefix (`pap-1`). |
| `roles`, `colors`, `ringColor` | Color role of each letter, the palette, the ring color. |
| `slab` | `{w,b,r}`: replaces the letter `I` with a slab-serif I (bar width, bar thickness, fillet radius). Omit for a plain glyph. |
| `spacing` | `minGap` closest-approach floor between neighbours; `minGapWith` `{"A": n}` smaller floor for pairs with that letter; `pad` margin around the one-line and stacked logos; `lineGap` between the two lines; `T`/`depth` optional optical term (0 = floor only). |
| `layout` | `extents` `cap` or `ink` (vertical margin from the cap band or the real ink); `stacking` `band` or `columns` (how the lines are separated); `center` `band` or `ink` (how the block is centered in the ring). Defaults: `cap` for all-caps words else `ink`, `columns`, `ink`. |
| `ring` | `margin` (gap from the farthest letter to the ring's inner edge), `widthPct` (percent of the stem; 100 is official) or `width` (absolute). |

## Rules that matter (the why is in the notes)

- **The ring grows outward.** Inner radius is fixed by the letters plus `margin`; a thicker ring only enlarges the outer edge and the `viewBox`. Never thicken a ring in place, it would eat the margin.
- **One letter, one path, ids in reading order.** `letra-N-X`, class `<classPrefix>-N`. Colors come from CSS variables with the plain hex as `fill` attribute fallback, so the file works inline, in `<img>`, and in editors.
- **Transparent means nothing behind the letters.** No `<rect>`; `verify.js` checks the corners' alpha.
- **Spacing is closest approach, not font metrics.** A heavy rounded font needs an even gap between the nearest points; pairs with diagonals get a smaller floor.
- **Do not claim what you could not see.** The local renderer (librsvg through sharp) ignores CSS variables, and jsdom has no layout, so neither can prove that variable painting or `getBBox` works in a browser. Say so, and ask the user to try the lab page.

## Keeping the lab

The page is public at **`xplaya.com/logo-oficial`**: route `logo_oficial` in `src/routes/pages.rs`, template `templates/pages/logo-oficial.html`. That template is generated, never edited by hand:

```bash
cd /home/ro/code/xplaya
node $SK/scripts/build-lab.js $SK/assets/papeleria/lab.json templates/pages/logo-oficial.html --root . --site
```

- **Regenerate it every time a file in `static/img/logo/` changes.** The logos are inlined in the page, so a stale page shows old logos, and `test-lab.js` will not notice.
- `--site` makes it a standalone document (it does not extend `base.html`, whose Bulma would fight the page's own CSS), marks it `noindex`, adds the site's social meta, wraps the generated markup in `{% raw %}` so minijinja leaves the CSS and JS alone, and adds a download table under each version: one row per shape (line, circular, two lines) and one button per format found on disk (SVG, PNG, JPG). `manifest.site.downloadNote` is the explanation printed under the versions.
- The download links carry `?v=<hash of the file>` because `/static` is served with a one-year `immutable` cache; the hash changes only when that file does.
- `manifest.site` in `lab.json` holds the page title, description and social image. Remove `noindex` in `siteDocument()` only if the user wants the page found by search engines.
- To test the served page: run the site on a spare port (`PORT=3199 cargo run`), save the response of `/logo-oficial`, run `test-lab.js` on it, then stop only that process (find its pid with `ss -ltnp | grep :3199`; do not `pkill` by name, the user may have their own server running).
- Adding the route was authorized by the user (2026-09-19): the logos are public brand information.

An Artifact is private, lives on the user's Claude account and is not a backup, so never treat it as the source of truth. The source of truth is this skill plus `static/img/logo/`; `build-lab.js` recreates the page at any time.
