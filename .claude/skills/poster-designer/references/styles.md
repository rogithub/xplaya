# Styles, integrity and iteration

## Choosing a style

If the user has not chosen a visual direction, propose 5 to 7 that are **genuinely different**, not the same layout with new names. Fit them to the brand: read the "style fit" note in the brand profile, and for a client with no such note judge from their logo, colors, audience and tone. Say briefly which ones read as on-brand and why (each card has a **Fit** line), and let the user pick. Do not force a historical style when a modern equivalent would be clearer.

If the user is torn between two, build both on the same facts in one Artifact so they can compare them side by side, instead of describing them.

Every poster needs one clear visual concept, chosen on purpose. The strongest cards below are built on a **tension** (naive painting against crisp type, a huge empty field against one small object), not on a texture.

## Rules every card shares

- **Texture serves the concept, never the information.** Grain, ghosting, misregistration, displacement and halftone go on shapes and backgrounds. Price, date, time, phone and URL sit on a flat solid block with full contrast, unless the card's **Guard** says exactly how far it may go.
- **Print-process styles look freshly printed.** Letterpress, risograph, halftone and mimeograph keep their inks saturated. No sepia, no yellowing, no "aged paper" filter; those read as dirty and cost contrast.
- **Imagery is graphic, not photographic.** Draw the thing itself as a shape (a frame, a sheet, a pictogram). No photos of people, no crowds, no invented realistic scenes. Hand-made marks (an underline, an arrow, a painted blob) are drawings; the real text is never hand-lettered.
- **The recipes are starting points, not verified.** This machine has no browser, so none of the CSS below has been rendered here; the Artifact review is where it gets checked. If a texture hurts legibility, take it off the text first.
- **Fonts** below are suggestions, all on Google Fonts under OFL. Embed them as base64 (`formats.md`) and check the glyphs á é í ó ú ñ ¿ ¡ before choosing.

## The cards

Each card has the same fields, adapted from the structure John Hartnup uses for his prompts (see the last section), plus three of our own: **CSS**, **Guard** and **Fit**.

### 1. Modern Letterpress / Stamp
- **Idea.** Heavy ink pressed into cream paper; a boutique print shop, not a craft fair.
- **Traits.** Two inks at most; bold simplified marks; a repeated motif; a slight pressed-in feel.
- **Type.** Serif or wood-type slab, generous letter-spacing on small caps (Alfa Slab One, Bevan).
- **Palette.** Near-black, cream, one accent (red or green).
- **Layout.** One centred graphic; text in bands above and below.
- **Mood.** Crafted, substantial.
- **Avoid.** Digital smoothness, pastel craft-fair colors, bunting.
- **CSS.** Cream `--paper`; paper grain as a low-opacity SVG `feTurbulence` overlay; a 0.5 pt light `text-shadow` under dark type for the pressed edge.
- **Guard.** Keep the pressed effect subtle on the price and phone; the accent ink is never a thin stroke.
- **Fit.** Weak with the logo's colors; only if the user wants the tactile metal or wood feel.

### 2. Memphis / 80s postmodern
- **Idea.** Playful postmodern rebellion: squiggles, clashing colors, shapes that do not take themselves seriously.
- **Traits.** Zigzags and squiggles; a mix of geometric and organic shapes; terrazzo speckle; a plastic, laminated feel.
- **Type.** Bold geometric sans at playful scale contrasts; a slight angle on the headline only (Poppins ExtraBold).
- **Palette.** Hot pink, electric blue, yellow, teal, black, white.
- **Layout.** Scattered and energetic, no single focal point.
- **Mood.** Fun, irreverent, deliberately kitsch.
- **Avoid.** Muted natural palettes, serious minimalism, craft-fair looks.
- **CSS.** Inline SVG shapes; zigzags with `repeating-linear-gradient`; terrazzo as small SVG shapes inside a defined panel, not floating over the poster.
- **Guard.** "No focal point" applies to the decoration only. The hierarchy of the information stays clear, on solid blocks, never over squiggles.
- **Fit.** Leading option: the logo is flat, heavy and rounded, and its teal, mustard and orange sit naturally on it.

