# Brief: what to collect before designing

Applies to both modes. For the papelería's own posters most of the brand answers are already in `brands/papeleria.md`, so only the poster-specific ones remain. For a client's poster, brand answers come from the client and go into a brand profile (`brands/_template.md`).

Ask only for what is missing, in one short message, and never more than you need to start. A poster with a missing price or date is worse than a slow start, but a list of twenty questions is worse than both.

## The content (every poster)

- **Headline / main message.** What must be understood in three seconds.
- **What is offered:** product, service or event.
- **Price**, with the exact wording ("$45", "2x1", "desde $30") and its validity.
- **Date, time, place** if it is an event or a limited offer.
- **Call to action and contact:** phone or WhatsApp, address, web or social handle, QR destination.
- **Required or legal text:** conditions, "hasta agotar existencias", registration numbers.
- **Where it will be seen and how it will be produced** (see the print reality in `formats.md`): window, counter, street, WhatsApp Status; in-house printer or print shop; size; quantity; deadline.

Copy every price, date, time, phone number, URL and business name exactly as the user wrote it. Do not add slogans or invented descriptions unless the user asks for creative copy.

## A client's brand (client mode only)

Ask for these, and record what you get in the brand profile:

1. **The business name exactly as it should be written.**
2. **The logo file**, vector (SVG, PDF, AI) if possible, otherwise PNG with a transparent background. A photo of a sign, a screenshot or a WhatsApp-compressed image is the last resort.
3. **Colors** they use (hex if they have it; otherwise sample them from the logo file and say so).
4. **Typeface** they use, if any.
5. **Tone:** formal or playful, tú or usted, examples of their own texts.
6. **What they like and dislike**: posters or brands they admire, anything to avoid.

If they have no brand at all, propose one small direction (colors and a typeface) and get it approved before designing, so later posters stay consistent.

## Logo intake checks (client files and any raster)

Look at the file (Read shows images) before placing it, and record what you find.

- **Format.** Vector scales without limit. A raster is limited by its pixels.
- **Resolution against the print size.** Pixels needed = printed width in inches × 300 (about × 150 is acceptable for large posters read from a distance). A logo printed 3 in wide needs 900 px, or 450 at the lower bar. If the file is smaller, tell the user, place it smaller, or ask for a better file. Do not upscale or trace it yourself.
- **Baked-in backgrounds.** A white box or a checkerboard around the logo looks wrong on any colored ground. Ask for the transparent version, or set the logo on a panel of its own background color.
- **Watermarks and compression damage.** Crop or ask for a clean file.
- **Variants.** Use the client's own one-color or reversed version on dark grounds; do not recolor their logo unless their brand allows it.

Never redraw, "clean up" or recreate a client's logo.

## Keeping the two brands apart

A client's poster carries the client's brand, not the papelería's. Do not use our logo, palette, Fredoka look or contact data on it. If the client agrees, a small credit line such as "Diseño: Papelería xplaya" can go at the bottom; add it only when asked.

## Where client files live

The repo is hosted on GitHub (`rogithub/xplaya`) and `static/` is served publicly by the site, so a client's logo, brand profile or finished poster must not go into `static/` or be committed without the user's explicit yes. Ask once where they want client work stored (a folder outside the repo, such as `~/carteles/<cliente>/`, is the safe default), and use that folder for the brand profile, the logo files and the finished posters.
