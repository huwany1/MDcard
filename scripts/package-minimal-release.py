"""Build a fresh runtime-only 0.1.0 candidate from an explicit file whitelist.

This never deletes existing packages or publishes assets. The input runtime stages
are retained, and every copied file has a reason in manifest.json.
"""
from __future__ import annotations
import argparse, collections, concurrent.futures, datetime, hashlib, json, re
import shutil, struct, subprocess, tarfile, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
P = argparse.ArgumentParser()
P.add_argument('--source-commit', required=True)
P.add_argument('--source-root', type=Path, default=ROOT)
P.add_argument('--runtime-base', type=Path, required=True)
P.add_argument('--renderer-root', type=Path)
P.add_argument('--chromium-root', type=Path, required=True, help='Directory with windows/chromium and linux/chromium runtime trees')
P.add_argument('--build-dir', type=Path, required=True)
P.add_argument('--output', type=Path, required=True)
P.add_argument('--trace', type=Path, required=True)
P.add_argument('--jre-root', type=Path)
P.add_argument('--elf-strip-command', default='["strip", "--strip-unneeded"]', help='JSON argv prefix; copied Linux ELF path is appended. Never changes source inputs.')
A = P.parse_args()
ROOT = A.source_root.resolve()
RUNTIME_BASE = A.runtime_base.resolve()
RENDERER = A.renderer_root.resolve() if A.renderer_root else ROOT/'renderer'
A.build_dir = A.build_dir.resolve()
OUT = A.output.resolve()
if not OUT.is_relative_to(ROOT) or OUT.exists():
    raise ValueError('Use a new output directory inside the workspace')
OUT.mkdir(parents=True)
COMMIT = subprocess.check_output(['git', 'rev-parse', A.source_commit+'^{commit}'], cwd=ROOT, text=True).strip()
if subprocess.check_output(['git','diff','--name-only',COMMIT,'--','Cargo.toml','Cargo.lock','build.rs','src','assets','locales','vendor','renderer'],cwd=ROOT,text=True).strip():
    raise RuntimeError('Product inputs differ from the frozen source commit')
TRACE = json.loads(A.trace.read_text('utf8'))
NOW = datetime.datetime.now(datetime.timezone.utc).isoformat()

def sha(p):
    with p.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()

def legal_name(p):
    return bool(re.search(r'(^|[/_.-])(licen[cs]e|copying|copyright|notice|authors|ofl)([._/-]|$)', Path(p).as_posix(), re.I))

def category(name):
    if name.startswith(('source/','src/','tests/')): return 'source-and-tests'
    if name.startswith(('docs/','evidence/','examples/')): return 'reports-docs-examples'
    if name.startswith('assets/branding/'): return 'brand-handoff-and-embedded-icons'
    if name.startswith(('assets/','locales/')): return 'embedded-resources-and-metadata'
    if name.startswith('renderer/runtime/pandoc/'): return 'pandoc-duplicate-binaries-and-manuals'
    if name.startswith('renderer/runtime/'): return 'runtime-unused-tools-caches-locales'
    if name.startswith('renderer/node_modules/'): return 'unreachable-node-dependencies-types-tests-docs'
    return 'development-and-nonessential-content'

def generated(stage, name, text, reason, rules):
    path=stage/name; path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(text,encoding='utf8'); rules[name]=reason

packages=[]
transforms=[]
def elf_allocated(path):
    raw=path.read_bytes()
    if raw[:6]!=b'\x7fELF\x02\x01': raise RuntimeError('Expected ELF64 little endian')
    offset=struct.unpack_from('<Q',raw,40)[0];count=struct.unpack_from('<H',raw,60)[0];entry=struct.unpack_from('<H',raw,58)[0];strings=struct.unpack_from('<H',raw,62)[0]
    sections=[struct.unpack_from('<IIQQQQIIQQ',raw,offset+i*entry) for i in range(count)]
    st=sections[strings];names=raw[st[4]:st[4]+st[5]]
    return [{'name':names[s[0]:].split(b'\0',1)[0].decode(),'type':s[1],'flags':s[2],'address':s[3],'bytes':s[5],'sha256':hashlib.sha256(raw[s[4]:s[4]+s[5]]).hexdigest() if s[1]!=8 else None} for s in sections if s[2]&2]
