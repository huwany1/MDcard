// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import {readFile,writeFile,stat,mkdir,open} from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {createHash} from 'node:crypto';
import path from 'node:path';
const site=path.resolve(import.meta.dirname,'..'),repo=path.resolve(site,'..');
const latest=JSON.parse(await readFile(path.join(repo,'dist/latest.json'),'utf8'));
const config=JSON.parse(await readFile(path.join(site,'data/downloads.json'),'utf8'));
const sha256=file=>new Promise((resolve,reject)=>{const h=createHash('sha256');createReadStream(file).on('data',d=>h.update(d)).on('error',reject).on('end',()=>resolve(h.digest('hex')));});
const releases=[];
for(const item of latest){
 const archive=item.archive,manifest=JSON.parse(await readFile(path.join(item.stage,'manifest.json'),'utf8'));
 const hash=await sha256(archive);if(hash!==item.archive_sha256)throw Error(item.platform+' archive hash mismatch');
 const binary=path.join(item.stage,item.platform==='windows'?'MDcard.exe':'mdcard');if(await sha256(binary)!==item.binary_sha256)throw Error('binary mismatch');
 if(manifest.platform!==item.platform)throw Error('Manifest platform mismatch');
 const descriptor=await open(binary,'r'),header=Buffer.alloc(4096);await descriptor.read(header,0,4096,0);await descriptor.close();
 if(item.platform==='windows'){const pe=header.readUInt32LE(0x3c);if(header.toString('ascii',0,2)!=='MZ'||header.toString('ascii',pe,pe+2)!=='PE'||header.readUInt16LE(pe+4)!==0x8664)throw Error('Windows binary is not PE x86_64');}
 else if(header.toString('hex',0,4)!=='7f454c46'||header[4]!==2||header.readUInt16LE(18)!==62)throw Error('Linux binary is not ELF x86_64');
 const url=config[item.platform]?.url??null;if(url&&!/^https:\/\//.test(url))throw Error('Downloads require a public HTTPS URL');
 const promotion=JSON.parse(await readFile(path.join(repo,item.promotion_record),'utf8'));
 if(!JSON.stringify(promotion).includes(item.build_id))throw Error('Promotion build mismatch');
 releases.push({platform:item.platform,version:manifest.version,release_name:item.release_name,build_id:item.build_id,architecture:'x86_64',filename:path.basename(archive),bytes:(await stat(archive)).size,sha256:hash,binary_sha256:item.binary_sha256,updated_at:item.promoted_at,source_commit:item.source_commit,binary_source_commit:item.binary_source_commit,promotion_record:item.promotion_record,download_url:url,download_status:url?'available':'unavailable',installation:item.platform==='windows'?['下载后核对 SHA256；将完整 ZIP 解压到可写目录。','双击完整目录里的 MDcard.exe。请保留 renderer、assets 等伴随资源。','随附 Create-MDcardShortcut.ps1 可创建快捷方式。']:['需 x86_64 Linux 桌面；当前二进制基于 Debian 12 / glibc 2.36。','安装页面列出的桌面运行依赖；使用 tar -xzf 完整解压 tar.gz。','在解压目录运行 chmod +x mdcard，然后 ./mdcard。']});
}
await mkdir(path.join(site,'src/data'),{recursive:true});
await writeFile(path.join(site,'src/data/releases.json'),JSON.stringify({schema_version:1,generated_at:new Date().toISOString(),source:'dist/latest.json',releases},null,2)+'\n');
console.log(releases.map(r=>({platform:r.platform,build:r.build_id,bytes:r.bytes,sha256:r.sha256,status:r.download_status})));
