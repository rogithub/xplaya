---
name: poster-designer
description: Create distinctive, print-ready and WhatsApp-ready promotional posters for xplaya/la papelería from a brief (offer, price, date, evento, producto, etc.). Use this skill whenever the user asks for a poster, flyer, promoción, anuncio, cartel, imagen para WhatsApp Status, or anything to print/imprimir — even if they just say "hazme algo para promocionar X" without using the word "poster." Always use it before generating any promotional image so real prices/dates/text are preserved exactly and the papelería's own logo/brand assets are used instead of invented ones.
---

# Poster Designer — xplaya / la papelería

## Purpose

Create distinctive, print-ready and WhatsApp-ready promotional posters from a user's brief.

This skill's methodology is portable across agents that support the open Agent Skills format, but it is installed here specifically for **la papelería** (xplaya.com). It deliberately separates the design methodology from any specific image-generation provider.

## Brand assets — use these, don't invent a logo

Real brand assets live in `static/img/`. Read them before proposing a design and use them instead of generating a new logo.

### Official logos: vector, transparent, repaintable

These two SVGs are the official logo. They were built from the original 3D renders (`line.jpg`, `circleai.jpg`) as flat vector letters in Fredoka Bold (an open OFL font, so outlining it into a logo is allowed), with a slab-serif "I" drawn by hand to match the brand's I.

| File | Aspect | Use |
|---|---|---|
| `static/img/logo/papeleria-linea.svg` | viewBox 827.6 × 125.7 (about 6.6:1) | One-line wordmark PAPELERIA in a block with equal margin on all four sides. Headers, footers, banners, lockups. |
| `static/img/logo/papeleria-circular.svg` | viewBox 545.5 × 545.5 (1:1) | PAPE / LERIA on two lines inside a ring, block centered in the ring. Avatars, stamps, corner badges, round spots. |

Both have a transparent background and no fixed pixel size, so they scale to any size (an 11×17 print included) with no resolution limit. Always set the size yourself (`height="48"` or CSS), because a bare `<img>` with no size stretches to its container's width.

**Structure.** Each letter is its own `<path>` (`letra-1-P` … `letra-9-A`, in reading order: PAPE = 1–4, LERIA = 5–9). The circular one also has `aro` (the ring, a stroke) and `disco` (a circle behind the letters, transparent by default).

**Default colors** (sampled from the original logos): letters P A P E L E R I A are teal `#7DC5C3`, mustard `#EBB728`, orange `#EC652A`, teal, mustard, orange, teal, mustard, teal. Ring: copper `#A0603E`.

**Repainting.**
- In an HTML poster, paste the SVG inline and set CSS custom properties on it or on any ancestor: `--papeleria-teal`, `--papeleria-yellow`, `--papeleria-orange` change every letter of that color; `--papeleria-1` … `--papeleria-9` change a single letter; the circular one also takes `--papeleria-ring` (ring color) and `--papeleria-disc` (fill behind the letters).
- A logo loaded through `<img src>` or `background-image` cannot be recolored from page CSS. When the poster needs other colors, inline the SVG, or copy the file and edit the `fill` attributes.
- Change colors only. Never edit the letter shapes, spacing or proportions.

**Contrast: this palette was made for dark backgrounds.** Measured contrast of the default colors: on white the teal is 2.0:1 and the mustard 1.9:1, on cream (`#F4ECDD`) 1.7:1 and 1.6:1, but on dark brown (`#231916`) they reach 8.7:1 and 9.3:1. The orange sits in between (3.3 on white, 5.3 on dark). So:
- On a dark or saturated background, use the defaults.
- On a light background, either put the logo on a dark panel (or set `--papeleria-disc:#231916` for the circular one), or darken the letters: teal `#2A8783` (4.3:1 on white), orange `#D9501A` (4.1:1), mustard `#B98600` (3.2:1).
- For WhatsApp and for older viewers, prefer the dark-background version; do not put default-color letters on white and hope they read.

### Original raster renders: reference and special looks

These are the photographic/3D renditions the vector was drawn from. Prefer the SVGs. Use a raster only when the user explicitly wants that material look (glossy plastic, engraved metal, pencil), and check each file first because they have baked-in problems:

