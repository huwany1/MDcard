# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Hash the delivered binaries, complete packages and reviewed search evidence."""
import datetime,hashlib,json,subprocess,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.file_digest(path.open('rb'),'sha256').hexdigest()
def record(path):return dict(path=path.relative_to(ROOT).as_posix(),bytes=path.stat().st_size,sha256=digest(path))
output=ROOT/'evidence/search';temporary=list(output.rglob('.tmp*'))
assert not temporary,'Remove interrupted evidence temporary files before manifest: '+str(temporary)
packages=json.loads((output/'packages/result.json').read_text(encoding='utf-8'))
for package in packages:
    stage=Path(package['stage']);archive=Path(package['archive'])
    assert digest(archive)==package['sha256']
    metadata=json.loads((stage/'manifest.json').read_text(encoding='utf-8'))
    for entry in metadata['files']:
        path=stage/entry['path'];assert path.is_file() and path.stat().st_size==entry['bytes'] and digest(path)==entry['sha256'],str(path)
    assert (stage/'renderer/worker.mjs').is_file() and (stage/'docs/GLOBAL_SEARCH.md').is_file()
sources=[ROOT/p for p in ['.gitattributes','.gitignore','Cargo.toml','Cargo.lock','README.md','THIRD_PARTY_NOTICES.txt','src/main.rs','src/document.rs','src/search.rs','src/search_ui.rs','src/text_files.rs','src/structure_ui.rs','locales/ui.json','docs/GLOBAL_SEARCH.md','docs/ACCEPTANCE.md','tests/search-acceptance/Cargo.toml','tests/search-acceptance/Cargo.lock','tests/search-acceptance/src/lib.rs','scripts/add-search-translations.py','scripts/collect-licenses.py','scripts/package-rich.py','scripts/verify-search-linux.py','scripts/verify-search-ui-performance.py','scripts/verify-search-master-windows.ps1','scripts/verify-search-ui-performance-windows.ps1','scripts/verify-search-packages.py','scripts/write-search-manifest.py','renderer/runtime-sources.json']]
manifest=dict(verification_id=str(uuid.uuid4()),created_at=datetime.datetime.now(datetime.timezone(datetime.timedelta(hours=8))).isoformat(),timezone='Asia/Shanghai',branch=subprocess.check_output(['git','branch','--show-current'],cwd=ROOT,text=True).strip(),base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),binaries=[record(ROOT/'artifacts/global-search/MDcard.exe'),record(ROOT/'artifacts/global-search/mdcard')],sources=[record(p) for p in sources],packages=packages,evidence=[record(p) for p in sorted(output.rglob('*')) if p.is_file() and p.name!='manifest.json'],cleanup=dict(temporary_files=0,fixture_root='artifacts/global-search and artifacts/search (gitignored)',gui_test_processes='closed by each verification script',build_container='mdcard-dev retained for reproducibility'),boundaries=['Windows native and Debian/X11/Openbox only','No claim for untested Wayland/macOS/IME/DPI/hardware','128 MiB index and explicit regex/output budgets','Per-file atomic writes with durable recovery snapshots; not filesystem-wide atomicity'])
(output/'manifest.json').write_text(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(dict(verification_id=manifest['verification_id'],binary_hashes=[b['sha256'] for b in manifest['binaries']],evidence_files=len(manifest['evidence']),package_integrity='all staged files verified'),ensure_ascii=False))
