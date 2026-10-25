// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
// Generate candidates without changing checked-in samples.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const site=path.resolve(import.meta.dirname,'..');
const renderer=path.resolve(process.env.MDCARD_RENDERER_DIR||path.join(site,'../renderer'));
const out=path.resolve(process.env.EVIDENCE_DIR||path.join(site,'evidence/product/exports'));
await mkdir(out,{recursive:true});
const {exportDocument}=await import(pathToFileURL(path.join(renderer,'export.mjs')));
const source=await readFile(path.join(site,'samples/从原稿到作品.md'),'utf8');
const records=[];
for(const format of ['html','pdf']){
 const output=path.join(out,'从原稿到作品.'+format);
 const result=await exportDocument({source,format,output,baseDir:path.join(site,'samples'),options:{title:'一份技术文档的诞生',language:'zh-CN',lineNumbers:true}});
 const bytes=await readFile(output);
 if(createHash('sha256').update(bytes).digest('hex')!==result.sha256)throw Error('Export SHA mismatch');
 records.push({...result,source_sha256:createHash('sha256').update(source).digest('hex'),renderer,recorded_at:new Date().toISOString()});
 console.log(format,result.bytes,result.sha256);
}
await writeFile(path.join(out,'provenance.json'),JSON.stringify(records,null,2)+'\n');
