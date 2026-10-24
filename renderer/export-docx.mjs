// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {createHash,randomUUID} from 'node:crypto';
import JSZip from 'jszip';
import {DOMParser,XMLSerializer} from '@xmldom/xmldom';
import {portableFontSfnt} from './export-fonts.mjs';
const W='http://schemas.openxmlformats.org/wordprocessingml/2006/main',R='http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const parse=s=>new DOMParser().parseFromString(s,'application/xml');const serialize=d=>new XMLSerializer().serializeToString(d);
const all=(d,name)=>Array.from(d.getElementsByTagNameNS(W,name));
function node(d,name,attrs={}){const n=d.createElementNS(W,'w:'+name);for(const[k,v]of Object.entries(attrs))n.setAttribute('w:'+k,String(v));return n;}
function prop(d,root,name,attrs={}){let n=Array.from(root.childNodes).find(x=>x.localName===name);if(!n){n=node(d,name);root.appendChild(n);}for(const[k,v]of Object.entries(attrs)){if(name==='rFonts')n.removeAttribute('w:'+k+'Theme');if(name==='color'&&k==='val')for(const theme of ['themeColor','themeTint','themeShade'])n.removeAttribute('w:'+theme);n.setAttribute('w:'+k,String(v));}return n;}
// Word ignores a font part without its measured signature, even when rFonts names it.
// Read the portable SFNT's OS/2 metadata before obfuscation (ECMA-376 font table).
function describeFont(d,font,bytes){
 const tables=new Map();for(let i=0;i<bytes.readUInt16BE(4);i++){const p=12+i*16;tables.set(bytes.toString('ascii',p,p+4),bytes.readUInt32BE(p+8));}
 const os=tables.get('OS/2');if(os===undefined)throw new Error('Embedded font has no OS/2 metadata');
 const hex=n=>(n>>>0).toString(16).padStart(8,'0').toUpperCase();const cp=bytes.readUInt16BE(os)>=1?bytes.readUInt32BE(os+78):0;
 font.appendChild(node(d,'panose1',{val:bytes.subarray(os+32,os+42).toString('hex').toUpperCase()}));
 font.appendChild(node(d,'charset',{val:cp&(1<<17)?'80':cp&(1<<18)?'86':cp&(1<<19)?'81':cp&(1<<20)?'88':'00'}));
 font.appendChild(node(d,'family',{val:'swiss'}));font.appendChild(node(d,'pitch',{val:'variable'}));
 font.appendChild(node(d,'sig',{usb0:hex(bytes.readUInt32BE(os+42)),usb1:hex(bytes.readUInt32BE(os+46)),usb2:hex(bytes.readUInt32BE(os+50)),usb3:hex(bytes.readUInt32BE(os+54)),csb0:hex(cp&~0x20000000),csb1:'00000000'}));
}
function hash(b){return createHash('sha256').update(b).digest('hex');}
export async function finalizeDocx(filename,options,work,mediaCount,root){
 const zip=await JSZip.loadAsync(await readFile(filename));const doc=parse(await zip.file('word/document.xml').async('string'));
 const styles=parse(await zip.file('word/styles.xml').async('string'));const dark=!!options.dark;const color=(options.foreground||(dark?'#e7e7ed':'#242432')).slice(1);const background=(options.background||(dark?'#20212a':'#ffffff')).slice(1);const accent=(options.accent||'#527bf0').slice(1);const codeBg=dark?'292D3A':'F0F2F8';
 const palette={KeywordTok:dark?'C792EA':'773BC2',ControlFlowTok:dark?'C792EA':'773BC2',StringTok:dark?'C3E88D':'287443',CharTok:dark?'C3E88D':'287443',DecValTok:dark?'F78C6C':'A45120',FloatTok:dark?'F78C6C':'A45120',CommentTok:dark?'8D98AD':'6A737D',DataTypeTok:dark?'82AAFF':'155EB6',BuiltInTok:dark?'82AAFF':'155EB6',NormalTok:color};
 const nativeFonts={ascii:'MDcard Diagram Sans',hAnsi:'MDcard Diagram Sans',eastAsia:'MDcard Diagram Sans',cs:'MDcard Diagram Sans'};
 for(const style of all(styles,'style')){
  const id=style.getAttribute('w:styleId');const rPr=prop(styles,style,'rPr');
  if(!options.referenceDocx){
   prop(styles,rPr,'rFonts',/Tok$|^(SourceCode|VerbatimChar)$/.test(id)?{...nativeFonts,ascii:'Consolas',hAnsi:'Consolas'}:nativeFonts);
   const isAccent=/^(Heading[1-6]|TOCHeading|Hyperlink|FollowedHyperlink)$/.test(id);
   prop(styles,rPr,'color',{val:isAccent?accent:color});
   if(['Normal','BodyText','FirstParagraph','Compact'].includes(id))prop(styles,rPr,'sz',{val:Math.round((Number(options.fontSize)||16)*1.5)});
  }
  if(palette[id])prop(styles,rPr,'color',{val:palette[id]});
  if(id==='SourceCode'){
   prop(styles,rPr,'color',{val:color});prop(styles,rPr,'sz',{val:Math.round((Number(options.fontSize)||16)*1.25)});
   prop(styles,rPr,'rFonts',{ascii:'Consolas',hAnsi:'Consolas'});
   const pp=prop(styles,style,'pPr');prop(styles,pp,'shd',{val:'clear',fill:codeBg});
   if(!options.referenceDocx)prop(styles,pp,'spacing',{before:80,after:120});
  }
 }
 if(!options.referenceDocx){const defaults=all(styles,'docDefaults')[0];if(defaults){const rDefault=prop(styles,defaults,'rPrDefault');const rPr=prop(styles,rDefault,'rPr');prop(styles,rPr,'color',{val:color});prop(styles,rPr,'sz',{val:Math.round((Number(options.fontSize)||16)*1.5)});prop(styles,rPr,'rFonts',nativeFonts);}let bg=all(doc,'background')[0];if(!bg){bg=node(doc,'background');doc.documentElement.insertBefore(bg,doc.documentElement.firstChild);}bg.setAttribute('w:color',background);
  const sect=all(doc,'sectPr').at(-1);if(sect){const letter=options.paper==='Letter';prop(doc,sect,'pgSz',{w:letter?12240:11906,h:letter?15840:16838});const margin=Math.round((Number(options.marginMm)||18)/25.4*1440);prop(doc,sect,'pgMar',{top:margin,bottom:margin,left:margin,right:margin,header:Math.min(margin,420),footer:Math.min(margin,420),gutter:0});}}
 if(dark&&!options.referenceDocx){
  // Word's page background is not printed under its default global settings.
  // A document-owned header drawing prints on each page without changing Word.
  const sect=all(doc,'sectPr').at(-1);const size=all(sect,'pgSz')[0];const width=Number(size.getAttribute('w:w'))/20,height=Number(size.getAttribute('w:h'))/20;
  const backgroundHeader=`<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr xmlns:w="${W}" xmlns:r="${R}" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word"><w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr><w:r><w:pict><v:rect id="MDcardPrintedPageBackground" o:allowincell="f" fillcolor="#${background}" stroked="f" style="position:absolute;left:0;top:0;width:${width}pt;height:${height}pt;z-index:-251654144;mso-position-horizontal-relative:page;mso-position-vertical-relative:page"><w10:wrap anchorx="page" anchory="page"/></v:rect></w:pict></w:r></w:p></w:hdr>`;
  zip.file('word/headerMDcardBackground.xml',backgroundHeader);const ref=node(doc,'headerReference',{type:'default'});ref.setAttributeNS(R,'r:id','rIdMDcardBackground');sect.insertBefore(ref,sect.firstChild);
  for(const row of all(doc,'tr')){const header=all(row,'tblHeader').length>0;for(const cell of all(row,'tc'))prop(doc,prop(doc,cell,'tcPr'),'shd',{val:'clear',fill:header?'34394B':background});}
 }
 for(const p of all(doc,'p')){const style=all(p,'pStyle')[0]?.getAttribute('w:val')||'';if(style==='SourceCode'){
   const pp=prop(doc,p,'pPr');prop(doc,pp,'shd',{val:'clear',fill:codeBg});prop(doc,pp,'keepLines');
   if(options.lineNumbers!==false){let number=1;const addBefore=before=>{const run=node(doc,'r');const rp=node(doc,'rPr');rp.appendChild(node(doc,'color',{val:dark?'A4A9B6':'747B8D'}));run.appendChild(rp);const text=node(doc,'t');text.setAttribute('xml:space','preserve');text.textContent=String(number++).padStart(3,' ')+'  ';run.appendChild(text);p.insertBefore(run,before);};addBefore(pp.nextSibling);for(const br of all(p,'br')){let run=br;while(run.parentNode&&run.parentNode!==p)run=run.parentNode;addBefore(run.nextSibling);}}
 }}
 for(const table of all(doc,'tbl')){const rows=all(table,'tr');for(let i=0;i<rows.length;i++){const paras=all(rows[i],'p');for(let j=0;j<paras.length;j++){const pp=prop(doc,paras[j],'pPr');prop(doc,pp,'keepLines');if(i<rows.length-1||j<paras.length-1)prop(doc,pp,'keepNext');}}}
 const relPath='word/_rels/document.xml.rels';const rels=parse(await zip.file(relPath).async('string'));const vectorByPng=new Map();for(let i=0;i<mediaCount;i++){try{const svg=await readFile(path.join(work,`vector-${i}.svg`));const png=await readFile(path.join(work,`media-${i}.png`));vectorByPng.set(hash(png),{svg,index:i});}catch{}}
 if(dark&&!options.referenceDocx){const r=rels.createElementNS('http://schemas.openxmlformats.org/package/2006/relationships','Relationship');r.setAttribute('Id','rIdMDcardBackground');r.setAttribute('Type',R+'/header');r.setAttribute('Target','headerMDcardBackground.xml');rels.documentElement.appendChild(r);}
 const mapped=new Map();for(const rel of Array.from(rels.documentElement.childNodes)){if(rel.nodeType!==1||!rel.getAttribute('Type').endsWith('/image'))continue;const file=zip.file(path.posix.normalize('word/'+rel.getAttribute('Target')));if(!file)continue;const vector=vectorByPng.get(hash(await file.async('nodebuffer')));if(!vector)continue;const id='rIdMDcardSvg'+vector.index;zip.file('word/media/mdcard-vector-'+vector.index+'.svg',vector.svg);const r=rels.createElementNS('http://schemas.openxmlformats.org/package/2006/relationships','Relationship');r.setAttribute('Id',id);r.setAttribute('Type',R+'/image');r.setAttribute('Target','media/mdcard-vector-'+vector.index+'.svg');rels.documentElement.appendChild(r);mapped.set(rel.getAttribute('Id'),id);}
 for(const blip of Array.from(doc.getElementsByTagNameNS('http://schemas.openxmlformats.org/drawingml/2006/main','blip'))){const id=mapped.get(blip.getAttribute('r:embed'));if(!id)continue;const lst=doc.createElementNS(blip.namespaceURI,'a:extLst');const ext=doc.createElementNS(blip.namespaceURI,'a:ext');ext.setAttribute('uri','{96DAC541-7B7A-43D3-8B79-37D633B846F1}');const svg=doc.createElementNS('http://schemas.microsoft.com/office/drawing/2016/SVG/main','asvg:svgBlip');svg.setAttributeNS(R,'r:embed',id);ext.appendChild(svg);lst.appendChild(ext);blip.appendChild(lst);}
 if(!options.referenceDocx){const text=all(doc,'t').map(t=>t.textContent).join('');const korean=/[\uac00-\ud7af\u1100-\u11ff\u3130-\u318f]/.test(text);const fontTable=parse(await zip.file('word/fontTable.xml').async('string'));const relationships=[];
  for(let i=0;i<(korean?2:1);i++){const fontName=i?'MDcard Diagram Sans KR':'MDcard Diagram Sans';const key=randomUUID().toUpperCase();const raw=Buffer.from(await portableFontSfnt(text,!!i));const font=node(fontTable,'font',{name:fontName});describeFont(fontTable,font,raw);const guid=Buffer.from(key.replaceAll('-',''),'hex').reverse();for(let j=0;j<32;j++)raw[j]^=guid[j%16];const filename=`mdcard-${i}.odttf`;zip.file('word/fonts/'+filename,raw);const embed=node(fontTable,'embedRegular',{fontKey:'{'+key+'}',subsetted:'1'});embed.setAttributeNS(R,'r:id','rIdMDcardFont'+i);font.appendChild(embed);fontTable.documentElement.appendChild(font);relationships.push(`<Relationship Id="rIdMDcardFont${i}" Type="${R}/font" Target="fonts/${filename}"/>`);}
  if(korean)for(const run of all(doc,'r')){
   // One Word run has only one eastAsia font. Isolate Hangul instead of assigning
   // KR to an entire mixed Chinese/Korean sentence (KR lacks some simplified Han).
   if(!/[\uac00-\ud7af\u1100-\u11ff\u3130-\u318f]/.test(all(run,'t').map(t=>t.textContent).join('')))continue;
   const rp=Array.from(run.childNodes).find(n=>n.localName==='rPr');
   for(const child of Array.from(run.childNodes)){
    if(child===rp)continue;
    const pieces=child.localName==='t'?child.textContent.split(/([\uac00-\ud7af\u1100-\u11ff\u3130-\u318f]+)/u).filter(Boolean):[null];
    for(const piece of pieces){const part=run.cloneNode(false);if(rp)part.appendChild(rp.cloneNode(true));const content=child.cloneNode(true);if(piece!==null){content.textContent=piece;prop(doc,prop(doc,part,'rPr'),'rFonts',{eastAsia:/^[\uac00-\ud7af\u1100-\u11ff\u3130-\u318f]/u.test(piece)?'MDcard Diagram Sans KR':'MDcard Diagram Sans'});}part.appendChild(content);run.parentNode.insertBefore(part,run);}
   }
   run.parentNode.removeChild(run);
  }
  const settings=parse(await zip.file('word/settings.xml').async('string'));prop(settings,settings.documentElement,'embedTrueTypeFonts');prop(settings,settings.documentElement,'saveSubsetFonts');zip.file('word/settings.xml',serialize(settings));
  zip.file('word/fontTable.xml',serialize(fontTable));zip.file('word/_rels/fontTable.xml.rels',`<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${relationships.join('')}</Relationships>`);
 }
 let types=await zip.file('[Content_Types].xml').async('string');for(const[ext,mime]of[['svg','image/svg+xml'],['odttf','application/vnd.openxmlformats-officedocument.obfuscatedFont']])if(!types.includes(`Extension="${ext}"`))types=types.replace('</Types>',`<Default Extension="${ext}" ContentType="${mime}"/></Types>`);
 if(dark&&!options.referenceDocx)types=types.replace('</Types>','<Override PartName="/word/headerMDcardBackground.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>');
 zip.file('[Content_Types].xml',types);zip.file('word/styles.xml',serialize(styles));zip.file('word/document.xml',serialize(doc));zip.file(relPath,serialize(rels));await writeFile(filename,await zip.generateAsync({type:'nodebuffer',compression:'DEFLATE'}));return{vectorAlternates:mapped.size,embeddedFont:!options.referenceDocx};
}
