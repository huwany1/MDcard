# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Stage or publish the accepted Windows icon suite; retain R29 and Linux identities."""
from __future__ import annotations

import argparse
import concurrent.futures
import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('phase', choices=['stage', 'publish'])
parser.add_argument('--candidate', default='artifacts/icon-integration-20261024/packages/icon-suite-20261024-r3/packages.json')
parser.add_argument('--independent-report', help='A final independent JSON report with passed=true')
args = parser.parse_args()


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def checked(path):
    path = Path(path).resolve()
    if not path.is_relative_to(ROOT) or path == ROOT:
        raise ValueError('Path escapes the named workspace: ' + str(path))
    if path.is_symlink() or path.is_junction():
        raise ValueError('Links are not promotion targets: ' + str(path))
    return path


def dump(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.icon-suite.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    temporary.replace(path)


record = json.loads((ROOT / args.candidate).read_text(encoding='utf-8'))[0]
build_id = record['build_id']
if build_id != 'icon-suite-20261024-r3' or record['platform'] != 'windows':
    raise ValueError('This promotion script requires the reviewed r3 Windows candidate')
source = checked(record['stage'])
candidate_archive = checked(record['archive'])
formal = checked(ROOT / 'dist/windows')
archive = checked(ROOT / 'dist' / candidate_archive.name)
backup = checked(ROOT / 'dist' / ('.before-' + build_id) / 'windows')
out = checked(ROOT / 'artifacts/icon-integration-20261024/implementation/release')
out.mkdir(parents=True, exist_ok=True)


def verify(stage):
    manifest = json.loads((stage / 'manifest.json').read_text(encoding='utf-8'))
    def entry_check(entry):
        item = stage / entry['path']
        if not item.resolve().is_relative_to(stage) or item.is_symlink() or item.is_junction() or item.stat().st_size != entry['bytes'] or sha(item) != entry['sha256']:
            raise RuntimeError('Manifest mismatch: ' + str(item))
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(entry_check, manifest['files']))
    actual = {path.relative_to(stage).as_posix() for path in stage.rglob('*') if path.is_file()}
    if actual != {entry['path'] for entry in manifest['files']} | {'manifest.json'}:
        raise RuntimeError('Payload file set mismatch: ' + str(stage))
    if manifest['buildId'] != build_id or sha(stage / 'MDcard.exe') != record['binary_sha256']:
        raise RuntimeError('Wrong product build identity')
    return manifest


if sha(candidate_archive) != record['archive_sha256']:
    raise RuntimeError('Frozen candidate archive changed')
if args.phase == 'stage':
    if backup.exists() or (out / 'staged.json').exists():
        raise FileExistsError('Keep the existing promotion and its backup')
    manifest = verify(source)
    if archive.exists():
        if sha(archive) != record['archive_sha256']:
            raise FileExistsError('A different formal archive already uses this name')
    else:
        shutil.copy2(candidate_archive, archive)
    incoming = checked(ROOT / 'dist' / ('.incoming-' + build_id))
    if incoming.exists():
        raise FileExistsError('Preserve the existing incoming stage')
    incoming.mkdir()
    def copy_entry(entry):
        destination = incoming / entry['path']
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / entry['path'], destination)
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(copy_entry, manifest['files']))
    shutil.copy2(source / 'manifest.json', incoming / 'manifest.json')
    verify(incoming)
    previous = json.loads((ROOT / 'dist/latest.json').read_text(encoding='utf-8'))
    dump(out / 'latest-before.json', previous)
    old_manifest = sha(formal / 'manifest.json')
    old_binary = sha(formal / 'MDcard.exe')
    backup.parent.mkdir(parents=True)
    # Absolute source/destination were confined to the explicit workspace above.
    formal.rename(backup)
    try:
        incoming.rename(formal)
    except BaseException:
        backup.rename(formal)
        raise
    staged = {'status':'staged; latest pointer unchanged pending final-directory GUI',
              'build_id':build_id,'candidate':record,'formal_stage':str(formal),'formal_archive':str(archive),
              'backup':str(backup),'retired_binary_sha256':old_binary,'retired_manifest_sha256':old_manifest,
              'created_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
    dump(out / 'staged.json', staged)
    print(json.dumps(staged, indent=2))
else:
    if not args.independent_report:
        raise ValueError('Publishing requires the final independent report')
    report_path = checked(ROOT / args.independent_report)
    independent = json.loads(report_path.read_text(encoding='utf-8'))
    if independent.get('passed') is not True:
        raise RuntimeError('Independent acceptance has not passed')
    if independent.get('binary_sha256') != record['binary_sha256'] or independent.get('archive_sha256') != record['archive_sha256']:
        raise RuntimeError('Independent acceptance belongs to another binary or archive')
    staged = json.loads((out / 'staged.json').read_text(encoding='utf-8'))
    manifest = verify(formal)
    if sha(archive) != record['archive_sha256']:
        raise RuntimeError('Formal archive differs from accepted archive')
    previous = json.loads((out / 'latest-before.json').read_text(encoding='utf-8'))
    current = json.loads((ROOT / 'dist/latest.json').read_text(encoding='utf-8'))
    if current != previous:
        raise RuntimeError('Release pointer changed since staging; preserve concurrent release work')
    stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
    accepted_commit = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    new_windows = dict(next(entry for entry in previous if entry['platform']=='windows'))
    new_windows.update(record, stage=str(formal), archive=str(archive), release_name='Icon Suite R3',
                       release_channel='stable', acceptance_status='passed', promoted_at=stamp,
                       acceptance_commit=accepted_commit, promotion_record='artifacts/icon-integration-20261024/implementation/release/promotion.json')
    # R29-only evidence fields stay in history, never label them as evidence of this build.
    for key in ['evidence_archive','evidence_archive_sha256']:
        new_windows.pop(key,None)
    new_windows.update(independent_report=report_path.relative_to(ROOT).as_posix(), independent_report_sha256=sha(report_path))
    latest = [new_windows if entry['platform']=='windows' else entry for entry in previous]
    release = {'status':'accepted and promoted','created_utc':stamp,'windows':new_windows,
               'linux_preserved':next(entry for entry in previous if entry['platform']=='linux'),
               'retired_windows':next(entry for entry in previous if entry['platform']=='windows'),
               'runtime_backup':str(backup),'final_payload_files':len(manifest['files']),
               'final_manifest_sha256':sha(formal/'manifest.json'),'independent_report':str(report_path)}
    dump(ROOT / 'docs/releases/2026-10-24-icon-suite-r3.json', release)
    dump(ROOT / 'dist/latest.json', latest)
    dump(out / 'promotion.json', release)
    print(json.dumps(release, indent=2))
