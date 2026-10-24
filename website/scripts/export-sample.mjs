// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
const site=path.resolve(import.meta.dirname,'..'),repo=path.resolve(site,'..');
const stage=path.join(repo,'dist/windows'),out=path.join(site,'evidence/product/exports');await mkdir(out,{recursive:true});
const source=await readFile(path.join(site,'samples/从原稿到作品.md'),'utf8');
const records=[];
for(const format of ['html','pdf']){
 const output=path.join(out,'从原稿到作品.'+format),request=path.join(out,'request-'+format+'.json');
 await writeFile(request,JSON.stringify({source,format,output,baseDir:path.join(site,'samples'),options:{title:'一份技术文档的诞生',language:'zh-CN',lineNumbers:true}},null,2));
 const run=await new Promise((resolve,reject)=>{const child=spawn(path.join(stage,'renderer/runtime/node.exe'),[path.join(stage,'renderer/export.mjs'),request],{windowsHide:true,cwd:out,env:{...process.env,PATH:''},stdio:['ignore','pipe','pipe']});let stdout='',stderr='';child.stdout.on('data',d=>stdout+=d);child.stderr.on('data',d=>stderr+=d);child.on('error',reject);child.on('exit',code=>resolve({code,stdout,stderr}));});
 await writeFile(path.join(out,format+'-stdout.jsonl'),run.stdout);await writeFile(path.join(out,format+'-stderr.log'),run.stderr);
 const events=run.stdout.trim().split('\n').filter(Boolean).map(x=>JSON.parse(x)),complete=events.find(x=>x.type==='complete');if(!complete||run.code!==0)throw Error(JSON.stringify(run));
 const bytes=await readFile(output),sha256=createHash('sha256').update(bytes).digest('hex');if(sha256!==complete.sha256)throw Error('Export SHA mismatch');
 records.push({format,bytes:bytes.length,sha256,source_sha256:createHash('sha256').update(source).digest('hex'),packaged_engine:'dist/windows/renderer/export.mjs',binary_build:'icon-suite-20261024-r3',exit_code:run.code,recorded_at:new Date().toISOString()});
 console.log(format,bytes.length,sha256);
}
await writeFile(path.join(out,'provenance.json'),JSON.stringify(records,null,2)+'\n');