### 3. Japanese Minimal
- **Idea.** A vast empty field and one strong graphic element; calm and unusual for a local shop.
- **Traits.** One color field; one small or medium element off-centre; asymmetric balance; no decoration.
- **Type.** Refined sans or mincho serif (Zen Kaku Gothic New, Shippori Mincho).
- **Palette.** White or one field color, black, one muted accent (indigo, vermillion, moss).
- **Layout.** A tiny element low or off-centre; minimal text.
- **Mood.** Calm, elegant, contemplative.
- **Avoid.** Busy illustration, several colors, decoration, bunting.
- **CSS.** Big `padding` and `margin`; absolute placement of one shape; no filters at all.
- **Guard.** "Small, precisely placed text" is where this style fights our rules. Restraint comes from removing elements, never from shrinking below the minimum sizes in `formats.md` (36 pt print, 64 px WhatsApp).
- **Fit.** Leading option, if the logo sits on a dark field or as the single small element.

### 4. Cut Paper / Collage
- **Idea.** Bold organic shapes with visible paper edges, Matisse cut-out energy.
- **Traits.** Flat shapes with no shading or gradient inside; slightly irregular edges; playful scale; white gaps between shapes.
- **Type.** Simple bold sans, kept apart from the shapes (Poppins, Rubik).
- **Palette.** Bright blue, orange, green, pink on a white ground.
- **Layout.** Shapes fill the field; text in a clear zone.
- **Mood.** Joyful, bold.
- **Avoid.** Realistic illustration, gradient shading, pastel craft-fair colors.
- **CSS.** SVG paths; roughen the edges with a tiny `feDisplacementMap`; a paper-colored stroke makes the white gaps.
- **Guard.** Text is never on a shape that has a texture or a busy edge behind it.
- **Fit.** Leading option.

### 5. Bauhaus / Modernist
- **Idea.** Function over decoration: primary colors, geometry, a clear typographic hierarchy.
- **Traits.** Blocks of primary color; circles, rectangles, semicircles; flat color, no texture; abstracted symbolic forms, not literal illustration.
- **Type.** Bold geometric sans, few sizes, asymmetric placement (Jost).
- **Palette.** Primary red, blue, yellow, plus black and white.
- **Layout.** Asymmetric grid; information first; generous negative space.
- **Mood.** Confident, design-led.
- **Avoid.** Pastel florals, decorative borders, bunting, hand-drawn looks.
- **CSS.** Grid plus `border-radius` shapes (a circle, a semicircle with `border-radius: 50% 50% 0 0`); no filters.
- **Guard.** Little to guard: text is already on flat color. Check red on blue never carries text.
- **Fit.** Leading option.

### 6. Swiss / International Typographic
- **Idea.** A strict grid, abundant white space, objective typography.
- **Traits.** Mathematical alignment; large white or one-color areas; one abstract or photographic element; minimal decoration.
- **Type.** Grotesque, flush left and ragged right, one accent of weight or size (Archivo).
- **Palette.** White, black, one accent (red, orange or green).
- **Layout.** Strict grid; text and image in clear zones.
- **Mood.** Precise, calm authority.
- **Avoid.** Ornament, competing colors, hand-drawn elements, craft-fair looks.
- **CSS.** A CSS grid of 12 columns and a fixed baseline; `text-align: left` everywhere.
- **Guard.** Little to guard. The risk is too much white and small text: keep the minimum sizes.
- **Fit.** Leading option, with the logo on a dark block or repainted darker on white.

### 7. Risograph
- **Idea.** Limited ink colors with slight misregistration and grain: handmade but designed, not crafty.
- **Traits.** Two or three spot colors only; layers slightly out of register; overprint where colors mix; bold simplified shapes.
- **Type.** Clean sans or simple serif, solid fills, no gradients (Rubik).
- **Palette.** Fluorescent pink, teal, yellow, black; pick two or three.
- **Layout.** Bold graphic shapes; text in solid color blocks.
- **Mood.** Indie, zine culture, deliberately lo-fi.
- **Avoid.** Smooth digital gradients, photographic realism, craft-fair pastels.
- **CSS.** Two or three flat layers with `mix-blend-mode: multiply`; offset one layer by 2 to 4 px for the misregistration; `feTurbulence` grain at low opacity on shapes.
- **Guard.** No grain and no offset on the text blocks. The misregistration lives in the shapes.
- **Fit.** Leading option, with the teal and yellow already in the logo.

