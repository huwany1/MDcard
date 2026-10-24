# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Plan and audit the explicit post-icon-suite cleanup; never delete files here."""
from pathlib import Path
import argparse, concurrent.futures, datetime, hashlib, json, os, shutil, stat, subprocess

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'evidence/cleanup-icon-suite-20261024'
OUT.mkdir(parents=True,exist_ok=True)

def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def sha(path):
 with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
def linked(info):return stat.S_ISLNK(info.st_mode) or bool(getattr(info,'st_file_attributes',0)&0x400)
def files(path):
 pending=[path]
 while pending:
  p=pending.pop();s=p.lstat()
  if linked(s):yield p,s,'link'
  elif stat.S_ISDIR(s.st_mode):pending.extend(p.iterdir())
  else:yield p,s,'file'
def verify(label):
 latest=read(ROOT/'dist/latest.json');records=[]
 for release in latest:
  stage=Path(release['stage']);manifest=read(stage/'manifest.json')
  expected={e['path'] for e in manifest['files']}|{'manifest.json'}
  actual={p.relative_to(stage).as_posix() for p,s,k in files(stage) if k=='file'}
  def check(entry):
   p=stage/entry['path']
   return entry['path'] if not p.resolve().is_relative_to(stage) or p.is_symlink() or p.stat().st_size!=entry['bytes'] or sha(p)!=entry['sha256'] else None
  with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:errors=[e for e in pool.map(check,manifest['files']) if e]
  exe=stage/('MDcard.exe' if release['platform']=='windows' else 'mdcard')
  record={'platform':release['platform'],'payload_files':len(manifest['files']),'exact_inventory':actual==expected,'hash_errors':errors,'binary_sha256':sha(exe),'archive_sha256':sha(Path(release['archive']))}
  record['passed']=record['exact_inventory'] and not errors and record['binary_sha256']==release['binary_sha256'] and record['archive_sha256']==release['archive_sha256']
  records.append(record)
 result={'passed':all(r['passed'] for r in records),'latest_sha256':sha(ROOT/'dist/latest.json'),'packages':records,'utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
 (OUT/f'verification-{label}.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
 print(json.dumps(result,indent=2),flush=True)
 if not result['passed']:raise RuntimeError('Current release verification failed')

def plan():
 if (OUT/'plan.json').exists():raise FileExistsError('Preserve the original cleanup plan; choose a new cleanup scope for another run')
 before=read(OUT/'verification-before.json')
 if not before['passed'] or before['latest_sha256']!=sha(ROOT/'dist/latest.json'):raise RuntimeError('Verify current releases first')
 tracked=set(subprocess.check_output(['git','ls-files','-z'],cwd=ROOT).decode('utf-8').split('\0'))
 targets=[];saved=[]
 choices=[
 ('artifacts/icon-integration-20261024/packages','superseded_candidates','R1/R2已被R3替代；R3候选与正式包及运行目录重复，构建身份小型元数据另存'),
 ('artifacts/icon-integration-20261024/master/最终解压 中文 空格','duplicate_extraction','最终Windows完整包验收解压副本；原始报告/截图/夹具保留'),
 ('artifacts/icon-integration-20261024/master/迁移后 新目录 中文 空格','duplicate_extraction','迁移验收解压副本；正式长期目录已验证，迁移报告保留'),
 ('dist/.before-icon-suite-20261024-r3','duplicate_rollback_runtime','旧Windows展开回滚目录；同版本R29完整ZIP保留，可据原哈希重新解压'),
 ('artifacts/icon-design/2026-10-24/browser-profile','browser_cache','已关闭的图标预览专用浏览器配置/缓存，不是用户浏览器配置'),
 ('artifacts/icon-design/2026-10-24/browser-profile-no-extensions','browser_cache','已关闭的独立验收浏览器配置/缓存；JSON与真实截图保留'),
 ('artifacts/icon-design/2026-10-24/unpacked-3404e05d5dcc','duplicate_icon_kit','初稿资源包验收解压副本，最终可编辑套装和资源ZIP保留'),
 ('artifacts/icon-design/2026-10-24/unpacked-af77c9454bff','duplicate_icon_kit','最终资源包重复解压，原始套装与资源ZIP保留'),
 ('assets/branding/mdcard-icon/tools/node_modules','dev_cache','图标生成工具依赖缓存，可由固定package-lock重装；运行时不依赖此目录'),
 ('scripts/__pycache__','python_cache','可重新生成的Python字节码缓存'),
 ('artifacts/icon-integration-20261024/implementation/MDcard.exe','duplicate_binary','与正式Windows EXE同SHA256的实现侧副本；实际测试PE及日志保留'),
 ]
 # Preserve identities before removing obsolete or duplicate full package trees.
 packages=ROOT/'artifacts/icon-integration-20261024/packages'
 if packages.exists():
  for candidate in packages.iterdir():
   for relative in ['packages.json','windows/manifest.json','windows/build/compile-inputs.json','windows/build/compiler-deps.d','windows/README-ICON-SUITE.md']:
    source=candidate/relative
    if source.is_file():
     destination=OUT/'retired-metadata'/source.relative_to(ROOT);destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,destination)
     if sha(source)!=sha(destination):raise RuntimeError('Metadata preservation failed')
     saved.append({'original':source.relative_to(ROOT).as_posix(),'preserved':destination.relative_to(ROOT).as_posix(),'sha256':sha(destination)})
 backup=ROOT/'dist/.before-icon-suite-20261024-r3/windows/manifest.json'
 if backup.exists():
  target=OUT/'retired-metadata'/backup.relative_to(ROOT);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(backup,target)
  saved.append({'original':backup.relative_to(ROOT).as_posix(),'preserved':target.relative_to(ROOT).as_posix(),'sha256':sha(target)})
 for name,category,reason in choices:
  path=ROOT/name
  if not path.exists():continue
  if not path.resolve().is_relative_to(ROOT) or linked(path.lstat()) or any(t==name or t.startswith(name+'/') for t in tracked):raise RuntimeError('Unsafe or tracked cleanup target: '+name)
  items=list(files(path));links=[str(p) for p,s,k in items if k=='link']
  if links:raise RuntimeError('Review linked entries before cleaning: '+name)
  if name.endswith('/implementation/MDcard.exe') and sha(path)!=next(r['binary_sha256'] for r in before['packages'] if r['platform']=='windows'):raise RuntimeError('Implementation binary is not an exact duplicate')
  targets.append({'path':name,'absolute':str(path),'category':category,'reason':reason,'files':len(items),'bytes':sum(s.st_size for p,s,k in items),'markdown_files':sum(p.suffix.lower()=='.md' for p,s,k in items),'kind':'tree' if path.is_dir() else 'file'})
 result={'workspace':str(ROOT),'no_follow':True,'latest_sha256':sha(ROOT/'dist/latest.json'),'verification_sha256':sha(OUT/'verification-before.json'),'targets':targets,'saved_metadata':saved,'files':sum(e['files'] for e in targets),'bytes':sum(e['bytes'] for e in targets),'markdown_files':sum(e['markdown_files'] for e in targets)}
 (OUT/'plan.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
 print(json.dumps({k:result[k] for k in ['files','bytes','markdown_files']}|{'targets':len(targets)},indent=2))

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['verify-before','plan','verify-after']);a=parser.parse_args()
if a.action=='plan':plan()
else:verify(a.action.removeprefix('verify-'))
