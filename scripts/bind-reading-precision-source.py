# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Bind frozen compilation inputs and binaries to an independently checked commit."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = (ROOT / args.output).resolve()
    assert output.is_relative_to(ROOT / 'evidence/reading-precision/master')
    assert not output.exists(), 'Retain previous bindings; use a fresh output'
    manifest_path = (ROOT / args.manifest).resolve()
    manifest_raw = manifest_path.read_bytes()
    manifest = json.loads(manifest_raw.decode('utf-8-sig'))
    commit = subprocess.check_output(
        ['git', 'rev-parse', args.commit + '^{commit}'], cwd=ROOT, text=True).strip()
    inputs = []
    for category in ('source_sha256', 'compile_resource_inputs'):
        for name, expected in manifest[category].items():
            path = (ROOT / name).resolve()
            assert path.is_relative_to(ROOT), name
            raw = path.read_bytes()
            assert sha(raw) == expected, ('Compilation input drift', name)
            committed = subprocess.check_output(['git', 'show', commit + ':' + name], cwd=ROOT)
            method = 'identical bytes'
            if raw != committed:
                # Git may normalize working-tree CRLF source files. Do not
                # normalize fonts/images or arbitrary binary resources.
                raw.decode('utf-8')
                committed.decode('utf-8')
                assert raw.replace(b'\r\n', b'\n') == committed.replace(b'\r\n', b'\n'), name
                method = 'UTF-8 text equal after CRLF to LF normalization'
            inputs.append(dict(path=name, category=category, compiled_raw_sha256=sha(raw),
                               committed_sha256=sha(committed), equality=method))
    binaries = {}
    for role, entry in manifest['binaries'].items():
        path = (ROOT / entry['path']).resolve()
        assert path.is_relative_to(ROOT), str(path)
        raw = path.read_bytes()
        assert len(raw) == entry['bytes'] and sha(raw) == entry['sha256'], role
        binaries[role] = entry
    result = dict(passed=True, created_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  commit=commit, manifest=str(manifest_path.relative_to(ROOT)),
                  manifest_sha256=sha(manifest_raw), inputs=inputs, binaries=binaries,
                  boundary='Independently verifies frozen build-input bytes and commit content. '
                           'Documentation-only changes are not compilation inputs. This does not '
                           'replace GUI, runtime-resource, archive, or package acceptance.')
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, commit=commit, inputs=len(inputs), binaries=len(binaries))))


if __name__ == '__main__':
    main()
