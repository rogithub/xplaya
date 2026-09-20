#!/usr/bin/env node
'use strict';
// Exports the official SVGs as PNG (transparent) and JPG (on a solid ground) next to them.
// Usage: node build-rasters.js <svgDir> [slug] [--long 3000] [--bg "#231916"]
//   For every <slug>-linea / -circular / -dos-lineas .svg in <svgDir> it writes the same name as .png and .jpg,
//   scaled so the longest side is --long pixels.
// JPG has no transparency, so it needs a ground; the default is the brand's dark brown, because the logo's colors
// are made for dark grounds. Needs the npm package sharp.
const fs = require('fs');
const path = require('path');
const sharp = require('sharp');

const args = process.argv.slice(2);
const opt = (name, def) => { const i = args.indexOf(name); return i >= 0 ? args.splice(i, 2)[1] : def; };
const long = +opt('--long', 3000);
const bg = opt('--bg', '#231916');
const [dir, slug = 'papeleria'] = args;
if (!dir) { console.error('usage: node build-rasters.js <svgDir> [slug] [--long 3000] [--bg "#231916"]'); process.exit(2); }

const hex = bg.replace('#', '');
const ground = { r: parseInt(hex.slice(0, 2), 16), g: parseInt(hex.slice(2, 4), 16), b: parseInt(hex.slice(4, 6), 16) };

(async () => {
  for (const kind of ['linea', 'circular', 'dos-lineas']) {
    const file = path.join(dir, `${slug}-${kind}.svg`);
    if (!fs.existsSync(file)) continue;
    const svg = fs.readFileSync(file);
    const [, vw, vh] = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(svg.toString());
    const scale = long / Math.max(+vw, +vh);
    const w = Math.round(vw * scale), h = Math.round(vh * scale);
    // render at the target density so the vector is rasterized sharp, not upscaled
    const render = () => sharp(svg, { density: Math.ceil(72 * scale) }).resize(w, h, { fit: 'fill' });
    const png = path.join(dir, `${slug}-${kind}.png`), jpg = path.join(dir, `${slug}-${kind}.jpg`);
    await render().png({ compressionLevel: 9 }).toFile(png);
    await render().flatten({ background: ground }).jpeg({ quality: 92, chromaSubsampling: '4:4:4', mozjpeg: true }).toFile(jpg);
    console.log(`${slug}-${kind}: ${w} x ${h}  png ${(fs.statSync(png).size / 1024).toFixed(0)} KB  jpg ${(fs.statSync(jpg).size / 1024).toFixed(0)} KB`);
  }
})().catch((e) => { console.error(e); process.exit(1); });
