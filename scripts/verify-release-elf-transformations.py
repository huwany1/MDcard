"""External evidence for removing only non-runtime ELF symbols in fresh staging."""
from pathlib import Path
import argparse,hashlib,json,struct,subprocess
p=argparse.ArgumentParser();p.add_argument('--before',type=Path,required=True);p.add_argument('--after',type=Path,required=True);p.add_argument('--container',required=True);p.add_argument('--mount-root',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def sections(path):
 raw=path.read_bytes();offset=struct.unpack_from('<Q',raw,40)[0];count=struct.unpack_from('<H',raw,60)[0];entry=struct.unpack_from('<H',raw,58)[0];strings=struct.unpack_from('<H',raw,62)[0];items=[struct.unpack_from('<IIQQQQIIQQ',raw,offset+i*entry) for i in range(count)];s=items[strings];names=raw[s[4]:s[4]+s[5]]
 return [{'name':names[s[0]:].split(b'\0',1)[0].decode(),'type':s[1],'flags':s[2],'address':s[3],'bytes':s[5],'sha256':hashlib.sha256(raw[s[4]:s[4]+s[5]]).hexdigest() if s[1]!=8 else None} for s in items]
def tool(*args):return subprocess.check_output(['docker','exec',a.container,*args])
def guest(path):return '/work/'+path.resolve().relative_to(a.mount_root.resolve()).as_posix()
proof=[]
for after in sorted(a.after.rglob('*')):
 if not after.is_file():continue
 with after.open('rb') as f:magic=f.read(6)
 if magic!=b'\x7fELF\x02\x01':continue
 rel=after.relative_to(a.after).as_posix();before=a.before/rel
 b=sections(before);c=sections(after);allocated_before=[s for s in b if s['flags']&2];allocated_after=[s for s in c if s['flags']&2]
 assert allocated_before==allocated_after,rel
 dynamic=[]
 for option,label in [('--dynamic','dynamic'),('--dyn-syms','dynsym')]:
  left=tool('readelf',option,'--wide',guest(before));right=tool('readelf',option,'--wide',guest(after));assert left==right,(rel,label)
  dynamic.append({'kind':label,'bytes':len(left),'sha256':hashlib.sha256(left).hexdigest(),'identical':True})
 changed=sha(before)!=sha(after)
 proof.append({'path':rel,'before':{'bytes':before.stat().st_size,'sha256':sha(before)},'after':{'bytes':after.stat().st_size,'sha256':sha(after)},'changed':changed,'allAllocatedSectionsIdentical':True,'allocatedSections':allocated_after,'removedNonallocatedSections':[s for s in b if s['name'] not in {v['name'] for v in c}],'dynamicReadelfComparison':dynamic,'noDebugOrStaticSymbols':not any(s['type']==2 or s['name'].startswith(('.debug','.zdebug')) for s in c)})
result={'passed':all(x['allAllocatedSectionsIdentical'] and x['noDebugOrStaticSymbols'] for x in proof),'command':'GNU strip --strip-unneeded -- COPIED_STAGING_FILE','stripVersion':tool('strip','--version').decode().splitlines()[0],'readelfVersion':tool('readelf','--version').decode().splitlines()[0],'elfFiles':len(proof),'changedFiles':sum(x['changed'] for x in proof),'savedBytes':sum(x['before']['bytes']-x['after']['bytes'] for x in proof),'proof':proof}
(a.output/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf8');print(json.dumps({k:v for k,v in result.items() if k!='proof'}))
