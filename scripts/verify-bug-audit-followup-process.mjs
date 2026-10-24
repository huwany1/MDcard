// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
// Independent portable checks of the candidate's converter lifecycle module.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {readFile,writeFile,mkdir,mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash,randomUUID} from 'node:crypto';

const modulePath=path.resolve(process.argv[2]);
const output=path.resolve(process.argv[3]);
await mkdir(output,{recursive:true});await rm(path.join(output,'result.json'),{force:true});
const {runOwned,killOwnedTree}=await import(pathToFileURL(modulePath));
const fixture=await mkdtemp(path.join(tmpdir(),'mdcard-independent-lifecycle-'));
const untouched=path.join(fixture,'old-output');await writeFile(untouched,'OLD OUTPUT');
const unrelated=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore',windowsHide:true});
const unrelatedClosed=once(unrelated,'close');
const records=[];const created=new Set();
async function terminal(pid){
 if(process.platform==='linux'){
  try{const raw=await readFile(`/proc/${pid}/stat`,'utf8');return ['Z','X'].includes(raw.slice(raw.lastIndexOf(') ')+2).split(' ')[0]);}
  catch(e){if(e.code==='ENOENT')return true;throw e;}
 }
 try{process.kill(pid,0);return false;}catch(e){if(e.code==='ESRCH')return true;throw e;}
}
async function marker(file){
 const until=performance.now()+5000;
 while(performance.now()<until){try{const ids=JSON.parse(await readFile(file,'utf8'));if(ids.length===2)return ids;}catch{}await new Promise(r=>setTimeout(r,2));}
 throw Error('Converter tree fixture never became ready');
}
const code=`const {spawn}=require('node:child_process');const fs=require('node:fs');
const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit',windowsHide:true});
fs.writeFileSync(process.argv[1],JSON.stringify([process.pid,child.pid]));setInterval(()=>{},1000);`;
try{
 assert.equal(await runOwned(process.execPath,['-e','process.stdout.write("success")']),'success');
 await assert.rejects(runOwned(process.execPath,['-e','process.exit(3)']),e=>e.stage==='convert');
 await assert.rejects(runOwned(path.join(fixture,'missing-converter'),[]),e=>e.stage==='dependency');
 const pre=new AbortController();pre.abort(new Error('pre-aborted'));
 await assert.rejects(runOwned(process.execPath,['-e','throw Error("must not run")'],{signal:pre.signal}),/pre-aborted/);
 for(let n=0;n<3;n++){
  const file=path.join(fixture,`tree-${n}.json`);const abort=new AbortController();
  const pending=runOwned(process.execPath,['-e',code,file],{signal:abort.signal,timeout:10000});
  const observed=assert.rejects(pending,/independent cancellation/);
  const ids=await marker(file);for(const id of ids)created.add(id);
  const start=performance.now();abort.abort(new Error('independent cancellation'));await observed;
  for(const id of ids)assert(await terminal(id),`Owned PID ${id} alive after rejection`);
  assert.equal(await readFile(untouched,'utf8'),'OLD OUTPUT');
  assert(!await terminal(unrelated.pid));
  assert.equal(await runOwned(process.execPath,['-e','process.stdout.write("retried")']),'retried');
  records.push({iteration:n,ids,elapsed_ms:performance.now()-start,terminal_at_rejection:true,retry:true});
 }
 await assert.rejects(runOwned(process.execPath,['-e','setInterval(()=>{},1000)'],{timeout:200}),e=>e.stage==='timeout');
 const result={verification_id:randomUUID(),date:new Date().toISOString(),platform:process.platform,
  module:modulePath,module_sha256:createHash('sha256').update(await readFile(modulePath)).digest('hex'),
  natural_completion:true,exit_error:true,spawn_error:true,preabort:true,timeout:true,
  unrelated_preserved:true,old_target_preserved:true,cancel_and_retry:records};
 await writeFile(path.join(output,'result.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
}finally{
 for(const id of created)if(!await terminal(id))await killOwnedTree(id).catch(()=>{});
 unrelated.kill('SIGKILL');await unrelatedClosed;
 await rm(fixture,{recursive:true,force:true});
}