### 8. Wayfinding / Signage
- **Idea.** A park map or transport-system poster: functional, trustworthy, quietly distinctive.
- **Traits.** Simple pictograms with one consistent stroke weight; directional arrows; color-coded zones; a map-like organisation of the information.
- **Type.** Clean sans, all caps for labels, small caps for the secondary line (Barlow, Barlow Condensed).
- **Palette.** White, dark gray, plus signal colors: safety green, directional blue, alert orange.
- **Layout.** A grid of icon plus label pairs; clear information zones.
- **Mood.** Functional, trustworthy.
- **Avoid.** Decorative illustration, script fonts, craft-fair ornament.
- **CSS.** CSS grid of cells; every pictogram an inline SVG with the same `stroke-width` and `stroke-linecap`; color bands as flat blocks.
- **Guard.** Pictograms must be recognisable at a glance and never carry a fact alone; every fact is also a text label.
- **Fit.** Strong for anything with several services, formats or prices to compare. The flat logo and orange match the signal palette.

### 9. Halftone / Newsprint
- **Idea.** A cheap, urgent, freshly inked newspaper poster: black ink, huge dots.
- **Traits.** A large exaggerated halftone screen (blobs, not fine dots); high-contrast patches; detail sacrificed to the dot pattern.
- **Type.** Tabloid: massive condensed sans headline in capitals (Anton). Broadsheet: authoritative serif headline in a column grid (Libre Baskerville).
- **Palette.** Black on white; at most one red, only for a masthead band or a drop capital.
- **Layout.** A masthead band, one big halftone patch, text in columns or bands.
- **Mood.** Urgent, journalistic, democratic.
- **Avoid.** Yellowed or aged paper, color photography, fine dots, decorative illustration.
- **CSS.** A dot pattern from `radial-gradient` at 10 to 14 px, masked with a `mask-image` gradient so the dots grow toward the dark side; the patch is a shape (a circle, a frame), not a photo.
- **Guard.** Dots never sit behind text. Text stays black on white or white on solid black.
- **Fit.** Good for offers and notices. On the white ground the logo's default colors fail (teal 2.0:1), so put it on a black panel.

### 10. Child Poster Paint + Pro Typography
- **Idea.** A child's poster-paint drawing with professional typesetting: the tension between the two is the concept.
- **Traits.** Thick brush strokes; a naive drawing of the subject; drips and uneven coverage; white paper showing through.
- **Type.** Crisp and professional, perfectly aligned, deliberately unlike the painting (Archivo, Libre Franklin).
- **Palette.** Poster-paint primaries: red, blue, yellow, green.
- **Layout.** The painting fills the upper area; the professional type sits in a lower band.
- **Mood.** Charming, honest, community-made.
- **Avoid.** Polished vector art pretending to be naive, craft-fair AI slop, bunting.
- **CSS.** SVG strokes with `stroke-linecap: round` and irregular widths; wobble them with `feTurbulence` plus `feDisplacementMap`; drips as rounded rectangles under the strokes; leave gaps of bare paper.
- **Guard.** All the naivety is in the painting. Every letter of every fact is set type in the lower band. The painting must be visibly imperfect or it becomes the polished-vector problem in Avoid.
- **Fit.** Suits school and family messages. Put the logo on the type band, on a dark block.

### 11. Punk Photocopied Fanzine
- **Idea.** A 1980s gig flyer, photocopied and pasted on a wall, with a little color snuck in.
- **Traits.** Photocopy grain and hard contrast; torn paper edges; ransom-note cut-out letters; black and white with one or two xerox color layers.
- **Type.** A mix of typewriter, newspaper cut-out and marker, with deliberately rough alignment (Special Elite, Courier Prime, Permanent Marker for the marker accents only).
- **Palette.** Black, white, photocopy gray, an occasional red or yellow.
- **Layout.** Dense collage; text and shapes competing for space.
- **Mood.** DIY, underground, confrontational.
- **Avoid.** Polished design, craft-fair looks, pastels.
- **CSS.** Torn edges with a jagged `clip-path: polygon(...)`; ransom letters as `<span>`s with alternating fonts, backgrounds and small rotations; photocopy grain with `feTurbulence` plus a high-contrast `feColorMatrix`.
- **Guard.** Ransom letters and marker are for the headline only. Price, date and phone go in typewriter on a clean paper strip, straight and large.
- **Fit.** The photocopy look is a natural wink for a copy shop. Confrontational is not this brand's voice, so keep the tone friendly in the copy and let the style carry the edge.

