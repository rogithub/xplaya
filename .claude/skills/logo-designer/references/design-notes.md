# Design notes

Read this before changing the algorithm, adding a font, or explaining a choice to the user. `SKILL.md` has the workflow; this file has the reasons and the traps.

## Decisions already made (2026-09-19)

- **Palette** sampled from the original renders: teal `#7DC5C3`, mustard `#EBB728`, orange `#EC652A`. `metalico.jpeg` (flat enamel) is the cleanest source; `line.jpg` is shaded by its lighting, so its colors read darker. Ring copper `#A0603E` is a midpoint of `circleai.jpg`'s ring, which varies from `#774930` to `#AB7355` with the light. Letter order is teal, mustard, orange, teal, mustard, orange, teal, mustard, teal, as in `line.jpg`.
- **Font: Fredoka** (SIL OFL 1.1). Chosen by rendering five rounded candidates against `line.jpg`: Fredoka Bold matched the round terminals and weight; Nunito 900 was thinner; three others failed to parse (see pitfalls).
- **Three official versions, all kept on purpose:** v1 Fredoka Bold capitals with slab I (closest to the original renders), v2 Fredoka SemiBold "Papelería" in title case with the accent (the same face and weight as the lab page's own title, which is what the user asked for), v3 Fredoka SemiBold capitals with slab I.
- **Slab-serif I** (top and bottom bars, pill-shaped ends) is drawn by hand because Fredoka has a plain I. It is the brand's signature letter in every original render. v2 drops it because the í is a plain letter.
- **Ring width:** first 57 % of the stem, then the user compared it on the lab slider and set **100 % as official** (the ring is as thick as the letter strokes). The gap between letters and ring stays constant; the ring grows outward.
- **Ring-less two-line files** exist so the logo can go inside a rectangle or any other shape.
- **Contrast:** the palette was designed on dark backgrounds. Teal is 2.0:1 and mustard 1.9:1 on white, but 8.7:1 and 9.3:1 on `#231916`. Presets for light grounds: teal `#2A8783`, orange `#D9501A`, mustard `#B98600`.

## Tuning ratios (multiply by the stem thickness)

Stem = width of the plain `I` glyph: 121 at weight 500, 152 at 600, 183 at 700. Cap height is about 700 at all three. A config in font units follows from these; the values in `assets/papeleria/` are these ratios, rounded.

| Parameter | Ratio | Notes |
|---|---|---|
| `spacing.minGap` | 0.52 | Closest approach between neighbours. |
| `spacing.minGapWith.A` | 0.34 to 0.49 | 0.34 was fine at Bold (v1); at SemiBold (v3) the P and A nearly touched, 0.49 fixed it. Start at 0.45. |
| `spacing.pad` | 0.49 | Same margin all around the one-line and stacked logos. |
| `spacing.lineGap` | 0.52 | Closest approach between the two lines. |
| `ring.margin` | 0.55 | Farthest letter point to the ring's inner edge. |
| `ring.widthPct` | 100 | Official. |
| `slab.w` / `slab.b` / `slab.r` | 2.4 / 0.61 / 0.30 | Bar width, bar thickness, fillet where the bars meet the stem. |

Title-case (v2) uses the optical term as well (`T` 110, `depth` 110, `minGap` 60, `lineGap` 100) because round lowercase letters need to sit closer than straight ones.

## How the layout works, and why

- **Horizontal spacing.** Each glyph is rasterized (0.5 px per unit) and its left and right silhouette is read row by row. The advance to the next glyph is `max(mean clamped gap + T, worst row gap + floor)`. With `T = 0` it is pure closest approach, which is what the capital versions use: in trials the floor decided every pair, so the optical mean was doing nothing. Font metrics were rejected because the hand-drawn I has no metrics, and because a heavy rounded font is spaced too tight by default.
- **Diagonals.** Pairs with an A leave a triangular hole, so they get a smaller floor (`minGapWith`). Otherwise I-A looks loose next to the rest.
- **Circular block.** Each line is centered on x = 0. The circle center is the middle of the block (band or ink), not the minimum enclosing circle: that one balances the corners, not what the eye sees, and pushed the text toward the top.
- **Two lines.** `band` stacking is cap height plus `lineGap` (fine for all capitals). `columns` compares the bottom silhouette of line one with the top silhouette of line two column by column, so a descender (the p) can tuck between letters of the next line without touching the accent or an ascender.
- **Ring.** `R = farthest letter distance + margin + width/2`; the `viewBox` is `R + width/2 + 8` units on each side (the 8 keeps antialiasing from clipping). A thicker ring only changes the outer edge.
- **Vertical margin of the one-line logo.** `extents: cap` uses the cap band so letters with overshoot (A, R) do not make the margin uneven; `ink` uses the real ink (needed with descenders and accents).

## Painting contract

Every letter has `fill="#hex"` as an attribute and a class rule `fill:var(--<prefix>-N, var(--<prefix>-<role>, #hex))` in an embedded `<style>`. Result: as `<img>` or in Illustrator/Inkscape/Figma the attribute wins; inline in HTML, CSS variables on the SVG or any ancestor repaint by role or by letter. A page rule beats a presentation attribute, which is why the fallback hex can live in both places.

## Pitfalls we hit

1. **`opentype.js` `toPathData` returned `NaN`** for some x offsets (one glyph lost its coordinates and vanished from the render). `build-logo.js` writes paths with its own serializer and throws on any non-finite number.
2. **XML comments cannot contain `--`**, and the CSS variable names start with it. The variable docs live in a CSS comment inside `<style>`. A literal `<img>` inside that comment also broke the XML (read as an unclosed tag).
3. **Some woff subsets parse into broken outlines** in `opentype.js`: Baloo 2 800, M PLUS Rounded 1c 900 and Varela Round 400 lost letters (R, E, and a stray shape). Render every letter you need before choosing a font.
4. **The local renderer ignores CSS variables** (librsvg via sharp). Previews therefore always show the fallback colors, and painting by variables is unverified until someone opens the lab page in a browser. jsdom (`test-lab.js`) proves the page's logic only, not layout or rendering.
5. **A bare `<img>` cannot be repainted from page CSS** and, without width or height, stretches to its container. The SVGs have no width or height on purpose.
6. **Duplicate ids** appear as soon as several logos share a page. `build-lab.js` suffixes every id per version.
7. **Fonts with a serif I** make the stem measurement wrong: set `stem` in the config.
8. **The accent:** `í` gets the id `letra-8-i` (ASCII ids).
9. **Source renders have traps** (see `poster-designer/brands/papeleria.md`): `metalico.jpeg` has its "transparent" checkerboard baked in, and two files carry a small generator watermark. Do not trace or reuse them without cropping.

## Icons and the badge

- The badge (`papeleria-insignia.svg`) is the v3 circular logo with its `disco` circle filled `#231916`. Icons use it because they need a ground of their own; the transparent circular logo is meant for dark posters or for a dark panel behind it.
- Sizes: `favicon.ico` holds 16, 32 and 48 px as PNG-in-ICO; at 16 px the badge is a dark dot with color specks, which is expected for a tab icon (32 px still reads). `favicon.svg` is the badge itself for browsers that take SVG.
- `apple-touch-icon` (180) and the manifest icons (192, 512) are opaque dark squares with the badge centered at 86 % and 74 % of the side; 74 % keeps it inside the maskable safe zone (central circle of 80 % of the width).
- `site.webmanifest` lists each size twice, purpose `any` and `maskable`, with absolute paths under `/static/img/favicon/`. The previous manifest said "MyWebSite" and pointed at paths that did not exist.
- Rasterizing: the SVG is rendered at 2x the target size or more and downsampled, otherwise small sizes look soft.
- The social images `static/img/og_*.jpeg` still carry the old 3D logo baked into AI photos (some with garbled text). They are not part of this set and were left for later.

## The lab page

`assets/logo-lab.template.html` is fully data-driven: colors, roles, letters, presets and versions come from `lab.json` and the configs. Behaviors worth knowing:
- Ring width is a percentage of the stem; the official value is 100 % and the readout says "oficial" at that value.
- Ring off hides `aro` and `disco` and sets the `viewBox` to the letters' bounding box plus `data-pad`, the same margin as the `-dos-lineas.svg` files (this uses `getBBox`, so it needs a real browser).
- The size strips are clones of each version's logos taken after the ring settings are applied, and are rebuilt when the ring changes.
- `test-lab.js` covers the controls with 55 checks; keep it passing when editing the template.
