# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Prepare locked offline renderer assets; downloads are verified before installation.
Node >=20 + npm are required to build browser.js. Java >=17 and a local Chromium
are runtime dependencies, deliberately not silently downloaded by the app.
"""
import argparse,hashlib,json,os,shutil,subprocess,tarfile,tempfile,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser();parser.add_argument('--proxy');parser.add_argument('--reuse-npm',action='store_true');args=parser.parse_args()
spec=json.loads((ROOT/'renderer/runtime-sources.json').read_text());artifacts=ROOT/'artifacts/rich-downloads';artifacts.mkdir(parents=True,exist_ok=True)
opener=urllib.request.build_opener(urllib.request.ProxyHandler({'https':args.proxy,'http':args.proxy}) if args.proxy else urllib.request.ProxyHandler())
def sha(path):return hashlib.file_digest(path.open('rb'),'sha256').hexdigest()
def obtain(record,destination,candidate=None):
 destination.parent.mkdir(parents=True,exist_ok=True)
 if destination.exists() and sha(destination)==record['sha256']:return
 if candidate and candidate.exists() and sha(candidate)==record['sha256']:shutil.copyfile(candidate,destination);return
 fd,name=tempfile.mkstemp(prefix='verified-download-',dir=destination.parent);os.close(fd);temporary=Path(name)
 try:
  with opener.open(record['url'],timeout=180) as response,temporary.open('wb') as output:shutil.copyfileobj(response,output)
  if sha(temporary)!=record['sha256']:raise RuntimeError('SHA256 mismatch: '+record['url'])
  os.replace(temporary,destination)
 finally:temporary.unlink(missing_ok=True)
runtime=ROOT/'renderer/runtime';runtime.mkdir(parents=True,exist_ok=True)
obtain(spec['plantuml'],ROOT/'renderer/vendor/plantuml.jar',ROOT/'renderer/vendor/plantuml.jar')
node=shutil.which('node');obtain(spec['nodeWindows'],runtime/'node.exe',Path(node) if node and os.name=='nt' else None)
archive=artifacts/'node-linux.tar.xz';obtain(spec['nodeLinux'],archive,ROOT/'artifacts/node-linux/node.tar.xz')
# Extract only two explicitly named regular members, never archive-controlled paths.
with tarfile.open(archive,'r:xz') as tar:
 for member_name,destination in [('node-v24.21.0-linux-x64/bin/node',runtime/'node'),('node-v24.21.0-linux-x64/LICENSE',ROOT/'renderer/Node-LICENSE.txt')]:
  member=tar.getmember(member_name)
  if not member.isfile():raise RuntimeError('Unexpected non-regular runtime member')
  with tar.extractfile(member) as source,destination.open('wb') as output:shutil.copyfileobj(source,output)
(runtime/'node').chmod(0o755)
if not args.reuse_npm:subprocess.run(['npm.cmd' if os.name=='nt' else 'npm','ci','--force','--ignore-scripts','--no-fund','--no-audit'],cwd=ROOT/'renderer',check=True)
subprocess.run([node or 'node','build.mjs'],cwd=ROOT/'renderer',check=True)
license_text=subprocess.check_output([os.environ.get('MDCARD_JAVA','java'),'-jar',str(ROOT/'renderer/vendor/plantuml.jar'),'-license'])
(ROOT/'renderer/PlantUML-MIT-LICENSE.txt').write_bytes(license_text)
subprocess.run([node or 'node','licenses.mjs'],cwd=ROOT/'renderer',check=True)
print('Prepared verified Node 24.21.0, PlantUML MIT, locked local JS engines, fonts and notices.')
