// SPDX-License-Identifier: AGPL-3.0-only
// Launch only the installed bundled Node/Java/Chromium/Pandoc, with PATH empty.
import {spawn} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
const stage=path.resolve(process.argv[2]),out=path.resolve(process.argv[3]);await mkdir(out,{recursive:true});
const node=path.join(stage,'renderer/runtime',process.platform==='win32'?'node.exe':'node');
const env=Object.fromEntries(Object.entries(process.env).filter(([k])=>!k.startsWith('MDCARD_')));Object.assign(env,{PATH:'',MDCARD_CHROMIUM:'',MDCARD_JAVA:'',MDCARD_PANDOC:'',PLAYWRIGHT_BROWSERS_PATH:path.join(out,'empty-cache')});
async function invoke(entry,args,input){return await new Promise((resolve,reject)=>{
 const child=spawn(node,[path.join(stage,'renderer',entry),...args],{env,cwd:out,windowsHide:true,stdio:['pipe','pipe','pipe']});let stdout='',stderr='';
 const timer=setTimeout(()=>{child.kill();reject(Error(`${entry} timeout`));},180000);
 child.on('error',reject);child.stdout.on('data',d=>stdout+=d);child.stderr.on('data',d=>stderr+=d);
 child.on('exit',code=>{clearTimeout(timer);resolve({code,stdout,stderr});});child.stdin.end(input);
});}
const results=[];
const kinds=[['math','\\frac{1}{2}+x^2'],['mermaid','graph LR\n A[中文] --> B[Offline]'],['plantuml','@startuml\nAlice -> Bob: 中文\n@enduml'],['flow','st=>start: Start\ne=>end: End\nst->e']];
for(const [kind,source]of kinds){const run=await invoke('worker.mjs',[],JSON.stringify({id:kind,kind,source})+'\n');const data=JSON.parse(run.stdout.trim());assert(data.ok&&data.png&&(kind==='math'||data.svg),`${kind}: ${run.stderr} ${run.stdout.slice(0,150)}`);results.push({kind,ok:true,bytes:Buffer.from(data.png,'base64').length,ms:data.ms});}
await writeFile(path.join(out,'local.png'),Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==','base64'));
const source=['# 离线完整导出','Inline $x^2+1$.','','![local](local.png)','','| Name | Value |','| --- | --- |','| 中文 | $y=1$ |','','```mermaid','graph LR',' A[中文] --> B[Offline]','```','','```plantuml','Alice -> Bob: 中文','```','','```flow','st=>start: Start','e=>end: End','st->e','```','','<!-- pagebreak -->','','## Next page'].join('\n');
await writeFile(path.join(out,'full.md'),source);
const slides='---\nmarp: true\npaginate: true\n---\n# 中文 $x^2+1$\n\n---\n\n# Offline\n\n```mermaid\ngraph LR\n A --> B\n```';
for(const format of ['html','pdf','png','jpeg','docx','epub','marp-html','marp-pdf','marp-pptx']){
 const input=path.join(out,'request-'+format+'.json');await writeFile(input,JSON.stringify({source:format.startsWith('marp-')?slides:source,baseDir:out,output:path.join(out,'result.'+format.replace('marp-','slides.')),format,options:{scale:2,language:'zh-CN'}}));
 const run=await invoke('export.mjs',[input]);await writeFile(path.join(out,format+'.jsonl'),run.stdout);await writeFile(path.join(out,format+'.stderr.log'),run.stderr);
 const events=run.stdout.trim().split('\n').map(s=>JSON.parse(s));const result=events.find(e=>e.type==='complete');
 assert(result&&run.code===0,`${format}: ${run.stdout.slice(-1000)} ${run.stderr}`);
 const data=await readFile(result.output);assert(data.length>500);assert.equal(createHash('sha256').update(data).digest('hex'),result.sha256);
 results.push({format,ok:true,bytes:data.length,elapsedMs:result.elapsedMs,sha256:result.sha256});console.log(format,data.length,result.elapsedMs);
}
await writeFile(path.join(out,'result.json'),JSON.stringify({installedRoot:stage,pathEmpty:true,network:process.env.MDCARD_QA_NETWORK||'not externally blocked; local-only inputs and renderer request blocking',results},null,2)+'\n');
