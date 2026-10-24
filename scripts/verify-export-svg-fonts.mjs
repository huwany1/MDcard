// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {chromium} from '../renderer/node_modules/playwright/index.mjs';
import {exportDocument,bundledBrowser} from '../renderer/export.mjs';
import JSZip from '../renderer/node_modules/jszip/lib/index.js';
const root=path.resolve(import.meta.dirname,'..');const platform=process.platform==='win32'?'windows':'linux';const output=path.join(root,'artifacts/export-fonts',platform);await mkdir(output,{recursive:true});
const source='# 独立字体 검증\n\n```mermaid\ngraph LR\n A[中文 한국어 English] --> B[離線 Русский]\n```\n\n```plantuml\nAlice -> Bob: 中文 한국어 Русский\n```\n\n```flow\nst=>start: 中文 한국어\ne=>end: Русский\nst->e\n```';
const filename=path.join(output,'font-fixture.html');await exportDocument({source,output:filename,format:'html',baseDir:output});
const html=await readFile(filename,'utf8');const browser=await chromium.launch({executablePath:await bundledBrowser(),headless:true,chromiumSandbox:process.platform==='win32'||process.getuid?.()!==0});const results=[];
try{
 const context=await browser.newContext({serviceWorkers:'block'});await context.route('**/*',r=>r.abort());const page=await context.newPage();await page.setContent(html);const svgs=await page.locator('img.diagram').evaluateAll(els=>els.map(el=>el.getAttribute('src')));assert.equal(svgs.length,3);
 for(let i=0;i<svgs.length;i++){
  const uri=svgs[i];const svg=Buffer.from(uri.split(',')[1],'base64').toString('utf8');assert(svg.includes('data:font/woff2;base64,'));await writeFile(path.join(output,`diagram-${i}.svg`),svg);
  await page.goto(uri);await page.evaluate(()=>document.fonts.ready);
  const state=await page.evaluate(()=>({faces:[...document.fonts].map(f=>({family:f.family,status:f.status})),labels:[...document.querySelectorAll('text')].map(t=>({text:t.textContent,family:getComputedStyle(t).fontFamily}))}));
  assert(state.faces.some(f=>f.family==='MDcard Diagram Sans'&&f.status==='loaded'));assert(state.faces.some(f=>f.family==='MDcard Diagram Sans KR'&&f.status==='loaded'));assert(state.labels.some(l=>/[中文한국어]/.test(l.text)));assert(state.labels.every(l=>l.family.includes('MDcard Diagram Sans')));
  const client=await context.newCDPSession(page);const capture=await client.send('Page.captureScreenshot',{format:'png',captureBeyondViewport:true,clip:{x:0,y:0,width:1280,height:720,scale:1}});await writeFile(path.join(output,`diagram-${i}.png`),Buffer.from(capture.data,'base64'));await client.detach();results.push({diagram:i,ok:true,svgBytes:Buffer.byteLength(svg),standalone:true,requestsBlocked:true,...state});
 }
 await context.close();
}finally{await browser.close();}
const docx=await exportDocument({source,output:path.join(output,'font-fixture.docx'),format:'docx',baseDir:output});const zip=await JSZip.loadAsync(await readFile(docx.output));const table=await zip.file('word/fontTable.xml').async('string');assert(table.includes('MDcard Diagram Sans KR'));assert.equal(Object.keys(zip.files).filter(n=>n.endsWith('.odttf')).length,2);assert.equal(docx.vectorAlternates,3);results.push({format:'docx',ok:true,fonts:2,vectorAlternates:docx.vectorAlternates,bytes:docx.bytes,output:docx.output});
await writeFile(path.join(output,'result.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results.map(r=>({diagram:r.diagram,format:r.format,ok:r.ok,svgBytes:r.svgBytes,loadedFonts:r.faces,bytes:r.bytes}))));
