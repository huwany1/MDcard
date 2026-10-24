// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
// Isolated export process. No source writes; strict preprocessing precedes publication.
import {readFile,writeFile,mkdir,mkdtemp,rm,rename,access,stat,open,realpath} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import {spawn} from 'node:child_process';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
import {chromium} from 'playwright';
import MarkdownIt from 'markdown-it';
import footnote from 'markdown-it-footnote';
import YAML from 'yaml';
import hljs from 'highlight.js';
import {Marp} from '@marp-team/marp-core';
import sharp from 'sharp';
import JSZip from 'jszip';
import {DOMParser,XMLSerializer} from '@xmldom/xmldom';
import {mathjax} from 'mathjax-full/js/mathjax.js';
import {TeX} from 'mathjax-full/js/input/tex.js';
import {SVG} from 'mathjax-full/js/output/svg.js';
import {liteAdaptor} from 'mathjax-full/js/adaptors/liteAdaptor.js';
import {RegisterHTMLHandler} from 'mathjax-full/js/handlers/html.js';
import {AllPackages} from 'mathjax-full/js/input/tex/AllPackages.js';
import {render as richRender,close as richClose} from './worker.mjs';
import {finalizeDocx} from './export-docx.mjs';
import {portableFontCss,portableSvg} from './export-fonts.mjs';
import {runOwned} from './export-process.mjs';

const ROOT=path.dirname(fileURLToPath(import.meta.url));
const MAX_RESOURCE=20*1024*1024;
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function resourceLabel(value){if(value==null)return value;value=String(value);if(/^data:/i.test(value))return value.slice(0,Math.min(value.indexOf(',')+1||80,100))+'… ('+value.length+' characters)';return value.length>512?value.slice(0,380)+'…'+value.slice(-100):value;}
export class ExportError extends Error {constructor(stage,message,line,resource){super(String(message).slice(0,4000));this.stage=stage;this.line=line;this.resource=resourceLabel(resource);}}
function fail(stage,message,line,resource){throw new ExportError(stage,message,line,resource);}
const adaptor=liteAdaptor();RegisterHTMLHandler(adaptor);
const mathDoc=mathjax.document('',{InputJax:new TeX({packages:AllPackages.filter(x=>!['noerrors','noundefined','autoload','require','html'].includes(x)),maxBuffer:65536,maxMacros:1000,formatError:(_jax,e)=>{throw e;}}),OutputJax:new SVG({fontCache:'none'})});
function mathSvg(source,display,line,options={}){
  try {const html=adaptor.outerHTML(mathDoc.convert(source,{display}));let svg=html.match(/<svg[\s\S]*<\/svg>/)?.[0];if(!svg||/data-mjx-error/.test(svg))throw Error('Invalid TeX');const color=options.foreground||(options.dark?'#e7e7ed':'#242432');svg=svg.replaceAll('currentColor',color);return svg;}catch(e){fail('math',e.message,line);}
}
function mathRules(md){
  md.inline.ruler.before('escape','export_math',(state,silent)=>{
    const start=state.pos;let open,close;
    if(state.src.startsWith('\\(',start)){open='\\(';close='\\)';}
    else if(state.src[start]==='$'&&state.src[start+1]!=='$'&&!/\s/.test(state.src[start+1]||' ')){open='$';close='$';}
    else return false;
    const escaped=at=>{let n=0;for(let i=at-1;i>=0&&state.src[i]==='\\';i--)n++;return n%2===1;};
    let end=state.src.indexOf(close,start+open.length);
    while(end>=0&&escaped(end))end=state.src.indexOf(close,end+close.length);
    if(end<0||end>=state.posMax||end===start+open.length)return false;
    const body=state.src.slice(start+open.length,end);if(/[\r\n]/.test(body)||(close==='$'&&(/\s$/.test(body)||/\d/.test(state.src[end+1]||''))))return false;
    for(let i=start+open.length;i<end;i++)if(state.src[i]==='`'&&!escaped(i))return false;
    if(!silent){const t=state.push('export_math','',0);t.content=state.src.slice(start+open.length,end);}
    state.pos=end+close.length;return true;
  });
  md.block.ruler.before('fence','export_math_block',(state,start,end,silent)=>{
    const text=state.src.slice(state.bMarks[start]+state.tShift[start],state.eMarks[start]).trim();
    const open=text.startsWith('$$')?'$$':text.startsWith('\\[')?'\\[':null;if(!open)return false;
    const close=open==='$$'?'$$':'\\]';let body=text.slice(2);let next=start;
    if(body.endsWith(close)&&body.length>=close.length)body=body.slice(0,-2);
    else {body+='\n';for(next=start+1;next<end;next++){const line=state.src.slice(state.bMarks[next],state.eMarks[next]);const at=line.indexOf(close);if(at>=0){body+=line.slice(0,at);break;}body+=line+'\n';}if(next>=end)return false;}
    if(silent)return true;const token=state.push('export_math_block','',0);token.map=[start,next+1];token.content=body;state.line=next+1;return true;
  });
}
function highlightedLines(source,language){
  let code=esc(source);if(hljs.getLanguage(language)){try{code=hljs.highlight(source,{language,ignoreIllegals:true}).value;}catch{}}
  code=code.replace(/\n$/,'');const stack=[];const lines=[];let current='';
  // A highlighter token can span multiple source lines. Close and reopen its
  // spans at every newline so each code-line owns exactly one source line.
  for(const match of code.matchAll(/<\/?span\b[^>]*>|\n|[^<\n]+|</g)){const token=match[0];if(token==='\n'){lines.push(current+'</span>'.repeat(stack.length));current=stack.join('');}else if(token.startsWith('</span')){current+=token;stack.pop();}else if(token.startsWith('<span')){current+=token;stack.push(token);}else current+=token;}
  lines.push(current+'</span>'.repeat(stack.length));return lines.map(line=>`<span class="code-line">${line}</span>`).join('');
}
async function compiledMarkdown(source,options,progress){
  source=source.replace(/^\uFEFF/,'');
  const front=source.match(/^---\r?\n[\s\S]*?\r?\n(?:---|\.\.\.)(?:\r?\n|$)/);
  if(front)source='\n'.repeat(front[0].split('\n').length-1)+source.slice(front[0].length);
  const md=new MarkdownIt({html:true,linkify:false,typographer:false}).use(mathRules).use(footnote);
  const validateLink=md.validateLink;md.validateLink=uri=>/^data:image\/svg\+xml(?:;[^,]*)?,/i.test(uri)||validateLink(uri);
  const tokens=md.parse(source,{});let completed=0;
  const work=[];
  function visit(items,parentLine=1){for(const t of items){const line=(t.map?.[0]??parentLine-1)+1;t.meta={...t.meta,exportLine:line};if(t.children)visit(t.children,line);if(t.type==='fence'||t.type==='export_math'||t.type==='export_math_block')work.push(t);}}
  visit(tokens);
  for(const t of work){
    const lang=t.info?.trim().split(/\s+/)[0].toLowerCase();const kind=({mermaid:'mermaid',plantuml:'plantuml',puml:'plantuml',flow:'flow',flowchart:'flow',math:'math',latex:'math'})[lang];
    if(t.type.startsWith('export_math')||kind==='math'){
      const svg=mathSvg(t.content,t.type!=='export_math',t.meta.exportLine,options);t.meta.compiled=`<img class="formula ${t.type==='export_math'?'inline':'display'}" alt="${esc(t.content)}" src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}" data-line="${t.meta.exportLine}">`;
    } else if(kind){
      try{const result=await richRender({kind,source:t.content,dark:options.dark,font:options.fontSize||16},options.signal);if(!result.svg)throw Error('No vector output');const vector=await portableSvg(result.svg);t.meta.compiled=`<img class="diagram" alt="${kind} diagram" src="data:image/svg+xml;base64,${Buffer.from(vector).toString('base64')}" data-line="${t.meta.exportLine}">`;}
      catch(e){fail('diagram',e.message,t.meta.exportLine,kind);}
    }
    progress({stage:'compile',progress:10+Math.round(++completed/Math.max(1,work.length)*25)});
  }
  md.renderer.rules.export_math=(ts,i)=>ts[i].meta.compiled;
  md.renderer.rules.export_math_block=(ts,i)=>`<p class="figure" data-line="${ts[i].meta.exportLine}">${ts[i].meta.compiled}</p>`;
  md.renderer.rules.fence=(ts,i)=>{
    const t=ts[i];if(t.meta.compiled)return `<figure data-line="${t.meta.exportLine}">${t.meta.compiled}</figure>`;
    const language=(t.info||'').trim().split(/\s+/)[0]||'plaintext';
    // Preserve a single semantic code block for Pandoc; HTML line numbers use CSS counters.
    const lines=highlightedLines(t.content,language);
    return `<pre class="sourceCode ${esc(language)} ${options.lineNumbers!==false?'numberLines':''}" data-line="${t.meta.exportLine}"><code class="language-${esc(language)}">${lines}</code></pre>`;
  };
  md.renderer.rules.code_block=md.renderer.rules.fence;
  const original=md.renderer.rules.html_block||((ts,i)=>ts[i].content);
  md.renderer.rules.html_block=(ts,i,opts,env,self)=>{
    if(/^\s*<!--\s*pagebreak\s*-->\s*$/i.test(ts[i].content))return `<div class="pagebreak" data-line="${ts[i].map[0]+1}"></div>`;
    return `<div data-line="${(ts[i].map?.[0]??0)+1}">${original(ts,i,opts,env,self)}</div>`;
  };
  // Transform only complete paragraph tokens, so code spans/fences and escaped text remain literal.
  for(let i=0;i<tokens.length-2;i++)if(tokens[i].type==='paragraph_open'&&tokens[i+1].type==='inline'&&tokens[i+1].content.trim()==='\\newpage'&&source.split(/\r?\n/)[tokens[i+1].map?.[0]??-1]?.trim()==='\\newpage'){
    tokens[i].type='html_block';tokens[i].content=`<div class="pagebreak" data-line="${tokens[i+1].map[0]+1}"></div>`;tokens[i+1].hidden=true;tokens[i+1].children=[];tokens[i+2].hidden=true;
  }
  // Attach source locations to semantic blocks for actionable DOM errors.
  const defaultRender=md.renderer.renderToken.bind(md.renderer);
  md.renderer.renderToken=(ts,i,opts)=>{const t=ts[i];if(t.nesting===1&&t.map)t.attrSet('data-line',String(t.map[0]+1));return defaultRender(ts,i,opts);};
  return md.renderer.render(tokens,md.options,{});
}