### 12. Mimeograph / Ditto Machine
- **Idea.** A school or church bulletin from the ditto machine: one smudgy purple-blue ink, pre-digital warmth.
- **Traits.** Waxy purple-blue or gray-blue ink; slight ghosting and a double image; soft edges on the drawings; typewriter text next to a crude drawing.
- **Type.** Typewriter monospace, with manual underlines and one hand-added emphasis (Courier Prime).
- **Palette.** One purple-blue ink on off-white paper.
- **Layout.** Informal, text and rough drawings sharing the page.
- **Mood.** Community, democratic, low-resource, warm.
- **Avoid.** Clean digital type, full color, corporate polish, artificial aging.
- **CSS.** One ink color as `--ink`; ghosting as a `text-shadow: 1.5px 1px 0` at 35 % of the ink; a `blur(.4px)` filter on the drawings only.
- **Guard.** Choose an ink dark enough for 4.5:1 or better on the paper and measure it; no blur on the price, date or phone; ghosting stays at 35 % or less on them.
- **Fit.** One ink means the logo must be repainted in a single color; the CSS variables allow that (`brands/papeleria.md`), but confirm it with the user because it departs from the three-color logo.

Other directions that can work: Brutalist graphic, contemporary editorial, monochrome plus a single accent, contemporary indie festival, vintage screen print, Mexican modernist graphic design, Mexican Calavera / Posada (seasonal, its own Avoid: no horror or Northern-European Halloween), retro-futurist, scientific diagram, geometric constructivist. John Hartnup's catalogue has 100 styles; to add a card, read its page and fill the same fields.

## Style integrity

Once a style is selected:
- Keep the visual language coherent.
- Do not mix unrelated styles merely to add decoration.
- Do not invent additional copy. A style tends to want extra text ("what would a fanzine say?"); it does not get it.
- Do not let style override factual readability.
- On a revision, keep the selected style unless the user asks for a new direction.
- **A new style is rebuilt from the facts in the brief, not by patching the previous HTML.** The old file carries its layout and any text the last style invented.

## Iteration: read feedback as design changes

Change one thing at a time. The variation between attempts is large, and several changes at once lose what already worked.

| The user says | Do this |
|---|---|
| "Está muy cargado" | Reduce elements, increase whitespace, simplify the hierarchy. |
| "Se ve muy pequeño" | Enlarge the essential text and remove secondary information. |
| "Hazlo más llamativo" | Strengthen contrast, scale, composition or color. Do not automatically add decorations. |
| "No parece de mi negocio" | Keep the brand's assets, colors and identity. Check that the right logo file is really the one placed and that nothing from another brand leaked in. |
| "Se ve sucio" / "no se lee" | Take the texture off the text first (see each card's Guard), then lower it everywhere. |
| "Quiero otro" | Offer a substantially different direction, not a minor variation. Treat the rejected poster as the "not this" and rebuild from the brief's facts. |
| "Para WhatsApp" | Recompose for 9:16; do not crop the print version. |
| "Para imprimir" | Recompose for the print size and its safe margins. |

If the user asks for another style, regenerate with the same facts unless they give new content.

## Inspiration and attribution

The approach is inspired by John Hartnup's June 7, 2026 article "AI-generated posters don't have to be horrible" (https://john.hartnup.uk/2026/06/07/ai-event-posters.html) and his poster prompts catalogue (https://john.hartnup.uk/poster-prompts/). The central idea is to avoid the generic, repetitive AI-poster look by deliberately choosing a distinctive direction. The catalogue gives each style the same fields (an idea, a real-world context, visual traits, typography, palette, layout, mood and a style-specific "avoid" that names the neighbouring cliché); the cards above adopt that shape and add CSS, Guard and Fit for real-text HTML posters. The traits are summarised in our own words from the catalogue pages for these twelve styles; do not reproduce the pages verbatim.

His article also warns that a chat accumulates the extra text a style "would" have added; that is why a restyle here starts from the brief and not from the last file.
