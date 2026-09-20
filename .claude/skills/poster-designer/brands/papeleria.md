# Brand profile: Papelería xplaya (own posters)

Read this only for posters made **for the papelería itself**. For a client's poster use that client's own profile (`_template.md`) and never borrow anything from here.

## Identity and facts

Facts below come from the public site's own source, so they are safe to reuse; confirm hours and prices with the user before printing, because they change.

| | |
|---|---|
| Names | Site and brand: "Papelería xplaya.com". Local name: Papelería y Mercería "El Gordo". The wordmark in every logo reads PAPELERIA (capitals versions) or Papelería (v2). Do not "correct" or mix them without asking. |
| Website | xplaya.com (catalog, prices for copies, photos, FAQ, Monedero) |
| WhatsApp | +52 452 201 8336 (`https://wa.me/524522018336`; a QR of that link is a good default call to action) |
| Email | papeleria@xplaya.com |
| TikTok | @papeleria.xplaya.com |
| Address | Villas del Sol, a un costado de Palmas Turquesa, Playa del Carmen, Quintana Roo, C.P. 77726 |
| Hours | Monday to Friday 6:30–18:00, Saturday 9:00–14:00 (confirm) |
| Services | Copies and prints, special paper, enmicado, engargolado, scanner, online paperwork (CURP, acta de nacimiento), photos (carnet, trámites, polaroid), ID photo from home, curtains made to measure, free WiFi to send documents, Monedero Electrónico (cashback), imagina (photo sheets for other stationers). |

Sources: `src/routes/pages.rs` (`llms_txt`), `templates/base.html` (WhatsApp link), `templates/pages/faq.html`. **Prices live in `templates/pages/impresiones.html`, `fotos.html` and the product database. Never quote a price that the user did not give you or that a page does not state.**

## Voice

Spanish (México), informal "tú", direct and friendly, short sentences. Examples from the site: "Trae tu diseño, nosotros lo imprimimos", "¿Te gustó nuestro servicio?", "Envía tu archivo como prefieras". Keep accents and ñ; do not translate.

## Logo: use the official SVGs, never invent one

Real assets live in `static/img/`. All three versions are official; the user chose to keep them all. They are flat vector letters drawn from the original 3D renders, in Fredoka (an open OFL font), with a hand-drawn slab-serif "I" in the capital versions. Pick one per piece and never mix versions inside the same poster.

| Version | Character | Reach for it when |
|---|---|---|
| **v1** `static/img/logo/v1/` | Fredoka Bold, capitals, slab-serif I. The heaviest and the closest to the original renders. | Small sizes, reading from a distance, WhatsApp Status, anything that must be recognized at a glance. The safe default. |
| **v2** `static/img/logo/v2/` | Fredoka SemiBold, "Papelería" in title case **with the accent**; the circular one reads Pape / lería. The friendliest: it reads as a name, not a sign. No slab I. | Warm, conversational pieces, or a wordmark that sits next to running text. |
| **v3** `static/img/logo/v3/` | Fredoka SemiBold, capitals, slab-serif I. Lighter and more refined than v1. | Large formats with room to breathe (posters, banners). Its thinner strokes need more size than v1 to hold up small. |

Every folder has the same three files:

| File | Use |
|---|---|
| `papeleria-linea.svg` | One-line wordmark in a block with an equal margin on all four sides. Headers, footers, banners, lockups. Aspect (width:height): v1 6.6:1, v2 4.0:1, v3 6.5:1. |
| `papeleria-circular.svg` | Two lines (PAPE / LERIA, or Pape / lería in v2) inside a ring, block centered in the ring. 1:1. Avatars, stamps, corner badges, round spots. |
| `papeleria-dos-lineas.svg` | The same two lines with no ring and an equal margin on all sides. Aspect: v1 2.0:1, v2 1.4:1, v3 1.9:1. For placing the logo inside a rectangle, banner or any other shape. |

**Badge (v3 only):** `static/img/logo/v3/papeleria-insignia.svg` is the circular logo on its dark disc (`#231916`). It is what the site uses for its icons and in-page logo, and it works as a small round mark on any ground, light included. It is not one of the three formats and not a fourth version.

Every folder also has a **PNG** (transparent) and a **JPG** (on dark brown `#231916`, because JPG cannot be transparent) of each shape, 3000 px on the longest side, for tools that do not take SVG. Prefer the SVG in anything HTML.

All three formats have a transparent background and no fixed pixel size, so they scale to any size (an 11×17 print included) with no resolution limit. Always set the size yourself (`height="48"` or CSS), because a bare `<img>` with no size stretches to its container's width.

**Structure.** Each letter is its own `<path>` (`letra-1-P` … `letra-9-A`, in reading order: PAPE / Pape = 1–4, LERIA / lería = 5–9; in v2 the í is `letra-8-i`). The circular one also has `aro` (the ring, a stroke) and `disco` (a circle behind the letters, transparent by default).

**Ring.** The official ring is 100 % of the letter stroke thickness, and it grows outward: the gap between the letters and the ring's inner edge does not depend on its width. Do not scale the ring stroke by hand, because that would eat into that gap. A different ring width means regenerating the file with the `logo-designer` skill, which also holds the generator, configs and comparison page for all three versions. The public page `xplaya.com/logo-oficial` shows all three versions with download links, so it is the URL to give a printer, designer or partner who asks for the logo.

