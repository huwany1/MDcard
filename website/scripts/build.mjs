// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,writeFile,mkdir,cp,rm,stat} from 'node:fs/promises';
import path from 'node:path';
const site=path.resolve(import.meta.dirname,'..');const src=path.join(site,'src'),out=path.join(site,'dist');
await mkdir(out,{recursive:true});
for(const file of ['index.html','style.css','app.js','assets','data'])await cp(path.join(src,file),path.join(out,file),{recursive:true});
await cp(path.join(site,'samples/从原稿到作品.md'),path.join(out,'assets/manuscript.md'));
await cp(path.join(src,'assets/exports/manuscript.html'),path.join(out,'assets/manuscript.html'));
await cp(path.join(src,'assets/exports/manuscript.pdf'),path.join(out,'assets/manuscript.pdf'));
const release=JSON.parse(await readFile(path.join(src,'data/releases.json'),'utf8'));const sample=await readFile(path.join(site,'samples/从原稿到作品.md'),'utf8');
await writeFile(path.join(out,'data/sample.json'),JSON.stringify({filename:'从原稿到作品.md',source:sample}));
const escape=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
const panels=release.releases.map(r=>`<article class="release" id="release-${r.platform}"><div class="release-top"><h3>${r.platform==='windows'?'Windows':'Linux'} <span>x64</span></h3><span class="status">${r.download_url?'正式完整包':'公开下载待开放'}</span></div><p class="release-title">v${r.version} · ${r.release_name}</p><dl><div><dt>构建</dt><dd>${escape(r.build_id)}</dd></div><div><dt>架构 / 包大小</dt><dd>x86_64 / ${(r.bytes/1024/1024).toFixed(1)} MiB <small>（${r.bytes.toLocaleString('en-US')} bytes）</small></dd></div><div><dt>更新</dt><dd>${new Date(r.updated_at).toISOString().replace('T',' ').slice(0,16)} UTC</dd></div></dl><details><summary>SHA256 与安装说明</summary><p class="file-name">${escape(r.filename)}</p><code class="hash">${r.sha256}</code><ol>${r.installation.map(x=>'<li>'+escape(x)+'</li>').join('')}</ol>${r.platform==='linux'?'<p>Debian/Ubuntu 运行依赖：</p><pre tabindex="0"><code>sudo apt-get install libxkbcommon-x11-0 libgl1 libegl1 libxcursor1 libxrandr2 libxi6 libvulkan1 mesa-vulkan-drivers zenity xdg-desktop-portal xdg-desktop-portal-gtk</code></pre><p>本地验收环境为 Debian 12 / X11。其他发行版、Wayland、桌面与驱动组合仍需验证。</p>':'<p>保存文件需要目录写权限。完整包含 Node、Chromium、Java、Pandoc 与字体资源，解压后请保留完整目录。</p>'}</details>${r.download_url?`<a class="button primary" href="${escape(r.download_url)}" download="${escape(r.filename)}">下载 ${r.platform==='windows'?'Windows':'Linux'} 完整包 <span aria-hidden="true">↓</span></a>`:'<button class="button unavailable" disabled>暂不可下载</button><p class="release-note">完整包已在本地验收；尚未提供可验证的公开下载地址。</p>'}</article>`).join('');
let html=await readFile(path.join(out,'index.html'),'utf8');html=html.replace('<!-- RELEASE_PANELS -->',panels);await writeFile(path.join(out,'index.html'),html);
await writeFile(path.join(out,'robots.txt'),'User-agent: *\nAllow: /\n');
await writeFile(path.join(out,'sitemap.xml'),'<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>https://mdcard-document-journey.wholeangel11.chatgpt.site/</loc></url></urlset>\n');
console.log('Built static release page at '+out+'; platform data embedded for no-JavaScript access.');
