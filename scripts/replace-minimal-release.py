"""Reviewable, restartable GitHub asset replacement after local acceptance.

Each phase is explicit. This script never moves a tag, deletes a Release, or
changes Git history. Credentials remain in memory. No phase has been run during
preparation; --check performs only local validation and authenticated GETs.
"""
from pathlib import Path
import argparse,datetime,hashlib,json,os,re,subprocess,time
import requests
if not __debug__:raise RuntimeError('Optimized Python disables acceptance assertions; run without -O or PYTHONOPTIMIZE')
p=argparse.ArgumentParser();p.add_argument('--assets',type=Path,required=True);p.add_argument('--clearance',type=Path,required=True);p.add_argument('--inventory',type=Path,required=True);p.add_argument('--backup-root',type=Path,required=True);p.add_argument('--notes',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);p.add_argument('--phase',choices=['check','pause','delete','upload','publish','verify'],default='check');a=p.parse_args()
REPO='huwany1/MDcard';API='https://api.github.com/repos/'+REPO;TAG='v0.1.0';a.evidence.mkdir(parents=True,exist_ok=True)
PENDING_NOTE='历史应用压缩包已删除；本发布记录和标签继续保留。0.1.0 的上传与公开下载验收正在进行，完成前暂停应用下载，请以官网实际下载状态为准。'
PAUSE_NOTE='MDcard 0.1.0 发行替换与校验进行中，当前正式应用下载入口暂停。以下旧应用资产不再作为当前发行入口，待下一阶段实际移除；本发布记录和历史标签继续保留。'
COMPLETE_NOTE='旧应用压缩包已由 MDcard 0.1.0 取代；本发布记录和历史标签继续保留。最新发行：https://github.com/'+REPO+'/releases/tag/v0.1.0 。下载校验结果以最新发行验收报告为准。'
def sha(path):
 with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def save(name,value):
 path=a.evidence/name;temp=path.with_suffix('.tmp');temp.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n',encoding='utf8');os.replace(temp,path)
