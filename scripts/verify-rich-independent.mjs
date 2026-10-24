// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
// Independent acceptance harness. Run after the implementation checkpoint.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
const root=fileURLToPath(new URL('../',import.meta.url));
const renderer=path.join(root,'renderer');
const out=process.env.MDCARD_RICH_INDEPENDENT_OUTPUT||path.join(root,'evidence/rich/master-review/engines');
await mkdir(out,{recursive:true});
const {render,close}=await import(pathToFileURL(path.join(renderer,'worker.mjs')));
const {chromium}=await import(pathToFileURL(path.join(renderer,'node_modules/playwright/index.mjs')));
const sources={};
for(const name of ['worker.mjs','browser.js','vendor/browser.js','vendor/katex.css','package-lock.json'])
 sources[name]=createHash('sha256').update(await readFile(path.join(renderer,name))).digest('hex');
const browser=await chromium.launch({headless:true,executablePath:process.env.MDCARD_CHROMIUM||undefined});
const page=await browser.newPage();
await page.route('**/*',r=>r.abort());
const records=[],failures=[];
async function test(name,fn){try{await fn();console.log('PASS '+name);}catch(e){failures.push({name,error:String(e.stack||e)});console.log('FAIL '+name+': '+e.message);}}
async function inspectSvg(svg,dark,kind){
 await page.setContent(`<body style="background:${dark?'#20212a':'white'}">${svg}</body>`);
 const styles=await page.evaluate(()=>({
  nodes:[...document.querySelectorAll('g.node')].map(n=>{
   const shape=n.querySelector('.label-container,.node-bkg,circle,rect,path');
   const text=n.querySelector('text tspan:not(:has(tspan))')||n.querySelector('text');
   return {text:n.textContent.trim(),fill:shape?getComputedStyle(shape).fill:null,textFill:text?getComputedStyle(text).fill:null};
  }),
  messages:[...document.querySelectorAll('.messageLine0,.messageLine1')].map(n=>({stroke:getComputedStyle(n).stroke})),
  text:[...document.querySelectorAll('text')].flatMap(n=>n.querySelector('tspan')?[...n.querySelectorAll('tspan:not(:has(tspan))')]:[n]).map(n=>{
   const rect=n.getBoundingClientRect();const x=rect.x+rect.width/2,y=rect.y+rect.height/2;
   const shape=document.elementsFromPoint(x,y).find(e=>/^(rect|circle|ellipse|path|polygon)$/.test(e.localName)&&getComputedStyle(e).fill!=='none');
   return {text:n.textContent.trim(),fill:getComputedStyle(n).fill,background:shape?getComputedStyle(shape).fill:getComputedStyle(document.body).backgroundColor};
  })
 }));
 assert(styles.text.some(n=>n.text),'SVG contains no real readable text nodes');
 if(kind==='mermaid') {
  for(const n of styles.nodes.filter(n=>n.text&&n.textFill&&n.fill))assert.notEqual(n.textFill,n.fill,`Hidden text ${n.text}`);
  for(const m of styles.messages)assert.notEqual(m.stroke,'none','Sequence message line disappeared');
 }
 for(const label of styles.text.filter(t=>t.text)) {
  const luminance=color=>{
   const match=color.match(/^rgba?\(([^)]+)\)$/);if(!match)return null;
   const rgb=match[1].split(/[,\s]+/).slice(0,3).map(Number).map(n=>n/255).map(n=>n<=.04045?n/12.92:((n+.055)/1.055)**2.4);
   return rgb[0]*.2126+rgb[1]*.7152+rgb[2]*.0722;
  };
  const a=luminance(label.fill),b=luminance(label.background);
  if(a!==null&&b!==null){label.contrast=(Math.max(a,b)+.05)/(Math.min(a,b)+.05);assert(label.contrast>=1.5,`Unreadable SVG label ${JSON.stringify(label)}`);}
 }
 return styles;
}
const cases=[
 ['inline-integral','math',String.raw`\int_0^1 x^2\,dx=\frac13`,true],
 ['matrix','math',String.raw`\begin{bmatrix}1&\alpha\\\beta&4\end{bmatrix}`,false],
 ['multiline','math',String.raw`\begin{aligned}x+y&=3\\x-y&=1\end{aligned}`,false],
 ['chemical','math',String.raw`\ce{SO4^2- + Ba^2+ -> BaSO4 v}`,false],
 ['numeric','math','42',true],
 ['mermaid-click-label','mermaid','flowchart LR\n A[Click to continue] --> B[Import file]',false],
 ['mermaid-sequence','mermaid','sequenceDiagram\n participant Alice\n participant Bob\n Alice->>Bob: Hello!\n Bob-->>Alice: 100% ready',false],
 ['mermaid-mindmap','mermaid','mindmap\n root((Root))\n  Branch A\n  Branch B',false],
 ['plantuml-activity','plantuml','@startuml\nstart\n:Import;\n:Load path;\nstop\n@enduml',false],
 ['plantuml-sequence','plantuml','@startuml\nAlice -> Bob: Hello!\nBob --> Alice: 100% ready\n@enduml',false],
 ['plantuml-mindmap','plantuml','@startmindmap\n* Root\n** Branch A\n** Branch B\n@endmindmap',false],
 ['flowchart-condition','flow','s=>start: Start\nc=>condition: Ready?\na=>operation: Work\ne=>end: End\ns->c\nc(yes)->a->e\nc(no)->e',false],
 ['flowchart-comparisons','flow','s=>start: Start\nc=>condition: count > 0 && count < 10\na=>operation: Work\ne=>end: End\ns->c\nc(yes)->a->e\nc(no)->e',false],
 ['mermaid-cjk','mermaid','flowchart TD\n A[中文公式] --> B[数学图表]',false],
 ['mermaid-korean','mermaid','flowchart LR\n A[한글 수식] --> B[다이어그램]',false]
];
try {
 for(const [name,kind,source,inline] of cases)for(const dark of [false,true])await test(`${name}-${dark?'dark':'light'}`,async()=>{
  const r=await render({kind,source,inline,dark});
  assert(r.width>0&&r.height>0&&r.elementCount>2);
  const png=Buffer.from(r.png,'base64');assert.equal(png.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  if(name==='flowchart-comparisons'){
   assert(r.text.includes('count > 0 && count < 10'),'Comparison operators changed in a plain text label');
   assert(r.text.includes('yes')&&r.text.includes('no'),'Condition branch labels disappeared');
  }
  await writeFile(path.join(out,`${name}-${dark?'dark':'light'}.png`),png);
  let styles;
  if(kind!=='math'){
   assert(r.svg&&r.svg.startsWith('<svg'));
   assert(!/<(?:script|foreignObject|iframe|image|animate|set)(?:\s|>)/i.test(r.svg));
   assert(!/\son\w+\s*=/i.test(r.svg));
   styles=await inspectSvg(r.svg,dark,kind);
   await writeFile(path.join(out,`${name}-${dark?'dark':'light'}.svg`),r.svg);
  }
  const again=await render({kind,source,inline,dark});assert.equal(again.cached,true);assert.equal(again.renders,r.renders);
  records.push({name,kind,dark,width:r.width,height:r.height,ms:r.ms,cachedMs:again.ms,key:r.key,styles});
 });
 const rejects=[
  ['broken-math','math',String.raw`\begin{matrix}a&b`],
  ['macro-loop','math',String.raw`\def\loop{\loop}\loop`],
  ['broken-mermaid','mermaid','flowchart TD\n A['],
  ['mermaid-config','mermaid','%%{init: {securityLevel:"loose"}}%%\ngraph TD\nA-->B'],
  ['mermaid-frontmatter','mermaid','---\nconfig:\n securityLevel: loose\n---\ngraph TD\nA-->B'],
  ['plantuml-include','plantuml','@startuml\n!include C:/Windows/win.ini\n@enduml'],
  ['plantuml-network','plantuml','@startuml\n!includeurl https://example.invalid/private\n@enduml'],
  ['flowchart-html','flow','s=>start: <script>alert(1)</script>\ne=>end: End\ns->e'],
  ['flowchart-url','flow','s=>start: Start:>https://example.invalid\ne=>end: End\ns->e'],
  ['too-large','math','x'.repeat(65537)]
 ];
 for(const [name,kind,source]of rejects)await test(name,async()=>{
  await assert.rejects(()=>render({kind,source}));
  const r=await render({kind:'math',source:'x+'+rejects.findIndex(x=>x[0]===name)});assert(r.elementCount>2);
 });
 await test('svg-active-and-css-injection',async()=>{
  await page.setContent('<body></body>');await page.addScriptTag({content:await readFile(path.join(renderer,'vendor/browser.js'),'utf8')});
  const payload=String.raw`<svg xmlns="http://www.w3.org/2000/svg"><style>@im\70ort '//attacker.invalid/a.css';rect{fill:u\72l(//attacker.invalid/x)}</style><set attributeName="onload" to="alert(1)"/><image href="file:///private"/><a href="javascript:alert(1)"><text>safe</text></a><rect width="10" height="10"/></svg>`;
  const clean=await page.evaluate(s=>MDcardRenderer.sanitize(s),payload);
  assert(!/attacker|javascript:|file:|onload|<set|<image|<a\s/i.test(clean));
  assert(clean.includes('safe'));
 });
 await test('chemical-and-stretchy-math-retain-svg',async()=>{
  await page.setContent('<body><div id="target"></div></body>');
  await page.addStyleTag({content:await readFile(path.join(renderer,'vendor/katex.css'),'utf8')});
  await page.addScriptTag({content:await readFile(path.join(renderer,'vendor/browser.js'),'utf8')});
  for(const source of [String.raw`\ce{A -> B}`,String.raw`\sqrt{\frac{1}{1+x^2}}`,String.raw`\overrightarrow{AB}`]){
   const paths=await page.evaluate(async source=>{
    await MDcardRenderer.render({kind:'math',source,dark:false,inline:false});
    return document.querySelectorAll('#target svg path').length;
   },source);
   assert(paths>0,`Math SVG paths were stripped from ${source}`);
  }
 });
 await test('real-stdio-preserves-u64-ids-and-isolates-errors',async()=>{
  const child=spawn(process.execPath,[path.join(renderer,'worker.mjs')],{cwd:renderer,env:process.env,windowsHide:true,stdio:['pipe','pipe','pipe']});
  const lines=createInterface({input:child.stdout});
  let errors='';child.stderr.on('data',d=>{errors+=d});
  const responses=[];
  const timer=setTimeout(()=>child.kill(),20000);
  try{
   for(const request of [{id:'18446744073709551615',kind:'math',source:'x^2'}, {id:'9007199254740993',kind:'math',source:String.raw`\frac{`}, {id:'18446744073709551614',kind:'math',source:'y^2'}])child.stdin.write(JSON.stringify(request)+'\n');
   child.stdin.end();
   for await(const line of lines)responses.push(JSON.parse(line));
   assert.equal(responses.length,3,errors);
   assert.deepEqual(responses.map(r=>r.id),['18446744073709551615','9007199254740993','18446744073709551614']);
   assert.deepEqual(responses.map(r=>r.ok),[true,false,true]);
  }finally{clearTimeout(timer);child.kill();}
 });
 await writeFile(path.join(out,'result.json'),JSON.stringify({runId:randomUUID(),date:new Date().toISOString(),harnessSha256:createHash('sha256').update(await readFile(fileURLToPath(import.meta.url))).digest('hex'),sources,passed:failures.length===0,records,failures},null,2));
} finally {await close();await browser.close();}
if(failures.length)process.exitCode=1;
