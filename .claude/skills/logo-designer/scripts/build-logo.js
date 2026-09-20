#!/usr/bin/env node
'use strict';
// Builds a paintable, transparent SVG wordmark from an open font: one-line, circular (ring) and stacked (no ring).
// Usage: node build-logo.js <config.json> <outDir>
// Needs the npm packages opentype.js and sharp (see SKILL.md for how to install them outside the repo).
const fs = require('fs');
const path = require('path');
const opentype = require('opentype.js');
const sharp = require('sharp');

const r2 = (v) => +v.toFixed(2);
const RASTER = { x: -60, y: -900, w: 1000, h: 1300, sc: 0.5 }; // glyph profiles are measured on a 0.5 px/unit raster
const idChar = (c) => c.normalize('NFD').replace(/[̀-ͯ]/g, '');

function loadConfig(file) {
  const cfg = JSON.parse(fs.readFileSync(file, 'utf8'));
  cfg.baseDir = path.dirname(path.resolve(file));
  return cfg;
}

function resolveFont(cfg) {
  const candidates = [path.resolve(cfg.baseDir, cfg.font), path.resolve(__dirname, '../assets/fonts', path.basename(cfg.font))];
  const found = candidates.find((f) => fs.existsSync(f));
  if (!found) throw new Error('font not found: ' + cfg.font);
  const b = fs.readFileSync(found);
  return opentype.parse(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
}

// Everything geometric is in font units (y grows downward, baseline y=0) until it is written out scaled by K.
function setup(cfg) {
  const font = resolveFont(cfg);
  const box = (ch) => font.charToGlyph(ch).getPath(0, 0, 1000).getBoundingBox();
  const EB = box('E'), IB = box('I');
  const TOP = EB.y1, BOT = EB.y2;                       // cap band
  const STEM = cfg.stem || (IB.x2 - IB.x1);             // stroke thickness (override for fonts whose I has serifs)
  const K = 100 / (BOT - TOP);                          // output scale: cap height = 100 svg units
  const S = Object.assign({ T: 0, depth: 110, minGap: Math.round(STEM * 0.52), minGapWith: {}, pad: Math.round(STEM * 0.49), lineGap: Math.round(STEM * 0.52) }, cfg.spacing);
  const letters = Array.from(cfg.letters);
  const upper = cfg.letters === cfg.letters.toUpperCase();
  const L = Object.assign({ extents: upper ? 'cap' : 'ink', stacking: 'columns', center: 'ink' }, cfg.layout);
  const ringCfg = Object.assign({ margin: Math.round(STEM * 0.55), widthPct: 100 }, cfg.ring);
  const ring = { margin: ringCfg.margin, width: ringCfg.width !== undefined ? ringCfg.width : Math.round(STEM * ringCfg.widthPct / 100) };
  const cssPrefix = cfg.cssPrefix || 'logo', cls = cfg.classPrefix || 'l';
  return { cfg, font, TOP, BOT, STEM, K, S, L, ring, letters, cssPrefix, cls };
}

// Own path serializer: opentype.js toPathData produced NaN for some offsets.
function serialize(cmds, ox, oy, k) {
  const X = (v) => r2((v + ox) * k), Y = (v) => r2((v + oy) * k);
  let d = '', cx = null, cy = null;
  for (const c of cmds) {
    if (c.type === 'M') { d += `M${X(c.x)} ${Y(c.y)}`; cx = c.x; cy = c.y; }
    else if (c.type === 'L') { if (c.x !== cx || c.y !== cy) d += `L${X(c.x)} ${Y(c.y)}`; cx = c.x; cy = c.y; }
    else if (c.type === 'Q') { d += `Q${X(c.x1)} ${Y(c.y1)} ${X(c.x)} ${Y(c.y)}`; cx = c.x; cy = c.y; }
    else if (c.type === 'C') { d += `C${X(c.x1)} ${Y(c.y1)} ${X(c.x2)} ${Y(c.y2)} ${X(c.x)} ${Y(c.y)}`; cx = c.x; cy = c.y; }
    else if (c.type === 'Z') d += 'Z';
  }
  if (/NaN|Infinity/.test(d)) throw new Error('non-finite coordinate in path');
  return d;
}

function flatten(cmds, ox, oy) {
  const pts = []; let cx = 0, cy = 0, sx = 0, sy = 0;
  for (const c of cmds) {
    if (c.type === 'M') { cx = c.x; cy = c.y; sx = cx; sy = cy; pts.push([cx + ox, cy + oy]); }
    else if (c.type === 'L') { cx = c.x; cy = c.y; pts.push([cx + ox, cy + oy]); }
    else if (c.type === 'Q') {
      for (let t = 0.1; t <= 1.0001; t += 0.1) pts.push([(1 - t) ** 2 * cx + 2 * (1 - t) * t * c.x1 + t * t * c.x + ox, (1 - t) ** 2 * cy + 2 * (1 - t) * t * c.y1 + t * t * c.y + oy]);
      cx = c.x; cy = c.y;
    } else if (c.type === 'C') {
      for (let t = 0.1; t <= 1.0001; t += 0.1) { const u = 1 - t; pts.push([u ** 3 * cx + 3 * u * u * t * c.x1 + 3 * u * t * t * c.x2 + t ** 3 * c.x + ox, u ** 3 * cy + 3 * u * u * t * c.y1 + 3 * u * t * t * c.y2 + t ** 3 * c.y + oy]); }
      cx = c.x; cy = c.y;
    } else if (c.type === 'Z') { cx = sx; cy = sy; }
  }
  return pts;
}

function makeGlyphs(ctx) {
  const { font, TOP, BOT, STEM, K, cfg } = ctx;
  const fontGlyph = (ch) => {
    const p = font.charToGlyph(ch).getPath(0, 0, 1000);
    return {
      ch, bbox: p.getBoundingBox(),
      d1: (ox, oy) => serialize(p.commands, ox, oy, 1),
      emit: (ox, oy) => serialize(p.commands, ox, oy, K),
      points: (ox, oy) => flatten(p.commands, ox, oy),
    };
  };
  // Slab-serif letter (the brand's I): a stem with two pill-ended bars and small fillets where they meet.
  const slabGlyph = (ch) => {
    const { w, b, r: rr } = cfg.slab, rp = b / 2;
    const yT = TOP, yB = BOT, xs0 = (w - STEM) / 2, xs1 = (w + STEM) / 2;
    const segs = [
      ['M', rp, yT], ['L', w - rp, yT], ['A', w - rp, yT + b], ['L', xs1 + rr, yT + b],
      ['Q', xs1, yT + b, xs1, yT + b + rr], ['L', xs1, yB - b - rr], ['Q', xs1, yB - b, xs1 + rr, yB - b],
      ['L', w - rp, yB - b], ['A', w - rp, yB], ['L', rp, yB], ['A', rp, yB - b], ['L', xs0 - rr, yB - b],
      ['Q', xs0, yB - b, xs0, yB - b - rr], ['L', xs0, yT + b + rr], ['Q', xs0, yT + b, xs0 - rr, yT + b],
      ['L', rp, yT + b], ['A', rp, yT],
    ];
    const build = (ox, oy, k) => {
      const X = (v) => r2((v + ox) * k), Y = (v) => r2((v + oy) * k), R = r2(rp * k);
      let d = '';
      for (const sg of segs) {
        if (sg[0] === 'M') d += `M${X(sg[1])} ${Y(sg[2])}`;
        else if (sg[0] === 'L') d += `L${X(sg[1])} ${Y(sg[2])}`;
        else if (sg[0] === 'Q') d += `Q${X(sg[1])} ${Y(sg[2])} ${X(sg[3])} ${Y(sg[4])}`;
        else d += `A${R} ${R} 0 0 1 ${X(sg[1])} ${Y(sg[2])}`;
      }
      return d + 'Z';
    };
    const inset = rp * 0.3;
    return {
      ch, bbox: { x1: 0, x2: w, y1: yT, y2: yB },
      d1: (ox, oy) => build(ox, oy, 1),
      emit: (ox, oy) => build(ox, oy, K),
      points: (ox, oy) => [[inset + ox, yT + inset + oy], [w - inset + ox, yT + inset + oy], [inset + ox, yB - inset + oy], [w - inset + ox, yB - inset + oy]],
    };
  };
  const slabLetter = cfg.slab ? (cfg.slab.letter || 'I') : null;
  return (ch) => (slabLetter && ch === slabLetter ? slabGlyph(ch) : fontGlyph(ch));
}

// ---------- horizontal optical spacing ----------
async function rasterAlpha(paths, x, y, w, h) {
  const { sc } = RASTER;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w * sc}" height="${h * sc}" viewBox="${x} ${y} ${w} ${h}">${paths}</svg>`;
  return sharp(Buffer.from(svg)).ensureAlpha().extractChannel(3).raw().toBuffer({ resolveWithObject: true });
}

