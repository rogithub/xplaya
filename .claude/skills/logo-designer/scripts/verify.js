#!/usr/bin/env node
'use strict';
// Checks the SVGs written by build-logo.js and renders a preview sheet (white, paper and dark backgrounds).
// Usage: node verify.js <dir> [slug]   (writes <dir>/preview.png; look at it with the Read tool)
const fs = require('fs');
const path = require('path');
const sharp = require('sharp');

const dir = process.argv[2];
const slug = process.argv[3] || 'papeleria';
if (!dir) { console.error('usage: node verify.js <dir> [slug]'); process.exit(2); }

const files = [`${slug}-linea.svg`, `${slug}-circular.svg`, `${slug}-dos-lineas.svg`];
const widths = [520, 260, 300];
const bgs = ['#FFFFFF', '#F4ECDD', '#231916'];

(async () => {
  let failed = 0;
  const pngs = [];
  for (let i = 0; i < files.length; i++) {
    const file = path.join(dir, files[i]);
    const svg = fs.readFileSync(file, 'utf8');
    const problems = [];
    if (/NaN|Infinity/.test(svg)) problems.push('non-finite number in a path');
    if (/<rect/.test(svg)) problems.push('has a <rect> (background?)');
    const paths = (svg.match(/<path /g) || []).length;
    let png;
    try {
      // sharp throws on malformed XML, so a successful render doubles as the well-formedness check
      png = await sharp(Buffer.from(svg), { density: 96 }).resize({ width: widths[i] }).png().toBuffer();
      const { data, info } = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      const alphaAt = (x, y) => data[(y * info.width + x) * 4 + 3];
      const corners = [alphaAt(0, 0), alphaAt(info.width - 1, 0), alphaAt(0, info.height - 1), alphaAt(info.width - 1, info.height - 1)];
      if (corners.some((a) => a !== 0)) problems.push('corners are not transparent');
    } catch (e) { problems.push('does not render: ' + String(e.message).split('\n')[0]); }
    const vb = (/viewBox="([^"]+)"/.exec(svg) || [])[1];
    console.log(`${files[i].padEnd(30)} ${problems.length ? 'FAIL ' + problems.join('; ') : 'ok'}  (paths ${paths}, viewBox ${vb})`);
    if (problems.length) failed++;
    pngs.push(png);
  }
  if (failed) process.exit(1);

  const metas = await Promise.all(pngs.map((p) => sharp(p).metadata()));
  const rowH = Math.max(...metas.map((m) => m.height)) + 50;
  const W = widths.reduce((a, b) => a + b + 40, 40);
  const base = [], over = [];
  bgs.forEach((bg, r) => {
    let x = 0;
    metas.forEach((m, i) => {
      base.push({ input: { create: { width: widths[i] + 40, height: rowH, channels: 3, background: bg } }, left: x, top: r * rowH });
      over.push({ input: pngs[i], left: x + 20, top: r * rowH + Math.round((rowH - m.height) / 2) });
      x += widths[i] + 40;
    });
  });
  const sheet = await sharp({ create: { width: W, height: rowH * bgs.length, channels: 3, background: '#888' } }).composite(base).png().toBuffer();
  await sharp(sheet).composite(over).png().toFile(path.join(dir, 'preview.png'));
  console.log('preview ->', path.join(dir, 'preview.png'));
})();
