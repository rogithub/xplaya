---
name: poster-designer
description: Design distinctive, print-ready and WhatsApp-ready posters, flyers and promo graphics from a brief, either for the papelería itself (using its official logo and brand) or for a client's business (using that client's own brand). Use this skill whenever the user asks for a cartel, póster, flyer, volante, promoción, anuncio, lona, imagen para WhatsApp o Status, or anything to print or imprimir, whether it is for xplaya / la papelería or for someone else's business ("hazme un cartel para un cliente"), even if they only say "hazme algo para promocionar X". Use it before making any promotional graphic so prices, dates and contact text stay exact and real brand assets are used instead of invented ones.
---

# Poster designer

One method for every poster, with the brand kept separate as data. The craft rules (facts exact, clear hierarchy, distinctive style, readable at a distance) are the same for everyone; who the poster is *for* decides which brand it wears.

## Step 0: who is the poster for?

| Mode | When | Brand source |
|---|---|---|
| **Own (casa)** | A poster for the papelería: xplaya.com, its services, promotions, events | Read `brands/papeleria.md`: official logos, palette, contact facts, voice, style fit |
| **Client (cliente)** | A poster the papelería makes for another business | That client's own brand. Look for their profile; if there is none, collect it with `references/brief.md` and record it in the shape of `brands/_template.md` |

If it is not clear which one, ask. **Never mix them.** A client's poster does not use the papelería's logo, palette, Fredoka look or contact data, and ours does not borrow a client's. A small credit line ("Diseño: Papelería xplaya") goes on a client's poster only if asked.

## Workflow

1. **Load the brand** as above. Read the profile before proposing anything.
2. **Collect the facts** (`references/brief.md`): headline, offer, price, date, time, place, call to action and contact, required or legal text, where it will be seen and how it will be printed. Ask only for what is missing, in one short message. Do not invent facts.
3. **Choose the format**: `TABLOID_PRINT`, `LETTER_PRINT`, `WHATSAPP_STATUS`, `SOCIAL_FEED`, or a print plus WhatsApp pair (`references/formats.md`). Never crop one format into another; compose each on its own.
4. **Choose the style.** If the user has not picked a direction, propose 5 to 7 genuinely different ones, marking which fit the brand (`references/styles.md`), and wait for the pick.
5. **Build one HTML file with real text** from `assets/poster-starter.html`, following `references/formats.md`: brand tokens, the logo inline, the size in its three places, the safe area.
6. **Review with the user**: publish the HTML as a private Artifact so they see it in their own browser, then iterate on their feedback while keeping the chosen style (`references/styles.md`, iteration table).
7. **Deliver.** The user's current preference is to see posters as Artifacts and to keep them **out of the repo**: the Artifact is the deliverable, and no poster file is written to the repo or to a `carteles/` folder unless they ask. When they approve one and want to print it, save the HTML where they say and give the export steps (PDF for print, PNG for WhatsApp; in `references/formats.md`).

## Rules that hold in both modes

**Facts and copy**
- Copy every price, date, time, URL, address, phone number and business name exactly: same punctuation, no missing or changed digits, no "corrected" business names.
- No decorative copy, slogans or invented event descriptions unless the user asks for creative copy.
- Text is real text (HTML), never text rendered by an image generator, which garbles it.

**Brand**
- Use the brand's real assets. Never redraw, approximate, distort or recolor a logo beyond what its brand allows, and never invent a logo when none was supplied.
- Look at any image you are about to place (Read shows images) and check it for baked-in backgrounds, watermarks and resolution.

**Design**
- Every poster has one clear visual concept and a clear hierarchy: main message, then offer, then date/place/price, then call to action. Not everything can be equally prominent.
- Readability beats decoration. For audiences that include older adults or people with reduced vision: large text, strong contrast, no thin strokes, no low-contrast pastels, no decorative typefaces for prices, dates or contact, sensible line lengths, clear spacing between information groups. On WhatsApp Status, accessibility comes before fitting every detail.
- Avoid the default "AI poster" look unless the user asks for it: pastel gradients, airbrushed or oil-painted imagery, generic smiling crowds, floating confetti, excess flowers or bunting, random decorative objects, fake handwriting, heavy glow, unnecessary 3D effects, stock-photo compositions, tiny text, too much information in one visual, invented slogans. The goal is not to hide that AI was used; it is to make the design intentional, distinctive and useful. These rules apply to what you invent, not to a brand's own logo or assets.
- Once a style is chosen, keep its visual language coherent. Do not mix unrelated styles just to decorate.

## Where things go

- Posters stay in Artifacts for now (see step 7). A poster file is never put in `static/` (the site serves it publicly and caches it for a year) and is never committed without the user's yes; if they ask for a file, ask where it goes.
- A client's logo, brand profile and finished posters are their information: keep them in the agreed folder, not in the repo, unless the user explicitly says otherwise (`references/brief.md`).
- Publishing the preview as an Artifact keeps a copy of a client's logo on the user's Claude account. Mention it once when the poster is for a client.
- The papelería's public logo page is `xplaya.com/logo-oficial`; its logos are made and changed with the `logo-designer` skill.

## Final response

After drafting or delivering a poster, say briefly: the format, the style, which brand asset files were used, and what information is included. Do not lecture on design theory. If the user asks for another style, regenerate with the same facts unless they give new content.

## What is in this skill

| File | Read it when |
|---|---|
| `brands/papeleria.md` | The poster is for the papelería. Logos, palette, contact facts, voice, style fit. |
| `brands/_template.md` | Recording a client's brand. |
| `references/brief.md` | Collecting the content and a client's brand; logo intake checks; where client files live. |
| `references/formats.md` | Choosing sizes, print versus in-house, building and exporting the file. |
| `references/styles.md` | Proposing styles, keeping one coherent, reading feedback. |
| `assets/poster-starter.html` | The starting file for every poster. |
| `scripts/poster-kit.js` | Building a poster with no browser available: measures text with the real font metrics and checks the layout. |
