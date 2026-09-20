// Poster kit: build a poster without a browser and still know that it fits.
// It measures every line of text with the real font metrics (opentype.js), lays blocks out in inches inside the 10 x 16 in
// safe area of an 11 x 17 in poster, and stops if a line is wider than its box, a block leaves the safe area, or two blocks
// overlap unless the overlap was declared with allow(). It also inlines the official logo and a QR, and wraps the result as an
// Artifact fragment.
// Use: install opentype.js qrcode and the @fontsource/<font> packages in a scratch folder, run your poster script from that
// folder (font paths are relative to it). See references/formats.md.
// Shared helpers for the poster proposals: real font metrics, a layout that checks itself, and the artifact wrapper.
const fs = require('fs');
const opentype = require('opentype.js');
const QRCode = require('qrcode');

const REPO = process.env.REPO || '/home/ro/code/xplaya';
const cache = {};
const fontFile = (pkg, w) => `node_modules/@fontsource/${pkg}/files/${pkg}-latin-${w}-normal.woff`;
function loadFont(pkg, w) {
  const k = pkg + w;
  if (!cache[k]) { const b = fs.readFileSync(fontFile(pkg, w)); cache[k] = opentype.parse(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength)); }
  return cache[k];
}
const fontFaces = (pkg, family, weights) => weights.map((w) => `@font-face{font-family:'${family}';font-weight:${w};font-style:normal;src:url(data:font/woff;base64,${fs.readFileSync(fontFile(pkg, w)).toString('base64')}) format('woff')}`).join('\n');

// width in inches of one line at `pt` points, kerning and letter-spacing (in em) included
function textWidth(pkg, w, text, pt, track = 0) {
  const f = loadFont(pkg, w); let x = 0, prev = null;
  for (const ch of text) {
    const g = f.charToGlyph(ch);
    if (prev) x += f.getKerningValue(prev, g) * pt / f.unitsPerEm;
    x += g.advanceWidth * pt / f.unitsPerEm + track * pt; prev = g;
  }
  return x / 72;
}

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

// The official v3 one-line logo inline. `vars` sets the CSS custom properties that repaint it (allowed: colors only).
function logo(vars) {
  const svg = fs.readFileSync(`${REPO}/static/img/logo/v3/papeleria-linea.svg`, 'utf8')
    .replace(/^<\?xml[^>]*\?>\s*/, '').replace(/^<!--[\s\S]*?-->\s*/, '').trim();
  return vars ? svg.replace('<svg ', `<svg style="${vars}" `) : svg;
}
const LOGO_RATIO = 788.41 / 121.17;

async function qr(url, sizeIn, label) {
  return (await QRCode.toString(url, { type: 'svg', margin: 2, errorCorrectionLevel: 'M' }))
    .replace('<svg ', `<svg width="${sizeIn}in" height="${sizeIn}in" role="img" aria-label="${label}" `);
}

