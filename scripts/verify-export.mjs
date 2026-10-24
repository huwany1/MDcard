// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {exportDocument} from '../renderer/export.mjs';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import JSZip from '../renderer/node_modules/jszip/lib/index.js';
const root=path.resolve(import.meta.dirname,'..');const output=path.join(root,'artifacts/export-dev',process.platform==='win32'?'windows':'linux');await mkdir(output,{recursive:true});
const source=['---','title: Export integration','author: MDcard','lang: en','---','# Export verification','', '## Nested heading','', '### Third level','','Inline $x^2+1$ math and [jump](#next-page).','','```rust','fn main() { println!("hello"); }','```','','| Name | Value |','| --- | --- |','| table | $y=1$ |','','```mermaid','graph LR',' A[First] --> B[Second]','```','','```plantuml','Alice -> Bob: Hello','```','','```flow','st=>start: Start','e=>end: End','st->e','```','','<!-- pagebreak -->','','# Next page','','After page marker. Footnote[^1].','','[^1]: A real footnote.'].join('\n');
const pandoc=path.join(root,'renderer/runtime',process.platform==='win32'?'windows':'linux','pandoc',process.platform==='win32'?'pandoc.exe':'pandoc');
const results=[];
for(const format of (process.argv.slice(2).length?process.argv.slice(2):['html','pdf','png','jpeg','docx','epub','marp-html','marp-pdf','marp-pptx'])){
 const slides=['---','marp: true','theme: default','paginate: true','---','# First slide','','Math $x^2+1$','','---','','# Second slide','','```mermaid','graph LR',' A --> B','```'].join('\n');
 const request={source:format.startsWith('marp-')?slides:source,output:path.join(output,'sample.'+format.replace('marp-','slides.')),format,baseDir:root,options:{pandocPath:pandoc,scale:2,title:'Export integration',language:'en'}};
 await writeFile(path.join(output,'request-'+format+'.json'),JSON.stringify(request,null,2));
 const events=[];try{const result=await exportDocument(request,e=>events.push(e));assert(result.bytes>500);const bytes=await readFile(result.output);if(format==='html'){const html=bytes.toString();assert(!html.includes('lang: en'));assert(html.includes('id="nested-heading"'));assert(html.includes('footnote'));assert(html.includes('data:image/svg+xml;base64'));assert(!/<script\b/i.test(html));}if(format==='docx'){const zip=await JSZip.loadAsync(bytes);const xml=await zip.file('word/document.xml').async('string');assert(xml.includes('w:anchor="mdcard_toc_'));assert(xml.includes('TOC'));assert(xml.includes('w:type="page"'));}results.push({...result,events});console.log(format,result.bytes,result.elapsedMs);}
 catch(e){results.push({format,ok:false,message:e.message,stage:e.stage,line:e.line,stack:e.stack});console.error(format,e);}
}
const invalid=path.join(output,'preserved.html');await writeFile(invalid,'OLD OUTPUT');
for(const bad of ['![missing](missing-export-image.png)','<script>alert(1)</script>','<style>@import "https://invalid.test/a.css";</style>','<svg xmlns="http://www.w3.org/2000/svg"><image href="https://invalid.test/a.png"/></svg>','$$\n\\unknownExportMathCommand{x}\n$$']){
 let rejected=false;try{await exportDocument({source:bad,output:invalid,format:'html',baseDir:root});}catch(e){rejected=true;assert(e.stage);results.push({negative:true,ok:true,error:e.message,stage:e.stage});}assert(rejected);assert.equal(await readFile(invalid,'utf8'),'OLD OUTPUT');
}
await writeFile(path.join(output,'result.json'),JSON.stringify(results,null,2));if(results.some(r=>r.ok===false))process.exitCode=1;
