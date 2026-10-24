// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import test from 'node:test';
import './entrypoint.test.mjs';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {readFile,writeFile,mkdtemp,rm,access} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {Worker} from 'node:worker_threads';
import {killOwnedTree,runOwned} from './export-process.mjs';

const terminal=async pid=>{try{const stat=await readFile(`/proc/${pid}/stat`,'utf8');return ['Z','X'].includes(stat.slice(stat.lastIndexOf(') ')+2).split(' ')[0]);}catch(e){if(e.code==='ENOENT'||e.code==='ESRCH')return true;throw e;}};
test('Linux cancellation resolves only after detached descendants are terminal', {skip:process.platform!=='linux'},async()=>{
 const loads=Array.from({length:4},()=>new Worker('let n=1;for(;;){n=(n*1664525+1013904223)>>>0}',{eval:true}));
 const unrelated=spawn('/bin/sleep',['30']);const unrelatedClose=once(unrelated,'close');
 const observations=[];
 try{
  for(let n=0;n<30;n++){
   const child=spawn('/bin/sh',['-c','/usr/bin/setsid /bin/sleep 30 & echo $!; wait'],{stdio:['ignore','pipe','ignore']});const closed=once(child,'close');
   const data=await once(child.stdout,'data');const descendant=Number(data[0].toString().trim());
   const started=performance.now();await killOwnedTree(child.pid);const stopped=await terminal(descendant);
   await closed;observations.push({iteration:n,terminal_at_return:stopped,elapsed_ms:performance.now()-started});
   if(!stopped){for(let i=0;i<100&&!await terminal(descendant);i++)await new Promise(r=>setTimeout(r,1));}
  }
 }finally{for(const w of loads)await w.terminate();unrelated.kill('SIGKILL');await unrelatedClose;}
 const report={iterations:observations.length,live_at_return:observations.filter(o=>!o.terminal_at_return).length,observations};
 if(process.env.MDCARD_FOLLOWUP_CANCEL_REPORT)await writeFile(process.env.MDCARD_FOLLOWUP_CANCEL_REPORT,JSON.stringify(report,null,2));
 assert.equal(report.live_at_return,0);
});

test('converter lifecycle: natural completion, exit error and missing executable', {skip:process.platform==='win32'},async()=>{
 assert.equal(await runOwned('/bin/sh',['-c','printf hello']), 'hello');
 await assert.rejects(runOwned('/bin/sh',['-c','printf failure >&2; exit 3']),e=>e.stage==='convert'&&e.message.includes('3'));
 await assert.rejects(runOwned('/mdcard/nonexistent-converter',[]),e=>e.stage==='dependency');
});
test('a pre-aborted converter never starts', {skip:process.platform==='win32'},async()=>{
 const dir=await mkdtemp(path.join(tmpdir(),'mdcard-preabort-'));const marker=path.join(dir,'started');
 try{const abort=new AbortController();abort.abort(new Error('requested abort'));await assert.rejects(runOwned('/bin/sh',['-c','touch "$1"','fixture',marker],{signal:abort.signal}),/requested abort/);await assert.rejects(access(marker));}finally{await rm(dir,{recursive:true,force:true});}
});
test('an exited leader with inherited descendant pipes obeys timeout and terminates its owned session', {skip:process.platform!=='linux'},async()=>{
 const dir=await mkdtemp(path.join(tmpdir(),'mdcard-exited-leader-'));const pidfile=path.join(dir,'pid');
 try{const start=performance.now();await assert.rejects(runOwned('/bin/sh',['-c','/bin/sleep 30 & echo $! > "$1"; exit 0','fixture',pidfile],{timeout:100}),e=>e.stage==='timeout');assert(performance.now()-start<2000);const pid=Number(await readFile(pidfile,'utf8'));assert(await terminal(pid));}finally{await rm(dir,{recursive:true,force:true});}
});
test('abort waits for converter and inherited pipes, preserves old target and allows retry', {skip:process.platform!=='linux'},async()=>{
 const dir=await mkdtemp(path.join(tmpdir(),'mdcard-abort-retry-'));const pidfile=path.join(dir,'pids');const output=path.join(dir,'output');
 try{
  await writeFile(output,'OLD OUTPUT');const abort=new AbortController();
  const pending=runOwned('/bin/sh',['-c','/bin/sleep 30 & p=$!; echo "$$ $p" > "$1"; wait; printf BAD > "$2"','fixture',pidfile,output],{signal:abort.signal,timeout:5000});
  // Attach the rejection observer before the abort event can settle the task.
  const rejected=assert.rejects(pending,/requested abort/);
  let pids;for(let i=0;i<300;i++){try{pids=(await readFile(pidfile,'utf8')).trim().split(/\s+/).map(Number);if(pids.length===2)break;}catch{}await new Promise(r=>setTimeout(r,1));}
  assert.equal(pids?.length,2);abort.abort(new Error('requested abort'));await rejected;
  for(const pid of pids)assert(await terminal(pid));assert.equal(await readFile(output,'utf8'),'OLD OUTPUT');
  assert.equal(await runOwned('/bin/sh',['-c','printf RETRIED > "$1"; printf done','fixture',output]),'done');assert.equal(await readFile(output,'utf8'),'RETRIED');
 }finally{await rm(dir,{recursive:true,force:true});}
});
