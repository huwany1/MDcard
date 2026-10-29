# SPDX-License-Identifier: AGPL-3.0-only
"""Build one Debian 12 amd64 package. Run on Linux; no maintainer script downloads."""
import argparse,hashlib,json,os,shutil,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('payload',type=Path);p.add_argument('output',type=Path);p.add_argument('--compression',choices=['xz','gzip'],default='xz');a=p.parse_args()
manifest=json.loads((a.payload/'manifest.json').read_text());assert manifest['platform']=='linux'
version=manifest['version'];a.output.mkdir(parents=True,exist_ok=True)
temporary=tempfile.TemporaryDirectory(prefix='mdcard-deb-');stage=Path(temporary.name)
shutil.copytree(a.payload,stage/'opt/mdcard')
# Normalize permissions on the Linux staging filesystem, independently of a
# Windows bind mount's synthetic modes or a later manifest metadata rewrite.
for entry in (stage/'opt/mdcard').rglob('*'):
 if entry.is_dir():entry.chmod(0o755)
 elif entry.is_file():
  with entry.open('rb') as handle:is_executable=handle.read(4)==b'\x7fELF'
  entry.chmod(0o755 if is_executable else 0o644)
(stage/'opt/mdcard').chmod(0o755)
def write(name,data,mode=0o644):
 path=stage/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(data);path.chmod(mode)
write('usr/bin/mdcard','#!/bin/sh\nexec /opt/mdcard/mdcard "$@"\n',0o755)
write('usr/share/applications/mdcard.desktop',(root/'packaging/linux/mdcard.desktop').read_text())
icon=stage/'usr/share/icons/hicolor/scalable/apps/mdcard.svg';icon.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(root/'assets/branding/mdcard-icon/svg/mdcard-color.svg',icon)
icon.chmod(0o644)
write('usr/share/doc/mdcard/copyright',(root/'LICENSE').read_text()+'\nThird-party licenses: /opt/mdcard/LICENSES.txt and /opt/mdcard/licenses\n')
deps='libc6 (>= 2.36), libstdc++6, libgcc-s1, libx11-6, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libxcursor1, libxrandr2, libxi6, libgl1, libegl1, libgl1-mesa-dri, libvulkan1, mesa-vulkan-drivers, libnss3, libnspr4, libatk1.0-0, libatk-bridge2.0-0, libatspi2.0-0, libcups2, libdrm2, libdbus-1-3, libgbm1, libasound2, libpango-1.0-0, libcairo2, libfontconfig1, libfreetype6, xdg-desktop-portal, xdg-desktop-portal-gtk'
size=sum((f.stat().st_size+1023)//1024 for f in stage.rglob('*') if f.is_file())
write('DEBIAN/control',f'Package: mdcard\nVersion: {version}\nArchitecture: amd64\nMaintainer: MDcard contributors <mdcard@users.noreply.github.com>\nSection: editors\nPriority: optional\nInstalled-Size: {size}\nDepends: {deps}\nHomepage: https://github.com/huwany1/MDcard\nDescription: Local Markdown editor with complete offline rendering and exports\n Includes Node, Java, Chromium, Pandoc, fonts and third-party licenses.\n User documents, preferences and crash recovery remain in the user profile.\n')
# Guard both first upgrades from the previous package without hooks and future
# upgrades/removals. Rollback/repair callback parameters remain retryable.
guard=(root/'packaging/linux/check-running.sh').read_text()
backup=(root/'packaging/linux/upgrade-backup.sh').read_text()
paths=[e['path'] for e in manifest['files']]+['manifest.json']
for name in paths:
 if '\n' in name or '\r' in name or name.startswith('/') or '..' in name.split('/'):raise RuntimeError('Unsafe package member')
checks=''.join(e['sha256']+'  '+e['path']+'\n' for e in manifest['files'])+hashlib.file_digest((a.payload/'manifest.json').open('rb'),'sha256').hexdigest()+'  manifest.json\n'
write('usr/share/mdcard/payload.sha256',checks)
functions='mdcard_closed() {\n'+guard.removesuffix('exit 0\n')+'\n}\n'+backup+'\nmdcard_new_files() {\ncat <<\'MDCARD_PRODUCT_FILES\'\n'+'\n'.join(paths)+'\nMDCARD_PRODUCT_FILES\n}\n'
write('DEBIAN/preinst','#!/bin/sh\nset -eu\n'+functions+'\ncase "${1-}" in --mdcard-restore) mdcard_closed; mdcard_backup restore ;; install|upgrade) mdcard_closed; mdcard_backup prepare ;; esac\n',0o755)
write('DEBIAN/prerm','#!/bin/sh\nset -eu\ncase "${1-}" in remove|upgrade) ;; *) exit 0 ;; esac\n'+guard,0o755)
write('DEBIAN/postinst','#!/bin/sh\nset -eu\n'+functions+'\ncase "${1-}" in configure) mdcard_backup commit ;; esac\n',0o755)
write('DEBIAN/postrm','#!/bin/sh\nset -eu\n'+functions+'\ncase "${1-}" in abort-upgrade|failed-upgrade|abort-install) mdcard_closed; mdcard_backup restore ;; esac\n',0o755)
# dpkg owns only immutable application files; no user-home deletion, no process
# termination, no network activity, no developer-tool dependencies.
target=a.output.resolve()/f'mdcard_{version}_amd64.deb'
subprocess.run(['dpkg-deb','--root-owner-group','--threads-max=2','-Z'+a.compression,'-z6','--build',str(stage),str(target)],check=True)
print(target)
temporary.cleanup()
