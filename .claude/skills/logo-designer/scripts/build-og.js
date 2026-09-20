#!/usr/bin/env node
'use strict';
// Builds the site's social preview images (og_*.jpeg) in the Swiss style: white ground, official v3 logo in its light-ground
// colors, a big headline, a thin-ruled list of what the page offers. No photos, no contact data.
// An image with "layout": "ticket" is drawn instead as a big illustrated receipt (logo on the ticket, abstract rows, a QR-like
// icon and a check badge) with just a headline and one caption.
// Usage: node build-og.js <og.json> <outDir> [--root <repo root>] [--png]
//   <outDir> gets one JPEG per image at the size in og.json (the site's files are 1024 x 541); --png also writes PNGs.
// Text is converted to outlines with the bundled Archivo font, so the result does not depend on system fonts.
// Needs the npm packages opentype.js and sharp.
const fs = require('fs');
const path = require('path');
const opentype = require('opentype.js');
const sharp = require('sharp');
const { serialize } = require('./build-logo.js');

const args = process.argv.slice(2);
const flag = (n) => { const i = args.indexOf(n); if (i >= 0) { args.splice(i, 1); return true; } return false; };
const opt = (n, d) => { const i = args.indexOf(n); return i >= 0 ? args.splice(i, 2)[1] : d; };
const root = path.resolve(opt('--root', process.cwd()));
const wantPng = flag('--png');
const [cfgFile, outDir] = args;
if (!cfgFile || !outDir) { console.error('usage: node build-og.js <og.json> <outDir> [--root <repo root>] [--png]'); process.exit(2); }

const cfg = JSON.parse(fs.readFileSync(cfgFile, 'utf8'));
const { width: W, height: H, ink, accent } = cfg;
const M = 48;                                  // outer margin
const fonts = {};
const font = (w) => fonts[w] || (fonts[w] = (() => { const b = fs.readFileSync(path.resolve(__dirname, `../assets/fonts/archivo-latin-${w}-normal.woff`)); return opentype.parse(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength)); })());

// Lay out one line: glyph positions with kerning and tracking (em). Width in px.
function shape(w, text, px, track = 0) {
  const f = font(w); let x = 0, prev = null; const parts = [];
  for (const ch of text) {
    const g = f.charToGlyph(ch);
    if (prev) x += f.getKerningValue(prev, g) * px / f.unitsPerEm;
    parts.push({ g, x }); x += g.advanceWidth * px / f.unitsPerEm + track * px; prev = g;
  }
  return { parts, width: x - track * px };
}
const capHeight = (w, px) => { const b = font(w).charToGlyph('H').getPath(0, 0, 1000).getBoundingBox(); return -b.y1 / 1000 * px; };
function textPath(w, text, px, x, baseline, fill, track = 0) {
  const { parts } = shape(w, text, px, track);
  let d = '';
  for (const p of parts) d += serialize(p.g.getPath(0, 0, px).commands, x + p.x, baseline, 1);
  return `<path fill="${fill}" d="${d}"/>`;
}
// largest size (px) not above `max` at which every line fits in `avail` px
function fitPx(w, lines, avail, max, track = 0) {
  let px = max;
  for (const l of lines) { const wd = shape(w, l, 100, track).width; px = Math.min(px, Math.floor(avail / wd * 100)); }
  return px;
}

// the official logo, recolored for a light ground (the renderer ignores CSS variables, so the fill attributes are changed)
function logoSvg() {
  let s = fs.readFileSync(path.resolve(root, cfg.logo), 'utf8');
  const [, vw, vh] = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(s);
  let inner = s.slice(s.indexOf('>', s.indexOf('<svg')) + 1, s.lastIndexOf('</svg>'))
    .replace(/<title[\s\S]*?<\/title>/, '').replace(/<style[\s\S]*?<\/style>/, '');
  for (const [from, to] of Object.entries(cfg.logoColors)) inner = inner.split(`fill="${from}"`).join(`fill="${to}"`);
  return { inner, ratio: +vw / +vh, vw: +vw, vh: +vh };
}

