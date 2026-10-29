# SPDX-License-Identifier: AGPL-3.0-only
"""Build one Debian 12 amd64 package. Run on Linux; no maintainer script downloads."""
import argparse,json,os,shutil,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('payload',type=Path);p.add_argument('output',type=Path);p.add_argument('--compression',choices=['xz','gzip'],default='xz');a=p.parse_args()
manifest=json.loads((a.payload/'manifest.json').read_text());assert manifest['platform']=='linux'
version=manifest['version'];a.output.mkdir(parents=True,exist_ok=True)
temporary=tempfile.TemporaryDirectory(prefix='mdcard-deb-');stage=Path(temporary.name)
shutil.copytree(a.payload,stage/'opt/mdcard')
def write(name,data,mode=0o644):
 path=stage/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(data);path.chmod(mode)
write('usr/bin/mdcard','#!/bin/sh\nexec /opt/mdcard/mdcard "$@"\n',0o755)
write('usr/share/applications/mdcard.desktop',(root/'packaging/linux/mdcard.desktop').read_text())
icon=stage/'usr/share/icons/hicolor/scalable/apps/mdcard.svg';icon.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(root/'assets/branding/mdcard-icon/svg/mdcard-color.svg',icon)
write('usr/share/doc/mdcard/copyright',(root/'LICENSE').read_text()+'\nThird-party licenses: /opt/mdcard/LICENSES.txt and /opt/mdcard/licenses\n')
deps='libc6 (>= 2.36), libstdc++6, libgcc-s1, libx11-6, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libxcursor1, libxrandr2, libxi6, libgl1, libegl1, libgl1-mesa-dri, libvulkan1, mesa-vulkan-drivers, libnss3, libnspr4, libatk1.0-0, libatk-bridge2.0-0, libatspi2.0-0, libcups2, libdrm2, libdbus-1-3, libgbm1, libasound2, libpango-1.0-0, libcairo2, libfontconfig1, libfreetype6, xdg-desktop-portal, xdg-desktop-portal-gtk'
size=sum((f.stat().st_size+1023)//1024 for f in stage.rglob('*') if f.is_file())
write('DEBIAN/control',f'Package: mdcard\nVersion: {version}\nArchitecture: amd64\nMaintainer: MDcard contributors <mdcard@users.noreply.github.com>\nSection: editors\nPriority: optional\nInstalled-Size: {size}\nDepends: {deps}\nHomepage: https://github.com/huwany1/MDcard\nDescription: Local Markdown editor with complete offline rendering and exports\n Includes Node, Java, Chromium, Pandoc, fonts and third-party licenses.\n User documents, preferences and crash recovery remain in the user profile.\n')
# Guard both first upgrades from the previous package without hooks and future
# upgrades/removals. Rollback/repair callback parameters remain retryable.
guard=(root/'packaging/linux/check-running.sh').read_text()
for hook,operations in [('preinst','install|upgrade'),('prerm','remove|upgrade')]:
 write('DEBIAN/'+hook,'#!/bin/sh\nset -eu\ncase "${1-}" in '+operations+') ;; *) exit 0 ;; esac\n'+guard,0o755)
# dpkg owns only immutable application files; no user-home deletion, no process
# termination, no network activity, no developer-tool dependencies.
target=a.output.resolve()/f'mdcard_{version}_amd64.deb'
subprocess.run(['dpkg-deb','--root-owner-group','--threads-max=2','-Z'+a.compression,'-z6','--build',str(stage),str(target)],check=True)
print(target)
temporary.cleanup()
