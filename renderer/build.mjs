// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {build} from 'esbuild';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
await mkdir('vendor',{recursive:true});
// ELK uses EPL-2.0 without a secondary-license notice. Keep it out of the
// AGPL browser bundle rather than assuming it is GPL-compatible.
const bundle = await build({entryPoints:['browser.js'],bundle:true,format:'iife',globalName:'MDcardRenderer',platform:'browser',outfile:'vendor/browser.js',logLevel:'warning',metafile:true,plugins:[{
  name:'exclude-epl-elk-layout',
  setup(plugin) {
    plugin.onResolve({filter:/[/\\]elk-[^/\\]+\.mjs$/},()=>({path:'disabled-elk',namespace:'mdcard'}));
    plugin.onLoad({filter:/.*/,namespace:'mdcard'},()=>({contents:'export async function render(){throw new Error("ELK layout is not included in this build; use the default Dagre layout.");}',loader:'js'}));
  }
}]});
if(Object.keys(bundle.metafile.inputs).some(name=>name.includes('elkjs/')))throw Error('EPL ELK code unexpectedly entered the browser bundle');
await writeFile('vendor/build-inputs.json',JSON.stringify(bundle.metafile,null,2)+'\n');
let css=await readFile('node_modules/katex/dist/katex.min.css','utf8');
for(const m of [...css.matchAll(/url\((fonts\/[^)]+)\)/g)]) {
 const data=await readFile('node_modules/katex/dist/'+m[1]);
 css=css.replace(m[0],`url(data:font/woff2;base64,${data.toString('base64')})`);
}
await writeFile('vendor/katex.css',css);
const cjk=await readFile('../assets/MDcardDiagramSans-Regular.ttf');
const kr=await readFile('../assets/MDcardDiagramSansKR-Regular.ttf');
await writeFile('vendor/diagram-font.css',`@font-face{font-family:'MDcard Diagram Sans';src:url(data:font/ttf;base64,${cjk.toString('base64')}) format('truetype')}@font-face{font-family:'MDcard Diagram Sans KR';src:url(data:font/ttf;base64,${kr.toString('base64')}) format('truetype')}#target{font-family:'MDcard Diagram Sans','MDcard Diagram Sans KR',sans-serif}`);
await mkdir('vendor/fonts',{recursive:true});await writeFile('vendor/fonts/MDcardDiagramSans-Regular.ttf',cjk);await writeFile('vendor/fonts/MDcardDiagramSansKR-Regular.ttf',kr);
console.log('Bundled local KaTeX/mhchem, Mermaid, DOMPurify, flowchart.js and embedded KaTeX fonts.');