for platform in ['windows','linux']:
    source=RUNTIME_BASE/platform; stage=OUT/platform; stage.mkdir()
    rules={}; origins={}
    def add(path, relative, reason):
        relative=Path(relative).as_posix()
        if relative in rules: return
        if not path.is_file() or path.is_symlink(): raise RuntimeError('Not a regular payload input: '+str(path))
        dest=stage/relative; dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(path,dest); rules[relative]=reason
    exe='MDcard.exe' if platform=='windows' else 'mdcard'
    add(A.build_dir/exe,exe,'Native editor; current full product, version 0.1.0, frozen source '+COMMIT)
    for name in ['worker.mjs','export.mjs','export-docx.mjs','export-fonts.mjs','export-process.mjs']:
        add(ROOT/'renderer'/name,'renderer/'+name,'Application-invoked local rendering/export runtime entry or direct module')
    node_paths={}
    for entry in TRACE['files']:
        name=entry['path'].replace('\\','/')
        if not name.startswith('node_modules/'): continue
        if name.startswith('node_modules/@img/sharp-'): continue
        if name.endswith(('.d.ts','.map')) or '/module-test.' in name: continue
        if '/serve-index/public/' in name: continue  # HTTP directory-server UI is not used by exports.
        node_paths[name]='Static runtime import/require closure; parents: '+', '.join(p.replace('\\','/') for p in entry.get('parents',[]))
    # Native packages are selected explicitly for this OS; Windows trace cannot
    # resolve Linux conditional exports. Include only runtime binary/metadata.
    native=['@img/sharp-win32-x64'] if platform=='windows' else ['@img/sharp-linux-x64','@img/sharp-libvips-linux-x64']
    for package in native:
        native_source=RENDERER/'node_modules'/package
        if not native_source.exists():native_source=source/'renderer/node_modules'/package
        for f in native_source.rglob('*'):
            if f.is_file() and not f.name.lower().startswith(('readme','changelog')) and f.suffix.lower() not in {'.h','.hpp','.a','.la','.lib','.pdb','.obj','.o','.pc','.cmake'}:
                node_paths['node_modules/'+package+'/'+f.relative_to(native_source).as_posix()]='OS-specific Sharp/libvips native runtime for image exports'
    module_roots=set()
    for name,reason in sorted(node_paths.items()):
        path=RENDERER/name
        if not path.exists() and name.startswith('node_modules/@img/sharp-'):path=source/'renderer'/name
        if not path.exists(): raise RuntimeError('Traced dependency absent on '+platform+': '+name)
        if not legal_name(path.name):add(path,'renderer/'+name,reason)
        parts=Path(name).parts
        # Track nested package boundaries too, preserving all license obligations.
        for i,part in enumerate(parts):
            if part=='node_modules' and i+1<len(parts):
                end=i+3 if parts[i+1].startswith('@') else i+2
                if end<=len(parts): module_roots.add(Path(*parts[:end]))
    add(source/'renderer/runtime'/('node.exe' if platform=='windows' else 'node'),
        'renderer/runtime/'+('node.exe' if platform=='windows' else 'node'),'Bundled Node runtime; no system Node installation required')
    for name in ['browser.js','katex.css','diagram-font.css','plantuml.jar','fonts/MDcardDiagramSans-Regular.ttf','fonts/MDcardDiagramSansKR-Regular.ttf']:
        vendor=RENDERER/'vendor'/name
        if not vendor.exists() and name=='plantuml.jar':vendor=source/'renderer/vendor'/name
        add(vendor,'renderer/vendor/'+name,'worker/export references this local browser engine, stylesheet, font, or PlantUML engine')
    chrome=A.chromium_root.resolve()/platform/'chromium'
    names=(['chrome.exe','chrome.dll','chrome_elf.dll','chrome_wer.dll','d3dcompiler_47.dll','dxcompiler.dll','libEGL.dll','libGLESv2.dll','vk_swiftshader.dll','vulkan-1.dll']+[f.name for f in chrome.glob('*.manifest')] if platform=='windows' else ['chrome','chrome_crashpad_handler','libEGL.so','libGLESv2.so','libvk_swiftshader.so','libvulkan.so.1'])
    if platform=='linux' and (chrome/'chrome_sandbox').exists():names.append('chrome_sandbox')
    names+=['chrome_100_percent.pak','chrome_200_percent.pak','icudtl.dat','resources.pak','v8_context_snapshot.bin','vk_swiftshader_icd.json','locales/en-US.pak']
    for name in names:
        add(chrome/name,'renderer/runtime/chromium/'+name,'Chromium headless executable/shared library/resource dependency; local preview, raster/PDF and slide exports')
    java=(A.jre_root/('minimal-'+platform)).resolve() if A.jre_root else source/'renderer/runtime/java'
    # JRE's modules image is indivisible here. Debuggers, administration tools,
    # CDS compilation caches, static link libraries, and unused launchers omitted.
    java_names=(['bin/java.exe','bin/jli.dll','bin/java.dll','bin/verify.dll','bin/zip.dll','bin/net.dll','bin/nio.dll','bin/awt.dll','bin/fontmanager.dll','bin/freetype.dll','bin/javajpeg.dll','bin/lcms.dll','bin/jsvml.dll','bin/mlib_image.dll','bin/jimage.dll','bin/server/jvm.dll','bin/msvcp140.dll','bin/vcruntime140.dll','bin/vcruntime140_1.dll','bin/ucrtbase.dll'] if platform=='windows' else ['bin/java','lib/libjli.so','lib/libjava.so','lib/libverify.so','lib/libzip.so','lib/libnet.so','lib/libnio.so','lib/libawt.so','lib/libawt_headless.so','lib/libfontmanager.so','lib/libfreetype.so','lib/libjavajpeg.so','lib/liblcms.so','lib/libjsvml.so','lib/libmlib_image.so','lib/libjimage.so','lib/server/libjvm.so','lib/jspawnhelper'])
    java_names+=['lib/modules','lib/jvm.cfg','lib/tzdb.dat','lib/psfont.properties.ja','lib/psfontj2d.properties','lib/security/default.policy','conf/security/java.security','conf/security/java.policy']
    if platform=='windows':java_names+=['lib/fontconfig.bfc','lib/fontconfig.properties.src','lib/tzmappings']
    if A.jre_root:
        omitted={'release','lib/classlist','lib/jrt-fs.jar','lib/jexec','lib/libjsig.so','lib/server/libjsig.so','lib/fontconfig.properties.src','conf/security/policy/README.txt','bin/ucrtbase.dll'}
        java_names=[f.relative_to(java).as_posix() for f in java.rglob('*') if f.is_file() and not f.relative_to(java).as_posix().startswith(('legal/','conf/security/policy/limited/')) and f.relative_to(java).as_posix() not in omitted and not f.name.startswith('api-ms-win-') and f.suffix.lower() not in ['.lib','.a','.h','.jfc','.jsa'] and (f.parent.name!='bin' or f.name in ['java','java.exe'] or f.suffix.lower()=='.dll')]
    for name in java_names:
        if not (java/name).exists(): continue
        add(java/name,'renderer/runtime/java/'+name,'jlink module dependency closure selected by PlantUML jdeps; six supported locales; no JDK/debug/administrative launchers')
    add(source/'renderer/runtime/pandoc'/('pandoc.exe' if platform=='windows' else 'pandoc'),
        'renderer/runtime/pandoc/'+('pandoc.exe' if platform=='windows' else 'pandoc'),'Bundled Pandoc executable for DOCX/EPUB; duplicate binaries and manuals omitted'+('; original signed PE image retained byte-for-byte for publisher Authenticode verification, including its internal COFF/debug data' if platform=='windows' else ''))
    # Consolidate identical license texts once, with an attribution index naming
    # every original component path. No build tools or source snapshots included.
    legal={}
    def notice(path,label):
        if path.is_file(): legal.setdefault(sha(path),{'bytes':path.read_bytes(),'labels':[]})['labels'].append(label)
    for name in ['LICENSE','THIRD_PARTY_NOTICES.txt']:notice(ROOT/name,name)
    for f in (ROOT/'assets').glob('*'):
        if legal_name(f.name):notice(f,'assets/'+f.name)
    for name in ['Node-LICENSE.txt','PlantUML-MIT-LICENSE.txt','THIRD_PARTY_NOTICES.txt']:notice(ROOT/'renderer'/name,'renderer/'+name)
    for module in sorted(module_roots):
        module_path=RENDERER/module
        if not module_path.exists():module_path=source/'renderer'/module
        for f in module_path.rglob('*'):
            if f.is_file() and legal_name(f.relative_to(module_path)) and f.stat().st_size<5_000_000:
                notice(f,'renderer/'+module.as_posix()+'/'+f.relative_to(module_path).as_posix())
    for f in (java/'legal').rglob('*'):
        if f.is_file():notice(f,'JRE/'+f.relative_to(java).as_posix())
    notice(source/'renderer/runtime/java/NOTICE','JRE/NOTICE')
    for name in ['Chromium-BSD-LICENSE.txt','UG-Build-BSD-LICENSE.txt','UG-Core-BSD-LICENSE.txt',platform+'-credits.txt']:
        notice(ROOT/'renderer/browser-licenses'/name,'Bundled unbranded Chromium: '+name)
    notice(chrome/'ABOUT','Chromium/ABOUT')
    for name in ['COPYING.rtf','COPYRIGHT.txt']:
        notice(source/'renderer/runtime/pandoc'/name,'Pandoc/'+name)
    # Linux upstream tarball lacks license files; Pandoc embeds GPL notice in
    # --version. Distribute the upstream Windows license texts unchanged on both.
    if platform=='linux':
        for name in ['COPYING.rtf','COPYRIGHT.txt']:notice(RUNTIME_BASE/'windows/renderer/runtime/pandoc'/name,'Pandoc/'+name)
    # Chrome's ABOUT is a product identity file, not its open source notices.
    # The bundled runtime's credits.html contains the browser attribution texts.
    for f in chrome.rglob('*'):
        if f.is_file() and (legal_name(f.name) or f.name=='credits.html'):notice(f,'Chromium/'+f.relative_to(chrome).as_posix())
    for digest,item in sorted(legal.items()):
        name='licenses/'+digest+'.txt'
        dest=stage/name;dest.parent.mkdir(exist_ok=True);dest.write_bytes(item['bytes']);rules[name]='License/third-party attribution obligation: '+'; '.join(item['labels'])
    if platform=='linux':
        for name in sorted(rules):
            path=stage/name
            with path.open('rb') as handle:magic=handle.read(4)
            if magic!=b'\x7fELF':continue
            before={'bytes':path.stat().st_size,'sha256':sha(path)};allocated=elf_allocated(path)
            subprocess.run([*json.loads(A.elf_strip_command),str(path)],check=True,capture_output=True)
            if elf_allocated(path)!=allocated:raise RuntimeError('ELF allocated runtime sections changed during symbol removal: '+name)
            after={'bytes':path.stat().st_size,'sha256':sha(path)}
            if before!=after:transforms.append({'platform':platform,'path':name,'operation':'GNU strip --strip-unneeded: remove non-runtime debug/static-symbol sections','before':before,'after':after,'allocatedSectionsIdentical':True})
    generated(stage,'LICENSES.txt','MDcard 0.1.0 is AGPL-3.0-only licensed. Corresponding source: https://github.com/huwany1/MDcard/tree/'+COMMIT+'\nThird-party runtimes retain their own terms. Historical MIT releases retain their original rights.\n\n'+''.join(d+'  '+ '; '.join(v['labels'])+'\n' for d,v in sorted(legal.items())), 'License index maps each retained text to its original component(s), with exact corresponding-source link',rules)
    linux='Debian 12 x86_64 / glibc >= 2.36; X11 desktop. Required system desktop libraries: libx11-6 libxcb1 libxkbcommon0 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libgl1 libegl1 libvulkan1 libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libatspi2.0-0 libcups2 libdrm2 libdbus-1-3 libgbm1 libasound2 libpango-1.0-0 libcairo2 libfontconfig1 libfreetype6; a working desktop portal (xdg-desktop-portal plus desktop backend) for native file dialogs. These OS desktop libraries are system prerequisites, not developer runtimes.\n'
    readme='MDcard 0.1.0 — '+platform+' x86_64\n\nExtract the entire archive into a writable folder and run '+exe+'. Keep renderer alongside the program. No Node, Java, Chromium, Pandoc, Rust, compiler, npm, or developer tools need to be installed.\n\n'+(linux if platform=='linux' else 'Windows 11 x64. Run MDcard.exe. Tested Windows configurations and results are recorded in the release report.\n')+'\nSupports editing/saving Chinese text, local formulas, Mermaid/PlantUML/flowchart, and HTML/PDF/PNG/JPEG/DOCX/EPUB/Marp HTML/PDF/PPTX exports. Local-resource functions work offline; remote images and external links need networking.\n\nCtrl+O open, Ctrl+S save, Ctrl+Shift+S Save As; use the Export menu for output formats. User data/settings are created outside the archive. No autosave or crash recovery guarantee.\n\nIntegrity: manifest.json lists every payload file, byte count and SHA256. Verify this archive against the separately distributed SHA256SUMS.txt. Licenses are mapped in LICENSES.txt.\n'
    generated(stage,'README.txt',readme,'Concise end-user installation/use/system requirements and integrity instructions',rules)
    def record(name):
        p=stage/name
        return {'path':name,'bytes':p.stat().st_size,'sha256':sha(p),'reason':rules[name]}
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:files=list(pool.map(record,sorted(rules)))
    manifest={'schema':1,'product':'MDcard','version':'0.1.0','platform':platform,'architecture':'x86_64','sourceCommit':COMMIT,'binarySourceCommit':COMMIT,'createdAt':NOW,'build':{'toolchain':'rustc 1.92.0 (ded5c06cf 2025-12-08)','command':'cargo build --locked --release'+(' --target x86_64-pc-windows-gnu' if platform=='windows' else ''),'compilerDepInfoSHA256':sha(A.build_dir/('windows-deps.d' if platform=='windows' else 'linux-deps.d'))},'inventoryRule':'Exact allowed regular files = files[].path plus manifest.json itself. Manifest hash is bound by the external release index.','files':files}
    metafile=RENDERER/'vendor/build-inputs.json'
    inputs=json.loads(metafile.read_text('utf8'))['inputs']
    if any('elkjs/' in name for name in inputs):raise RuntimeError('EPL ELK unexpectedly included in renderer')
    xmldom=json.loads((RENDERER/'node_modules/@xmldom/xmldom/package.json').read_text('utf8'))['version']
    if xmldom!='0.9.12':raise RuntimeError('Patched xmldom version is not the frozen 0.9.12')
    manifest['license']='AGPL-3.0-only'
    manifest['sourceURL']='https://github.com/huwany1/MDcard/tree/'+COMMIT
    manifest['rendererBuild']={'sourceCommit':COMMIT,'packageLockSHA256':sha(ROOT/'renderer/package-lock.json'),'browserSourceSHA256':sha(ROOT/'renderer/browser.js'),'metafileSHA256':sha(metafile),'browserBundleSHA256':sha(RENDERER/'vendor/browser.js'),'elkExcluded':True,'xmldomVersion':xmldom}
    manifest['browserRuntime']=json.loads((ROOT/'renderer/export-runtime-sources.json').read_text('utf8'))[platform]['chromium']
    (stage/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    actual={p.relative_to(stage).as_posix() for p in stage.rglob('*') if p.is_file()}
    assert actual==set(rules)|{'manifest.json'}
    archive=OUT/('MDcard-0.1.0-'+platform+'-x64'+('.zip' if platform=='windows' else '.tar.gz'))
    def permissions(info):
        n=info.name.removeprefix('./');info.uid=info.gid=0;info.uname=info.gname=''
        info.mode=0o755 if info.isdir() or n in ['mdcard','renderer/runtime/node','renderer/runtime/java/bin/java','renderer/runtime/java/lib/jspawnhelper','renderer/runtime/pandoc/pandoc','renderer/runtime/chromium/chrome','renderer/runtime/chromium/chrome_crashpad_handler','renderer/runtime/chromium/chrome_sandbox'] or '.so' in n else 0o644
        return info
    if platform=='windows':
        with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
            for name in sorted(actual):z.write(stage/name,name)
    else:
        with tarfile.open(archive,'w:gz',compresslevel=6) as t:t.add(stage,arcname='.',filter=permissions)
    before=json.loads((source/'manifest.json').read_text('utf8'))['files']
    old_files={item['path']:{'bytes':item['bytes'],'sha256':item['sha256']} for item in before}
    old_files['manifest.json']={'bytes':(source/'manifest.json').stat().st_size,'sha256':sha(source/'manifest.json')}
    new_files={item['path']:{'bytes':item['bytes'],'sha256':item['sha256']} for item in files}
    new_files['manifest.json']={'bytes':(stage/'manifest.json').stat().st_size,'sha256':sha(stage/'manifest.json')}
    added=set(new_files)-set(old_files);deleted=set(old_files)-set(new_files)
    changed={name for name in set(old_files)&set(new_files) if old_files[name]!=new_files[name]}
    old_bytes=sum(item['bytes'] for item in old_files.values());new_bytes=sum(item['bytes'] for item in new_files.values())
    changes={'addedPaths':len(added),'addedBytes':sum(new_files[name]['bytes'] for name in added),'deletedPaths':len(deleted),'deletedBytes':sum(old_files[name]['bytes'] for name in deleted),'changedPaths':len(changed),'changedBytesDelta':sum(new_files[name]['bytes']-old_files[name]['bytes'] for name in changed),'netSavedBytes':old_bytes-new_bytes,'manifestIncludedInTotals':True}
    assert old_bytes+changes['addedBytes']-changes['deletedBytes']+changes['changedBytesDelta']==new_bytes
    removed=[x for x in before if x['path'] not in actual]
    stats=collections.defaultdict(lambda:{'files':0,'bytes':0})
    for x in removed:s=stats[category(x['path'])];s['files']+=1;s['bytes']+=x['bytes']
    package={'platform':platform,'version':'0.1.0','stage':platform,'archive':archive.name,'bytes':archive.stat().st_size,'sha256':sha(archive),'binary':{'path':exe,'bytes':(stage/exe).stat().st_size,'sha256':sha(stage/exe)},'manifestSHA256':sha(stage/'manifest.json'),'payloadFiles':len(actual),'sourceCommit':COMMIT,'before':{'payloadFiles':len(old_files),'payloadBytes':old_bytes},'after':{'payloadFiles':len(new_files),'payloadBytes':new_bytes},'pathChangeStats':changes,'removedCategories':dict(stats),'extraFiles':0}
    packages.append(package);print(json.dumps(package,ensure_ascii=False),flush=True)
index={'schema':1,'product':'MDcard','version':'0.1.0','sourceCommit':COMMIT,'createdAt':NOW,'status':'candidate-awaiting-platform-acceptance','passed':False,'packages':packages,'criteria':{f'C0{i}':{'status':'未验证'} for i in range(1,8)},'remoteReplacementPerformed':False}
(OUT/'MDcard-0.1.0-release.json').write_text(json.dumps(index,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
(OUT/'SHA256SUMS.txt').write_text(''.join(p['sha256']+'  '+p['archive']+'\n' for p in packages),encoding='utf8')
(OUT/'binary-transformations.json').write_text(json.dumps({'transforms':transforms,'savedBytes':sum(t['before']['bytes']-t['after']['bytes'] for t in transforms)},ensure_ascii=False,indent=2)+'\n',encoding='utf8')
