// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtemp,symlink,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';

const root=path.dirname(fileURLToPath(import.meta.url));
const execute=(entry,input)=>spawnSync(process.execPath,[entry],{input,encoding:'utf8',timeout:30000});
for(const name of ['worker.mjs','export.mjs']){
 test(`${name} CLI runs through a real-file alias`,async()=>{
  const entry=path.join(root,name);const dir=await mkdtemp(path.join(tmpdir(),'mdcard-entry-'));
  try{
   let alias;
   if(process.platform==='win32')alias=path.toNamespacedPath(entry);
   else{alias=path.join(dir,name);await symlink(entry,alias);}
   const input=name==='worker.mjs'?'{"id":"1","kind":"not-supported","source":""}\n':undefined;
   const normal=execute(entry,input),aliased=execute(alias,input);
   assert.ifError(normal.error);assert.ifError(aliased.error);
   assert.equal(normal.status,name==='worker.mjs'?0:1,normal.stderr);
   assert.equal(aliased.status,normal.status,aliased.stderr);
   const expected=normal.stdout.trim().split('\n').map(line=>JSON.parse(line));
   const actual=aliased.stdout.trim().split('\n').map(line=>JSON.parse(line));
   assert.deepEqual(actual,expected);assert.equal(actual[0].ok,false);
   assert.equal(actual[0][name==='worker.mjs'?'id':'type'],name==='worker.mjs'?'1':'error');
  }finally{await rm(dir,{recursive:true,force:true});}
 });
}
test('library imports never enter either CLI, including a non-file argv value',()=>{
 const code=`process.argv[1]='not-a-file';await import(${JSON.stringify(pathToFileURL(path.join(root,'worker.mjs')).href)});await import(${JSON.stringify(pathToFileURL(path.join(root,'export.mjs')).href)});console.log('import-only');`;
 const child=spawnSync(process.execPath,['--input-type=module','-e',code],{encoding:'utf8',timeout:30000});
 assert.ifError(child.error);assert.equal(child.status,0,child.stderr);assert.equal(child.stdout,'import-only\n');
});
