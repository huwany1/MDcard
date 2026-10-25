# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Build a complete Windows icon-suite candidate from a verified accepted runtime."""
from __future__ import annotations

import argparse
import concurrent.futures
import datetime
import hashlib
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import tomllib
import zipfile
from license_payload import copy_project_licenses

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--windows-exe', required=True)
parser.add_argument('--binary-source-commit', required=True)
parser.add_argument('--build-id', default='icon-suite-20261024-r3')
parser.add_argument('--base-stage', default='dist/windows')
parser.add_argument('--output-root', default='artifacts/icon-integration-20261024/packages')
parser.add_argument('--compiler-deps', default='artifacts/icon-integration-20261024/implementation/compiler-deps.d')
parser.add_argument('--binary-input-record', help='Preserved actual input record of the unchanged product executable')
args = parser.parse_args()
if not args.build_id or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in args.build_id):
    raise ValueError('Use a simple build ID')


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def git(*arguments):
    return subprocess.check_output(['git', *arguments], cwd=ROOT).decode('utf-8').strip()


def inventory(path, stage):
    return {'path': path.relative_to(stage).as_posix(), 'bytes': path.stat().st_size, 'sha256': sha(path)}


def pool_map(function, values):
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        return list(pool.map(function, values))


product_commit = git('rev-parse', args.binary_source_commit + '^{commit}')
snapshot_commit = git('rev-parse', 'HEAD')
source_paths = ['Cargo.toml', 'Cargo.lock', 'build.rs', 'src', 'vendor/egui_commonmark', 'assets', 'locales']
changed_inputs = git('diff', '--name-only', product_commit, '--', *source_paths).splitlines()
source_adjustments = []
for relative in changed_inputs:
    # The only allowed divergence is the missing field in an existing cfg(test)
    # constructor. Preserve the real product inputs instead of relabeling the EXE.
    before = subprocess.check_output(['git','show',f'{product_commit}:{relative}'],cwd=ROOT).decode('utf-8')
    after = (ROOT / relative).read_text(encoding='utf-8')
    expected = before.replace('            native_corners: Default::default(),\n',
                              '            native_corners: Default::default(),\n            native_icons: Default::default(),\n')
    if relative != 'src/structure_ui.rs' or after != expected or '#[cfg(test)]' not in before:
        raise RuntimeError('Compiled product inputs changed: ' + relative)
    if not args.binary_input_record:
        raise RuntimeError('Preserve the original binary input record for test-only snapshot changes')
    source_adjustments.append({'path':relative,'reason':'cfg(test) Editor constructor field only; product EXE unchanged',
                               'snapshotSHA256':sha(ROOT / relative)})
original = (ROOT / args.base_stage).resolve()
exe = (ROOT / args.windows_exe).resolve()
base = (ROOT / args.output_root / args.build_id).resolve()
if not original.is_relative_to(ROOT) or not base.is_relative_to(ROOT) or base.exists():
    raise ValueError('Use a new workspace candidate directory and a workspace runtime base')
baseline = json.loads((original / 'manifest.json').read_text(encoding='utf-8'))


def verify_base(entry):
    path = original / entry['path']
    if not path.resolve().is_relative_to(original) or path.is_symlink() or path.stat().st_size != entry['bytes'] or sha(path) != entry['sha256']:
        raise RuntimeError('Runtime baseline differs from its manifest: ' + str(path))


pool_map(verify_base, baseline['files'])
observed = {p.relative_to(original).as_posix() for p in original.rglob('*') if p.is_file()}
expected = {entry['path'] for entry in baseline['files']} | {'manifest.json'}
if observed != expected:
    raise RuntimeError('Runtime baseline contains missing or extra files')
print(f'Verified accepted runtime: {len(baseline["files"])} payload files', flush=True)
stage = base / 'windows'
stage.mkdir(parents=True)


def copy_baseline(entry):
    destination = stage / entry['path']
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(original / entry['path'], destination)


pool_map(copy_baseline, baseline['files'])
shutil.copy2(exe, stage / 'MDcard.exe')
copy_project_licenses(ROOT, stage)
tracked = git('ls-files', '-z', '--', 'README.md', 'LICENSE', 'COPYRIGHT', 'THIRD_PARTY_NOTICES.txt', 'docs', 'locales', 'assets', 'renderer').split('\0')
for relative in filter(None, tracked):
    source = ROOT / relative
    if not source.is_file() or source.is_symlink():
        raise RuntimeError('Tracked resource is not a regular file: ' + relative)
    destination = stage / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
shutil.copy2(ROOT / 'scripts/Create-MDcardShortcut.ps1', stage / 'Create-MDcardShortcut.ps1')
source_snapshot = git('ls-files', '-z', '--', 'Cargo.toml', 'Cargo.lock', 'build.rs', 'src', 'tests', 'vendor/egui_commonmark').split('\0')
for relative in filter(None, source_snapshot):
    destination = stage / 'source' / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / relative, destination)