| File | Pixels | Notes |
|---|---|---|
| `static/img/line.jpg` | 1280×194 | Glossy 3D plastic letters on a brown wall texture; background baked in. |
| `static/img/circleai.jpg` | 630×630 | Glossy letters in a wood-grain ring on a square dark-brown background, not a circular cutout. |
| `static/img/metalico.jpeg` | 1024×1024 | Engraved metal-enamel emblem in a bronze ring. The "transparent" checkerboard is baked into the pixels, so it must be masked. |
| `static/img/logocircle.png` | 630×630 RGBA | Painted-tile wordmark on black inside a white ring; the only raster with real transparency outside the circle. Its letters are white, mustard, orange (no teal). |
| `static/img/papeleria-lapiz-linea.png` | 2048×512 | Pencil-sketch wordmark on cream paper; background baked in; letters white, mustard, orange. |

Before placing any raster, look for baked-in backgrounds and for small AI-generator watermarks (a sparkle mark) in the bottom-right corner; `metalico.jpeg` and `papeleria-lapiz-linea.png` appear to have one. Crop or mask them out.

Rasters are also small for print. The largest clean size at 300 DPI is about 2.1 in for the 630 px files, 3.4 in for `metalico.jpeg`, 4.3 in for `line.jpg` and 6.8 in for the pencil one (roughly double at 150 DPI). Never scale past about 150 DPI; a soft, pixelated logo undermines the whole poster. Every raster is fine at 1080 px wide for WhatsApp.

`static/img/papeleria.png` is an earlier render of the wordmark that is not an official logo; do not default to it.

### Other files in static/img

| Files | Role |
|---|---|
| `favicon/` | Favicon set (`favicon.svg`, `favicon-96x96.png`, `apple-touch-icon.png`, `web-app-manifest-*.png`): small-scale versions of the mark, only ever a tiny badge. |
| `og_*.jpeg`, `resena.png`, `saldo.jpeg`, `recibo.jpg`, `cortinas/*.webp` | Page-specific social-preview and hero images. These are not the logo. Use them only if the brief is explicitly about that page or product (for example the `cortinas` photos for a cortinas promo), and look at the image first, since file names do not fully describe content. |

### How the logo shapes style choices

The logo is flat, heavy and rounded, in three warm colors. It fits **Memphis / 80s postmodern**, **Bauhaus**, **Swiss**, **Risograph**, **Cut Paper / Collage** and **Japanese Minimal** without any adaptation. Mustard and orange are the brand anchor in every poster; teal is the third color when the poster is on a dark ground. If the user wants a tactile metal or wood feel (**Modern Letterpress / Stamp**), that is where the `metalico.jpeg` / `circleai.jpg` renders belong, with the caveats above. Offer these as leading options, but still propose genuinely different alternatives per the process below.

The anti-generic rules further down (no unnecessary 3D effects, no fake handwriting) apply to elements you invent. They do not apply to the official logo or its raster renders.

Never redraw, distort or recreate the wordmark, and do not letter-space it or change its proportions.

## Reference / inspiration

Primary reference:
https://john.hartnup.uk/2026/06/07/ai-event-posters.html

The approach is inspired by John Hartnup's June 7, 2026 article, **“AI-generated posters don’t have to be horrible”**. The article's central idea is to avoid generic, repetitive AI-poster aesthetics by deliberately choosing a distinctive visual direction. It demonstrates styles including Bauhaus/Modernist, Swiss Style, Risograph, Cut Paper/Collage, Brutalist, 90s Rave, Memphis, Japanese Minimal, Wayfinding/Signage, Stamp/Letterpress and others.

Related catalogue mentioned by the author:
https://john.hartnup.uk/poster-prompts/

Do not reproduce the article or catalogue verbatim. Use them as design inspiration and attribution.

---

## Core workflow

When the user asks for a poster:

1. Extract the factual content:
   - headline
   - offer/product/service
   - price
   - date
   - time
   - location
   - contact / URL / QR requirement
   - call to action
   - brand name/logo
   - any required legal or business text

2. Do not invent factual information.

