// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import katex from 'katex';
import 'katex/contrib/mhchem';
import mermaid from 'mermaid';
import DOMPurify from 'dompurify';
import flowchart from 'flowchart.js';

const forbidden=['script','foreignObject','iframe','object','embed','image','use','a','audio','video','animate','animateTransform','animateMotion','set','discard'];
const presentation=['fill','stroke','stroke-width','font-size','font-family','font-weight','font-style','opacity','fill-opacity','stroke-opacity','stroke-dasharray','stroke-linecap','stroke-linejoin','text-anchor','dominant-baseline','alignment-baseline','paint-order','visibility','display','marker-end','marker-start','color'];
function safeCss(text) {
  const sheet=new CSSStyleSheet();try{sheet.replaceSync(text);}catch{return '';}
  const result=[];
  for(const rule of sheet.cssRules) {
    if(!(rule instanceof CSSStyleRule))continue;
    const declarations=[];
    for(const name of rule.style) {
      const value=rule.style.getPropertyValue(name);
      if(!presentation.includes(name)&&!name.startsWith('--'))continue;
      if(/[\\@]|\/\/|(?:javascript|https?|file|data):|expression\s*\(|url\((?!["']?#)/i.test(value))continue;
      declarations.push(`${name}:${value}`);
    }
    if(declarations.length)result.push(`${rule.selectorText}{${declarations.join(';')}}`);
  }
  return result.join('\n');
}
export function sanitize(svg) {
  const clean=DOMPurify.sanitize(svg,{USE_PROFILES:{svg:true,svgFilters:true},FORBID_TAGS:forbidden,FORBID_ATTR:['href','xlink:href','src','srcset','onload','onclick']});
  const doc=new DOMParser().parseFromString(clean,'image/svg+xml');
  if(doc.querySelector('parsererror') || !doc.documentElement.matches('svg')) throw Error('Invalid SVG output');
  for(const el of doc.querySelectorAll('*')) for(const attr of [...el.attributes]) {
    if(/^on/i.test(attr.name) || /(?:javascript:|https?:|file:|data:|@import|expression\s*\(|\\|\/\/)/i.test(attr.value) || /url\((?!["']?#)/i.test(attr.value)) el.removeAttribute(attr.name);
  }
  for(const style of doc.querySelectorAll('style')) style.textContent=safeCss(style.textContent);
  return new XMLSerializer().serializeToString(doc.documentElement);
}
export async function render({kind,source,dark,inline,font=18,svg:inputSvg}) {
  const target=document.getElementById('target');
  target.replaceChildren();target.style.padding=inline?'0':'8px';target.style.color=dark?'#e7e7ed':'#242432';target.style.fontSize=`${font}px`;
  document.body.style.background=dark?'#20212a':'#ffffff';
  await new Promise(resolve=>{const io=new IntersectionObserver(entries=>{if(entries.some(e=>e.isIntersecting)){io.disconnect();resolve();}});io.observe(target);});
  let svg;
  if(kind==='math') {
    const html=katex.renderToString(source,{displayMode:!inline,throwOnError:true,trust:false,strict:'error',maxSize:20,maxExpand:1000,output:'htmlAndMathml'});
    target.innerHTML=DOMPurify.sanitize(html,{USE_PROFILES:{html:true,mathMl:true,svg:true,svgFilters:true},FORBID_TAGS:forbidden,FORBID_ATTR:['href','src','onerror','onclick']});
  } else {
    if(kind==='mermaid') {
      if(/^\s*flowchart-elk\b/im.test(source)) throw Error('ELK layout is not included in this build; use the default Dagre layout.');
      if(/%%\s*\{|^\s*---|^\s*click\s+\w+\s+(?:call|href|["'])/im.test(source)) throw Error('Diagram configuration and callbacks are disabled.');
      mermaid.initialize({startOnLoad:false,layout:'dagre',securityLevel:'strict',theme:dark?'dark':'default',fontFamily:'MDcard Diagram Sans, MDcard Diagram Sans KR, sans-serif',htmlLabels:false,maxTextSize:65536,maxEdges:500,secure:['securityLevel','secure','startOnLoad','maxTextSize','maxEdges','theme','htmlLabels','themeCSS','themeVariables','flowchart','sequence','fontFamily']});
      svg=(await mermaid.render('diagram',source)).svg;
    } else if(kind==='flow') {
      // flowchart.js/Raphael supports URL labels. Navigation is never part of our source language.
      if(/:>|(?:https?|javascript|data|file):|<\s*\/?\s*(?:script|iframe|svg|img|object|style|a|foreignobject)\b|<\s*\/?\s*[a-z][a-z0-9:-]*(?:\s[^<>]*)?\/?>/i.test(source)) throw Error('Diagram links and HTML are disabled.');
      flowchart.parse(source).drawSVG('target',{'line-color':dark?'#d0d4e0':'#303442','element-color':dark?'#d0d4e0':'#303442',fill:dark?'#292d3a':'#f8faff','font-color':dark?'#e9edf8':'#242432','font-size':16,'font-family':'MDcard Diagram Sans, MDcard Diagram Sans KR'});
      svg=target.querySelector('svg').outerHTML;
    } else if(kind==='plantuml') svg=inputSvg;
    else throw Error('Unsupported engine');
    svg=sanitize(svg);
    target.innerHTML=svg;
    const el=target.querySelector('svg');
    // Resolve engine CSS while isolated, then export only inert presentation attributes.
    const resolved=[];
    for(const node of [el,...el.querySelectorAll('*')]) {
      if(node.localName==='style')continue;
      if(node.localName==='text')node.setAttribute('font-family','MDcard Diagram Sans, MDcard Diagram Sans KR, sans-serif');
      const css=getComputedStyle(node);const values={};
      for(const name of presentation) {
        let value=css.getPropertyValue(name);
        if(kind==='plantuml'&&dark&&(name==='fill'||name==='stroke')){
          const components=value.match(/[\d.]+/g)?.map(Number)||[];
          const brightness=components.length>=3?(components[0]*0.2126+components[1]*0.7152+components[2]*0.0722)/255:0;
          if(name==='fill'&&node.localName==='text')value='#e7e7ed';
          else if(name==='fill'&&value!=='none'&&brightness>0.65)value='#34394b';
          else if(name==='stroke'&&value!=='none'&&brightness<0.25)value='#d0d4e0';
        }
        if(name.startsWith('marker-')){const fragment=value.match(/url\(["']?(?:[^#]*)#([\w-]+)["']?\)/);value=fragment?`url(#${fragment[1]})`:'none';}
        if(value&&!/[\\@]|\/\/|(?:javascript|https?|file|data):/i.test(value))values[name]=value;
      }
      resolved.push([node,values]);
    }
    for(const [node,values] of resolved){for(const [name,value] of Object.entries(values))node.setAttribute(name,value);node.removeAttribute('style');}
    el.querySelectorAll('style').forEach(s=>s.remove());
    el.style.maxWidth='none';el.style.maxHeight='none';
    const b=el.getBBox();
    if(!el.getAttribute('viewBox')) el.setAttribute('viewBox',`0 0 ${Math.ceil(b.x+b.width+12)} ${Math.ceil(b.y+b.height+12)}`);
    const vb=el.viewBox.baseVal;
    if(vb.width>4096 || vb.height>4096 || vb.width*vb.height>8e6) throw Error('Diagram exceeds the 4096px / 8MP canvas limit.');
    el.setAttribute('width',Math.max(1,Math.ceil(vb.width)));el.setAttribute('height',Math.max(1,Math.ceil(vb.height)));
    const background=document.createElementNS('http://www.w3.org/2000/svg','rect');
    background.setAttribute('x',vb.x);background.setAttribute('y',vb.y);background.setAttribute('width',vb.width);background.setAttribute('height',vb.height);background.setAttribute('fill',dark?'#20212a':'#ffffff');background.setAttribute('data-mdcard-background','true');el.prepend(background);
    svg=sanitize(el.outerHTML);target.innerHTML=svg;
  }
  await document.fonts.ready;
  const box=target.getBoundingClientRect();
  if(box.width>4096 || box.height>4096 || box.width*box.height>8e6) throw Error('Node exceeds canvas size limit.');
  return {svg,width:Math.ceil(box.width),height:Math.ceil(box.height),text:target.textContent,elementCount:target.querySelectorAll('*').length};
}
