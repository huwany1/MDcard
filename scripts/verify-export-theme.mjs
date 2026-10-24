// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import JSZip from '../renderer/node_modules/jszip/lib/index.js';
import {exportDocument} from '../renderer/export.mjs';
const root=path.resolve(import.meta.dirname,'..');const output=path.join(root,'artifacts/export-theme',process.platform==='win32'?'windows':'linux');await mkdir(output,{recursive:true});
const source=['# Dark theme verification','', 'White text must remain legible when Word prints this page. 中文 한국어 Русский.','','```python','def export_book():','    return "THEME_STRING"','```','','```rust','fn main() { println!("THEME_RUST"); }','```','','| Column | Value |','| --- | --- |','| themed table | $x^2+1$ |','','<!-- pagebreak -->','','## Second dark page','','AFTER_DARK_PAGE_BREAK'].join('\n');
const results=[];
for(const format of ['html','pdf','docx','epub']){const result=await exportDocument({source,output:path.join(output,'dark.'+format),format,baseDir:output,options:{dark:true,background:'#20212a',foreground:'#e7e7ed',accent:'#82aaff',title:'Dark theme',paper:'Letter',customCss:'h1{color:#ffb86c}td{border-color:#82aaff}'}});results.push(result);console.log(format,result.bytes);
if(format==='docx'){const zip=await JSZip.loadAsync(await readFile(result.output));const header=await zip.file('word/headerMDcardBackground.xml').async('string');assert(header.includes('fillcolor="#20212a"'));assert(header.includes('width:612pt;height:792pt'));const xml=await zip.file('word/document.xml').async('string');assert(xml.includes('rIdMDcardBackground'));}
if(format==='epub'){const zip=await JSZip.loadAsync(await readFile(result.output));const styles=await Promise.all(Object.entries(zip.files).filter(([name,file])=>name.endsWith('.css')&&!file.dir).map(([,file])=>file.async('string')));const css=styles.join('\n');assert(css.includes('#c792ea'),'Pandoc EPUB keyword color must match dark theme');assert(css.includes('#c3e88d'),'Pandoc EPUB string color must match dark theme');assert(Object.keys(zip.files).some(n=>n.endsWith('.woff2')));}}
await writeFile(path.join(output,'result.json'),JSON.stringify(results,null,2));