# The .d file is copied from the compiler output, not reconstructed from assumptions.
dep_info = (ROOT / args.compiler_deps).read_text(encoding='utf-8')
dependencies = shlex.split(dep_info.splitlines()[0].split(': ', 1)[1])
compiled = sorted({name.removeprefix('/work/') for name in dependencies if name.startswith('/work/')})
compiled = sorted(set(compiled) | {'Cargo.toml', 'Cargo.lock', 'build.rs'})
compile_record = {
    'binarySourceCommit': product_commit,
    'binarySHA256': sha(exe),
    'command': 'cargo build --locked --release --target x86_64-pc-windows-gnu',
    'toolchain': 'rustc 1.92.0 / x86_64-pc-windows-gnu / MinGW windres',
    'compilerDepInfoSHA256': sha(ROOT / args.compiler_deps),
    'inputs': [inventory(ROOT / relative, ROOT) for relative in compiled],
}
if args.binary_input_record:
    compile_record = json.loads((ROOT / args.binary_input_record).read_text(encoding='utf-8'))
    if compile_record['binarySourceCommit'] != product_commit or compile_record['binarySHA256'] != sha(exe):
        raise RuntimeError('Preserved compiler input record belongs to another product')
    if compile_record['compilerDepInfoSHA256'] != sha(ROOT / args.compiler_deps):
        raise RuntimeError('Compiler dep-info changed')
for adjustment in source_adjustments:
    relative = adjustment['path']
    original_bytes = subprocess.check_output(['git','show',f'{product_commit}:{relative}'],cwd=ROOT)
    original_hash = hashlib.sha256(original_bytes).hexdigest()
    expected_input = next(item for item in compile_record['inputs'] if item['path'] == relative)
    if original_hash != expected_input['sha256']:
        raise RuntimeError('Git product source bytes do not match actual compiler input: ' + relative)
    destination = stage / 'build/original-compile-inputs' / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(original_bytes)
    adjustment.update(originalInputSHA256=original_hash,
                      originalInputPath=destination.relative_to(stage).as_posix())
for entry in compile_record['inputs']:
    relative = entry['path']
    destination = stage / 'source' / relative
    if not destination.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
(stage / 'build').mkdir(exist_ok=True)
(stage / 'build/compile-inputs.json').write_text(json.dumps(compile_record, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
(stage / 'build/compiler-deps.d').write_text(dep_info, encoding='utf-8', newline='\n')
(stage / 'README-ICON-SUITE.md').write_text(
    '# MDcard icon suite\n\n'
    f'Build: `{args.build_id}`. Product commit: `{product_commit}`.\n\n'
    'Run `MDcard.exe` after extracting this entire directory. '
    'Run `powershell -ExecutionPolicy Bypass -File .\\Create-MDcardShortcut.ps1` to create or repair the desktop shortcut. '
    'After moving this directory, run the script again from its new location.\n\n'
    'The system icon is blue. The titlebar brand automatically uses black on light backgrounds and white on dark backgrounds. '
    'The complete editable icon suite is in `assets/branding/mdcard-icon/`; see `docs/ICON_INTEGRATION.md`. '
    'This package is frozen before independent acceptance; the final external acceptance report binds its immutable hash.\n', encoding='utf-8')
paths = sorted(p for p in stage.rglob('*') if p.is_file() and p != stage / 'manifest.json')
manifest = dict(baseline)
manifest.update(sourceCommit=snapshot_commit, binarySourceCommit=product_commit, buildId=args.build_id, auditId=args.build_id,
                candidateKind='icon-suite', acceptanceCandidate=True,
                createdAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                baseRuntimeSourceCommit=baseline['sourceCommit'], baseRuntimeBinarySourceCommit=baseline.get('binarySourceCommit'),
                iconSuite={'svg':12,'png':27,'ico':3,'default':'color','lightBrand':'black','darkBrand':'white','appUserModelID':'MDcard.Desktop'},
                compileInputs=compile_record['inputs'], sourceSnapshotAdjustments=source_adjustments,
                files=pool_map(lambda p: inventory(p, stage), paths))
(stage / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'Staged complete candidate: {len(paths)} payload files', flush=True)
version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
archive = base / f'MDcard-{version}-{args.build_id}-windows-x64.zip'
with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as output:
    for path in sorted(stage.rglob('*')):
        if path.is_file():
            output.write(path, path.relative_to(stage).as_posix())
record = {'platform':'windows','stage':str(stage),'archive':str(archive),'archive_sha256':sha(archive),
          'binary_sha256':sha(exe),'manifest_files':len(paths),'source_commit':snapshot_commit,
          'binary_source_commit':product_commit,'build_id':args.build_id}
(base / 'packages.json').write_text(json.dumps([record], indent=2) + '\n', encoding='utf-8')
print(json.dumps(record, indent=2), flush=True)