async function profile(g) {
  const { x, y, w, h, sc } = RASTER;
  const { data, info } = await rasterAlpha(`<path d="${g.d1(0, 0)}" fill="#000"/>`, x, y, w, h);
  const L = [], R = [];
  for (let r = 0; r < info.height; r++) {
    let a = -1, b = -1;
    for (let c = 0; c < info.width; c++) if (data[r * info.width + c] >= 128) { if (a < 0) a = c; b = c; }
    if (a >= 0) { L.push(x + a / sc); R.push(x + (b + 1) / sc); } else { L.push(null); R.push(null); }
  }
  return { g, L, R, xmin: Math.min(...L.filter((v) => v != null)), xmax: Math.max(...R.filter((v) => v != null)) };
}

function advance(ctx, a, b) {
  const { S } = ctx;
  let sum = 0, n = 0, worst = -Infinity;
  for (let i = 0; i < a.R.length; i++) {
    if (a.R[i] == null || b.L[i] == null) continue;
    const ra = Math.max(a.R[i], a.xmax - S.depth), lb = Math.min(b.L[i], b.xmin + S.depth);
    sum += ra - lb; n++;
    worst = Math.max(worst, a.R[i] - b.L[i]);
  }
  // closest-approach floor; a letter with a diagonal (A) may use a smaller floor because it opens extra space
  let floor = S.minGap;
  for (const k of Object.keys(S.minGapWith)) if (a.g.ch === k || b.g.ch === k) floor = S.minGapWith[k];
  return Math.max(sum / n + S.T, worst + floor);
}

