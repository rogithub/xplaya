# Formats and production

## Formats

| Code | Finished size | Working size | Use |
|---|---|---|---|
| `TABLOID_PRINT` | 11 × 17 in (tabloide / doble carta) | 3300 × 5100 px at 300 DPI | Wall and window posters. Portrait or landscape, whichever the composition needs. |
| `LETTER_PRINT` | 8.5 × 11 in (carta) | 2550 × 3300 px at 300 DPI | Flyers, counter cards, handouts. |
| `WHATSAPP_STATUS` | 9:16 vertical | 1080 × 1920 px | WhatsApp Status and stories, viewed on a phone. |
| `SOCIAL_FEED` | 4:5 vertical | 1080 × 1350 px | Feed posts, only when asked. |

Any other paper size: ask for it and edit the starter (three places, see below). `BOTH` means one print format plus `WHATSAPP_STATUS`.

**Do not crop one format into another.** Keep the same headline, offer, colors and visual concept, and compose each format on its own: print is information-rich and built for distance; WhatsApp is simplified, phone-first and read in a few seconds.

## Print reality: ask how it will be printed

The right margins depend on the printer, so ask once and record the answer in the brand profile.

- **In-house printer** (copy center, office laser or inkjet): the printer cannot print to the paper edge, there is an unprintable margin of about 0.15 to 0.25 in. Do **not** add bleed. Keep a real border, or keep all critical content at least 0.5 in inside every edge and accept that a full-color background will show a white rim (or that the printer scales the page to fit).
- **Print shop** (offset, large format): add **0.125 in (3.175 mm) bleed** beyond the trim for anything that touches the edge, keep critical content 0.25 to 0.5 in inside the trim, and let the shop add crop marks and convert to CMYK. RGB-only files: keep contrast high and avoid subtle effects that will shift.

## Layout rules

**Print (tabloid, letter)**
- The headline must be readable from several feet away.
- Strong hierarchy, in this order: main message; offer, event or service; date, time, place or price; call to action and contact. Do not make everything equally prominent.
- Generous margins; nothing critical near the trim.
- If a QR code is needed, reserve a clean, high-contrast area for it.
- Never distort logos.

Rule-of-thumb minimum sizes for a poster read at one to two meters: headline 100 pt or more, key information 36 pt or more, nothing below 16 pt. For letter flyers read at arm's length, scale these down by about a third.

**WhatsApp Status**
- Fewer words than the print version, and the main message extremely large.
- Strong contrast; no thin or light type for essential information; no decorative typefaces for prices, dates, times or contact.
- One strong visual idea over many small elements. Assume the viewer sees it for a few seconds.
- Keep critical content in the central safe area, away from the top and bottom interface (about 250 px at each end of 1920). Headline 140 px or more, key information 64 px or more, nothing below 40 px.
- For a sticker or a status, immediate recognition beats completeness.

## Building the file

Deliver a single **HTML file with real text** built from `assets/poster-starter.html`. Real text stays exact and editable, prints sharp at any size, and a poster made this way can be corrected in seconds. An image generator garbles text; do not rely on one for anything that carries a price or a date.

1. Copy the starter and change the size in the three places marked `SIZE` (the `@page` rule, `--w` and `--h`, and `--safe-x` / `--safe-y`). Custom properties are not allowed inside `@page`, so the size is written there in plain numbers.
2. Fill the brand tokens (`--brand-*`, fonts) from the brand profile.
3. Build the layout inside `.safe` with the hierarchy above. Use physical units for print (`pt`, `in`) and `px` for the phone formats.
4. **Logo.** Own brand: inline the SVG so its colors can be set with CSS variables (see `brands/papeleria.md`). Client: use their file; a raster goes in an `<img>` with an explicit width.
5. **Fonts.** While iterating, a Google Fonts `<link>` is fine. Before delivering, embed the fonts as base64 so the file looks the same offline. Fredoka is bundled with the logo skill:
   ```bash
   node -e "const fs=require('fs');const b=fs.readFileSync('/home/ro/code/xplaya/.claude/skills/logo-designer/assets/fonts/fredoka-latin-600-normal.woff').toString('base64');console.log(\"@font-face{font-family:'Fredoka';font-weight:600;src:url(data:font/woff;base64,\"+b+\") format('woff')}\")"
   ```
   For any other free font, `npm install @fontsource/<font>` in a scratch directory and use its `files/<font>-latin-<weight>-normal.woff` the same way. Use only fonts whose license allows it.
6. **QR code.** Generate it as an inline SVG with the npm package `qrcode` (install it in a scratch directory):
   ```bash
   node -e "require('qrcode').toString('https://wa.me/524522018336',{type:'svg',margin:4,errorCorrectionLevel:'M'},(e,s)=>console.log(s))"
   ```
   Paste the SVG in place. It already carries its quiet zone; keep that zone, put the QR on a light panel even on a dark poster, make it at least about 1.2 in (30 mm) on paper, and ask the user to scan it with a phone before printing.
7. Open the file with `#guides` at the end of the address to see the safe area.

## Reviewing and exporting

This machine has no usable browser (a headless Chromium was tried and does not start here), so nothing can be rendered on it. The workflow is: build the HTML, publish it as a private Artifact so the user sees it in their own browser, take their feedback, iterate. Printing from inside an Artifact is not reliable, so the final export is done from the saved `.html` file, which is why the user needs these steps (say so; they have not been run on this machine):

- **PDF for printing.** Open the saved HTML in Chrome or Edge, Ctrl+P, destination "Save as PDF" (or the printer), paper size matching the `@page` size, margins "None", scale 100 %, "Background graphics" on.
- **PNG for WhatsApp or social.** Open the HTML in Chrome at 100 % zoom, press F12, in the Elements panel select `<main id="poster">`, right-click it and choose "Capture node screenshot". At a device pixel ratio of 1 the image is exactly the poster's size (1080 × 1920).

## If a host does have an image generator

This skill is written for real-text output. Where an image generator is all there is, add the equivalent intent to the prompt, and keep the exact text out of it so it can be composited afterwards:

- Print: "Create a professional 11×17 inch tabloid poster composition intended for physical printing at 300 DPI. Use strong hierarchy, large readable typography, safe margins and a clean print-oriented layout. No tiny critical text."
- WhatsApp: "Create a vertical 9:16 WhatsApp Status poster, optimized for phone viewing. Make the key message and essential information exceptionally large and legible. Use strong contrast and generous spacing. Keep critical content away from the top and bottom interface areas. Avoid tiny text and visual clutter."
