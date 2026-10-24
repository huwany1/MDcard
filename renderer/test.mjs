// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import assert from 'node:assert/strict';
import {render,close} from './worker.mjs';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {randomUUID} from 'node:crypto';
const out=process.env.MDCARD_RENDER_TEST_OUTPUT||'../evidence/rich/engines';await mkdir(out,{recursive:true});
const cases=[
 ['matrix','math',String.raw`\begin{pmatrix}a&b\\c&d\end{pmatrix}`],
 ['aligned','math',String.raw`\begin{aligned}a&=b+c\\d&=e\end{aligned}`],
 ['calculus','math',String.raw`\int_0^{\infty} e^{-x^2}\,dx=\frac{\sqrt{\pi}}2`],
 ['greek','math',String.raw`\alpha+\beta=\Gamma_{\delta}`],
 ['chemistry','math',String.raw`\ce{2H2 + O2 -> 2H2O}`],
 ['numeric','math','100'],
 ['mermaid-korean','mermaid','flowchart LR\nA[한글 수식] --> B[다이어그램]'],
 ['mermaid-flow','mermaid','flowchart TD\nA[Start] --> B{Ready}\nB --> C[Finish]'],
 ['mermaid-sequence','mermaid','sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World'],
 ['mermaid-mindmap','mermaid','mindmap\n  root((Editor))\n    Math\n    Diagrams'],
 ['plantuml-flow','plantuml','@startuml\nstart\n:Edit;\n:Preview;\nstop\n@enduml'],
 ['plantuml-sequence','plantuml','@startuml\nAlice -> Bob: Hello\nBob --> Alice: World\n@enduml'],
 ['plantuml-mindmap','plantuml','@startmindmap\n* Editor\n** Math\n** Diagrams\n@endmindmap'],
 ['flowchart-comparison','flow','st=>start: Start\ncond=>condition: count > 0\ne=>end: Done\nst->cond\ncond(yes)->e\ncond(no)->st'],
 ['flowchart','flow','st=>start: Start\nop=>operation: Edit\ne=>end: Done\nst->op->e'],
];
const results=[];
try {
const buildInputs=JSON.parse(await readFile(new URL('./vendor/build-inputs.json',import.meta.url),'utf8'));
assert(!Object.keys(buildInputs.inputs).some(name=>name.includes('elkjs/')),'EPL ELK code must not enter the AGPL bundle');
await assert.rejects(render({kind:'mermaid',source:'flowchart-elk TD\nA --> B'}),/ELK layout is not included/);
for(const [name,kind,source] of cases) for(const dark of [false,true]) {
 const r=await render({kind,source,dark});assert(r.width>8&&r.height>8);assert(r.elementCount>2);assert(Buffer.from(r.png,'base64').subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])));
 if(kind!=='math'){assert(r.svg.startsWith('<svg'));assert(!/<(?:script|foreignObject|image|a\s|animate|set\s)|\son\w+=|(?:https?:)?\/\//i.test(r.svg.replace('http://www.w3.org/2000/svg','')));await writeFile(`${out}/${name}-${dark?'dark':'light'}.svg`,r.svg);}
 await writeFile(`${out}/${name}-${dark?'dark':'light'}.png`,Buffer.from(r.png,'base64'));results.push({name,dark,key:r.key,width:r.width,height:r.height,ms:r.ms,blockedRequests:r.blockedRequests});
 const cached=await render({kind,source,dark});assert(cached.cached);assert.equal(cached.renders,r.renders);
}
for(const [kind,source] of [['math',String.raw`\frac{`],['mermaid','not a diagram'],['mermaid','%%{init: {securityLevel:"loose"}}%%\ngraph TD\na-->b'],['plantuml','!include /etc/passwd'],['plantuml','!includeurl https://example.com/a'],['flow','st=>start: <script>alert(1)</script>'],['math','x'.repeat(65537)]]) {
 let rejected=false;try{await render({kind,source});}catch{rejected=true;}assert(rejected,`${kind} should reject unsafe or invalid input`);
}
const final=await render({kind:'math',source:'x^2+y^2=z^2'});assert(final.elementCount>3);
await writeFile(`${out}/result.json`,JSON.stringify({runId:randomUUID(),date:new Date().toISOString(),passed:true,results,maliciousRejected:7,recoveredAfterError:true},null,2));console.log(`Passed ${results.length} real engine/theme cases, cache reuse, 7 invalid/malicious inputs and post-error recovery.`);
}finally{await close();}
