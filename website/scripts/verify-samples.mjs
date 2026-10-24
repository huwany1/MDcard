// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {chromium} from 'playwright';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';
const site=path.resolve(import.meta.dirname,'..');
const out=path.resolve(site,process.env.EVIDENCE_DIR||'evidence/browser');await mkdir(out,{recursive:true});
const browser=await chromium.launch({executablePath:process.env.CHROMIUM_PATH||path.join(site,'../dist/windows/renderer/runtime/chromium/chrome.exe'),headless:true});
try{
 const page=await browser.newPage();await page.goto(process.env.BASE_URL||'http://127.0.0.1:4173/');const records=[];
 for(const label of ['带走这份示例原稿','下载离线 HTML','下载示例 PDF']){
  const [download]=await Promise.all([page.waitForEvent('download'),page.getByRole('link',{name:new RegExp(label)}).click()]);
  const bytes=await readFile(await download.path()),filename=download.suggestedFilename();
  if(filename.endsWith('.md'))assert(bytes.equals(await readFile(path.join(site,'samples/从原稿到作品.md'))));
  else if(filename.endsWith('.zip'))assert(bytes.equals(await readFile(path.join(site,'src/assets/exports/manuscript-html.zip'))));
  else assert.equal(bytes.toString('ascii',0,4),'%PDF');
  records.push({label,filename,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex'),real_browser_download:true});
 }
 await writeFile(path.join(out,'sample-downloads.json'),JSON.stringify({recorded_at:new Date().toISOString(),url:process.env.BASE_URL||'http://127.0.0.1:4173/',downloads:records},null,2)+'\n');console.log(records);
}finally{await browser.close();}
