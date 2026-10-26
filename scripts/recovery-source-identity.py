# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Fingerprint runtime source inputs and immutable local recovery candidates."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--candidate', type=Path)
args = parser.parse_args()
paths = [root / name for name in ('Cargo.toml', 'Cargo.lock', 'build.rs') if (root / name).is_file()]
for folder in ('src', 'locales', 'assets', 'vendor'):
    paths.extend(p for p in (root / folder).rglob('*') if p.is_file())
files = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths, key=lambda p: p.relative_to(root).as_posix())}
canonical = ''.join(f'{name}\0{digest}\n' for name, digest in files.items()).encode()
result = {
    'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'runtime_source_sha256': hashlib.sha256(canonical).hexdigest(),
    'fingerprint_scope': 'Cargo.toml, Cargo.lock, build.rs if present, all files in src/locales/assets/vendor; excludes docs, scripts and generated artifacts',
    'files_sha256': files,
    'worktree_status': subprocess.check_output(['git', 'status', '--short'], cwd=root, text=True),
}
if args.candidate:
    candidate = args.candidate if args.candidate.is_absolute() else root / args.candidate
    result['executables'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(candidate.glob('*.exe'))}
    result['builds'] = {'mdcard.exe': 'cargo build --locked --release --target x86_64-pc-windows-gnu', 'mdcard-faults.exe': 'same plus --features acceptance-faults; table patch failure requires explicit environment flag'}
    (candidate / 'candidate-manifest.json').write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps({k: v for k, v in result.items() if k not in ('files_sha256', 'worktree_status')}, ensure_ascii=False, indent=2))
