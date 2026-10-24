// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {spawn} from 'node:child_process';
import {readdirSync,readFileSync} from 'node:fs';
import path from 'node:path';

const pause=()=>new Promise(resolve=>setTimeout(resolve,1));
function identity(pid){
 try{
  const stat=readFileSync(`/proc/${pid}/stat`,'utf8');const fields=stat.slice(stat.lastIndexOf(') ')+2).trim().split(/\s+/);
  return {pid,parent:Number(fields[1]),group:Number(fields[2]),session:Number(fields[3]),start:fields[19],state:fields[0]};
 }catch(e){if(e.code==='ENOENT'||e.code==='ESRCH')return undefined;throw e;}
}
function sameProcess(process){const current=identity(process.pid);return current?.start===process.start?current:undefined;}
const terminal=p=>!p||p.state==='Z'||p.state==='X';
export async function killOwnedTree(pid,{timeoutMs=5000,owner,ownedSession=false}={}){
 if(!pid)return {owned:0,confirmed:true};
 if(process.platform==='win32'){
  const taskkill=path.join(process.env.SystemRoot||'C:\\Windows','System32/taskkill.exe');
  await new Promise((resolve,reject)=>{
   const child=spawn(taskkill,['/F','/T','/PID',String(pid)],{windowsHide:true,stdio:'ignore'});
   child.once('error',reject);child.once('close',code=>{
    if(code===0)return resolve();
    try{process.kill(pid,0);reject(new Error('Export process-tree termination was not confirmed'));}catch(e){e.code==='ESRCH'?resolve():reject(e);}
   });
  });return {owned:1,confirmed:true};
 }
 if(process.platform!=='linux'){
  try{process.kill(pid,'SIGKILL');}catch(e){if(e.code!=='ESRCH')throw e;}
  return {owned:1,confirmed:false};
 }
 const root=identity(pid);
 if(owner&&root&&owner.start!==root.start)throw new Error('Export leader identity changed; refusing to signal a reused PID');
 const owned=new Map(root?[[pid,root]]:[]);const deadline=performance.now()+timeoutMs;
 if(ownedSession&&owner)for(const name of readdirSync('/proc'))if(/^\d+$/.test(name)){
  const item=identity(Number(name));if(item&&BigInt(item.start)>=BigInt(owner.start)&&(item.session===pid||item.group===pid))owned.set(item.pid,item);
 }
 if(!owned.size)return {owned:0,confirmed:true};
 const signal=(item,kind)=>{if(!sameProcess(item))return;try{process.kill(item.pid,kind);}catch(e){if(e.code!=='ESRCH')throw e;}};
 let failure;
 try{
  for(;;){
   for(const item of owned.values())signal(item,'SIGSTOP');
   while([...owned.values()].some(item=>{const current=sameProcess(item);return !terminal(current)&&!['T','t'].includes(current.state);})){if(performance.now()>=deadline)throw new Error('Export processes did not stop before the cleanup deadline');await pause();}
   const before=owned.size;const edges=readdirSync('/proc').filter(name=>/^\d+$/.test(name)).map(name=>identity(Number(name))).filter(Boolean);
   for(;;){const count=owned.size;for(const item of edges)if(!owned.has(item.pid)&&owned.has(item.parent)&&sameProcess(owned.get(item.parent)))owned.set(item.pid,item);if(count===owned.size)break;}
   if(before===owned.size)break;
   if(performance.now()>=deadline)throw new Error('Export descendant discovery exceeded its cleanup deadline');
  }
 }catch(e){failure=e;}
 // A failed freeze must still terminate already captured processes. Do not leave
 // them STOPped, and do not report success if discovery/termination was incomplete.
 for(const item of [...owned.values()].reverse())try{signal(item,'SIGKILL');}catch(e){failure??=e;}
 const exitDeadline=performance.now()+timeoutMs;
 while([...owned.values()].some(item=>!terminal(sameProcess(item)))){if(performance.now()>=exitDeadline)throw new Error('Export cleanup did not confirm every process exit');await pause();}
 if(failure)throw failure;
 return {owned:owned.size,confirmed:true};
}

// Rejection waits for tree termination and direct-child stream closure. Callers
// may then clean the private workspace and retry without racing a live converter.
export async function runOwned(command,args,{cwd,input,timeout=120000,env={},signal,makeError=(stage,message)=>Object.assign(new Error(message),{stage})}={}){
 if(signal?.aborted)throw signal.reason||makeError('timeout','Export exceeded its task deadline');
 const child=spawn(command,args,{cwd,stdio:['pipe','pipe','pipe'],detached:process.platform!=='win32',windowsHide:true,env:{...process.env,...env}});
 const owner=process.platform==='linux'&&child.pid?identity(child.pid):undefined;
 let out='',err='',spawnError,failure,cleanupFailure,termination;
 let rejectCleanup;const failedCleanup=new Promise((_,reject)=>{rejectCleanup=reject;});
 const terminate=error=>{
  failure??=error;
  termination??=(async()=>{
   await killOwnedTree(child.pid,{owner,ownedSession:true});
   let closeTimer;
   try{await Promise.race([closed,new Promise((_,reject)=>{closeTimer=setTimeout(()=>reject(new Error('Converter streams did not close after termination')),1000);})]);}
   finally{clearTimeout(closeTimer);}
  })().catch(error=>{cleanupFailure=error;rejectCleanup(makeError('cleanup',`Export cleanup was not confirmed: ${error.message}`));});
 };
 const abort=()=>terminate(signal.reason||makeError('timeout','Export exceeded its task deadline'));
 const closed=new Promise(resolve=>{child.once('error',error=>{spawnError=error;});child.once('close',(code,signal)=>resolve({code,signal}));});
 signal?.addEventListener('abort',abort,{once:true});
 const timer=setTimeout(()=>terminate(makeError('timeout','Converter timed out')),timeout);
 child.stdout.on('data',data=>{if(out.length+data.length<=64*1024*1024)out+=data;else terminate(makeError('convert','Converter output exceeds the 64 MiB limit'));});
 child.stderr.on('data',data=>{if(err.length<8000)err+=data.toString().slice(0,8000-err.length);});
 child.stdin.on('error',error=>{if(error.code!=='EPIPE')terminate(makeError('convert',error.message));});
 child.stdin.end(input);
 try{
  const result=await Promise.race([closed,failedCleanup]);
  if(termination)await termination;
  if(cleanupFailure)throw makeError('cleanup',`Export cleanup was not confirmed: ${cleanupFailure.message}`);
  if(failure)throw failure;
  if(spawnError)throw makeError('dependency',`${spawnError.message}. Configure the converter executable in Export settings.`);
  if(result.code!==0)throw makeError('convert',`${path.basename(command)} exited ${result.code}: ${err.slice(0,3000)}`);
  return out;
 }finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);if(cleanupFailure){child.stdin.destroy();child.stdout.destroy();child.stderr.destroy();child.unref();}}
}