async function layoutLine(ctx, mk, chars) {
  const gl = chars.map(mk);
  const pr = [];
  for (const g of gl) pr.push(await profile(g));
  const xs = [0];
  for (let i = 1; i < pr.length; i++) xs.push(xs[i - 1] + advance(ctx, pr[i - 1], pr[i]));
  return {
    gl, xs, chars,
    inkMin: xs[0] + pr[0].xmin, inkMax: xs[xs.length - 1] + pr[pr.length - 1].xmax,
    top: Math.min(...gl.map((g) => g.bbox.y1)), bot: Math.max(...gl.map((g) => g.bbox.y2)),
  };
}

// ---------- vertical stacking of two lines ----------
async function columnProfile(L, cx) {
  const X0 = -1600, W = 3200, { y, h, sc } = RASTER;
  const paths = L.gl.map((g, i) => `<path d="${g.d1(L.xs[i] + cx, 0)}" fill="#000"/>`).join('');
  const { data, info } = await rasterAlpha(paths, X0, y, W, h);
  const top = [], bottom = [];
  for (let c = 0; c < info.width; c++) {
    let a = -1, b = -1;
    for (let r = 0; r < info.height; r++) if (data[r * info.width + c] >= 128) { if (a < 0) a = r; b = r; }
    top.push(a < 0 ? null : y + a / sc); bottom.push(b < 0 ? null : y + (b + 1) / sc);
  }
  return { top, bottom };
}

async function place(ctx, L1, L2) {
  const { S, L, TOP, BOT } = ctx;
  const cx1 = -(L1.inkMin + L1.inkMax) / 2, cx2 = -(L2.inkMin + L2.inkMax) / 2; // each line centered on x=0
  let dy;
  if (L.stacking === 'band') dy = (BOT - TOP) + S.lineGap;
  else {
    // column by column: the tallest descender/ascender pairs decide, so lines can interlock
    const c1 = await columnProfile(L1, cx1), c2 = await columnProfile(L2, cx2);
    dy = -Infinity;
    for (let i = 0; i < c1.bottom.length; i++) if (c1.bottom[i] != null && c2.top[i] != null) dy = Math.max(dy, c1.bottom[i] - c2.top[i] + S.lineGap);
  }
  const placed = [];
  L1.gl.forEach((g, i) => placed.push({ n: i + 1, g, ox: L1.xs[i] + cx1, oy: 0 }));
  L2.gl.forEach((g, i) => placed.push({ n: i + 1 + L1.gl.length, g, ox: L2.xs[i] + cx2, oy: dy }));
  return { placed, dy };
}

