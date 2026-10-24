// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
// Exercise the packaged engine/Node/Java/browser/Pandoc with no PATH tools.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import path from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const stage=path.resolve(process.argv[2]);const root=path.resolve(import.meta.dirname,'..');const platform=process.platform==='win32'?'windows':'linux';
const output=process.argv[3]?path.resolve(process.argv[3]):path.join(root,'artifacts/export-packages',platform);await mkdir(output,{recursive:true});
// Valid Pillow-generated RGBA fixture; the prior hand-copied IDAT had an invalid CRC.
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==';
await writeFile(path.join(output,'local.png'),Buffer.from(png,'base64'));
const text=['---','title: Bundled export','author: MDcard','lang: zh-CN','---','# 离线导出 / Offline','', '## Nested','', '### Third level','', 'Inline $x^2+1$ and $\\frac{1}{2}$.','', '```rust','fn main() {','    println!("offline");','}','```','', '![local](local.png)','','```mermaid','graph LR',' A[离线] --> B[Export]','```','','```plantuml','@startuml','class A','class B','A --> B','@enduml','```','','```flow','st=>start: Start','e=>end: End','st->e','```','','<!-- pagebreak -->','','# Next page','', 'Page two. Footnote[^1].','','[^1]: Offline reference.'].join('\n');
const slides=['---','marp: true','paginate: true','---','# 第一页','', 'Math $x^2+1$','', '---','','# Code','','```python','def export():','    return "离线"','```','','---','','# End','','```mermaid','graph LR',' A --> B','```'].join('\n');
const results=[];
function invoke(requestPath){return new Promise((resolve,reject)=>{const node=path.join(stage,'renderer/runtime',platform==='windows'?'node.exe':'node');const env={...process.env,PATH:'',MDCARD_CHROMIUM:'',MDCARD_JAVA:'',MDCARD_PANDOC:'',PLAYWRIGHT_BROWSERS_PATH:path.join(output,'empty-browser-cache')};const child=spawn(node,[path.join(stage,'renderer/export.mjs'),requestPath],{env,cwd:output,windowsHide:true,stdio:['ignore','pipe','pipe']});let out='',err='';const timer=setTimeout(()=>{child.kill();reject(Error('Packaged export timed out'));},190000);child.stdout.on('data',d=>out+=d);child.stderr.on('data',d=>err+=d);child.on('error',reject);child.on('exit',code=>{clearTimeout(timer);resolve({code,out,err});});});}
for(const format of ['html','pdf','png','jpeg','docx','epub','marp-html','marp-pdf','marp-pptx']){
 const request={source:format.startsWith('marp-')?slides:text,baseDir:output,output:path.join(output,'bundled.'+format.replace('marp-','slides.')),format,options:{scale:2,language:'zh-CN',lineNumbers:true}};
 const input=path.join(output,'request-'+format+'.json');await writeFile(input,JSON.stringify(request));const run=await invoke(input);await writeFile(path.join(output,format+'-stdout.jsonl'),run.out);await writeFile(path.join(output,format+'-stderr.log'),run.err);
 const events=run.out.trim().split('\n').filter(Boolean).map(line=>JSON.parse(line));const result=events.find(e=>e.type==='complete');if(!result){results.push({format,ok:false,code:run.code,events,stderr:run.err});console.error(format,events.at(-1),run.err);continue;}
 const bytes=await readFile(result.output);assert.equal(result.sha256,createHash('sha256').update(bytes).digest('hex'));results.push({...result,exitCode:run.code,pathEmpty:true,stage,progressEvents:events.filter(e=>e.type==='progress').length});console.log(platform,format,result.bytes,result.elapsedMs);
}
const destination=path.join(output,'keep.html');await writeFile(destination,'OLD OUTPUT');const timeoutRequest=path.join(output,'timeout.json');await writeFile(timeoutRequest,JSON.stringify({source:text,output:destination,format:'html',baseDir:output,timeoutMs:1}));const timeout=await invoke(timeoutRequest);assert.equal(await readFile(destination,'utf8'),'OLD OUTPUT');assert(timeout.out.includes('"stage":"timeout"'));results.push({negative:'deadline-preserves-target',ok:true});
await writeFile(path.join(output,'result.json'),JSON.stringify(results,null,2));assert(!results.some(r=>r.ok===false),'Every packaged format must succeed');
