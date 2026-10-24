// SPDX-License-Identifier: AGPL-3.0-only
// Maintenance only: normal website builds use the generated public JSON.
import {readFile,writeFile,stat,mkdir,open} from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {createHash} from 'node:crypto';
import path from 'node:path';
const site=path.resolve(import.meta.dirname,'..');
function arg(name,fallback){const i=process.argv.indexOf(name);return i<0?fallback:process.argv[i+1];}
const input=arg('--release-index');
if(!input)throw Error('Usage: node scripts/releases.mjs --release-index <release.json> [--downloads <downloads.json>]');
const indexPath=path.resolve(input),base=path.dirname(indexPath),index=JSON.parse(await readFile(indexPath,'utf8'));
const config=JSON.parse(await readFile(path.resolve(arg('--downloads',path.join(site,'data/downloads.json'))),'utf8'));
const sha256=file=>new Promise((resolve,reject)=>{const h=createHash('sha256');createReadStream(file).on('data',d=>h.update(d)).on('error',reject).on('end',()=>resolve(h.digest('hex')));});
if(index.version!=='0.1.0'||!/^\w{40}$/.test(index.sourceCommit)||index.packages.length!==2)throw Error('Expected unified 0.1.0 source and both platforms');
const releases=[];
for(const item of index.packages){
 const platform=item.platform;if(!['windows','linux'].includes(platform))throw Error('Unsupported platform');
 const name=`MDcard-0.1.0-${platform}-x64${platform==='windows'?'.zip':'.tar.gz'}`;
 if(path.basename(item.archive)!==name)throw Error('Unexpected release filename');
 const archive=path.resolve(base,item.archive),stage=path.resolve(base,item.stage),manifest=JSON.parse(await readFile(path.join(stage,'manifest.json'),'utf8'));
 const hash=await sha256(archive),binary=path.join(stage,item.binary.path);
 if(hash!==item.sha256||(await stat(archive)).size!==item.bytes||await sha256(binary)!==item.binary.sha256||await sha256(path.join(stage,'manifest.json'))!==item.manifestSHA256)throw Error('Frozen release bytes differ');
 if(manifest.version!=='0.1.0'||manifest.sourceCommit!==index.sourceCommit||manifest.platform!==platform||manifest.license!=='AGPL-3.0-only')throw Error('Manifest identity differs');
 const descriptor=await open(binary,'r'),header=Buffer.alloc(4096);await descriptor.read(header,0,4096,0);await descriptor.close();
 if(platform==='windows'){const pe=header.readUInt32LE(0x3c);if(header.toString('ascii',0,2)!=='MZ'||header.toString('ascii',pe,pe+2)!=='PE'||header.readUInt16LE(pe+4)!==0x8664)throw Error('Expected Windows x86_64');}
 else if(header.toString('hex',0,4)!=='7f454c46'||header[4]!==2||header.readUInt16LE(18)!==62)throw Error('Expected Linux x86_64');
 const url=config[platform]?.url??null;
 if(url&&(!/^https:\/\/github\.com\/huwany1\/MDcard\/releases\/download\/v0\.1\.0\//.test(url)||!url.endsWith('/'+name)))throw Error('Download URL must match this 0.1.0 asset');
 if(url&&!index.localAcceptancePassed)throw Error('Downloads cannot be enabled before complete local acceptance');
 releases.push({platform,version:'0.1.0',release_name:'MDcard 0.1.0',build_id:'0.1.0-'+index.sourceCommit.slice(0,12),architecture:'x86_64',filename:name,bytes:item.bytes,sha256:hash,binary_sha256:item.binary.sha256,updated_at:index.createdAt,source_commit:index.sourceCommit,binary_source_commit:index.sourceCommit,promotion_record:'MDcard-0.1.0-release.json',download_url:url,download_status:url?'available':'unavailable',installation:platform==='windows'?['核对 SHA256 后完整解压 ZIP 到可写目录。','运行 MDcard.exe；保留 renderer、许可证、README.txt 与 manifest.json。','无需安装 Node、Java、Chromium、Pandoc 或开发工具。']:['需要 x86_64 Linux X11 桌面；基于 Debian 12 / glibc 2.36。','具备页面列出的系统桌面库，使用 tar -xzf 完整解压。','运行 ./mdcard；程序及辅助运行时已保留执行权限。']});
}
await mkdir(path.join(site,'src/data'),{recursive:true});
await writeFile(path.join(site,'src/data/releases.json'),JSON.stringify({schema_version:1,generated_at:new Date().toISOString(),source:path.basename(indexPath),releases},null,2)+'\n');
console.log(releases.map(r=>({platform:r.platform,version:r.version,bytes:r.bytes,sha256:r.sha256,status:r.download_status})));