// ---------- svg assembly ----------
function docs(ctx) {
  const { cfg, cssPrefix, cls, letters } = ctx;
  const roles = cfg.roles.slice(0, letters.length);
  const uniq = [...new Set(roles)];
  const colors = cfg.colors;
  const header = (kind) => `<?xml version="1.0" encoding="UTF-8"?>\n<!-- ${cfg.label} (${kind}).${cfg.note ? ' ' + cfg.note : ''} Fondo transparente; cada letra es un path independiente. Ver los colores en el bloque style. -->`;
  const cssDoc = `    /* Pintar inline con CSS: ${uniq.map((r) => `--${cssPrefix}-${r}`).join(', ')} (por color)\n       o --${cssPrefix}-1 a --${cssPrefix}-${letters.length} (una por letra). Como imagen (img) o en un editor (Inkscape, Illustrator,\n       Figma) se usan los fill de cada path. */\n`;
  const style = (extra) => {
    const lines = roles.map((role, i) => `    .${cls}-${i + 1}{fill:var(--${cssPrefix}-${i + 1},var(--${cssPrefix}-${role},${colors[role]}))}`);
    return `  <style>\n${cssDoc}${lines.join('\n')}${extra || ''}\n  </style>`;
  };
  const paths = (items, indent) => items.map((it) => `${indent}<path id="letra-${it.n}-${idChar(letters[it.n - 1])}" class="${cls}-${it.n}" fill="${colors[roles[it.n - 1]]}" d="${it.d}"/>`).join('\n');
  const head = (kind, viewBox) => `${header(kind)}\n<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}" role="img" aria-labelledby="${cls}-title">\n  <title id="${cls}-title">${cfg.title}</title>`;
  return { style, paths, head };
}

function lineLogo(ctx, L) {
  const { S, TOP, BOT, K, L: lay } = ctx;
  const d = docs(ctx);
  const top = lay.extents === 'cap' ? TOP : L.top, bot = lay.extents === 'cap' ? BOT : L.bot;
  const vbMinX = L.inkMin - S.pad, vbMinY = top - S.pad;
  const W = r2((L.inkMax + S.pad - vbMinX) * K), H = r2((bot + S.pad - vbMinY) * K);
  const items = L.gl.map((g, i) => ({ n: i + 1, d: g.emit(L.xs[i] - vbMinX, -vbMinY) }));
  return `${d.head('una línea', `0 0 ${W} ${H}`)}\n${d.style()}\n  <g id="letras">\n${d.paths(items, '    ')}\n  </g>\n</svg>\n`;
}

async function stackedLogo(ctx, L1, L2) {
  const { S, TOP, BOT, K, L: lay, cfg } = ctx;
  const d = docs(ctx);
  const { placed, dy } = await place(ctx, L1, L2);
  const minX = Math.min(...placed.map((p) => p.ox + p.g.bbox.x1)), maxX = Math.max(...placed.map((p) => p.ox + p.g.bbox.x2));
  const minY = lay.extents === 'cap' ? TOP : Math.min(...placed.map((p) => p.oy + p.g.bbox.y1));
  const maxY = lay.extents === 'cap' ? dy + BOT : Math.max(...placed.map((p) => p.oy + p.g.bbox.y2));
  const vbMinX = minX - S.pad, vbMinY = minY - S.pad;
  const W = r2((maxX - minX + 2 * S.pad) * K), H = r2((maxY - minY + 2 * S.pad) * K);
  const items = placed.map((p) => ({ n: p.n, d: p.g.emit(p.ox - vbMinX, p.oy - vbMinY) }));
  return `${d.head(`dos líneas, sin aro, ${cfg.lines.join(' / ')}`, `0 0 ${W} ${H}`)}\n${d.style()}\n  <g id="letras">\n${d.paths(items, '    ')}\n  </g>\n</svg>\n`;
}

function enclosing(pts) {
  let cx = pts.reduce((s, p) => s + p[0], 0) / pts.length, cy = pts.reduce((s, p) => s + p[1], 0) / pts.length;
  for (let i = 1; i <= 60000; i++) {
    let far = pts[0], fd = -1;
    for (const p of pts) { const q = (p[0] - cx) ** 2 + (p[1] - cy) ** 2; if (q > fd) { fd = q; far = p; } }
    cx += (far[0] - cx) / (i + 1); cy += (far[1] - cy) / (i + 1);
  }
  return { cx, cy };
}

