# SPDX-License-Identifier: AGPL-3.0-only
"""Receive only the disposable guest's explicit COM1 evidence ZIP."""
import base64,io,json,shutil,zipfile
from pathlib import Path
root=Path(__file__).resolve().parent.parent;qa=root/'.local/windows-clean-qa';out=root/'evidence/release-015/windows-clean-guest'
serial=max(qa.glob('serial*.log'),key=lambda p:p.stat().st_mtime)
raw=serial.read_bytes();begin=b'MDCARD_QA_ZIP_BEGIN';end=b'MDCARD_QA_ZIP_END'
if begin not in raw or end not in raw:
 recent=[s for s in raw.decode(errors='replace').splitlines() if 'MDCARD_' not in s][-4:]
 print(json.dumps({'complete':False,'serialBytes':len(raw),'recent':recent}));raise SystemExit(0)
last_end=raw.rfind(end);last_begin=raw.rfind(begin,0,last_end)
encoded=raw[last_begin+len(begin):last_end];data=base64.b64decode(b''.join(encoded.split()),validate=True)
target=out/'guest-evidence';target.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(io.BytesIO(data)) as z:
 if z.testzip():raise RuntimeError('Guest ZIP CRC failed')
 for name in z.namelist():
  p=Path(name)
  if p.is_absolute() or '..' in p.parts or ':' in name:raise RuntimeError('Unsafe guest evidence member')
 z.extractall(target)
(out/'guest-evidence.zip').write_bytes(data)
for name in ['acquisition.json','inputs-head.json','windowsISO-verified.json','qemuInstaller-verified.json','guest-process.json','guest-process-r2.json','guest-process-whpx.json','guest-process-whpx-r2.json','qemu.stderr.log','qemu-r2.stderr.log','qemu-whpx.stderr.log','qemu-whpx-r2.stderr.log']:
 if (qa/name).exists():shutil.copy2(qa/name,out/name)
print(json.dumps({'complete':True,'result':json.loads((target/'result.json').read_text(encoding='utf-8-sig'))},ensure_ascii=False))