**Default colors** (sampled from the original logos): letters P A P E L E R I A are teal `#7DC5C3`, mustard `#EBB728`, orange `#EC652A`, teal, mustard, orange, teal, mustard, teal. Ring: copper `#A0603E`.

**Repainting.**
- In an HTML poster, paste the SVG inline and set CSS custom properties on it or on any ancestor: `--papeleria-teal`, `--papeleria-yellow`, `--papeleria-orange` change every letter of that color; `--papeleria-1` … `--papeleria-9` change a single letter; the circular one also takes `--papeleria-ring` (ring color) and `--papeleria-disc` (fill behind the letters).
- A logo loaded through `<img src>` or `background-image` cannot be recolored from page CSS. When the poster needs other colors, inline the SVG, or copy the file and edit the `fill` attributes.
- Change colors only. Never edit the letter shapes, spacing or proportions, and never redraw, distort or letter-space the wordmark.

## Palette and contrast: made for dark backgrounds

The brand palette is the logo's: teal `#7DC5C3`, mustard `#EBB728`, orange `#EC652A`, plus copper `#A0603E` and the dark brown `#231916` the original renders sit on. Mustard and orange are the anchor in every poster; teal is the third color on a dark ground.

Measured contrast of the default colors: on white the teal is 2.0:1 and the mustard 1.9:1, on cream (`#F4ECDD`) 1.7:1 and 1.6:1, but on dark brown (`#231916`) they reach 8.7:1 and 9.3:1. The orange sits in between (3.3 on white, 5.3 on dark). So:
- On a dark or saturated background, use the defaults.
- On a light background, either put the logo on a dark panel (or set `--papeleria-disc:#231916` for the circular one), or darken the letters: teal `#2A8783` (4.3:1 on white), orange `#D9501A` (4.1:1), mustard `#B98600` (3.2:1).
- For WhatsApp and for older viewers, prefer the dark-background version; do not put default-color letters on white and hope they read.

## Typography (suggested, not yet a formal brand rule)

The site itself uses Bulma's defaults and has no brand typeface. The logo is Fredoka, so a poster headline in **Fredoka SemiBold** echoes it naturally (its font files ship in `.claude/skills/logo-designer/assets/fonts/`, license included), paired with a plain, sturdy sans for body text. If the user states a different preference, follow it and update this section.

## Style fit

The logo is flat, heavy and rounded, in three warm colors. It fits **Memphis / 80s postmodern**, **Bauhaus**, **Swiss**, **Risograph**, **Cut Paper / Collage** and **Japanese Minimal** without any adaptation. If the user wants a tactile metal or wood feel (**Modern Letterpress / Stamp**), that is where the raster renders below belong, with their caveats. Offer these as leading options, but still propose genuinely different alternatives.

## Other assets in the repo

**Original raster renders** are the photographic/3D renditions the vectors were drawn from. Prefer the SVGs. Use a raster only when the user explicitly wants that material look (glossy plastic, engraved metal, pencil), and check each file first because they have baked-in problems:

| File | Pixels | Notes |
|---|---|---|
| `static/img/line.jpg` | 1280×194 | Glossy 3D plastic letters on a brown wall texture; background baked in. |
| `static/img/circleai.jpg` | 630×630 | Glossy letters in a wood-grain ring on a square dark-brown background, not a circular cutout. |
| `static/img/metalico.jpeg` | 1024×1024 | Engraved metal-enamel emblem in a bronze ring. The "transparent" checkerboard is baked into the pixels, so it must be masked. |
| `static/img/logocircle.png` | 630×630 RGBA | Painted-tile wordmark on black inside a white ring; the only raster with real transparency outside the circle. Its letters are white, mustard, orange (no teal). |
| `static/img/papeleria-lapiz-linea.png` | 2048×512 | Pencil-sketch wordmark on cream paper; background baked in; letters white, mustard, orange. |

Before placing any raster, look for baked-in backgrounds and for small AI-generator watermarks (a sparkle mark) in the bottom-right corner; `metalico.jpeg` and `papeleria-lapiz-linea.png` appear to have one. Crop or mask them out. Rasters are also small for print: the largest clean size at 300 DPI is about 2.1 in for the 630 px files, 3.4 in for `metalico.jpeg`, 4.3 in for `line.jpg` and 6.8 in for the pencil one (roughly double at 150 DPI); never scale past about 150 DPI. `static/img/papeleria.png` is an earlier render and not an official logo.

| Files | Role |
|---|---|
| `static/img/favicon/` | Favicon set generated from the badge (`favicon.svg`, `favicon.ico`, `favicon-96x96.png`, `apple-touch-icon.png`, `web-app-manifest-*.png`): small-scale versions of the mark, only ever a tiny badge. |
| `static/img/og_*.jpeg` | The eight social preview images shown when a page link is shared (white Swiss layout with the v3 logo, generated by the `logo-designer` skill). They are site furniture, not artwork to reuse in a poster. |
| `resena.png`, `saldo.jpeg`, `recibo.jpg`, `cortinas/*.webp` | Page-specific hero images. These are not the logo. Use them only if the brief is explicitly about that page or product (for example the `cortinas` photos for a cortinas promo), and look at the image first, since file names do not fully describe content. |

Product photos for product posters live in MinIO: `{CONTENT_BASE_URL}/papeleria-fotos-productos/{filename}` (default base `https://cntnt.xplaya.com`). Look at a photo before using it.

## Exceptions to the generic rules

The anti-generic rules (no unnecessary 3D effects, no fake handwriting) apply to elements you invent. They do not apply to the official logo or to its raster renders.