3. Determine the target:
   - `TABLOID_PRINT`
   - `WHATSAPP_STATUS`
   - or `BOTH`

4. If important information is missing, ask only the minimum necessary question.

5. If the user has not specified a visual direction, propose 5–7 genuinely different styles, calling out which ones read as on-brand given the existing logo (see Brand assets above). Do not merely rename variations of the same layout.

6. After a style is selected, create a complete design brief/prompt for that style.

7. Preserve the exact user-provided wording for prices, dates, times, URLs, phone numbers and other factual text.

8. Do not add decorative copy, slogans or invented event descriptions unless the user explicitly asks for creative copy.

9. Prioritize readability and hierarchy over decorative complexity.

10. If generating an image, use the image-generation capability available in the host agent. If the host supports editable HTML/SVG/PDF generation, prefer real text and editable elements for print work.

---

## Anti-generic-AI rules

Avoid the default “AI event poster” look unless the user explicitly asks for it.

Avoid gratuitous:
- pastel gradients
- airbrushed/oil-painted imagery
- generic smiling crowds
- floating confetti
- excessive flowers/bunting
- random decorative objects
- fake handwritten typography
- excessive glow
- unnecessary 3D effects
- stock-photo-looking compositions
- tiny text
- excessive information packed into one visual
- invented slogans or filler text

The objective is not to hide that AI was used. The objective is to make the design intentional, distinctive and useful.

Every poster should have a clear visual concept.

---

# Output A — TABLOID_PRINT

## Physical specification

Design for:
- finished size: 11 × 17 inches
- orientation: choose portrait or landscape according to the composition
- print resolution target: 300 DPI
- raster equivalent at 300 DPI: 3300 × 5100 px
- provide bleed when the printing workflow supports it; default recommendation: 0.125 inch (3.175 mm) bleed
- keep critical text/logos inside a safe margin
- use CMYK-aware planning when producing a print file; if the generation system only outputs RGB, preserve high contrast and avoid relying on subtle RGB-only effects

For an actual press-ready file, prefer:
- PDF with embedded fonts, or
- editable SVG/HTML converted to PDF,
rather than a flattened image.

## Tabloid layout rules

- Headline must be readable from several feet away.
- Establish a strong hierarchy:
  1. main message
  2. offer/event/service
  3. date/time/location or price
  4. call to action/contact
- Do not make every element equally prominent.
- Use generous margins.
- Avoid placing critical information near trim edges.
- If a QR code is required, reserve a clean, high-contrast area for it.
- Never distort logos.
- Use the official SVG logos in `static/img/logo/` (see Brand assets above); never redraw or approximate the wordmark.

## Print prompt suffix

When producing a prompt for tabloid print, include the equivalent intent:

“Create a professional 11×17 inch tabloid poster composition intended for physical printing at 300 DPI. Use strong hierarchy, large readable typography, safe margins and a clean print-oriented layout. No tiny critical text. Keep all factual text exact.”

---

# Output B — WHATSAPP_STATUS

## Physical specification

Design for a vertical WhatsApp Status / Story presentation:
- aspect ratio: 9:16
- preferred working size: 1080 × 1920 px
- important content should occupy the central safe area
- avoid putting critical information too close to the top or bottom UI areas
- optimize for viewing on a phone, including older users and users with reduced visual acuity

## WhatsApp readability rules

- Use fewer words than the tabloid version when possible.
- Make the main message extremely large.
- Use strong contrast.
- Avoid thin/light type for essential information.
- Avoid decorative typefaces for prices, dates, times and contact information.
- Do not depend on tiny footnotes.
- Prefer one strong visual idea over many small elements.
- Assume the viewer may see the poster for only a few seconds.
- If the user asks for a sticker or status, prioritize immediate recognition over completeness.

## WhatsApp prompt suffix

When producing a prompt for WhatsApp Status, include the equivalent intent:

“Create a vertical 9:16 WhatsApp Status poster, optimized for phone viewing. Make the key message and essential information exceptionally large and legible. Use strong contrast and generous spacing. Keep critical content away from the top and bottom interface areas. Avoid tiny text and visual clutter.”

---

# Output C — BOTH

When the user requests both formats:

