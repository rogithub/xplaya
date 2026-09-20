#!/usr/bin/env node
'use strict';
// Builds the site's icon set from the logo badge (the circular logo on its dark disc):
// favicon.svg, favicon.ico (16/32/48), favicon-96x96.png, apple-touch-icon.png (180),
// web-app-manifest-192x192.png, web-app-manifest-512x512.png and site.webmanifest.
// Usage: node build-icons.js <icons.json> [--root <repo root>]
// Needs the npm package sharp.
const fs = require('fs');
const path = require('path');
const sharp = require('sharp');

const args = process.argv.slice(2);
const rootIdx = args.indexOf('--root');
const root = rootIdx >= 0 ? path.resolve(args.splice(rootIdx, 2)[1]) : process.cwd();
const [cfgFile] = args;
if (!cfgFile) { console.error('usage: node build-icons.js <icons.json> [--root <repo root>]'); process.exit(2); }

const cfg = JSON.parse(fs.readFileSync(cfgFile, 'utf8'));
const svgText = fs.readFileSync(path.resolve(root, cfg.insignia), 'utf8');
const svg = Buffer.from(svgText);
const vb = +/viewBox="0 0 ([\d.]+) [\d.]+"/.exec(svgText)[1];
const hex = cfg.background.replace('#', '');
const bg = { r: parseInt(hex.slice(0, 2), 16), g: parseInt(hex.slice(2, 4), 16), b: parseInt(hex.slice(4, 6), 16) };
const outDir = path.resolve(root, cfg.outDir);
fs.mkdirSync(outDir, { recursive: true });

// Rasterize the badge at 2x or more of the target and let sharp downsample, so small sizes stay clean.
const badge = (px) => sharp(svg, { density: Math.max(72, Math.ceil(72 * (px * 2) / vb)) })
  .resize(px, px, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();

// Opaque square in the brand's dark color with the badge centered. `scale` is the badge diameter as a fraction of the side;
// maskable icons keep it inside the central 80 % circle so any launcher mask leaves the whole badge visible.
async function onDark(px, scale) {
  const d = Math.round(px * scale), off = Math.round((px - d) / 2);
  return sharp({ create: { width: px, height: px, channels: 3, background: bg } })
    .composite([{ input: await badge(d), left: off, top: off }]).png().toBuffer();
}

// PNG-in-ICO: a 6-byte header, one 16-byte entry per image, then the PNG files.
function ico(pngs, sizes) {
  const head = Buffer.alloc(6); head.writeUInt16LE(1, 2); head.writeUInt16LE(pngs.length, 4);
  let offset = 6 + 16 * pngs.length;
  const entries = pngs.map((png, i) => {
    const e = Buffer.alloc(16);
    e.writeUInt8(sizes[i] >= 256 ? 0 : sizes[i], 0); e.writeUInt8(sizes[i] >= 256 ? 0 : sizes[i], 1);
    e.writeUInt16LE(1, 4); e.writeUInt16LE(32, 6); e.writeUInt32LE(png.length, 8); e.writeUInt32LE(offset, 12);
    offset += png.length;
    return e;
  });
  return Buffer.concat([head, ...entries, ...pngs]);
}

(async () => {
  const write = (name, data) => { fs.writeFileSync(path.join(outDir, name), data); console.log('  ' + name + ' (' + data.length + ' bytes)'); };
  write('favicon.svg', svg);
  const icoSizes = [16, 32, 48];
  write('favicon.ico', ico(await Promise.all(icoSizes.map(badge)), icoSizes));
  write('favicon-96x96.png', await badge(96));
  write('apple-touch-icon.png', await onDark(180, 0.86));
  write('web-app-manifest-192x192.png', await onDark(192, 0.74));
  write('web-app-manifest-512x512.png', await onDark(512, 0.74));
  const icon = (size, purpose) => ({ src: `${cfg.urlPrefix}/web-app-manifest-${size}x${size}.png`, sizes: `${size}x${size}`, type: 'image/png', purpose });
  write('site.webmanifest', Buffer.from(JSON.stringify({
    name: cfg.name, short_name: cfg.shortName, start_url: '/',
    icons: [icon(192, 'any'), icon(192, 'maskable'), icon(512, 'any'), icon(512, 'maskable')],
    theme_color: cfg.themeColor, background_color: cfg.background, display: 'standalone',
  }, null, 2) + '\n'));
})().catch((e) => { console.error(e); process.exit(1); });