// opts.disc: fill of the disc behind the letters (the badge); without it the circle stays transparent
async function circleLogo(ctx, L1, L2, opts = {}) {
  const { ring, TOP, BOT, K, L: lay, cfg, cls, cssPrefix } = ctx;
  const discFill = opts.disc || 'none';
  const d = docs(ctx);
  const { placed, dy } = await place(ctx, L1, L2);
  const pts = placed.flatMap((p) => p.g.points(p.ox, p.oy));
  const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
  const mx = (Math.min(...xs) + Math.max(...xs)) / 2;
  const my = lay.center === 'band' ? (TOP + dy + BOT) / 2 : (Math.min(...ys) + Math.max(...ys)) / 2;
  const rr = Math.max(...pts.map((p) => Math.hypot(p[0] - mx, p[1] - my)));
  // ring: inner edge stays `margin` away from the farthest letter point; a thicker ring only grows outward
  const R = rr + ring.margin + ring.width / 2;
  const half = R + ring.width / 2 + 8;
  const items = placed.map((p) => ({ n: p.n, d: p.g.emit(p.ox - (mx - half), p.oy - (my - half)) }));
  const size = r2(2 * half * K), ctr = r2(half * K), rad = r2(R * K), sw = r2(ring.width * K);
  const extra = `\n    .${cls}-ring{stroke:var(--${cssPrefix}-ring,${cfg.ringColor})}\n    .${cls}-disc{fill:var(--${cssPrefix}-disc,${discFill})}`;
  return `${d.head(`${opts.disc ? 'insignia' : 'circular'}, ${cfg.lines.join(' / ')}`, `0 0 ${size} ${size}`)}\n${d.style(extra)}
  <circle id="disco" class="${cls}-disc" cx="${ctr}" cy="${ctr}" r="${rad}" fill="${discFill}"/>
  <circle id="aro" class="${cls}-ring" cx="${ctr}" cy="${ctr}" r="${rad}" fill="none" stroke="${cfg.ringColor}" stroke-width="${sw}"/>
  <g id="letras">\n${d.paths(items, '    ')}\n  </g>\n</svg>\n`;
}

// Numbers the comparison page needs (stem thickness and margins in svg units).
function measure(cfg) {
  const ctx = setup(cfg);
  return { stem: r2(ctx.STEM * ctx.K), pad: r2(ctx.S.pad * ctx.K), letters: cfg.letters, roles: cfg.roles.slice(0, ctx.letters.length), colors: cfg.colors, ringColor: cfg.ringColor, cssPrefix: ctx.cssPrefix, classPrefix: ctx.cls };
}

async function build(cfg, outDir) {
  const ctx = setup(cfg);
  const mk = makeGlyphs(ctx);
  const split = cfg.lines[0].length;
  const one = await layoutLine(ctx, mk, ctx.letters);
  const l1 = await layoutLine(ctx, mk, ctx.letters.slice(0, split));
  const l2 = await layoutLine(ctx, mk, ctx.letters.slice(split));
  fs.mkdirSync(outDir, { recursive: true });
  const slug = cfg.slug || 'logo';
  const out = {
    [`${slug}-linea.svg`]: lineLogo(ctx, one),
    [`${slug}-circular.svg`]: await circleLogo(ctx, l1, l2),
    [`${slug}-dos-lineas.svg`]: await stackedLogo(ctx, l1, l2),
  };
  // optional badge: the circular logo on its own dark disc, for icons and small round marks
  if (cfg.badge) out[`${slug}-insignia.svg`] = await circleLogo(ctx, l1, l2, { disc: cfg.badge.disc });
  for (const [name, svg] of Object.entries(out)) {
    if (/NaN|Infinity/.test(svg)) throw new Error('non-finite number in ' + name);
    fs.writeFileSync(path.join(outDir, name), svg);
  }
  return { files: Object.keys(out), stemUnits: r2(ctx.STEM * ctx.K), ringPx: r2(ctx.ring.width * ctx.K) };
}

module.exports = { build, loadConfig, measure, serialize };

if (require.main === module) {
  const [cfgFile, outDir] = process.argv.slice(2);
  if (!cfgFile || !outDir) { console.error('usage: node build-logo.js <config.json> <outDir>'); process.exit(2); }
  build(loadConfig(cfgFile), outDir).then((m) => console.log(`${m.files.join(', ')} -> ${outDir} (stem ${m.stemUnits}, ring ${m.ringPx} svg units)`)).catch((e) => { console.error(e); process.exit(1); });
}