Do NOT simply crop one design into the other.

Create a shared visual identity but compose each version independently:
- Tabloid: information-rich, print-oriented, 11×17
- WhatsApp: simplified, vertical, 9:16, phone-first

The headline, offer, colors and visual concept should remain consistent unless the user asks otherwise.

---

# Style selection

When proposing styles, make the choices materially different.

Recommended starting menu:

1. **Modern Letterpress / Stamp** *(tactile; the metal/wood raster renders belong here)*
   - inked/engraved shapes
   - limited palette
   - tactile, dimensional print character
   - controlled imperfections

2. **Memphis / 80s postmodern** *(echoes the logo's playful mustard/orange/teal palette)*
   - bold geometric shapes
   - playful, tactile forms
   - bright limited palette
   - asymmetric, energetic layout

3. **Japanese Minimal** *(the flat logo works well on a quiet dark or paper ground)*
   - restrained composition
   - large negative space
   - one strong graphic element
   - careful typography

4. **Cut Paper / Collage**
   - hand-cut/painted shapes
   - irregular, tactile edges
   - layered composition

5. **Bauhaus / Modernist**
   - geometric shapes
   - asymmetric hierarchy
   - bold sans-serif typography
   - limited high-contrast palette

6. **Swiss / International Typographic**
   - strict grid
   - strong alignment
   - generous whitespace
   - typography-led

7. **Risograph**
   - limited ink palette
   - grain/print texture
   - bold shapes
   - slight registration imperfections

Other suitable directions can include:
- Brutalist Graphic
- Wayfinding/Signage
- contemporary editorial
- monochrome + single accent
- contemporary indie festival
- vintage screen print
- Mexican modernist graphic design
- photocopied punk/fanzine
- retro-futurist
- scientific diagram
- geometric constructivist

Do not force a historical style when a modern equivalent would be clearer.

---

# Style integrity

Once a style is selected:

- Keep the visual language coherent.
- Do not mix unrelated styles merely to add decoration.
- Do not let the model invent additional copy.
- Do not allow style to override factual readability.
- If the user asks for a revision, preserve the selected style unless they request a new direction.

---

# Iteration protocol

Interpret feedback as design changes.

Examples:

“Está muy cargado”
→ reduce elements, increase whitespace, simplify hierarchy.

“Se ve muy pequeño”
→ enlarge essential text and remove secondary information.

“Hazlo más llamativo”
→ strengthen contrast, scale, composition or color; do not automatically add more decorations.

“No parece de mi negocio”
→ preserve brand assets, brand colors and established visual identity — check that the official SVG from `static/img/logo/` is really the one placed.

“Quiero otro”
→ offer a substantially different visual direction, not a minor variation.

“Para WhatsApp”
→ recompose for 9:16; do not merely crop the tabloid poster.

“Para imprimir”
→ recompose for 11×17 and print-safe margins.

---

# Text accuracy

For prices, dates, times, URLs, addresses, phone numbers and business names:

- copy exactly
- preserve punctuation where practical
- do not hallucinate missing digits
- do not substitute similar-looking words
- do not “correct” business names without permission

AI image generation can render text incorrectly. When accurate text is critical, prefer an editable workflow using real text (HTML/SVG/PDF or another supported editable format).

---

# Accessibility / legibility

For audiences that may include older adults or people with reduced visual acuity:

- increase text size
- use strong foreground/background contrast
- avoid thin strokes
- avoid low-contrast pastel text
- avoid overly decorative typography
- keep line lengths reasonable
- use clear spacing between information groups

For WhatsApp Status, accessibility should take priority over fitting every detail.

---

# Final response behavior

After generating or drafting a poster concept, briefly state:
- target format
- selected style
- which brand asset file(s) were used
- what information is included

Do not overwhelm the user with design theory.

If the user asks for another style, regenerate using the same factual content unless they provide new content.

## Attribution

This skill is inspired by:

John Hartnup, “AI-generated posters don’t have to be horrible,” June 7, 2026:
https://john.hartnup.uk/2026/06/07/ai-event-posters.html

Poster Prompts catalogue:
https://john.hartnup.uk/poster-prompts/
