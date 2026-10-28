# SPDX-License-Identifier: AGPL-3.0-only
"""Prepare an isolated Linux export runtime without writing the mounted daily app."""
import hashlib
import io
import json
from pathlib import Path
import shutil
import tarfile
import urllib.request

repo=Path(__file__).resolve().parents[1]
base=Path('/tmp/mdcard-ui-runtime');root=base/'renderer';root.mkdir(parents=True,exist_ok=True)
for file in (repo/'renderer').iterdir():
    if file.is_file():shutil.copy2(file,root/file.name)
for name,target in [('node_modules','/opt/mdcard-renderer/node_modules'),('vendor','/opt/mdcard-renderer/vendor')]:
    link=root/name
    if not link.exists():link.symlink_to(target,target_is_directory=True)
if not (base/'assets').exists():(base/'assets').symlink_to(repo/'assets',target_is_directory=True)
(root/'runtime/linux').mkdir(parents=True,exist_ok=True)
if not (root/'runtime/linux/chromium').exists():(root/'runtime/linux/chromium').symlink_to('/opt/mdcard-renderer/runtime/linux/chromium',target_is_directory=True)
sources=json.loads((repo/'renderer/runtime-sources.json').read_text())
node=root/'runtime/node'
if not node.exists():
    spec=sources['nodeLinux'];payload=urllib.request.urlopen(spec['url'],timeout=60).read()
    assert hashlib.sha256(payload).hexdigest()==spec['sha256'],'Pinned Node archive hash mismatch'
    with tarfile.open(fileobj=io.BytesIO(payload),mode='r:xz') as archive:
        member=next(x for x in archive.getmembers() if x.name.endswith('/bin/node'))
        node.write_bytes(archive.extractfile(member).read());node.chmod(0o755)
result=dict(root=str(root),node_version=sources['nodeVersion'],source_archive_sha256=sources['nodeLinux']['sha256'],node_binary_sha256=hashlib.sha256(node.read_bytes()).hexdigest(),source='unchanged worktree renderer sources; read-only shared dependency/browser inputs; private task runtime')
(repo/'evidence/ui-ux/2026-10-28/export-runtime.json').write_text(json.dumps(result,indent=2))
print(root)
