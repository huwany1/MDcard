# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Verify packaged manifests and archive contents against frozen native binaries."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import zipfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--stage', type=Path, default=ROOT / 'dist')
p.add_argument('--out', type=Path, default=ROOT / 'evidence/performance/packages/verification.json')
p.add_argument('--windows-exe', type=Path, default=ROOT / 'dist/windows/MDcard.exe')
p.add_argument('--linux-exe', type=Path, default=ROOT / 'dist/linux/mdcard')
a = p.parse_args()
version = tomllib.loads((ROOT / 'Cargo.toml').read_text('utf8'))['package']['version']

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

results = []
for platform in ['windows', 'linux']:
    stage = a.stage / platform
    name = 'MDcard.exe' if platform == 'windows' else 'mdcard'
    frozen = a.windows_exe if platform == 'windows' else a.linux_exe
    expected = digest(frozen)
    manifest = json.loads((stage / 'manifest.json').read_text('utf8'))
    failures = []
    if manifest['version'] != version:
        failures.append('Manifest release version differs from Cargo.toml')
    paths = set()
    for record in manifest['files']:
        relative = Path(record['path'])
        if relative.is_absolute() or '..' in relative.parts:
            failures.append('Unsafe manifest path: ' + str(relative))
            continue
        paths.add(relative.as_posix())
        path = stage / relative
        if not path.is_file() or path.stat().st_size != record['bytes'] or digest(path) != record['sha256']:
            failures.append('Manifest mismatch: ' + str(relative))
    actual = {v.relative_to(stage).as_posix() for v in stage.rglob('*') if v.is_file() and v != stage / 'manifest.json'}
    if actual != paths:
        failures.append('Stage file set differs from manifest')
    if digest(stage / name) != expected:
        failures.append('Staged binary differs from frozen program')
    if digest(stage / 'renderer/worker.mjs') != digest(ROOT / 'renderer/worker.mjs'):
        failures.append('Staged renderer differs from source')
    archive = ROOT / ('dist/MDcard-' + version + '-' + platform + '-x64' + ('.zip' if platform == 'windows' else '.tar.gz'))
    if platform == 'windows':
        with zipfile.ZipFile(archive) as bundle:
            archive_names = set(bundle.namelist())
            for record in manifest['files']:
                with bundle.open(record['path']) as stream:
                    if hashlib.file_digest(stream, 'sha256').hexdigest() != record['sha256']:
                        failures.append('ZIP mismatch: ' + record['path'])
            if archive_names != paths | {'manifest.json'}:
                failures.append('ZIP file set differs from manifest')
            if bundle.read('manifest.json') != (stage / 'manifest.json').read_bytes():
                failures.append('ZIP manifest differs from stage')
    else:
        observed = set()
        archive_files = set()
        with tarfile.open(archive) as bundle:
            expected_records = {v['path']: v['sha256'] for v in manifest['files']}
            for member in bundle:
                name_in_bundle = member.name.removeprefix('./')
                if member.isfile():
                    archive_files.add(name_in_bundle)
                if member.isfile() and name_in_bundle == 'manifest.json':
                    with bundle.extractfile(member) as stream:
                        if stream.read() != (stage / 'manifest.json').read_bytes():
                            failures.append('Tar manifest differs from stage')
                if member.isfile() and name_in_bundle in expected_records:
                    observed.add(name_in_bundle)
                    with bundle.extractfile(member) as stream:
                        if hashlib.file_digest(stream, 'sha256').hexdigest() != expected_records[name_in_bundle]:
                            failures.append('Tar mismatch: ' + name_in_bundle)
            if observed != paths or archive_files != paths | {'manifest.json'}:
                failures.append('Tar file set differs from manifest')
    results.append(dict(platform=platform, stage=str(stage), binary_sha256=expected,
                        manifest_files=len(paths), archive=str(archive), archive_sha256=digest(archive),
                        failures=failures, passed=not failures))
report = dict(passed=all(v['passed'] for v in results), packages=results)
a.out.parent.mkdir(parents=True, exist_ok=True)
a.out.write_text(json.dumps(report, indent=2) + '\n', 'utf8')
print(json.dumps(report, indent=2))
assert report['passed'], 'Package manifest/archive verification failed'