def record(action,**details):
 with (a.evidence/'actions.jsonl').open('a',encoding='utf8') as f:f.write(json.dumps({'at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'phase':a.phase,'action':action,**details},ensure_ascii=False)+'\n')
clear=json.loads(a.clearance.read_text('utf-8-sig'));index=json.loads((a.assets/'MDcard-0.1.0-release.json').read_text('utf8'));inventory=json.loads(a.inventory.read_text('utf8'))
names=['MDcard-0.1.0-windows-x64.zip','MDcard-0.1.0-linux-x64.tar.gz','SHA256SUMS.txt','MDcard-0.1.0-release.json']
assert clear['localAcceptancePassed'] is True
assert all(clear['criteria'][key]['status']=='通过' for key in ['C02','C03','C04','C05'])
assert clear['windowsHost']['passed'] is True and clear['cleanLinux']['passed'] is True
assert clear['windowsHost']['environmentType'] == 'existing-windows-host'
assert index['version']=='0.1.0' and index['sourceCommit']==clear['sourceCommit']
frozen={name:{'bytes':(a.assets/name).stat().st_size,'sha256':sha(a.assets/name)} for name in names}
assert frozen==clear['frozenAssets'],'Assets changed after local clearance'
for line in (a.assets/'SHA256SUMS.txt').read_text('utf8').splitlines():
 digest,name=line.split(None,1);name=name.strip();assert name in frozen and frozen[name]['sha256']==digest
assert {line.split(None,1)[1].strip() for line in (a.assets/'SHA256SUMS.txt').read_text('utf8').splitlines()}==set(names)-{'SHA256SUMS.txt'}
for item in inventory:
 if item['applicationArchive'] or item['boundVerificationAsset']:
  backup=(a.backup_root/item['localBackup']).resolve();assert backup.is_relative_to(a.backup_root.resolve()) and backup.is_file();assert backup.stat().st_size==item['bytes'] and sha(backup)==item['sha256']
credential=subprocess.run(['git','-c','credential.interactive=never','credential','fill'],input='protocol=https\nhost=github.com\nusername=hub-default-github\n\n',capture_output=True,text=True,timeout=30,env={**os.environ,'GIT_TERMINAL_PROMPT':'0','GCM_INTERACTIVE':'never'})
if credential.returncode:raise RuntimeError('Stored credential unavailable')
token=dict(line.split('=',1) for line in credential.stdout.splitlines() if '=' in line).get('password');assert token
s=requests.Session();s.headers.update({'Authorization':'Bearer '+token,'Accept':'application/vnd.github+json','X-GitHub-Api-Version':'2022-11-28','User-Agent':'MDcard-release-0.1.0'})
def request(method,url,**kwargs):
 r=s.request(method,url,timeout=(30,600),**kwargs)
 if r.status_code>=400:raise RuntimeError('GitHub '+method+' returned '+str(r.status_code))
 return r
def get(path):return request('GET',API+path).json()
def paged(path):
 result=[]
 for page in range(1,10000):
  batch=get(path+('?' if '?' not in path else '&')+'per_page=100&page='+str(page));result.extend(batch)
  if len(batch)<100:return result
 raise RuntimeError('Unexpected pagination limit')
assert request('GET','https://api.github.com/user').json()['login'].lower()=='huwany1'
assert get('/commits/'+clear['sourceCommit'])['sha']==clear['sourceCommit'],'Frozen public source is not accessible'
public_source=requests.get(API+'/commits/'+clear['sourceCommit'],headers={'Accept':'application/vnd.github+json','User-Agent':'MDcard-public-source-verification'},timeout=60)
assert public_source.status_code==200 and public_source.json().get('sha')==clear['sourceCommit'],'Frozen source is not anonymously accessible'
save('public-source-access.json',{'sourceCommit':clear['sourceCommit'],'url':'https://github.com/'+REPO+'/tree/'+clear['sourceCommit'],'anonymousHTTPStatus':public_source.status_code})
historical_tags=get('/git/matching-refs/tags/');historical_tags={item['ref']:item['object']['sha'] for item in historical_tags}
tag_response=s.get(API+'/git/ref/tags/'+TAG,timeout=30)
if tag_response.status_code==404:tag=None
elif tag_response.ok:tag=tag_response.json()
else:raise RuntimeError('Cannot inspect existing tag: '+str(tag_response.status_code))
if tag:
 target=tag['object']
 while target['type']=='tag':target=request('GET',target['url']).json()['object']
 if target['sha']!=clear['sourceCommit']:raise RuntimeError('v0.1.0 tag conflict: '+target['sha']+' != '+clear['sourceCommit'])
releases=paged('/releases');all_assets=[{**asset,'releaseId':release['id'],'releaseTag':release['tag_name']} for release in releases for asset in paged('/releases/'+str(release['id'])+'/assets')]
save(a.phase+'-before.json',{'releases':releases,'assets':all_assets,'frozenAssets':frozen})
target_release=next((release for release in releases if release['tag_name']==TAG),None)
old_ids={item['id'] for item in inventory if item['applicationArchive'] or item['boundVerificationAsset']}
unexpected=[item for item in all_assets if re.search(r'^MDcard.*\.(zip|tar\.gz|7z)$',item['name'],re.I) and item['id'] not in old_ids and not(target_release and item['releaseId']==target_release['id'] and item['name'] in frozen)]
assert not unexpected,'New old application assets appeared; inventory and back up before deletion'
if a.phase in ['delete','upload','publish']:
 assert clear['publicEntriesUnavailableVerified'] is True,'Public entry transition has not been independently verified'
 assert clear['publicUnavailableEvidence'],'Missing public transition evidence'
if a.phase=='pause':
 # This phase requires frozen local acceptance, but deliberately does not
 # require a public-unavailable clearance that can only exist after this patch.
 for release in releases:
  old=[item for item in inventory if item['releaseId']==release['id'] and item['id'] in old_ids]
  if not old or release['tag_name']==TAG:continue
  body=release.get('body') or ''
  if body.startswith(PAUSE_NOTE):continue
  for item in old:
   url=re.escape(item['url'])
   body=re.sub(r'\[([^\]]*)\]\('+url+r'(?:\s+"[^"]*")?\)',r'\1（旧下载入口暂停）',body)
   body=re.sub(r'<a\b[^>]*href=[\"\x27]'+url+r'[\"\x27][^>]*>(.*?)</a>',r'\1（旧下载入口暂停）',body,flags=re.I|re.S)
   body=body.replace(item['url'],'（旧下载入口暂停）')
  body=PAUSE_NOTE+'\n\n<details>\n<summary>历史说明，仅代表原发行时状态</summary>\n\n'+body+'\n\n</details>\n'
  request('PATCH',API+'/releases/'+str(release['id']),json={'body':body});record('historical-release-paused',releaseId=release['id'])
elif a.phase=='delete':
 for release in releases:
  if any(item['releaseId']==release['id'] for item in inventory if item['id'] in old_ids):
   assert PAUSE_NOTE in (release.get('body') or '') or PENDING_NOTE in (release.get('body') or ''),'Pause and independently verify historical release entries before deletion'
 for item in all_assets:
  if item['id'] not in old_ids:continue
  expected=next(old for old in inventory if old['id']==item['id'])
  assert item['releaseId']==expected['releaseId'] and item['name']==expected['name'] and item['size']==expected['bytes'] and item.get('digest')=='sha256:'+expected['sha256'],'Remote asset identity drifted after rollback inventory'
  record('delete-start',id=item['id'],name=item['name'],releaseId=item['releaseId'])
  request('DELETE',API+'/releases/assets/'+str(item['id']));record('delete-success',id=item['id'],name=item['name'])
 # Keep each historical release and its identity, while removing stale downloads.
 for release in releases:
  old=[item for item in inventory if item['releaseId']==release['id'] and item['id'] in old_ids]
  if not old:continue
  body=release.get('body') or ''
  for item in old:body=body.replace(item['url'],'https://github.com/'+REPO+'/releases/tag/v0.1.0')
  note=PENDING_NOTE
  if PAUSE_NOTE in body:body=body.replace(PAUSE_NOTE,note)
  elif note not in body:body=note+'\n\n'+body
  request('PATCH',API+'/releases/'+str(release['id']),json={'body':body});record('historical-release-downloads-updated',releaseId=release['id'])
elif a.phase=='upload':
 remaining=[item for item in all_assets if item['id'] in old_ids];assert not remaining,'Old application assets remain; finish deletion first'
 if not tag:request('POST',API+'/git/refs',json={'ref':'refs/tags/'+TAG,'sha':clear['sourceCommit']});record('tag-created',tag=TAG,sourceCommit=clear['sourceCommit'])
 if not target_release:
  target_release=request('POST',API+'/releases',json={'tag_name':TAG,'target_commitish':clear['sourceCommit'],'name':'MDcard 0.1.0','draft':True,'prerelease':False,'body':a.notes.read_text('utf8')}).json();record('draft-created',releaseId=target_release['id'])
 for name in names:
  expected=frozen[name];existing=next((asset for asset in paged('/releases/'+str(target_release['id'])+'/assets') if asset['name']==name),None)
  if existing and existing['size']==expected['bytes'] and existing.get('digest')=='sha256:'+expected['sha256']:
   record('upload-already-present',id=existing['id'],name=name);continue
  if existing:
   assert target_release['draft'],'Refusing replacement of a nonmatching public final asset'
   request('DELETE',API+'/releases/assets/'+str(existing['id']));record('failed-draft-asset-removed',id=existing['id'],name=name)
  uploaded=False
  for attempt in range(1,9):
   try:
    with (a.assets/name).open('rb') as stream:
     asset=request('POST',target_release['upload_url'].split('{',1)[0],params={'name':name},headers={'Content-Type':'application/octet-stream'},data=stream).json()
    assert asset['state']=='uploaded' and asset['size']==expected['bytes'] and asset.get('digest')=='sha256:'+expected['sha256']
    record('upload-success',id=asset['id'],name=name,bytes=asset['size'],digest=asset.get('digest'));uploaded=True;break
   except (requests.RequestException,RuntimeError,AssertionError) as error:
    message=re.sub(r'https?://[^\s]+','<URL>',str(error))[:500]
    response=getattr(error,'response',None)
    record('upload-failed',name=name,attempt=attempt,error=type(error).__name__,reason=message,httpStatus=getattr(response,'status_code',None))
    partial=next((asset for asset in paged('/releases/'+str(target_release['id'])+'/assets') if asset['name']==name),None)
    record('upload-partial-observed',name=name,attempt=attempt,partial={key:partial.get(key) for key in ['id','state','size','digest']} if partial else None)
    if partial:
     if partial['size']==expected['bytes'] and partial.get('digest')=='sha256:'+expected['sha256']:uploaded=True;record('upload-recovered',id=partial['id'],name=name);break
     assert target_release['draft'];request('DELETE',API+'/releases/assets/'+str(partial['id']));record('failed-upload-cleaned',id=partial['id'],name=name)
    time.sleep(min(30,attempt*3))
  if not uploaded:raise RuntimeError('Upload remains incomplete: '+name+'; public entries must remain unavailable')
elif a.phase=='publish':
 assert target_release is not None
 assets=paged('/releases/'+str(target_release['id'])+'/assets');assert {asset['name'] for asset in assets}==set(names)
 for asset in assets:assert asset['size']==frozen[asset['name']]['bytes'] and asset.get('digest')=='sha256:'+frozen[asset['name']]['sha256']
 request('PATCH',API+'/releases/'+str(target_release['id']),json={'draft':False,'prerelease':False,'name':'MDcard 0.1.0','body':a.notes.read_text('utf8'),'make_latest':'true'});record('published-as-latest',releaseId=target_release['id'])
 for historical in releases:
  if historical['tag_name']==TAG or PENDING_NOTE not in (historical.get('body') or ''):continue
  request('PATCH',API+'/releases/'+str(historical['id']),json={'body':historical['body'].replace(PENDING_NOTE,COMPLETE_NOTE)});record('historical-release-completed-note',releaseId=historical['id'])
after_releases=paged('/releases');after=[{**asset,'releaseId':release['id'],'releaseTag':release['tag_name']} for release in after_releases for asset in paged('/releases/'+str(release['id'])+'/assets')]
assert {release['id'] for release in releases}.issubset({release['id'] for release in after_releases}),'Historical Release record disappeared'
tags_after={item['ref']:item['object']['sha'] for item in get('/git/matching-refs/tags/')}
assert all(tags_after.get(ref)==digest for ref,digest in historical_tags.items()),'Historical tag changed or disappeared'
old_remaining=[asset for asset in after if asset['id'] in old_ids];new=[asset for asset in after if asset['releaseTag']==TAG]
result={'phase':a.phase,'oldAssetIdsRemaining':[asset['id'] for asset in old_remaining],'newAssets':new,'frozenAssets':frozen,'tag':TAG,'sourceCommit':clear['sourceCommit']}
if a.phase in ['delete','upload','publish','verify']:assert not old_remaining
if a.phase in ['upload','publish','verify']:
 assert {asset['name'] for asset in new}==set(names)
 for asset in new:assert asset['size']==frozen[asset['name']]['bytes'] and asset.get('digest')=='sha256:'+frozen[asset['name']]['sha256']
if a.phase in ['publish','verify']:assert get('/releases/latest')['tag_name']==TAG
save(a.phase+'-after.json',result);record('phase-complete');print(json.dumps({'phase':a.phase,'oldRemaining':len(old_remaining),'newAssets':len(new)}))