export async function bundledBrowser(){
  const platformDirectory=path.join(ROOT,'runtime',process.platform==='win32'?'windows':'linux','chromium');
  let directory;try{await access(platformDirectory);directory=platformDirectory;}catch{directory=path.join(ROOT,'runtime/chromium');}
  const executable=path.join(directory,process.platform==='win32'?'chrome.exe':'chrome');
  try{await access(executable);}catch{fail('dependency','Bundled Chromium is missing. Run scripts/prepare-export-runtime.py or reinstall the complete MDcard package.',undefined,executable);}
  return executable;
}
async function run(command,args,options={}){
  return await runOwned(command,args,{...options,makeError:(stage,message)=>new ExportError(stage,message,undefined,command)});
}

function mimeOf(bytes,name){
  if(bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))return 'image/png';
  if(bytes[0]===255&&bytes[1]===216)return 'image/jpeg';
  if(bytes.subarray(0,3).toString()==='GIF')return 'image/gif';
  if(bytes.subarray(0,4).toString()==='RIFF'&&bytes.subarray(8,12).toString()==='WEBP')return 'image/webp';
  if(/<svg[\s>]/i.test(bytes.toString('utf8',0,1000)))return 'image/svg+xml';
  if(/\.(woff2?|ttf|otf)$/i.test(name))return /woff2$/i.test(name)?'font/woff2':/woff$/i.test(name)?'font/woff':'font/ttf';
  fail('resource','Unsupported or invalid image/font bytes',undefined,name);
}
function validateSvg(bytes,resource,line){
  const text=bytes.toString('utf8');if(/<!DOCTYPE|<!ENTITY|\\|@import/i.test(text))fail('resource','SVG entities, CSS escapes and stylesheet imports are not supported',line,resource);
  const doc=new DOMParser().parseFromString(text,'image/svg+xml');
  for(const el of Array.from(doc.getElementsByTagName('*'))){
    if(['script','foreignobject','iframe','object','embed','audio','video','animate','set'].includes(el.localName.toLowerCase()))fail('resource','Active SVG elements are not supported',line,resource);
    for(const attr of Array.from(el.attributes||[])){
      if(/^on/i.test(attr.name)||((/^(?:href|xlink:href|src)$/i.test(attr.name))&&!attr.value.startsWith('#')))fail('resource','SVG external/active references are not supported',line,resource);
      for(const m of attr.value.matchAll(/url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*?))\s*\)/gi)){const uri=(m[1]??m[2]??m[3]).trim();if(!uri.startsWith('#'))fail('resource','SVG external/active references are not supported',line,resource);}
    }
    if(el.localName==='style'){
      if(/@(?!font-face\b)|\\|expression\s*\(/i.test(el.textContent))fail('resource','SVG external CSS is not supported',line,resource);
      for(const match of el.textContent.matchAll(/url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*?))\s*\)/gi)){const uri=match[1]??match[2]??match[3];if(!uri.startsWith('#')&&!/^data:font\/(?:woff2?|ttf);base64,[a-z\d+/=]+$/i.test(uri))fail('resource','SVG external CSS is not supported',line,resource);}
    }
  }
}
async function resources(page,base,progress,signal){
  const cache=new Map();let bytesUsed=0;
  async function inline(uri,line){
    if(uri.startsWith('#'))return uri;if(cache.has(uri))return cache.get(uri);
    let bytes,name=uri;
    try {
      if(/^data:/i.test(uri)){
        const m=uri.match(/^data:([^,]*),([\s\S]*)$/i);if(!m)throw Error('Malformed data URL');bytes=/;base64(?:;|$)/i.test(m[1])?Buffer.from(m[2],'base64'):Buffer.from(decodeURIComponent(m[2]));
      } else if(/^https?:/i.test(uri)){
        const response=await fetch(uri,{signal:signal?AbortSignal.any([signal,AbortSignal.timeout(12000)]):AbortSignal.timeout(12000),redirect:'follow'});if(!response.ok)throw Error(`HTTP ${response.status}`);
        if(Number(response.headers.get('content-length'))>MAX_RESOURCE)throw Error('20 MiB resource limit exceeded');
        const reader=response.body.getReader();const chunks=[];let count=0;for(;;){const {value,done}=await reader.read();if(done)break;count+=value.length;if(count>MAX_RESOURCE){await reader.cancel();throw Error('20 MiB resource limit exceeded');}chunks.push(value);}bytes=Buffer.concat(chunks);
      } else {
        if(/^[a-z][a-z\d+.-]*:/i.test(uri)&&!/^file:/i.test(uri))throw Error('Unsupported resource protocol');
        const local=/^file:/i.test(uri)?fileURLToPath(uri):path.resolve(base,decodeURIComponent(uri.split('#')[0]));
        if((await stat(local)).size>MAX_RESOURCE)throw Error('20 MiB resource limit exceeded');bytes=await readFile(local);
      }
      if(bytes.length>MAX_RESOURCE)throw Error('20 MiB resource limit exceeded');bytesUsed+=bytes.length;if(bytesUsed>128*1024*1024)throw Error('128 MiB document resource budget exceeded');
      const embeddedMime=uri.match(/^data:(font\/[^;,]+)/i)?.[1];const mime=embeddedMime||mimeOf(bytes,name);if(mime==='image/svg+xml')validateSvg(bytes,uri,line);
      const data=`data:${mime};base64,${bytes.toString('base64')}`;cache.set(uri,data);return data;
    }catch(e){if(signal?.aborted)fail('timeout','Export exceeded its task deadline',line,uri);if(e instanceof ExportError){e.line??=line;throw e;}fail('resource',e.message,line,uri);}
  }
  const diagnostics=await page.evaluate(()=>{
    const problems=[];for(const el of document.querySelectorAll('*')){
      const line=Number(el.closest('[data-line]')?.getAttribute('data-line'))||undefined;
      if(['SCRIPT','IFRAME','OBJECT','EMBED','LINK','BASE','AUDIO','VIDEO','SOURCE','FORM','INPUT','BUTTON','CANVAS'].includes(el.tagName)||(el.tagName==='META'&&el.httpEquiv.toLowerCase()==='refresh'))problems.push({message:`Unsupported active/external element: ${el.tagName}`,line});
      for(const a of [...el.attributes])if(/^on/i.test(a.name)||['srcdoc','srcset','poster','action'].includes(a.name)||/^(?:javascript|vbscript):/i.test(a.value.trim()))problems.push({message:`Unsupported attribute: ${a.name}`,line});
      if(el.namespaceURI==='http://www.w3.org/2000/svg'&&el.tagName.toLowerCase()==='foreignobject')problems.push({message:'SVG foreignObject is not supported',line});
    }return problems;
  });
  if(diagnostics.length)fail('preprocess',diagnostics[0].message,diagnostics[0].line);
  const images=await page.locator('img').evaluateAll(els=>els.map((el,index)=>({index,uri:el.getAttribute('src')||'',line:Number(el.closest('[data-line]')?.getAttribute('data-line'))||undefined})));
  for(let i=0;i<images.length;i++){
    const entry=images[i];const data=await inline(entry.uri,entry.line);await page.locator('img').nth(entry.index).evaluate((el,url)=>el.setAttribute('src',url),data);progress({stage:'resources',progress:35+Math.round((i+1)/Math.max(1,images.length)*20)});
  }
  const svgs=await page.locator('svg').evaluateAll(els=>els.filter(e=>!e.parentElement?.closest('svg')).map(e=>({svg:e.outerHTML,line:Number(e.closest('[data-line]')?.getAttribute('data-line'))||undefined})));
  for(const item of svgs)validateSvg(Buffer.from(item.svg),'inline SVG',item.line);
  // CSS is deliberately parsed conservatively: unresolved escapes/imports fail visibly.
  const styles=await page.evaluate(()=>[...document.querySelectorAll('style,[style]')].map((el,index)=>({index,tag:el.tagName,text:el.tagName==='STYLE'?el.textContent:el.getAttribute('style'),line:Number(el.closest('[data-line]')?.getAttribute('data-line'))||undefined})));
  for(const s of styles){
    if(/@import|@namespace|\\|expression\s*\(|-moz-binding|behavior\s*:/i.test(s.text))fail('style','CSS imports, escapes or active styles are not supported; inline stylesheets directly.',s.line);
    let text=s.text;
    for(const m of [...text.matchAll(/url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*?))\s*\)/gi)]){const uri=m[1]??m[2]??m[3];if(uri.startsWith('#'))continue;const data=await inline(uri,s.line);text=text.replace(m[0],`url("${data}")`);}
    await page.locator('style,[style]').nth(s.index).evaluate((el,text)=>el.tagName==='STYLE'?el.textContent=text:el.setAttribute('style',text),text);
  }
  await page.evaluate(async()=>{await document.fonts.ready;await Promise.all([...document.images].map(img=>img.decode().catch(()=>{})));});
  const broken=await page.locator('img').evaluateAll(els=>els.filter(i=>!i.complete||!i.naturalWidth).map(i=>({src:i.getAttribute('src')?.slice(0,120),line:Number(i.closest('[data-line]')?.getAttribute('data-line'))||undefined})));
  if(broken.length)fail('resource','Image could not be decoded',broken[0].line,broken[0].src);
  return {resourceCount:cache.size,resourceBytes:bytesUsed};
}
function stylesheet(o){
  const dark=!!o.dark;const background=o.background||(dark?'#20212a':'#ffffff');const foreground=o.foreground||(dark?'#e7e7ed':'#242432');
  for(const color of [background,foreground,o.accent||'#527bf0'])if(!/^#[\da-f]{6}$/i.test(color))fail('style','Theme colors must be six-digit hex values');
  const fontSize=Math.max(10,Math.min(28,Number(o.fontSize)||16));const margin=Math.max(8,Math.min(40,Number(o.marginMm)||18));
  return `@page{size:${o.paper==='Letter'?'Letter':'A4'};margin:${margin}mm}*{box-sizing:border-box}html{background:${background};color:${foreground};font-size:${fontSize}px;-webkit-print-color-adjust:exact;print-color-adjust:exact}body{margin:0;overflow-wrap:anywhere;word-break:normal;font-family:'MDcard Diagram Sans','MDcard Diagram Sans KR',Arial,sans-serif;line-height:1.6}main{max-width:100%;padding:0}h1,h2,h3,h4,h5,h6{line-height:1.25;break-after:avoid}h1{font-size:2em}h2{font-size:1.5em}h3{font-size:1.25em}a{color:${o.accent||'#527bf0'}}img,svg{max-width:100%;height:auto}img.formula.inline{max-height:2.8em;width:auto;vertical-align:middle}img.formula.display{display:block;margin:1em auto}figure,.figure,pre,table,img,svg{break-inside:avoid;page-break-inside:avoid}figure{margin:1em 0}table{table-layout:fixed;border-collapse:collapse;width:100%}td,th{border:1px solid ${dark?'#666':'#bbb'};padding:.45em .65em}th{background:${dark?'#34394b':'#eef1f8'}}blockquote{margin:1em 0;padding:.2em 1em;border-left:3px solid ${o.accent||'#527bf0'}}pre{white-space:pre-wrap;overflow-wrap:anywhere;padding:12px 14px;background:${dark?'#292d3a':'#f0f2f8'};border-radius:5px;font:0.84em/1.5 Consolas,'Courier New',monospace}code{font-family:Consolas,'Courier New',monospace}.code-line{display:block;min-height:1.5em}.numberLines code{counter-reset:line}.numberLines .code-line{padding-left:3.5em;position:relative}.numberLines .code-line:before{counter-increment:line;content:counter(line);position:absolute;left:0;color:${dark?'#a4a9b6':'#747b8d'};width:2.5em;text-align:right;user-select:none}.hljs-keyword,.hljs-selector-tag{color:${dark?'#c792ea':'#773bc2'}}.hljs-string,.hljs-title{color:${dark?'#c3e88d':'#287443'}}.hljs-number,.hljs-literal{color:${dark?'#f78c6c':'#a45120'}}.hljs-comment{color:${dark?'#8d98ad':'#6a737d'}}.hljs-built_in,.hljs-type{color:${dark?'#82aaff':'#155eb6'}}.pagebreak{break-before:page;page-break-before:always;height:0}.toc{margin-bottom:2em}.toc ol{list-style:none;padding-left:1.3em}.toc>ol{padding-left:0}.toc a{text-decoration:none}@media screen{body{padding:24px}.pagebreak{border-top:1px dashed #aaa;margin:20px 0}}`;
}
async function decorate(page,options){
  return await page.evaluate(({toc})=>{
    const headings=[...document.querySelectorAll('main h1,main h2,main h3,main h4,main h5,main h6')];
    const entries=[];const used=new Set([...document.querySelectorAll('[id]')].map(e=>e.id));
    const semanticText=el=>{const copy=el.cloneNode(true);copy.querySelectorAll('img').forEach(image=>image.replaceWith(document.createTextNode(image.getAttribute('alt')||'')));return copy.textContent;};
    headings.forEach((el,i)=>{const title=semanticText(el);if(!el.id){const base=title.trim().toLowerCase().replace(/[^\p{L}\p{N}\p{M}\s_-]/gu,'').replace(/\s+/g,'-')||'section';let id=base;let n=1;while(used.has(id))id=base+'-'+n++;el.id=id;used.add(id);}entries.push({level:Number(el.tagName[1]),title,id:el.id});});
    if(toc!==false&&entries.length){const nav=document.createElement('nav');nav.className='toc';nav.setAttribute('aria-label','Table of contents');const top=document.createElement('ol');nav.append(top);const stack=[{level:0,list:top}];for(const entry of entries){while(stack.length>1&&stack.at(-1).level>=entry.level)stack.pop();let parent=stack.at(-1);if(parent.level<entry.level&&parent.list.lastElementChild){const sub=document.createElement('ol');parent.list.lastElementChild.append(sub);stack.push({level:entry.level,list:sub});parent=stack.at(-1);}const li=document.createElement('li');const a=document.createElement('a');a.href='#'+entry.id;a.textContent=entry.title;li.append(a);parent.list.append(li);if(stack.length===1)stack.push({level:entry.level,list:parent.list});}document.querySelector('main').prepend(nav);}
    return entries;
  },options);
}
async function printGuard(page,options){
  await page.emulateMedia({media:'print'});
  const contentHeight=options.printableHeightPx||((options.paper==='Letter'?279.4:297)-2*(Number(options.marginMm)||18))*96/25.4;
  const wide=await page.evaluate(()=>{const root=document.querySelector('main');const left=root.getBoundingClientRect().left;const right=Math.min(root.getBoundingClientRect().right,window.innerWidth);const bad=[root,...root.querySelectorAll('*')].map(el=>{const r=el.getBoundingClientRect();return {tag:el.tagName,left:r.left,right:r.right,width:r.width,height:r.height,scroll:el.scrollWidth,client:el.clientWidth,line:Number(el.closest('[data-line]')?.getAttribute('data-line'))||undefined};}).filter(e=>e.width>0.5&&e.height>0.5&&(e.left<left-3||e.right>right+3||(e.client>0&&e.scroll>e.client+3)));return bad.find(e=>e.line)||bad[0];});
  if(wide)fail('pagination',`${wide.tag} extends beyond the printable width or clips its contents. Allow text wrapping, reduce its size or split the content.`,wide.line);
  const bad=await page.evaluate(height=>[...document.querySelectorAll('main pre,main table,main figure,main img:not(.inline),main svg')].map(el=>({tag:el.tagName,height:el.getBoundingClientRect().height,line:Number(el.closest('[data-line]')?.getAttribute('data-line'))||undefined})).find(e=>e.height>height-4),contentHeight);
  if(bad)fail('pagination',`${bad.tag} is ${Math.round(bad.height)}px high and exceeds the ${Math.floor(contentHeight)}px printable page. Reduce the block, font size or margins.`,bad.line);
}
async function docxFix(filename,headings,options){
  const zip=await JSZip.loadAsync(await readFile(filename));let xml=await zip.file('word/document.xml').async('string');
  const doc=new DOMParser().parseFromString(xml,'application/xml');const ns='http://schemas.openxmlformats.org/wordprocessingml/2006/main';
  const paras=Array.from(doc.getElementsByTagNameNS(ns,'p'));const ids=[];let count=50000;
  for(const p of paras){const style=p.getElementsByTagNameNS(ns,'pStyle')[0]?.getAttribute('w:val')||'';if(/^Heading[1-6]$/i.test(style)){const name='mdcard_toc_'+ids.length;const start=doc.createElementNS(ns,'w:bookmarkStart');start.setAttribute('w:id',String(count));start.setAttribute('w:name',name);const end=doc.createElementNS(ns,'w:bookmarkEnd');end.setAttribute('w:id',String(count++));p.insertBefore(start,p.firstChild?.nextSibling||null);p.appendChild(end);ids.push({name,level:Number(style.slice(-1)),title:headings[ids.length]?.title||Array.from(p.getElementsByTagNameNS(ns,'t')).map(t=>t.textContent).join('')});}}
  // Replace Pandoc's deferred empty TOC field with cached clickable entries + a refreshable field.
  let field=doc.getElementsByTagNameNS(ns,'sdt')[0];if(field?.textContent.includes('TOC'))field.parentNode.removeChild(field);
  const body=doc.getElementsByTagNameNS(ns,'body')[0];
  if(options.toc!==false&&ids.length){const container=doc.createElementNS(ns,'w:sdt');const content=doc.createElementNS(ns,'w:sdtContent');container.appendChild(content);const intro=doc.createElementNS(ns,'w:p');intro.appendChild(doc.createElementNS(ns,'w:r'));intro.firstChild.appendChild(doc.createElementNS(ns,'w:t'));intro.firstChild.firstChild.textContent='Table of contents';content.appendChild(intro);
    const begin=doc.createElementNS(ns,'w:p');begin.appendChild(doc.createElementNS(ns,'w:r'));const b=doc.createElementNS(ns,'w:fldChar');b.setAttribute('w:fldCharType','begin');begin.firstChild.appendChild(b);const r=doc.createElementNS(ns,'w:r');const ins=doc.createElementNS(ns,'w:instrText');ins.setAttribute('xml:space','preserve');ins.textContent=' TOC \\o "1-6" \\h \\z \\u ';r.appendChild(ins);begin.appendChild(r);const sep=doc.createElementNS(ns,'w:r');const c=doc.createElementNS(ns,'w:fldChar');c.setAttribute('w:fldCharType','separate');sep.appendChild(c);begin.appendChild(sep);content.appendChild(begin);
    for(const entry of ids){const p=doc.createElementNS(ns,'w:p');const props=doc.createElementNS(ns,'w:pPr');const style=doc.createElementNS(ns,'w:pStyle');style.setAttribute('w:val','TOC'+entry.level);props.appendChild(style);const ind=doc.createElementNS(ns,'w:ind');ind.setAttribute('w:left',String((entry.level-1)*240));props.appendChild(ind);p.appendChild(props);const a=doc.createElementNS(ns,'w:hyperlink');a.setAttribute('w:anchor',entry.name);const run=doc.createElementNS(ns,'w:r');const t=doc.createElementNS(ns,'w:t');t.textContent=entry.title;run.appendChild(t);a.appendChild(run);p.appendChild(a);content.appendChild(p);}
    const end=doc.createElementNS(ns,'w:p');end.appendChild(doc.createElementNS(ns,'w:r'));const e=doc.createElementNS(ns,'w:fldChar');e.setAttribute('w:fldCharType','end');end.firstChild.appendChild(e);content.appendChild(end);body.insertBefore(container,body.firstChild);
  }
  // Native Word pagination keeps blocks together and honors explicit page markers.
  for(const p of Array.from(doc.getElementsByTagNameNS(ns,'p'))){let props=p.getElementsByTagNameNS(ns,'pPr')[0];if(!props){props=doc.createElementNS(ns,'w:pPr');p.insertBefore(props,p.firstChild);}props.appendChild(doc.createElementNS(ns,'w:keepLines'));const style=props.getElementsByTagNameNS(ns,'pStyle')[0]?.getAttribute('w:val')||'';if(/^Heading/.test(style))props.appendChild(doc.createElementNS(ns,'w:keepNext'));}
  for(const row of Array.from(doc.getElementsByTagNameNS(ns,'tr'))){let props=row.getElementsByTagNameNS(ns,'trPr')[0];if(!props){props=doc.createElementNS(ns,'w:trPr');row.insertBefore(props,row.firstChild);}props.appendChild(doc.createElementNS(ns,'w:cantSplit'));}
  zip.file('word/document.xml',new XMLSerializer().serializeToString(doc));await writeFile(filename,await zip.generateAsync({type:'nodebuffer',compression:'DEFLATE'}));return {tocEntries:ids.length};
}
async function pandocExport(page,filename,format,options,work,headings){
  const pandoc=options.pandocPath||process.env.MDCARD_PANDOC||'pandoc';
  const version=await run(pandoc,['--version'],{timeout:10000,signal:options.signal});
  if(format==='docx'){
    let printOptions=options;
    if(options.referenceDocx){const reference=await JSZip.loadAsync(await readFile(path.resolve(options.referenceDocx)));const xml=await reference.file('word/document.xml').async('string');const ref=new DOMParser().parseFromString(xml,'application/xml');const ns='http://schemas.openxmlformats.org/wordprocessingml/2006/main';const size=Array.from(ref.getElementsByTagNameNS(ns,'pgSz')).at(-1);const margin=Array.from(ref.getElementsByTagNameNS(ns,'pgMar')).at(-1);if(size&&margin){const h=Number(size.getAttribute('w:h'));const w=Number(size.getAttribute('w:w'));const t=Number(margin.getAttribute('w:top'));const b=Number(margin.getAttribute('w:bottom'));const left=Number(margin.getAttribute('w:left'));const right=Number(margin.getAttribute('w:right'));printOptions={...options,printableHeightPx:(h-t-b)/15};await page.setViewportSize({width:Math.floor((w-left-right)/15),height:900});}}
    await printGuard(page,printOptions);
  }
  // Materialize every data URI before Pandoc so no converter performs a network request.
  let mediaIndex=0;
  for(const image of await page.locator('img').all()){
    if(format==='docx'){const bounds=await image.boundingBox();if(bounds)await image.evaluate((el,size)=>{el.setAttribute('width',String(size.width));el.setAttribute('height',String(size.height));},bounds);}
    const uri=await image.getAttribute('src');const match=uri.match(/^data:([^;]+);base64,(.*)$/s);if(!match)fail('pandoc','Unresolved image before Pandoc');
    let bytes=Buffer.from(match[2],'base64');let ext=match[1]==='image/svg+xml'?'svg':match[1].split('/')[1];
    // Word supports SVG in recent versions, but Pandoc requires raster fallback without a converter.
    // Embed SVG plus a high-resolution PNG fallback later, preserving vector source in the package.
    if(format==='docx'&&ext==='svg'){const png=await image.screenshot({type:'png',omitBackground:true,animations:'disabled'});const svgPath=path.join(work,`vector-${mediaIndex}.svg`);await writeFile(svgPath,bytes);bytes=png;ext='png';}
    const local=path.join(work,`media-${mediaIndex++}.${ext}`);await writeFile(local,bytes);await image.evaluate((el,url)=>el.setAttribute('src',url),local);
  }
  await page.locator('.toc').evaluateAll(els=>els.forEach(e=>e.remove()));
  await page.locator('a[id]').evaluateAll(els=>els.forEach(el=>{const span=document.createElement('span');span.id=el.id;el.removeAttribute('id');el.replaceWith(span);span.append(el);}));
  // Pandoc uses native CodeBlock nodes for its own highlighter; CSS counters never enter the text.
  await page.locator('pre').evaluateAll(els=>els.forEach(el=>{const lang=el.querySelector('code')?.className.match(/language-([^ ]+)/)?.[1]||'';const numbered=el.classList.contains('numberLines');const lines=[...el.querySelectorAll('.code-line')];const text=lines.length?lines.map(l=>l.textContent).join('\n'):el.textContent;el.className=lang+(numbered?' numberLines':'');el.innerHTML='';const code=document.createElement('code');code.className='language-'+lang;code.textContent=text+'\n';el.appendChild(code);}));
  const html=await page.locator('main').innerHTML();const input=path.join(work,'compiled.html');await writeFile(input,html);
  const ast=JSON.parse(await run(pandoc,['--from=html','--to=json',input],{timeout:30000,signal:options.signal}));
  function walk(value){if(!value||typeof value!=='object')return;if(value.t==='Div'&&value.c?.[0]?.[1]?.includes('pagebreak')){value.t='RawBlock';value.c=format==='docx'?['openxml','<w:p><w:r><w:br w:type="page"/></w:r></w:p>']:['html','<div style="break-before:page;page-break-before:always"></div>'];}if(Array.isArray(value))value.forEach(walk);else Object.values(value).forEach(walk);}
  walk(ast);const astPath=path.join(work,'document.json');await writeFile(astPath,JSON.stringify(ast));
  const args=['--from=json',astPath,'--standalone','--toc','--toc-depth=6','--output='+filename,'--metadata=title:'+(options.title||'MDcard document'),'--metadata=lang:'+(options.language||'en'),'--metadata=author:'+(options.author||''),'--metadata=identifier:'+(options.identifier||'urn:uuid:'+randomUUID())];
  // Pandoc emits its own token classes. Supply a theme, rather than expecting
  // highlight.js selectors in the document CSS to style EPUB/Word code blocks.
  const highlight=JSON.parse(await run(pandoc,['--print-highlight-style=pygments'],{timeout:10000,signal:options.signal}));const dark=!!options.dark;const foreground=options.foreground||(dark?'#e7e7ed':'#242432');const codeBg=dark?'#292d3a':'#f0f2f8';
  highlight['text-color']=foreground;highlight['background-color']=codeBg;highlight['line-number-color']=dark?'#a4a9b6':'#747b8d';highlight['line-number-background-color']=codeBg;
  const tokenColors={Keyword:dark?'#c792ea':'#773bc2',ControlFlow:dark?'#c792ea':'#773bc2',String:dark?'#c3e88d':'#287443',VerbatimString:dark?'#c3e88d':'#287443',SpecialString:dark?'#c3e88d':'#287443',Char:dark?'#c3e88d':'#287443',Function:dark?'#c3e88d':'#287443',DataType:dark?'#82aaff':'#155eb6',BuiltIn:dark?'#82aaff':'#155eb6',DecVal:dark?'#f78c6c':'#a45120',BaseN:dark?'#f78c6c':'#a45120',Float:dark?'#f78c6c':'#a45120',Comment:dark?'#8d98ad':'#6a737d',CommentVar:dark?'#8d98ad':'#6a737d',Documentation:dark?'#8d98ad':'#6a737d'};
  for(const [name,style]of Object.entries(highlight['text-styles'])){style['text-color']=tokenColors[name]||foreground;style['background-color']=null;}
  const highlightPath=path.join(work,'code.theme');await writeFile(highlightPath,JSON.stringify(highlight));args.push('--highlight-style='+highlightPath);
  if(format==='docx'&&options.referenceDocx)args.push('--reference-doc='+path.resolve(options.referenceDocx));
  if(format==='epub'){const cssPath=path.join(work,'theme.css');let fontCss=options.fontCss||'';let fontIndex=0;for(const match of [...fontCss.matchAll(/url\(data:font\/woff2;base64,([a-z\d+/=]+)\)/gi)]){const name=`mdcard-${fontIndex++}.woff2`;const filename=path.join(work,name);await writeFile(filename,Buffer.from(match[1],'base64'));args.push('--epub-embed-font='+filename);fontCss=fontCss.replace(match[0],`url('../fonts/${name}')`);}await writeFile(cssPath,stylesheet(options)+fontCss+String(options.customCss||''));args.push('--css='+cssPath,'--split-level=1','--epub-title-page=false');}
  await run(pandoc,args,{timeout:120000,signal:options.signal});
  if(format==='docx'){
    const result=await docxFix(filename,headings,options);
    // Add original vector sources as OPC media with PNG fallback relationships.
    const styles=await finalizeDocx(filename,options,work,mediaIndex,ROOT);return {pandocVersion:version.split('\n')[0],...result,...styles};
  }
  return {pandocVersion:version.split('\n')[0]};
}

export async function exportDocument(request,onProgress=()=>{}){
  let {source,output,format}=request;const o={...request.options};
  if(typeof source!=='string'||Buffer.byteLength(source)>16*1024*1024)fail('request','Markdown must be UTF-8 text up to 16 MiB');
  if(!['html','pdf','png','jpeg','docx','epub','marp-html','marp-pdf','marp-pptx'].includes(format))fail('request','Unknown export format');
  if(!output||typeof output!=='string')fail('request','Output path is required');
  const destination=path.resolve(output);await mkdir(path.dirname(destination),{recursive:true});
  const canonical=async p=>{try{return await realpath(p);}catch{return path.resolve(p);}};
  if(request.sourcePath&&(await canonical(request.sourcePath)).toLowerCase()===(await canonical(destination)).toLowerCase())fail('request','Export destination cannot overwrite the source document');
  if(request.workDir&&(path.basename(request.workDir)!=='render'||!path.basename(path.dirname(request.workDir)).startsWith('.mdcard-task-')||path.dirname(path.dirname(path.resolve(request.workDir)))!==path.dirname(destination)))fail('request','Invalid private export workspace');
  const work=request.workDir?path.resolve(request.workDir):await mkdtemp(path.join(path.dirname(destination),'.mdcard-export-'));await mkdir(work,{recursive:true});const temp=path.join(work,'output.'+({jpeg:'jpg','marp-html':'html','marp-pdf':'pdf','marp-pptx':'pptx'}[format]||format));
  const started=Date.now();let browser;let timeout;let cancelled=false;
  const deadline=Number(request.timeoutMs)||180000;
  const controller=new AbortController();o.signal=controller.signal;
  timeout=setTimeout(()=>{cancelled=true;controller.abort(new ExportError('timeout','Export exceeded its task deadline'));richClose().catch(()=>{});},deadline);
  const guard=()=>{if(cancelled||Date.now()-started>deadline)fail('timeout','Export exceeded its task deadline');};
  const progress=event=>{guard();onProgress({type:'progress',...event,elapsedMs:Date.now()-started});};
  try{
    source=source.replace(/^\uFEFF/,'');const front=source.match(/^---\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)(?:\r?\n|$)/);if(front){try{const metadata=YAML.parse(front[1],{maxAliasCount:0})||{};for(const key of ['title','author','identifier'])if(!o[key]&&typeof metadata[key]==='string')o[key]=metadata[key];if(!o.language&&typeof metadata.lang==='string')o.language=metadata.lang;}catch(e){fail('parse','Invalid front matter: '+e.message,1);}}
    if(!o.pandocPath){o.pandocPath=process.env.MDCARD_PANDOC;for(const candidate of [path.join(ROOT,'runtime/pandoc',process.platform==='win32'?'pandoc.exe':'pandoc'),path.join(ROOT,'runtime',process.platform==='win32'?'windows':'linux','pandoc',process.platform==='win32'?'pandoc.exe':'pandoc')]){if(o.pandocPath)break;try{await access(candidate);o.pandocPath=candidate;}catch{}}}
    progress({stage:'parse',progress:2});
    const executablePath=await bundledBrowser();
    // Fail quickly and guide configuration before costly preprocessing.
    if(format==='docx'||format==='epub'||o.pipeline==='pandoc')await run(o.pandocPath||process.env.MDCARD_PANDOC||'pandoc',['--version'],{timeout:10000,signal:o.signal});
    const html=await compiledMarkdown(source,o,progress);
    await richClose();
    browser=await chromium.launch({executablePath,headless:true,chromiumSandbox:process.platform==='win32'||process.getuid?.()!==0,args:['--disable-background-networking','--disable-component-update','--disable-sync','--no-first-run','--disable-extensions']});
    o.signal.addEventListener('abort',()=>browser?.close().catch(()=>{}),{once:true});
    const paperWidth=((o.paper==='Letter'?215.9:210)-2*(Number(o.marginMm)||18))*96/25.4;
    const context=await browser.newContext({viewport:{width:Math.round(paperWidth),height:900},deviceScaleFactor:Math.max(1,Math.min(4,Number(o.scale)||2)),serviceWorkers:'block'});
    await context.route('**/*',route=>route.abort('blockedbyclient'));await context.routeWebSocket('**/*',ws=>ws.close());
    const page=await context.newPage();
    const fonts=await portableFontCss(source+'\n'+[o.title,o.author,o.identifier].filter(Boolean).join('\n'));o.fontCss=fonts;
    const css=stylesheet(o)+fonts+String(o.customCss||'');
    // CSP disables all source scripts, handlers, forms and network requests from the first parse.
    const csp="default-src 'none'; img-src data:; font-src data:; style-src 'unsafe-inline'; script-src 'none'; connect-src 'none'; base-uri 'none'; form-action 'none'";
    await page.setContent(`<!doctype html><html lang="${esc(o.language||'en')}"><head><meta charset="UTF-8"><meta http-equiv="Content-Security-Policy" content="${csp}"><title>${esc(o.title||'MDcard document')}</title><style>${css}</style></head><body><main>${html}</main></body></html>`,{waitUntil:'domcontentloaded'});
    const resourceResult=await resources(page,request.baseDir||process.cwd(),progress,o.signal);const headings=await decorate(page,o);progress({stage:'layout',progress:60});
    let details={};
    if(format.startsWith('marp-')){
      // Precompiled resources/math/diagrams are injected into Marp's actual parser output.
      // A DOM-to-Markdown rewrite retains slide separators/directives by replacing rich tokens in source.
      const marp=new Marp({html:true,math:false,highlightCode:true,inlineSVG:false});
      const validateSlideLink=marp.markdown.validateLink;marp.markdown.validateLink=uri=>/^data:image\/svg\+xml(?:;[^,]*)?,/i.test(uri)||validateSlideLink(uri);
      // Core itself handles front matter, directives, pagination, theme and slide separation.
      let slideSource=source;const md=new MarkdownIt().use(mathRules);const ts=md.parse(source,{});const replacements=[];
      const collect=items=>{for(const t of items){if(t.type==='fence'&&/^(mermaid|plantuml|puml|flow|flowchart|math|latex)\b/.test(t.info)){const from=source.split(/\n/).slice(0,t.map[0]).join('\n').length+(t.map[0]?1:0);const to=source.split(/\n/).slice(0,t.map[1]).join('\n').length+(t.map[1]<source.split(/\n/).length?1:0);replacements.push({from,to,line:t.map[0]+1});}if(t.children)collect(t.children);}};collect(ts);
      for(const r of replacements.sort((a,b)=>b.from-a.from)){const image=await page.locator(`img[data-line="${r.line}"]`).first().getAttribute('src');slideSource=slideSource.slice(0,r.from)+`\n<img src="${image}">\n`+slideSource.slice(r.to);}
      // Compile inline/block formulas while preserving code fences via Marp's markdown-it rules.
      marp.markdown.use(mathRules);marp.markdown.renderer.rules.export_math=(ts,i)=>`<img class="formula inline" src="data:image/svg+xml;base64,${Buffer.from(mathSvg(ts[i].content,false,undefined,o)).toString('base64')}">`;
      marp.markdown.renderer.rules.export_math_block=(ts,i)=>`<img class="formula" src="data:image/svg+xml;base64,${Buffer.from(mathSvg(ts[i].content,true,undefined,o)).toString('base64')}">`;
      marp.markdown.renderer.rules.fence=(ts,i)=>{const t=ts[i];const language=(t.info||'').trim().split(/\s+/)[0]||'plaintext';const lines=highlightedLines(t.content,language);return `<pre class="${o.lineNumbers!==false?'numberLines':''}"><code class="language-${esc(language)}">${lines}</code></pre>`;};marp.markdown.renderer.rules.code_block=marp.markdown.renderer.rules.fence;
      const deck=marp.render(slideSource);deck.html=deck.html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi,'');const dimensions=deck.css.match(/@page\s*\{size:(\d+)px (\d+)px/);const slideWidth=Number(dimensions?.[1])||1280,slideHeight=Number(dimensions?.[2])||720;
      const commonCss=fonts+stylesheet(o).replace(/@page\{[^}]*\}/,'')+`section,section :is(h1,h2,h3,h4,h5,h6,p,li,td,th,blockquote){font-family:'MDcard Diagram Sans','MDcard Diagram Sans KR',sans-serif}`+String(o.customCss||'')+`\n@page{size:${slideWidth}px ${slideHeight}px;margin:0}section{break-after:page;break-inside:avoid;width:${slideWidth}px;height:${slideHeight}px;overflow:hidden}`;
      await page.setViewportSize({width:slideWidth,height:slideHeight});
      await page.setContent(`<!doctype html><html><head><meta charset="UTF-8"><meta http-equiv="Content-Security-Policy" content="${csp}"><title>${esc(o.title||'MDcard slides')}</title><style>${deck.css}\n${commonCss}\nbody{padding:0;margin:0}section{font-family:'MDcard Diagram Sans','MDcard Diagram Sans KR',sans-serif}section pre{white-space:pre-wrap}</style></head><body>${deck.html}</body></html>`);
      await resources(page,request.baseDir||process.cwd(),progress,o.signal);
      details.slides=await page.locator('section').count();if(!details.slides)fail('marp','Marp did not produce slides');
      const overflow=await page.locator('section').evaluateAll(els=>els.map((el,i)=>({slide:i+1,scroll:el.scrollHeight,height:el.clientHeight})).find(x=>x.scroll>x.height+2));if(overflow)fail('pagination','Slide content exceeds slide height; split this slide.',overflow.slide);
      if(format==='marp-html')await writeFile(temp,await page.content());
      else if(format==='marp-pdf')await page.pdf({path:temp,printBackground:true,preferCSSPageSize:true,outline:true,tagged:true});
      else {
        // Marp CLI renders the same precompiled HTML fragments using the bundled browser.
        const compiledDeck=path.join(work,'slides.md');
        const sections=await page.locator('section').evaluateAll(els=>els.map(el=>el.innerHTML));
        await writeFile(compiledDeck,'---\nmarp: true\n---\n<style>'+deck.css+'\n'+commonCss+'</style>\n'+sections.join('\n\n---\n\n'));
        await run(process.execPath,[path.join(ROOT,'node_modules/@marp-team/marp-cli/marp-cli.js'),compiledDeck,'--html','--pptx','--allow-local-files','--browser-path',executablePath,'--output',temp],{cwd:work,timeout:120000,signal:o.signal,env:{CHROME_PATH:executablePath,CHROME_DISABLE_SANDBOX:process.getuid?.()===0?'1':''}});
      }
    } else if(format==='docx'||format==='epub')details=await pandocExport(page,temp,format,o,work,headings);
    else if(format==='html')await writeFile(temp,await page.content());
    else if(format==='pdf'){
      await printGuard(page,o);progress({stage:'write',progress:80});await page.pdf({path:temp,printBackground:true,preferCSSPageSize:true,outline:true,tagged:true});
    }else{
      await page.emulateMedia({media:'screen'});const scale=Math.max(1,Math.min(4,Number(o.scale)||2));const size=await page.evaluate(()=>({width:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight}));
      const width=Math.ceil(size.width*scale),height=Math.ceil(size.height*scale);if(width*height>100e6||height>60000)fail('image','Long image exceeds 100 megapixels or 60000px height. Reduce scale or split the document.');
      const cdp=await context.newCDPSession(page);const tiles=[];for(let y=0;y<size.height;y+=2048){const h=Math.min(2048,size.height-y);const captured=await cdp.send('Page.captureScreenshot',{format:'png',fromSurface:true,captureBeyondViewport:true,clip:{x:0,y,width:size.width,height:h,scale}});const input=Buffer.from(captured.data,'base64');const actual=await sharp(input).metadata();if(actual.width!==Math.ceil(size.width*scale)||actual.height!==Math.ceil(h*scale))fail('capture',`Raster scale mismatch: expected ${Math.ceil(size.width*scale)}x${Math.ceil(h*scale)}, received ${actual.width}x${actual.height}`);tiles.push({input,top:Math.round(y*scale),left:0});progress({stage:'capture',progress:60+Math.round((y+h)/size.height*25)});}
      let image=sharp({create:{width,height,channels:4,background:o.background||(o.dark?'#20212a':'#ffffff')},limitInputPixels:100e6}).composite(tiles);image=format==='jpeg'?image.flatten({background:o.background||(o.dark?'#20212a':'#ffffff')}).jpeg({quality:95,chromaSubsampling:'4:4:4'}):image.png();await image.toFile(temp);details={width,height,tiles:tiles.length};
    }
    if(cancelled)fail('timeout','Export exceeded its task deadline');
    progress({stage:'verify',progress:94});const outputBytes=(await stat(temp)).size;if(!outputBytes)fail('verify','Converter produced an empty output');
    const bytes=await readFile(temp);
    if(format.endsWith('pdf')&&!bytes.subarray(0,5).equals(Buffer.from('%PDF-')))fail('verify','Invalid PDF signature');
    if(['docx','epub','marp-pptx'].includes(format)){const zip=await JSZip.loadAsync(bytes);const entry={docx:'word/document.xml',epub:'META-INF/container.xml','marp-pptx':'ppt/presentation.xml'}[format];if(!zip.file(entry))fail('verify','Incomplete container: '+entry);}
    // No destination replacement until every rendering and validation step succeeds.
    const f=await open(temp,'r+');try{await f.sync();}finally{await f.close();}guard();
    // Rust can distinguish cancellation before publication from the narrow
    // rename/acknowledgement window, even if this process is killed in between.
    if(request.workDir)await writeFile(path.join(path.dirname(work),'publication-started'),'publication starting\n');
    guard();await rename(temp,destination);
    // Publication is the commit point. A late abort cannot turn a published
    // output into a false "cancelled, previous output preserved" result.
    onProgress({type:'progress',stage:'complete',progress:100,elapsedMs:Date.now()-started});return {type:'complete',ok:true,output:destination,format,bytes:outputBytes,sha256:createHash('sha256').update(bytes).digest('hex'),headings,...resourceResult,...details,elapsedMs:Date.now()-started};
  }catch(e){if(o.signal.aborted&&e.stage!=='timeout'&&e.stage!=='cleanup')fail('timeout','Export exceeded its task deadline',e.line,e.resource);throw e;}finally{clearTimeout(timeout);await browser?.close().catch(()=>{});await richClose().catch(()=>{});await rm(work,{recursive:true,force:true});}
}

if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  // JSON request path avoids shell quoting, command injection and stdout ambiguity.
  const requestPath=process.argv[2];let request;
  const emit=e=>process.stdout.write(JSON.stringify(e)+'\n');
  try{request=JSON.parse(await readFile(requestPath,'utf8'));const result=await exportDocument(request,emit);emit(result);}
  catch(e){emit({type:'error',ok:false,stage:e.stage||'export',message:e.message,line:e.line,resource:e.resource});process.exitCode=1;}
}
