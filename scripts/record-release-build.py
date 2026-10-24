"""Bind actual compiler dependency files and binary bytes to frozen Git inputs."""
from pathlib import Path
import argparse,hashlib,json,shlex,subprocess
P=argparse.ArgumentParser();P.add_argument('--source-root',type=Path,required=True);P.add_argument('--source-commit',required=True);P.add_argument('--build-dir',type=Path,required=True);P.add_argument('--runtime-base',type=Path,required=True);P.add_argument('--output',type=Path,required=True);A=P.parse_args()
ROOT=A.source_root.resolve();OUT=A.output.resolve();COMMIT=A.source_commit;BUILD=A.build_dir.resolve();BASE=A.runtime_base.resolve()
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
records=[]
for platform,exe in [('windows','MDcard.exe'),('linux','mdcard')]:
    dep=BUILD/(platform+'-deps.d')
    inputs=sorted(set(p.removeprefix('/work/') for p in shlex.split(dep.read_text('utf8').splitlines()[0].split(': ',1)[1]) if p.startswith('/work/'))|{'Cargo.toml','Cargo.lock','build.rs'})
    proof=[]
    for name in inputs:
        path=ROOT/name;raw=subprocess.check_output(['git','show',COMMIT+':'+name],cwd=ROOT)
        # Git LF normalization applies to tracked textual source files.
        disk=path.read_bytes();normalized=disk.replace(b'\r\n',b'\n') if b'\0' not in raw else disk
        if disk!=raw and normalized!=raw.replace(b'\r\n',b'\n'):raise RuntimeError('Compiled input differs from frozen source: '+name)
        equal=(normalized==raw.replace(b'\r\n',b'\n')) if b'\0' not in raw else disk==raw
        proof.append({'path':name,'bytes':len(disk),'sha256':sha(path),'gitBlobSHA256':hashlib.sha256(raw).hexdigest(),'byteExactToGitBlob':disk==raw,'normalizedEqualToGitBlob':equal,'normalization':'Git checkout CRLF/LF conversion for text files; actual compiler bytes recorded separately' if disk!=raw else 'none'})
    records.append({'platform':platform,'sourceCommit':COMMIT,'binarySHA256':sha(BUILD/exe),'compilerDepInfoSHA256':sha(dep),'inputs':proof})
record={'sourceCommit':COMMIT,'sourceURL':'https://github.com/huwany1/MDcard/tree/'+COMMIT,'platforms':records,'applicationRenderer':{'packageLockSHA256':sha(ROOT/'renderer/package-lock.json'),'browserBundleSHA256':sha(ROOT/'renderer/vendor/browser.js'),'buildMetafileSHA256':sha(ROOT/'renderer/vendor/build-inputs.json'),'rebuild':'Fresh npm run build from public source; patched xmldom 0.9.12; EPL ELK excluded'},'runtimeBases':[{'platform':p,'manifestSHA256':sha(BASE/p/'manifest.json'),'legacyRendererLockSHA256':sha(BASE/p/'renderer/package-lock.json'),'reuseBoundary':'Pinned OS Node/Pandoc/PlantUML and matching platform Sharp inputs reused; application modules and browser bundle rebuilt from current public source; JRE rebuilt by jlink; browser replaced by authenticated unbranded source in renderer/export-runtime-sources.json; Linux ELF postprocessing bound by separate transformation evidence'} for p in ['windows','linux']]}
OUT.parent.mkdir(parents=True,exist_ok=True);OUT.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n',encoding='utf8');print(json.dumps({'sourceCommit':COMMIT,'compiledInputCounts':[len(x['inputs']) for x in records],'compiledInputsMatchGitAfterCheckoutNewlineNormalization':all(f['normalizedEqualToGitBlob'] for x in records for f in x['inputs'])}))
