# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Check every candidate archive member against its embedded manifest."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('packages_json', type=Path)
parser.add_argument('--output', type=Path, default=ROOT / 'evidence/bug-audit/master-review/packages.json')
args = parser.parse_args()
records = json.loads(args.packages_json.read_text('utf8'))
results = []


def digest(stream):
    sha = hashlib.sha256()
    size = 0
    while chunk := stream.read(1024 * 1024):
        sha.update(chunk)
        size += len(chunk)
    return size, sha.hexdigest()


def check_path(name):
    path = PurePosixPath(name)
    assert not path.is_absolute() and '..' not in path.parts and '\\' not in name, name
    return path.as_posix().removeprefix('./')


for package in records:
    archive = Path(package['archive'])
    stage = Path(package['stage'])
    manifest = json.loads((stage / 'manifest.json').read_text('utf8'))
    assert manifest['acceptanceCandidate'] is True
    assert manifest['sourceCommit'] == package['source_commit']
    subprocess.run(['git', 'diff', '--quiet', package['source_commit'], '--', 'src', 'vendor', 'Cargo.toml', 'Cargo.lock', 'assets', 'locales', 'renderer'], cwd=ROOT, check=True)
    expected = {entry['path']: entry for entry in manifest['files']}
    assert len(expected) == len(manifest['files']), 'Duplicate manifest paths'
    actual = {}

    def verify(name, stream):
        normalized = check_path(name)
        assert normalized not in actual, 'Duplicate archive member: ' + normalized
        actual[normalized] = digest(stream)

    if package['platform'] == 'windows':
        with zipfile.ZipFile(archive) as source:
            for entry in source.infolist():
                assert (entry.external_attr >> 16) & 0o170000 != 0o120000, 'Archive link'
                if not entry.is_dir():
                    with source.open(entry) as stream:
                        verify(entry.filename, stream)
    else:
        with tarfile.open(archive, 'r:gz') as source:
            for entry in source:
                name = check_path(entry.name)
                assert entry.isdir() or entry.isfile(), 'Archive special member'
                if entry.isfile():
                    with source.extractfile(entry) as stream:
                        verify(name, stream)
                    if name == 'mdcard' or name == 'renderer/runtime/node' or name.startswith('renderer/runtime/java/bin/') or name in {'renderer/runtime/chromium/chrome', 'renderer/runtime/pandoc/pandoc'}:
                        assert entry.mode == 0o755, (name, entry.mode)
    assert set(actual) == set(expected) | {'manifest.json'}, 'Archive and manifest file sets differ'
    for name, entry in expected.items():
        assert actual[name] == (entry['bytes'], entry['sha256']), name
    with (stage / 'manifest.json').open('rb') as stream:
        assert actual['manifest.json'] == digest(stream)
    with archive.open('rb') as stream:
        archive_size, archive_sha = digest(stream)
    assert archive_sha == package['archive_sha256']
    executable = 'MDcard.exe' if package['platform'] == 'windows' else 'mdcard'
    assert actual[executable][1] == package['binary_sha256']
    results.append(dict(platform=package['platform'], passed=True, files=len(expected), archive_bytes=archive_size,
                        archive_sha256=archive_sha, binary_sha256=actual[executable][1], source_commit=package['source_commit']))
    print('Verified', package['platform'], len(expected), 'files', flush=True)
out = args.output
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(results, indent=2) + '\n', 'utf8')
print(json.dumps(results, indent=2), flush=True)