function composeSwiss(img) {
  const problems = [];
  const parts = [`<rect width="${W}" height="${H}" fill="#ffffff"/>`, `<rect width="${W}" height="8" fill="${ink}"/>`];

  // logo
  const lg = logoSvg(), lh = 44, lw = lh * lg.ratio;
  parts.push(`<svg x="${M}" y="32" width="${lw.toFixed(2)}" height="${lh}" viewBox="0 0 ${lg.vw} ${lg.vh}">${lg.inner}</svg>`);

  // headline: as large as fits, one shared size for all its lines
  const hpx = Math.min(88, fitPx(900, img.headline, W - 2 * M, 88, -0.02));
  if (hpx < 64) problems.push(`headline too long (${hpx}px)`);
  const hcap = capHeight(900, hpx), lhH = hpx * 0.95, capTop = 116;
  img.headline.forEach((l, i) => parts.push(textPath(900, l, hpx, M, capTop + hcap + i * lhH, ink, -0.02)));
  const headBottom = capTop + hcap + (img.headline.length - 1) * lhH;
  const accentY = headBottom + 28;
  parts.push(`<rect x="${M}" y="${accentY.toFixed(1)}" width="96" height="10" fill="${accent}"/>`);

  // list: one column up to three items, two columns from four; rows separated by thin rules
  const n = img.items.length, cols = n <= 3 ? 1 : 2, rows = Math.ceil(n / cols), rowH = 46, gap = 48;
  const colW = cols === 1 ? W - 2 * M : (W - 2 * M - gap) / 2;
  const ipx = Math.min(30, fitPx(700, img.items, colW, 30));
  if (ipx < 26) problems.push(`an item is too long for its column (${ipx}px)`);
  const icap = capHeight(700, ipx), listTop = H - M - rows * rowH;
  if (listTop < accentY + 10 + 22) problems.push(`headline block (${(accentY + 10).toFixed(0)}px) collides with the list (${listTop}px)`);
  img.items.forEach((it, i) => {
    const r = Math.floor(i / cols), c = i % cols, x = M + c * (colW + gap), top = listTop + r * rowH;
    parts.push(`<rect x="${x}" y="${top}" width="${colW}" height="2" fill="${ink}"/>`);
    parts.push(textPath(700, it, ipx, x, top + rowH / 2 + icap / 2 + 1, ink));
  });
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">${parts.join('')}</svg>`, problems, hpx, ipx };
}

