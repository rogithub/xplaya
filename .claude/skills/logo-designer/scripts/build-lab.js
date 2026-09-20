#!/usr/bin/env node
'use strict';
// Assembles the logo lab page (compare versions, repaint by color or letter, ring width, no-ring) from the SVGs on disk.
// Usage: node build-lab.js <lab.json> <out.html> [--root <repo root>] [--site]
// --site writes the version served by xplaya at /logo-oficial: a full HTML document for the minijinja loader (content in
// {% raw %} blocks), with the site's social meta, noindex, and a download table (SVG, PNG, JPG) for every shape of every version.
// The manifest lists the versions; each version has a build-logo config (for its measurements) and a folder of SVGs.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const sharp = require('sharp');
const { loadConfig, measure } = require('./build-logo.js');

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const stripHeader = (svg) => svg.replace(/^<\?xml[^>]*\?>\s*/, '').replace(/^<!--[\s\S]*?-->\s*/, '').trim();

// Two logos of several versions live on one page, so every id gets a version suffix to stay unique.
function inline(svg, kind, suffix, cls, extraAttrs) {
  return stripHeader(svg)
    .replace('<svg xmlns=', `<svg class="logo-${kind}"${extraAttrs || ''} xmlns=`)
    .split(`${cls}-title`).join(`${cls}-title-${suffix}`)
    .replace('id="letras"', `id="letras-${suffix}"`)
    .replace(/id="letra-(\d+)-/g, `id="${suffix}-letra-$1-`)
    .replace('id="disco"', `id="disco-${suffix}"`)
    .replace('id="aro"', `id="aro-${suffix}"`);
}

// Full document for the site's minijinja loader. Everything generated goes inside {% raw %} so that braces in the
// CSS and JS are never read as template syntax; the only template variable is site_url, outside the raw blocks.
function siteDocument(page, meta) {
  const cut = page.indexOf('</style>') + '</style>'.length;
  let head = page.slice(0, cut);
  const body = page.slice(cut);
  const title = esc(meta.title || 'Logo oficial — xplaya.com');
  head = head.replace(/<title>[\s\S]*?<\/title>/, `<title>${title}</title>`);
  const desc = esc(meta.description || '');
  const img = meta.ogImage ? `<meta property="og:image" content="{{ site_url }}${esc(meta.ogImage)}">\n<meta name="twitter:image" content="{{ site_url }}${esc(meta.ogImage)}">\n` : '';
  if (/\{%\s*endraw/.test(page)) throw new Error('page contains an endraw tag');
  return `<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="light dark">
<meta name="robots" content="noindex">
<meta name="description" content="${desc}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="xplaya.com">
<meta property="og:locale" content="es_MX">
<meta property="og:title" content="${title}">
<meta property="og:description" content="${desc}">
${meta.ogWidth ? `<meta property="og:image:width" content="${meta.ogWidth}">\n<meta property="og:image:height" content="${meta.ogHeight}">\n` : ''}${img}<meta name="twitter:card" content="summary_large_image">
{% raw %}
${head}
<style>body { margin: 0; }</style>
{% endraw %}
</head>
<body>
{% raw %}
${body.trim()}
{% endraw %}
</body>
</html>
`;
}

async function main() {
  const args = process.argv.slice(2);
  const siteIdx = args.indexOf('--site');
  const site = siteIdx >= 0;
  if (site) args.splice(siteIdx, 1);
  const rootIdx = args.indexOf('--root');
  const root = rootIdx >= 0 ? path.resolve(args.splice(rootIdx, 2)[1]) : process.cwd();
  const [manifestFile, outFile] = args;
  if (!manifestFile || !outFile) { console.error('usage: node build-lab.js <lab.json> <out.html> [--root <repo root>] [--site]'); process.exit(2); }

  const manifest = JSON.parse(fs.readFileSync(manifestFile, 'utf8'));
  const manifestDir = path.dirname(path.resolve(manifestFile));
  const slug = manifest.slug;
  let template = fs.readFileSync(path.resolve(__dirname, '../assets/logo-lab.template.html'), 'utf8');

  let pairs = '', files = '', first = null;
  for (const v of manifest.versions) {
    const cfg = loadConfig(path.resolve(manifestDir, v.config));
    const m = measure(cfg);
    if (!first) first = m;
    const dir = path.resolve(root, manifest.logoDir, v.id);
    const read = (kind) => fs.readFileSync(path.join(dir, `${slug}-${kind}.svg`), 'utf8');
    const line = inline(read('linea'), 'line', `${v.id}l`, m.classPrefix);
    const circ = inline(read('circular'), 'circle', `${v.id}c`, m.classPrefix, ` data-stem="${m.stem}" data-pad="${m.pad}"`);
    // site mode: a download table per version, one row per shape and one button per format that exists on disk
    let links = '';
    if (site) {
      const shapes = [['linea', 'Una línea'], ['circular', 'Circular'], ['dos-lineas', 'Dos líneas']];
      const rows = [];
      for (const [k, label] of shapes) {
        const btns = [];
        const dims = fs.existsSync(path.join(dir, `${slug}-${k}.png`)) ? await sharp(path.join(dir, `${slug}-${k}.png`)).metadata() : null;
        for (const [ext, name, tip] of [['svg', 'SVG', 'Vector: escala sin límite y se puede pintar'], ['png', 'PNG', dims && `${dims.width} × ${dims.height} px, fondo transparente`], ['jpg', 'JPG', dims && `${dims.width} × ${dims.height} px, sobre fondo oscuro`]]) {
          const file = path.join(dir, `${slug}-${k}.${ext}`);
          if (!fs.existsSync(file)) continue;
          // /static is served with a one-year immutable cache, so each link carries a hash of its file: it changes only when the file does
          const h = crypto.createHash('sha1').update(fs.readFileSync(file)).digest('hex').slice(0, 8);
          btns.push(`<a class="dl-btn" href="/${esc(manifest.logoDir)}/${esc(v.id)}/${esc(slug)}-${k}.${ext}?v=${h}" download title="${esc(tip)}">${name}</a>`);
        }
        rows.push(`            <div class="dl-row"><span class="dl-name">${label}</span>${btns.join('')}</div>`);
      }
      links = `\n          <div class="pair-files">\n${rows.join('\n')}\n          </div>`;
    }
    pairs += `        <div class="pair" data-ver="${esc(v.id)}" data-short="${esc(v.short || v.id)}">\n          <h3>${esc(v.label)}</h3>\n          <div class="pair-logos">${line}${circ}</div>${links}\n        </div>\n`;
    files += `<code>${esc(manifest.logoDir)}/${esc(v.id)}/${esc(slug)}-*.svg</code> `;
  }

  if (site && manifest.site && manifest.site.downloadNote) pairs += `        <p class="dl-note">${esc(manifest.site.downloadNote)}</p>\n`;

  const lab = {
    cssPrefix: first.cssPrefix, classPrefix: first.classPrefix, roles: first.roles,
    colorKeys: [...new Set(first.roles)], colors: first.colors, ringColor: first.ringColor,
    colorLabels: manifest.colorLabels || {}, presets: manifest.presets || [],
    letters: manifest.chipLetters || first.letters.toUpperCase(),
  };
  const json = JSON.stringify(lab).replace(/</g, '\\u003c');
  template = template
    .split('{{TITLE}}').join(esc(manifest.title))
    .replace('{{INTRO}}', () => esc(manifest.intro || ''))
    .replace('{{FILES}}', () => files.trim())
    .replace('{{PAIRS}}', () => pairs.trimEnd())
    .replace('{{LAB_JSON}}', () => json);
  if (/\{\{[A-Z_]+\}\}/.test(template)) throw new Error('unreplaced placeholder in template');
  if (site) template = siteDocument(template, manifest.site || {});
  fs.mkdirSync(path.dirname(path.resolve(outFile)), { recursive: true });
  fs.writeFileSync(outFile, template);
  console.log(`lab page -> ${outFile} (${manifest.versions.length} versions, ${(template.length / 1024).toFixed(0)} KB)`);
}

main().catch((e) => { console.error(e); process.exit(1); });
