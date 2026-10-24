# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Clear only the verified mdcard-dev /work/target mount, retaining the mount."""
from pathlib import Path
import datetime, json, os, shutil, stat

root=Path('/work');target=root/'target'
out=root/'evidence/cleanup-icon-suite-20261024/docker-result.json'
if out.exists():raise FileExistsError('Preserve the original cleanup evidence')
if root.resolve()!=Path('/work') or target.resolve()!=Path('/work/target') or not os.path.ismount(target):
    raise RuntimeError('Expected the explicit /work/target mount')
if 'name = "mdcard"' not in (root/'Cargo.toml').read_text():
    raise RuntimeError('Not the MDcard workspace')
active=[]
for process in Path('/proc').iterdir():
    if not process.name.isdigit():continue
    try:
        state=(process/'stat').read_text().split()[2]
        name=(process/'comm').read_text().strip()
        if state!='Z' and name in {'cargo','rustc','rustdoc','mdcard'}:active.append({'pid':process.name,'comm':name})
    except FileNotFoundError:pass
if active:raise RuntimeError('Active workspace build or application: '+json.dumps(active))
files=size=links=0;pending=[target]
while pending:
    current=pending.pop()
    for child in current.iterdir():
        info=child.lstat()
        if stat.S_ISLNK(info.st_mode):links+=1
        elif stat.S_ISDIR(info.st_mode):pending.append(child)
        else:files+=1;size+=info.st_size
started=datetime.datetime.now(datetime.timezone.utc).isoformat()
for child in target.iterdir():
    if child.is_symlink() or not child.is_dir():child.unlink()
    else:shutil.rmtree(child)
if any(target.iterdir()) or not os.path.ismount(target):raise RuntimeError('Cache not empty or mount lost')
result={'complete':True,'target':'/work/target','volume':'mdcard-target','deleted_files':files,'deleted_bytes':size,'deleted_links':links,'mount_retained':True,'cargo_download_cache_retained':Path('/usr/local/cargo/registry').is_dir(),'started_utc':started,'finished_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'boundary':'Logical file bytes; Docker virtual disk is not claimed to shrink immediately.'}
out.write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result,indent=2))