// A big illustrated receipt on a color panel, with a hard ink shadow like the rest of the Swiss family.
function composeTicket(img) {
  const problems = [], parts = [];
  const panel = cfg.panel || '#7DC5C3', shadow = cfg.panelShadow || '#5EA3A1', panelW = 470;
  parts.push(`<rect width="${W}" height="${H}" fill="#ffffff"/>`, `<rect width="${panelW}" height="${H}" fill="${panel}"/>`, `<rect width="${W}" height="8" fill="${ink}"/>`);

  const tw = 300, tx = (panelW - tw) / 2, ty = 50, th = 436, tooth = 20, toothH = 12;
  const ticket = (dx, dy) => {
    let d = `M${tx + dx} ${ty + dy}H${tx + tw + dx}V${ty + th + dy}`;
    const n = Math.floor(tw / tooth), step = tw / n;
    for (let i = 0; i < n; i++) { const xr = tx + tw - i * step; d += `L${(xr - step / 2 + dx).toFixed(2)} ${ty + th + toothH + dy}L${(xr - step + dx).toFixed(2)} ${ty + th + dy}`; }
    return d + 'Z';
  };
  parts.push(`<path d="${ticket(16, 16)}" fill="${shadow}"/>`);
  parts.push(`<path d="${ticket(0, 0)}" fill="#ffffff" stroke="${ink}" stroke-width="5" stroke-linejoin="round"/>`);

  const pad = 26, ix = tx + pad, iw = tw - 2 * pad;
  const lg = logoSvg(), lh = iw / lg.ratio;
  parts.push(`<svg x="${ix}" y="${ty + pad}" width="${iw}" height="${lh.toFixed(2)}" viewBox="0 0 ${lg.vw} ${lg.vh}">${lg.inner}</svg>`);

  // abstract rows: name on the left, price on the right
  const rowY = ty + 106, names = [156, 112, 138];
  names.forEach((nw, i) => {
    const y = rowY + i * 34;
    parts.push(`<rect x="${ix}" y="${y}" width="${nw}" height="12" rx="6" fill="${ink}"/>`, `<rect x="${ix + iw - 46}" y="${y}" width="46" height="12" rx="6" fill="${ink}"/>`);
  });
  const sepY = rowY + 3 * 34 + 8;
  parts.push(`<line x1="${ix}" y1="${sepY}" x2="${ix + iw}" y2="${sepY}" stroke="${ink}" stroke-width="3" stroke-dasharray="11 8"/>`);
  const totY = sepY + 26;
  parts.push(`<rect x="${ix}" y="${totY}" width="74" height="16" rx="8" fill="${ink}"/>`, `<rect x="${ix + iw - 96}" y="${totY}" width="96" height="16" rx="8" fill="${accent}"/>`);

  // QR-like icon: three finder squares and a fixed pattern of modules (an icon, not a real code)
  const q = 96, qx = tx + tw / 2 - q / 2, qy = totY + 42, u = q / 12;
  const finder = (x, y) => `<rect x="${x + 3}" y="${y + 3}" width="${3 * u - 6}" height="${3 * u - 6}" fill="none" stroke="${ink}" stroke-width="6"/><rect x="${x + 1.15 * u}" y="${y + 1.15 * u}" width="${0.7 * u * 1.2}" height="${0.7 * u * 1.2}" fill="${ink}"/>`;
  parts.push(finder(qx, qy), finder(qx + q - 3 * u, qy), finder(qx, qy + q - 3 * u));
  let seed = 7; const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  for (let r = 0; r < 12; r++) for (let c = 0; c < 12; c++) {
    const inFinder = (r < 4 && c < 4) || (r < 4 && c > 7) || (r > 7 && c < 4);
    if (!inFinder && rnd() > 0.5) parts.push(`<rect x="${(qx + c * u).toFixed(2)}" y="${(qy + r * u).toFixed(2)}" width="${(u - 1.5).toFixed(2)}" height="${(u - 1.5).toFixed(2)}" fill="${ink}"/>`);
  }
  if (qy + q > ty + th - 8) problems.push('QR icon runs past the ticket');

  // confirmation badge on the corner
  const bx = tx + tw + 6, by = ty + th - 26;
  parts.push(`<circle cx="${bx}" cy="${by}" r="44" fill="${accent}" stroke="${ink}" stroke-width="5"/>`, `<path d="M${bx - 19} ${by + 2}L${bx - 5} ${by + 16}L${bx + 20} ${by - 14}" fill="none" stroke="#ffffff" stroke-width="10" stroke-linecap="round" stroke-linejoin="round"/>`);

  // text: a headline and one line
  const x0 = panelW + 60, avail = W - M - x0;
  const hpx = Math.min(112, fitPx(900, img.headline, avail, 112, -0.02));
  if (hpx < 64) problems.push(`headline too long (${hpx}px)`);
  const hcap = capHeight(900, hpx), lhH = hpx * 0.95, capTop = 150;
  img.headline.forEach((l, i) => parts.push(textPath(900, l, hpx, x0, capTop + hcap + i * lhH, ink, -0.02)));
  const headBottom = capTop + hcap + (img.headline.length - 1) * lhH, accentY = headBottom + 30;
  parts.push(`<rect x="${x0}" y="${accentY.toFixed(1)}" width="96" height="10" fill="${accent}"/>`);
  const cpx = Math.min(32, fitPx(700, [img.caption], avail, 32));
  if (cpx < 26) problems.push(`caption too long (${cpx}px)`);
  parts.push(textPath(700, img.caption, cpx, x0, accentY + 10 + 30 + capHeight(700, cpx), ink));
  return { svg: `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">${parts.join('')}</svg>`, problems, hpx, ipx: cpx };
}

(async () => {
  fs.mkdirSync(outDir, { recursive: true });
  let bad = 0;
  for (const img of cfg.images) {
    const r = img.layout === 'ticket' ? composeTicket(img) : composeSwiss(img);
    if (r.problems.length) { bad++; console.error(`${img.file}: ${r.problems.join('; ')}`); continue; }
    // rendered at 3x and downsampled, so the outlined text is smooth
    const render = () => sharp(Buffer.from(r.svg), { density: 216 }).resize(W, H, { fit: 'fill' });
    const jpg = path.join(outDir, img.file);
    await render().flatten({ background: '#ffffff' }).jpeg({ quality: 88, mozjpeg: true, chromaSubsampling: '4:4:4' }).toFile(jpg);
    if (wantPng) await render().png().toFile(jpg.replace(/\.jpe?g$/, '.png'));
    console.log(`${img.file.padEnd(20)} ${W}x${H}  ${(fs.statSync(jpg).size / 1024).toFixed(0)} KB  headline ${r.hpx}px, items ${r.ipx}px`);
  }
  if (bad) process.exit(1);
})().catch((e) => { console.error(e); process.exit(1); });
