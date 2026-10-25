# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Scan exactly the staged blobs and named public ancestors; never print values.

This is a review aid, not proof that no secret exists. No .gitignore exclusions
are used: binary blobs are inventoried, textual blobs are scanned in full.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--ancestor', help='optional public history ref to scan separately')
parser.add_argument('--index-file', help='scan an isolated candidate index without modifying the real index')
parser.add_argument('--output', required=True)
args = parser.parse_args()

def git(*a):
    env = dict(os.environ)
    if args.index_file:
        env['GIT_INDEX_FILE'] = str(Path(args.index_file).resolve())
    return subprocess.check_output(['git', *a], env=env)

rules = {
    'private-key': r'-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----',
    'service-token': r'\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{30,}|AKIA[A-Z0-9]{16}|sk-(?:proj-)?[A-Za-z0-9_-]{30,}|xox[baprs]-[A-Za-z0-9-]{15,})\b',
    'jwt': r'\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\b',
    'credential-url': r'https?://[^\s/:"\x27<>]+:[^\s/@"\x27<>]+@',
    'secret-assignment': r'(?i)\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|password|passwd|credential)\b\s*[:=]\s*["\x27][^"\x27\r\n]{3,}["\x27]',
    'user-profile-path': r'(?i)(?:[A-Z]:[/\\]Users[/\\][^/\\\s"\x27<>]+|/home/[A-Za-z0-9_.-]+)',
    'email': r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b',
}
compiled = {key: re.compile(value) for key, value in rules.items()}
objects = {}
for row in git('ls-files', '-s', '-z').split(b'\0'):
    if row:
        meta, name = row.split(b'\t', 1)
        objects.setdefault(meta.split()[1].decode(), set()).add(name.decode('utf8'))
commits = git('rev-list', args.ancestor).decode().splitlines() if args.ancestor else []
for commit in commits:
    for row in git('ls-tree', '-r', '-z', commit).split(b'\0'):
        if row:
            meta, name = row.split(b'\t', 1)
            if meta.split()[1] == b'blob':
                objects.setdefault(meta.split()[2].decode(), set()).add(name.decode('utf8'))
findings, binaries = [], []
text_bytes = 0
for oid, paths in sorted(objects.items()):
    blob = git('cat-file', 'blob', oid)
    try:
        if b'\0' in blob:
            raise UnicodeError()
        value = blob.decode('utf8')
    except UnicodeError:
        binaries.append({'blob': oid, 'paths': sorted(paths), 'bytes': len(blob), 'sha256': hashlib.sha256(blob).hexdigest()})
        continue
    text_bytes += len(blob)
    for key, rule in compiled.items():
        for match in rule.finditer(value):
            findings.append({'rule': key, 'blob': oid, 'paths': sorted(paths), 'line': value.count('\n', 0, match.start()) + 1, 'value_sha256': hashlib.sha256(match.group().encode()).hexdigest()})
report = {'ancestor': args.ancestor, 'ancestor_commits': commits, 'unique_blobs': len(objects), 'text_bytes': text_bytes, 'rules': rules, 'findings': findings, 'binary_inventory': binaries, 'limitations': 'Pattern review cannot prove absence of secrets; all findings and binary/user content require human review.'}
Path(args.output).write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', 'utf8')
print(json.dumps({'blobs': len(objects), 'findings': len(findings), 'binary_blobs': len(binaries), 'output': args.output}))
