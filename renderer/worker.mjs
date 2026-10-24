// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {chromium} from 'playwright';
import {createHash} from 'node:crypto';
import {readFile,writeFile,mkdir,access} from 'node:fs/promises';
import {createInterface} from 'node:readline';
import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {existsSync} from 'node:fs';

const root=path.dirname(fileURLToPath(import.meta.url));
const sha=s=>createHash('sha256').update(s).digest('hex');
const cache=new Map();let cacheBytes=0;let browser,context,page;let renders=0,hits=0,blockedRequests=0;
const platformRuntime=path.join(root,'runtime',process.platform==='win32'?'windows':'linux');
const runtime=existsSync(path.join(platformRuntime,'chromium'))?platformRuntime:path.join(root,'runtime');
async function chromiumExecutable(){
  if(process.env.MDCARD_CHROMIUM)return process.env.MDCARD_CHROMIUM;
  const bundled=path.join(runtime,'chromium',process.platform==='win32'?'chrome.exe':'chrome');
  try{await access(bundled);return bundled;}catch{}
  const candidates=process.platform==='win32'?[path.join(process.env['ProgramFiles(x86)']||'C:/Program Files (x86)','Microsoft/Edge/Application/msedge.exe'),path.join(process.env.ProgramFiles||'C:/Program Files','Google/Chrome/Application/chrome.exe')]:['/usr/bin/chromium','/usr/bin/chromium-browser','/usr/bin/google-chrome'];
  for(const candidate of candidates){try{await access(candidate);return candidate;}catch{}}
  return undefined;
}
async function boot() {
  if(browser) return;
  browser=await chromium.launch({headless:true,chromiumSandbox:process.platform==='win32'||process.getuid?.()!==0,executablePath:await chromiumExecutable(),args:['--disable-background-networking','--disable-component-update','--disable-extensions','--disable-sync','--no-first-run','--host-resolver-rules=MAP * ~NOTFOUND']});
  context=await browser.newContext({viewport:{width:1024,height:768},deviceScaleFactor:1,serviceWorkers:'block',acceptDownloads:false});
  await context.route('**/*',route=>{blockedRequests++;return route.abort('blockedbyclient');});
  await context.routeWebSocket('**/*',ws=>ws.close());
  await newPage();
}
async function newPage() {
  if(page) await page.close().catch(()=>{});
  page=await context.newPage();
  page.on('dialog',d=>d.dismiss());page.on('popup',p=>p.close());
  await page.setContent(`<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; font-src data:; img-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'"><style>html,body{margin:0;padding:0;width:max-content;min-width:1px}#target{display:inline-block;padding:8px;width:max-content;min-width:1px;min-height:1px}.katex-display{margin:0!important}</style><div id="target"></div>`);
  await page.addStyleTag({content:await readFile(path.join(root,'vendor/katex.css'),'utf8')});
  await page.addStyleTag({content:await readFile(path.join(root,'vendor/diagram-font.css'),'utf8')});
  await page.addScriptTag({content:await readFile(path.join(root,'vendor/browser.js'),'utf8')});
}
async function plantuml(source,dark,signal) {
  if(/^\s*!|%\w+\s*\(|\[\[|https?:|file:|<img|<image|<svg|^\s*(?:skinparam|skin|url|path|include|load|import)\s/im.test(source)) throw Error('PlantUML preprocessing, files, URLs and custom resources are disabled.');
  let java=process.env.MDCARD_JAVA;
  if(!java) {let candidate=path.join(runtime,'java','bin',process.platform==='win32'?'java.exe':'java');try{await access(candidate);java=candidate;}catch{java='java';}}
  const jar=path.join(root,'vendor/plantuml.jar');
  return await new Promise((resolve,reject)=>{
    const child=spawn(java,['-Xmx192m','-Djava.awt.headless=true','-Dsun.java2d.fontpath='+path.join(root,'vendor/fonts'),'-DPLANTUML_SECURITY_PROFILE=SANDBOX','-jar',jar,'-tsvg','-pipe','-charset','UTF-8','-failfast2'],{signal,stdio:['pipe','pipe','pipe'],windowsHide:true,env:{PATH:process.env.PATH||'',SystemRoot:process.env.SystemRoot||'',PLANTUML_SECURITY_PROFILE:'SANDBOX'}});
    let output='',err='';const timer=setTimeout(()=>{child.kill();reject(Error('PlantUML timed out.'));},6000);
    child.stdout.on('data',d=>{output+=d;if(output.length>4e6)child.kill();});child.stderr.on('data',d=>{if(err.length<4000)err+=d;});
    child.on('error',e=>{clearTimeout(timer);reject(e);});child.on('exit',code=>{clearTimeout(timer);if(code!==0||!/\<svg\b/.test(output))reject(Error('PlantUML syntax error: '+err.slice(0,250)));else resolve(output);});
    const isComplete=/^\s*@start/m.test(source);
    let input=isComplete?source:`@startuml\n${source}\n@enduml`;
    input=input.replace(/(@start\w+[^\n]*\n)/,'$1!pragma layout smetana\n');
    input=input.replace(/(@start\w+[^\n]*\n)/,`$1skinparam defaultFontName ${/[\uac00-\ud7a3]/.test(source)?'MDcard Diagram Sans KR':'MDcard Diagram Sans'}\n`);
    if(dark) input=input.replace(/(@start\w+[^\n]*\n)/,'$1skinparam backgroundColor #20212a\nskinparam defaultFontColor #e7e7ed\nskinparam ArrowColor #d0d4e0\nskinparam sequenceLifeLineBorderColor #d0d4e0\nskinparam defaultBackgroundColor #34394b\nskinparam defaultBorderColor #d0d4e0\nskinparam participantBackgroundColor #34394b\nskinparam participantBorderColor #d0d4e0\nskinparam activityBackgroundColor #34394b\nskinparam activityBorderColor #d0d4e0\nskinparam activityDiamondBackgroundColor #34394b\nskinparam activityDiamondBorderColor #d0d4e0\nskinparam activityStartColor #d0d4e0\nskinparam activityEndColor #d0d4e0\n');
    child.stdin.end(input);
  });
}
export async function render(request,signal) {
  signal?.throwIfAborted();
  const started=performance.now();
  if(!request || !['math','mermaid','plantuml','flow'].includes(request.kind) || typeof request.source!=='string' || Buffer.byteLength(request.source)>65536) throw Error('Invalid node or 64 KiB limit exceeded.');
  request={kind:request.kind,source:request.source,dark:!!request.dark,inline:!!request.inline,font:Math.min(32,Math.max(10,Number(request.font)||18))};
  const key=sha(JSON.stringify(request));
  if(cache.has(key)){hits++;const hit=cache.get(key);cache.delete(key);cache.set(key,hit);return {...hit,cached:true,renders,hits,blockedRequests,ms:performance.now()-started};}
  await boot();
  signal?.throwIfAborted();
  if(request.kind==='plantuml') request.svg=await plantuml(request.source,request.dark,signal);
  let timer;
  try {
    const details=await Promise.race([page.evaluate(r=>MDcardRenderer.render(r),request),new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Rendering timed out.')),8000);})]);
    const png=await page.locator('#target').screenshot({type:'png',timeout:5000});renders++;
    const result={key,...details,png:png.toString('base64'),cached:false,renders,hits,blockedRequests,ms:performance.now()-started};
    const bytes=Buffer.byteLength(result.png)+Buffer.byteLength(result.svg||'');
    while(cache.size && (cache.size>=1024 || cacheBytes+bytes>32*1024*1024)){const oldest=cache.keys().next().value;const item=cache.get(oldest);cacheBytes-=Buffer.byteLength(item.png)+Buffer.byteLength(item.svg||'');cache.delete(oldest);}
    if(bytes<=32*1024*1024){cache.set(key,result);cacheBytes+=bytes;}return result;
  } catch(e) {await newPage();throw e;} finally {clearTimeout(timer);}
}
export async function close(){await browser?.close();browser=undefined;}

if(process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const lines=createInterface({input:process.stdin,crlfDelay:Infinity});
  for await(const line of lines) {
    if(line.length>150000){process.stdout.write(JSON.stringify({ok:false,error:'IPC line too large'})+'\n');continue;}
    let request;
    try {request=JSON.parse(line);const result=await render(request);process.stdout.write(JSON.stringify({ok:true,id:request.id,...result})+'\n');}
    catch(e){process.stdout.write(JSON.stringify({ok:false,id:request?.id,error:String(e.message).slice(0,1000)})+'\n');}
  }
  await close();
}
