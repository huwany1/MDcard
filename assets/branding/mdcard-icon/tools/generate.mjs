// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { Resvg } from '@resvg/resvg-js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sizes = [16, 24, 32, 48, 64, 128, 256, 512, 1024];
const icoSizes = [16, 24, 32, 48, 64, 128, 256];
const variants = { color: '#2D5BCA', black: '#000000', white: '#FFFFFF' };
const geometry = {
  16: { rect: [2, 1, 12, 14, 3], points: '5,11 5,5.5 8,8.5 11,5.5 11,11', stroke: 2 },
  24: { rect: [3, 1.5, 18, 21, 4.5], points: '7.5,16.5 7.5,8.25 12,12.75 16.5,8.25 16.5,16.5', stroke: 3 },
  32: { rect: [4, 2, 24, 28, 6], points: '10,22 10,11 16,17 22,11 22,22', stroke: 4 },
};
const master = { rect: [8, 4, 48, 56, 12], points: '20,43 20,22 32,34 44,22 44,43', stroke: 7.5 };
function svg(variant, size, geo) {
  const [x, y, width, height, radius] = geo.rect;
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}" role="img" aria-labelledby="title desc">
  <title id="title">MDcard — ${variant}${size < 64 ? ` ${size}px optical` : ''}</title>
  <desc id="desc">A rounded portrait card with a transparent geometric M cutout. The outside of the card and the M are genuinely transparent.</desc>
  <defs>
    <mask id="cutout" maskUnits="userSpaceOnUse" x="0" y="0" width="${size}" height="${size}" style="mask-type:luminance">
      <rect x="${x}" y="${y}" width="${width}" height="${height}" rx="${radius}" fill="#FFFFFF"/>
      <polyline points="${geo.points}" fill="none" stroke="#000000" stroke-width="${geo.stroke}" stroke-linecap="round" stroke-linejoin="round"/>
    </mask>
  </defs>
  <rect x="${x}" y="${y}" width="${width}" height="${height}" rx="${radius}" fill="${variants[variant]}" mask="url(#cutout)"/>
</svg>
`;
}
for (const dir of ['svg', 'svg/optical', 'png/color', 'png/black', 'png/white', 'ico', 'preview']) fs.mkdirSync(path.join(root, dir), { recursive: true });
for (const variant of Object.keys(variants)) {
  const masterSource = svg(variant, 64, master);
  fs.writeFileSync(path.join(root, 'svg', `mdcard-${variant}.svg`), masterSource);
  for (const size of [16, 24, 32]) fs.writeFileSync(path.join(root, 'svg/optical', `mdcard-${variant}-${size}.svg`), svg(variant, size, geometry[size]));
  for (const size of sizes) {
    const source = size in geometry ? svg(variant, size, geometry[size]) : masterSource;
    const rendered = new Resvg(source, { fitTo: { mode: 'width', value: size } }).render();
    fs.writeFileSync(path.join(root, 'png', variant, `mdcard-${size}.png`), rendered.asPng());
  }
}
// Preserve rasterized alpha but normalize straight RGB to the one intended ink.
// This prevents premultiply/unpremultiply rounding from creating matte fringes.
const normalized = spawnSync(process.env.PYTHON || 'python', [path.join(root, 'tools', 'normalize.py')], { stdio: 'inherit' });
if (normalized.status !== 0) throw new Error(`PNG RGB normalization failed: ${normalized.status}`);
for (const variant of Object.keys(variants)) {
  // One directory entry per real PNG payload. A zero width/height byte means 256.
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2); header.writeUInt16LE(icoSizes.length, 4);
  const entries = []; const payloads = [];
  let offset = 6 + 16 * icoSizes.length;
  for (const size of icoSizes) {
    const payload = fs.readFileSync(path.join(root, 'png', variant, `mdcard-${size}.png`));
    const entry = Buffer.alloc(16);
    entry[0] = size === 256 ? 0 : size; entry[1] = size === 256 ? 0 : size;
    entry.writeUInt16LE(1, 4); entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(payload.length, 8); entry.writeUInt32LE(offset, 12);
    offset += payload.length; entries.push(entry); payloads.push(payload);
  }
  fs.writeFileSync(path.join(root, 'ico', `mdcard-${variant}.ico`), Buffer.concat([header, ...entries, ...payloads]));
}
console.log(`Rendered ${Object.keys(variants).length * sizes.length} PNGs, 12 SVGs, and 3 ICOs with resvg.`);
