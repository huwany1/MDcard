"""Run Linux strip on a copied file inside an explicitly mapped Docker root."""
from pathlib import Path
import argparse,subprocess
p=argparse.ArgumentParser();p.add_argument('--container',required=True);p.add_argument('--mount-root',type=Path,required=True);p.add_argument('--guest-root',required=True);p.add_argument('path',type=Path);a=p.parse_args()
root=a.mount_root.resolve();path=a.path.resolve()
if not path.is_file() or not path.is_relative_to(root):raise RuntimeError('Copied ELF must be inside the declared mount')
guest=a.guest_root.rstrip('/')+'/'+path.relative_to(root).as_posix()
subprocess.run(['docker','exec',a.container,'strip','--strip-unneeded','--',guest],check=True)
