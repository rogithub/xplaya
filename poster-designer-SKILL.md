# Poster Designer — Portable Agent Skill

## Purpose

Create distinctive, print-ready and WhatsApp-ready promotional posters from a user's brief.

This skill is designed to be portable across agents that support the open Agent Skills format. It deliberately separates the design methodology from any specific image-generation provider.

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

5. If the user has not specified a visual direction, propose 5–7 genuinely different styles. Do not merely rename variations of the same layout.

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
- Do not recreate a logo inaccurately if the original asset is available.

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

1. **Bauhaus / Modernist**
   - geometric shapes
   - asymmetric hierarchy
   - bold sans-serif typography
   - limited high-contrast palette

2. **Swiss / International Typographic**
   - strict grid
   - strong alignment
   - generous whitespace
   - typography-led

3. **Risograph**
   - limited ink palette
   - grain/print texture
   - bold shapes
   - slight registration imperfections

4. **Brutalist Graphic**
   - raw high contrast
   - oversized type
   - strong blocks
   - intentionally stark

5. **Japanese Minimal**
   - restrained composition
   - large negative space
   - one strong graphic element
   - careful typography

6. **Wayfinding / Signage**
   - icons
   - arrows
   - structured information
   - functional visual language

7. **Modern Letterpress / Stamp**
   - inked shapes
   - limited palette
   - tactile print character
   - controlled imperfections

Other suitable directions can include:
- Memphis / 80s postmodern
- contemporary editorial
- cut-paper collage
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
→ preserve brand assets, brand colors and established visual identity.

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

# Brand assets

If the user supplies:
- logo
- product photograph
- existing sign
- brand colors
- typography reference

use them as authoritative references.

Do not redraw or redesign the logo unless explicitly requested.

When no brand asset is supplied, do not invent a logo.

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
- what information is included

Do not overwhelm the user with design theory.

If the user asks for another style, regenerate using the same factual content unless they provide new content.

## Attribution

This skill is inspired by:

John Hartnup, “AI-generated posters don’t have to be horrible,” June 7, 2026:
https://john.hartnup.uk/2026/06/07/ai-event-posters.html

Poster Prompts catalogue:
https://john.hartnup.uk/poster-prompts/
