'use strict';
// Illustrated icons for the social images, drawn on the color panel (470 x 541 px) of build-og.js.
// Every icon is one object with the v3 logo on it, a hard shadow, and an orange badge on its bottom-right corner that
// says what the click does (cursor, info, magnifier, pin, dollar, cart). The ticket and the document live in build-og.js.
module.exports = function makeIcons(h) {
  const { ink, accent, logoSvg, shape, textPath, capHeight } = h;
  const BRAND = { teal: '#7DC5C3', mustard: '#EBB728', orange: '#EC652A' };
  const LIGHT = '#DCE3E2';
  const SW = `stroke="${ink}" stroke-width="5" stroke-linejoin="round"`;
  const bar = (x, y, w, hh = 12, fill = ink) => `<rect x="${x}" y="${y}" width="${w}" height="${hh}" rx="${hh / 2}" fill="${fill}"/>`;
  const logoAt = (x, y, w) => { const lg = logoSvg(); return `<svg x="${x}" y="${y}" width="${w}" height="${(w / lg.ratio).toFixed(2)}" viewBox="0 0 ${lg.vw} ${lg.vh}">${lg.inner}</svg>`; };
  const badge = (parts, bx, by, glyph) => parts.push(`<circle cx="${bx}" cy="${by}" r="44" fill="${accent}" stroke="${ink}" stroke-width="5"/>`, glyph);
  const star = (cx, cy, R) => { const pts = []; for (let i = 0; i < 10; i++) { const a = -Math.PI / 2 + i * Math.PI / 5, r = i % 2 ? R * 0.42 : R; pts.push(`${(cx + r * Math.cos(a)).toFixed(2)},${(cy + r * Math.sin(a)).toFixed(2)}`); } return pts.join(' '); };
  const white = (d, sw = 9) => `<path d="${d}" fill="none" stroke="#ffffff" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round"/>`;

  return {
    // a shop front: sign with the logo, striped awning, window with products, door. Badge: mouse cursor.
    store(parts, g) {
      const x = 80, w = 310, sh = g.shadow;
      parts.push(`<rect x="${x + 16}" y="72" width="${w}" height="414" fill="${sh}"/>`);
      parts.push(`<rect x="${x}" y="56" width="${w}" height="76" fill="#ffffff" ${SW}/>`, logoAt(x + 34, 76, w - 68));
      parts.push(`<rect x="${x + 16}" y="180" width="${w - 32}" height="290" fill="#ffffff" ${SW}/>`);
      // window with three brand shapes
      parts.push(`<rect x="${x + 38}" y="250" width="140" height="130" fill="${LIGHT}" stroke="${ink}" stroke-width="4"/>`);
      parts.push(`<circle cx="${x + 72}" cy="345" r="20" fill="${BRAND.teal}" stroke="${ink}" stroke-width="3"/>`, `<rect x="${x + 100}" y="325" width="40" height="40" fill="${BRAND.orange}" stroke="${ink}" stroke-width="3"/>`, `<polygon points="${x + 148},365 ${x + 172},365 ${x + 160},327" fill="${BRAND.mustard}" stroke="${ink}" stroke-width="3" stroke-linejoin="round"/>`);
      parts.push(`<rect x="${x + 38}" y="366" width="140" height="4" fill="${ink}"/>`);
      // door
      parts.push(`<rect x="${x + 206}" y="290" width="70" height="180" fill="${BRAND.mustard}" stroke="${ink}" stroke-width="4"/>`, `<circle cx="${x + 262}" cy="384" r="6" fill="${ink}"/>`);
      // awning: six scalloped stripes
      const n = 6, sw = w / n;
      for (let i = 0; i < n; i++) { const xi = x + i * sw; parts.push(`<path d="M${xi} 148H${xi + sw}V196A${sw / 2} ${sw / 2} 0 0 1 ${xi} 196Z" fill="${i % 2 ? '#ffffff' : BRAND.orange}" stroke="${ink}" stroke-width="4" stroke-linejoin="round"/>`); }
      const bx = x + w + 6, by = 444;
      badge(parts, bx, by, `<path d="M${bx - 12} ${by - 22}L${bx - 12} ${by + 16}L${bx - 3} ${by + 8}L${bx + 4} ${by + 23}L${bx + 11} ${by + 20}L${bx + 4} ${by + 5}L${bx + 16} ${by + 5}Z" fill="#ffffff" stroke="#ffffff" stroke-width="3" stroke-linejoin="round"/>`);
    },

    // a clipboard with a checklist. Badge: info.
    clipboard(parts, g) {
      const x = 85, y = 52, w = 300, hh = 434, sh = g.shadow;
      parts.push(`<rect x="${x + 16}" y="${y + 16}" width="${w}" height="${hh}" rx="18" fill="${sh}"/>`);
      parts.push(`<rect x="${x}" y="${y}" width="${w}" height="${hh}" rx="18" fill="#ffffff" ${SW}/>`);
      parts.push(`<rect x="${x + 100}" y="${y - 20}" width="100" height="56" rx="14" fill="${ink}"/>`, `<rect x="${x + 122}" y="${y - 8}" width="56" height="14" rx="7" fill="#ffffff"/>`);
      parts.push(logoAt(x + 40, y + 62, w - 80));
      const widths = [150, 116, 138, 96];
      widths.forEach((bw, i) => {
        const ry = y + 142 + i * 66;
        parts.push(`<rect x="${x + 32}" y="${ry}" width="32" height="32" rx="7" fill="#ffffff" stroke="${ink}" stroke-width="4"/>`, white(`M${x + 39} ${ry + 17}L${x + 46} ${ry + 24}L${x + 58} ${ry + 8}`, 6).replace('#ffffff', accent), bar(x + 82, ry + 10, bw));
      });
      const bx = x + w + 6, by = y + hh - 26;
      badge(parts, bx, by, `<circle cx="${bx}" cy="${by - 18}" r="6" fill="#ffffff"/><rect x="${bx - 5.5}" y="${by - 6}" width="11" height="28" rx="5.5" fill="#ffffff"/>`);
    },

    // a phone with a balance, an input, a dial pad and a button. Badge: magnifier.
    phone(parts, g) {
      const x = 125, y = 40, w = 220, hh = 460, sh = g.shadow;
      parts.push(`<rect x="${x + 16}" y="${y + 16}" width="${w}" height="${hh}" rx="38" fill="${sh}"/>`);
      parts.push(`<rect x="${x}" y="${y}" width="${w}" height="${hh}" rx="38" fill="${ink}"/>`, `<rect x="${x + 14}" y="${y + 18}" width="${w - 28}" height="${hh - 36}" rx="26" fill="#ffffff"/>`);
      parts.push(logoAt(x + 36, y + 46, w - 72));
      parts.push(`<rect x="${x + 34}" y="${y + 92}" width="${w - 68}" height="40" rx="20" fill="${accent}"/>`, bar(x + 48, y + 106, 56, 12, '#ffffff'));
      parts.push(`<rect x="${x + 34}" y="${y + 150}" width="${w - 68}" height="44" rx="12" fill="#ffffff" stroke="${ink}" stroke-width="4"/>`, bar(x + 48, y + 166, 66), `<rect x="${x + 122}" y="${y + 160}" width="3" height="24" fill="${accent}"/>`);
      for (let r = 0; r < 3; r++) for (let c = 0; c < 3; c++) parts.push(`<circle cx="${x + w / 2 + (c - 1) * 46}" cy="${y + 240 + r * 42}" r="14" fill="${LIGHT}" stroke="${ink}" stroke-width="3"/>`);
      parts.push(`<rect x="${x + 34}" y="${y + 372}" width="${w - 68}" height="46" rx="23" fill="${ink}"/>`, bar(x + w / 2 - 34, y + 389, 68, 12, '#ffffff'));
      const bx = x + w + 6, by = y + hh - 26;
      badge(parts, bx, by, `<circle cx="${bx - 4}" cy="${by - 4}" r="14" fill="none" stroke="#ffffff" stroke-width="7"/>` + white(`M${bx + 7} ${by + 7}L${bx + 19} ${by + 19}`, 9));
    },

    // a speech bubble with five stars. Badge: map pin.
    bubble(parts, g) {
      const x = 60, y = 96, w = 350, hh = 270, r = 44, sh = g.shadow;
      const body = (dx, dy) => `M${x + r + dx} ${y + dy}H${x + w - r + dx}A${r} ${r} 0 0 1 ${x + w + dx} ${y + r + dy}V${y + hh - r + dy}A${r} ${r} 0 0 1 ${x + w - r + dx} ${y + hh + dy}H${196 + dx}L${112 + dx} ${440 + dy}V${y + hh + dy}H${x + r + dx}A${r} ${r} 0 0 1 ${x + dx} ${y + hh - r + dy}V${y + r + dy}A${r} ${r} 0 0 1 ${x + r + dx} ${y + dy}Z`;
      parts.push(`<path d="${body(16, 16)}" fill="${sh}"/>`, `<path d="${body(0, 0)}" fill="#ffffff" ${SW}/>`);
      parts.push(logoAt(x + 50, y + 34, w - 100));
      for (let i = 0; i < 5; i++) parts.push(`<polygon points="${star(x + 50 + 23 + i * 51, y + 150, 26)}" fill="${BRAND.mustard}" stroke="${ink}" stroke-width="4" stroke-linejoin="round"/>`);   // five stars across the logo's width
      parts.push(bar(x + 50, y + 222, 190));
      const bx = x + w - 6, by = y + hh - 4;
      badge(parts, bx, by, `<path d="M${bx} ${by + 24}C${bx - 26} ${by - 4} ${bx - 18} ${by - 26} ${bx} ${by - 26}C${bx + 18} ${by - 26} ${bx + 26} ${by - 4} ${bx} ${by + 24}Z" fill="#ffffff"/><circle cx="${bx}" cy="${by - 8}" r="7.5" fill="${accent}"/>`);
    },

    // a wallet with a card peeking out. Badge: dollar sign.
    wallet(parts, g) {
      const x = 70, y = 200, w = 330, hh = 260, sh = g.shadow;
      parts.push(`<g transform="rotate(-7 235 200)"><rect x="112" y="100" width="250" height="170" rx="20" fill="${BRAND.teal}" ${SW}/><rect x="134" y="132" width="46" height="36" rx="7" fill="${BRAND.mustard}" stroke="${ink}" stroke-width="3"/>${bar(134, 190, 120, 12, '#ffffff')}</g>`);
      parts.push(`<rect x="${x + 16}" y="${y + 16}" width="${w}" height="${hh}" rx="36" fill="${sh}"/>`, `<rect x="${x}" y="${y}" width="${w}" height="${hh}" rx="36" fill="#ffffff" ${SW}/>`);
      parts.push(`<rect x="${x + 16}" y="${y + 16}" width="${w - 32}" height="${hh - 32}" rx="24" fill="none" stroke="${ink}" stroke-width="3" stroke-dasharray="10 8"/>`);
      parts.push(logoAt(x + 46, y + 54, w - 150));
      parts.push(bar(x + 46, y + 132, 120), bar(x + 46, y + 158, 84));
      parts.push(`<rect x="${x + w - 74}" y="${y + 84}" width="92" height="72" rx="22" fill="${accent}" ${SW}/>`, `<circle cx="${x + w - 28}" cy="${y + 120}" r="9" fill="#ffffff"/>`);
      const bx = x + w + 6, by = y + hh - 26, px = 60, cap = capHeight(900, px), wd = shape(900, '$', px).width;
      badge(parts, bx, by, textPath(900, '$', px, bx - wd / 2, by + cap / 2, '#ffffff'));
    },

    // a paper shopping bag with the logo and three brand shapes. Badge: cart.
    bag(parts, g) {
      const sh = g.shadow;
      const body = (dx, dy) => `M${104 + dx} ${150 + dy}H${372 + dx}L${394 + dx} ${482 + dy}H${82 + dx}Z`;
      parts.push(`<path d="M172 150C172 58 304 58 304 150" fill="none" stroke="${sh}" stroke-width="12" transform="translate(16 16)"/>`, `<path d="${body(16, 16)}" fill="${sh}"/>`);
      parts.push(`<path d="M172 150C172 58 304 58 304 150" fill="none" stroke="${ink}" stroke-width="10" stroke-linecap="round"/>`);
      parts.push(`<path d="${body(0, 0)}" fill="#ffffff" ${SW}/>`);
      parts.push(logoAt(124, 196, 228));
      parts.push(`<circle cx="160" cy="322" r="30" fill="${BRAND.teal}" stroke="${ink}" stroke-width="4"/>`, `<rect x="210" y="292" width="58" height="58" fill="${BRAND.orange}" stroke="${ink}" stroke-width="4"/>`, `<polygon points="290,350 350,350 320,292" fill="${BRAND.mustard}" stroke="${ink}" stroke-width="4" stroke-linejoin="round"/>`);
      parts.push(bar(124, 400, 130), bar(124, 426, 92));
      const bx = 400, by = 456;
      badge(parts, bx, by, white(`M${bx - 24} ${by - 18}H${bx - 15}L${bx - 6} ${by + 8}H${bx + 15}L${bx + 21} ${by - 10}H${bx - 13}`, 7) + `<circle cx="${bx - 3}" cy="${by + 21}" r="5" fill="#ffffff"/><circle cx="${bx + 13}" cy="${by + 21}" r="5" fill="#ffffff"/>`);
    },
  };
};
