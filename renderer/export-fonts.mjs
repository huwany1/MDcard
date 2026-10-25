// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
// Portable SVG images cannot inherit fonts from the parent HTML document.
// HarfBuzz/WASM subsets the licensed bundled fonts to actual glyphs per artifact.
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import subsetFont from 'subset-font';
import {DOMParser,XMLSerializer} from '@xmldom/xmldom';
const root=path.dirname(fileURLToPath(import.meta.url));const buffers=new Map();
async function font(name){if(!buffers.has(name))buffers.set(name,await readFile(path.join(root,'vendor/fonts',name+'-Regular.ttf')));return buffers.get(name);}
export async function portableFontSfnt(text,korean=false){return await subsetFont(await font(korean?'MDcardDiagramSansKR':'MDcardDiagramSans'),text||' ',{targetFormat:'sfnt',preserveNameIds:[0,1,2,3,4,6,13,14]});}
export async function portableFontCss(text){
 const names=['MDcardDiagramSans','MDcardDiagramSansKR'];const families=['MDcard Diagram Sans','MDcard Diagram Sans KR'];const css=[];
 for(let i=0;i<names.length;i++){const bytes=await subsetFont(await font(names[i]),text||' ',{targetFormat:'woff2',preserveNameIds:[0,1,2,3,4,6,13,14]});css.push(`@font-face{font-family:'${families[i]}';src:url(data:font/woff2;base64,${bytes.toString('base64')}) format('woff2')}`);}
 return css.join('\n');
}
export async function portableSvg(svg){
 const doc=new DOMParser().parseFromString(svg,'image/svg+xml');const text=Array.from(doc.getElementsByTagName('text')).map(el=>el.textContent).join('');if(!text)return svg;
 const style=doc.createElementNS('http://www.w3.org/2000/svg','style');style.textContent=await portableFontCss(text);doc.documentElement.insertBefore(style,doc.documentElement.firstChild);return new XMLSerializer().serializeToString(doc);
}