// A layout that measures its own text and reports anything that leaves the safe area (10 x 16 in) or overlaps.
class Layout {
  constructor(pkgByFamily) { this.fam = pkgByFamily; this.blocks = []; this.out = []; this.problems = []; this.intended = new Set(); }
  allow(a, b) { this.intended.add(a + '|' + b); this.intended.add(b + '|' + a); }
  reg(name, x, y, w, h) { this.blocks.push({ name, x, y, w, h }); }
  // a decorative or structural box
  box(name, { x, y, w, h, style = '', cls = '', inner = '', check = true }) {
    if (check) this.reg(name, x, y, w, h);
    this.out.push(`<div class="${cls}" style="left:${x}in;top:${y}in;width:${w}in;height:${h}in;${style}">${inner}</div>`);
  }
  // a text block made of explicit lines, so every width is known
  text(name, { x, y, w, family, weight, pt, lh = 1.1, lines, track = 0, upper = false, align = 'left', style = '', cls = '', check = true }) {
    const pkg = this.fam[family];
    const shown = lines.map((l) => (upper ? l.toUpperCase() : l));
    const widths = shown.map((l) => textWidth(pkg, weight, l, pt, track));
    const wm = Math.max(...widths), h = lines.length * pt * lh / 72;
    if (wm > w + 0.001) this.problems.push(`${name}: widest line ${wm.toFixed(2)} in > box ${w} in (max ${(pt * w / wm).toFixed(0)} pt)`);
    const bx = align === 'center' ? x + (w - wm) / 2 : align === 'right' ? x + w - wm : x;
    if (check) this.reg(name, bx, y, wm, h);
    this.out.push(`<div class="${cls}" style="left:${x}in;top:${y}in;width:${w}in;font:${weight} ${pt}pt/${lh} '${family}',sans-serif;${track ? `letter-spacing:${track}em;` : ''}${upper ? 'text-transform:uppercase;' : ''}text-align:${align};white-space:nowrap;${style}">${lines.map(esc).join('<br>')}</div>`);
    return { h, w: wm };
  }
  raw(html) { this.out.push(html); }
  verify() {
    for (const b of this.blocks) if (b.x < -0.001 || b.y < -0.001 || b.x + b.w > 10.001 || b.y + b.h > 16.001) this.problems.push(`${b.name} leaves the safe area (${b.x.toFixed(2)},${b.y.toFixed(2)} ${b.w.toFixed(2)}x${b.h.toFixed(2)})`);
    for (let i = 0; i < this.blocks.length; i++) for (let j = i + 1; j < this.blocks.length; j++) {
      const a = this.blocks[i], b = this.blocks[j];
      const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x), oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
      if (ox > 0.001 && oy > 0.001 && !this.intended.has(a.name + '|' + b.name)) this.problems.push(`${a.name} overlaps ${b.name} by ${ox.toFixed(2)} x ${oy.toFixed(2)} in`);
    }
    if (this.problems.length) { console.error('LAYOUT PROBLEMS:\n  ' + this.problems.join('\n  ')); process.exit(1); }
    console.log(`layout ok: ${this.blocks.length} blocks inside the 10 x 16 in safe area, every line fits, no unintended overlaps`);
  }
}

// Artifact fragment: title, styles and the poster, single-theme (a printed poster is one visual world) with an explicit ground.
function artifact({ title, css, fonts, safe, extraSafe = '' }) {
  return `<title>${title}</title>
<style>
${fonts}
@page { size: 11in 17in; margin: 0; }
:root { --w: 11in; --h: 17in; --safe-x: 0.5in; --safe-y: 0.5in; }
* { box-sizing: border-box; }
html, body { margin: 0; background: #d5d5d5; }
body { padding: 24px; display: flex; justify-content: center; }
.poster { position: relative; flex: none; width: var(--w); height: var(--h); overflow: hidden; -webkit-print-color-adjust: exact; print-color-adjust: exact; isolation: isolate; }
@media screen { .poster { zoom: var(--zoom, 1); } }
.safe { position: absolute; inset: var(--safe-y) var(--safe-x); }
.safe > * { position: absolute; margin: 0; }
.guides-layer { display: none; position: absolute; inset: 0; pointer-events: none; z-index: 99; }
.guides-layer::after { content: ""; position: absolute; inset: var(--safe-y) var(--safe-x); outline: 2px dashed #ff2bd6; }
body.guides .guides-layer { display: block; }
@media print { html, body { background: none; padding: 0; display: block; } .guides-layer { display: none !important; } }
${css}
</style>
<main class="poster" id="poster">
  <div class="safe">
${safe}
  </div>${extraSafe}
  <div class="guides-layer" aria-hidden="true"></div>
</main>
<script>
(function () {
  var p = document.getElementById('poster');
  function fit() { document.documentElement.style.setProperty('--zoom', Math.min(1, (window.innerWidth - 48) / p.offsetWidth)); }
  fit(); window.addEventListener('resize', fit);
  if (location.hash === '#guides') document.body.classList.add('guides');
})();
</script>
`;
}

module.exports = { textWidth, fontFaces, logo, LOGO_RATIO, qr, Layout, artifact, esc };
