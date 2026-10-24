# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Actual old/new default-path isolation plus future-file writer protection."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'native-schema-helpers', 'exec'))
new_exe=EXE;old_exe=ROOT/'artifacts/reading-precision/packages/precision-a-20261023-r10/linux/mdcard'
ENV.pop('MDCARD_READING_STATE_PATH',None);ENV.pop('MDCARD_READING_GEOMETRY_REPORT',None);ENV['XDG_STATE_HOME']=str(OUT/'local-state')
old_path=OUT/'local-state/MDcard/reading-state.json';new_path=old_path.with_name('reading-state-v3.json')
try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    EXE=old_exe;start('ordinary.md');mode('Read');wheel('main/preview',8);old_position=settle('main/preview');stop()
    old_bytes=old_path.read_bytes();assert json.loads(old_bytes)['version']==2;assert not new_path.exists()
    EXE=new_exe;start('ordinary.md');wait(lambda s:s.get('schema')==3,'New process schema3');restored=settle('main/preview');assert max(abs(old_position['offset'][i]-restored['offset'][i]) for i in range(2))<=2
    time.sleep(.8);shot('01-new-runtime-migrated-default-history');stop()
    assert old_path.read_bytes()==old_bytes,'Migration modified old runtime file'
    new_bytes=new_path.read_bytes();assert json.loads(new_bytes)['version']==3
    EXE=old_exe;start('ordinary.md');wait(lambda s:s.get('schema')==2,'Legacy runtime schema2');wheel('main/preview',6);shot('02-old-runtime-writes-its-own-path');stop()
    assert old_path.read_bytes()!=old_bytes,'Legacy real writer did not execute'
    assert new_path.read_bytes()==new_bytes,'Legacy default writer touched schema3'
    future=OUT/'future-state.json';future_bytes=b'{"version":999,"future_metadata":{"must":"survive"}}';future.write_bytes(future_bytes)
    ENV['MDCARD_READING_STATE_PATH']=str(future);EXE=new_exe;start('ordinary.md');wait(lambda s:s.get('schema')==3,'Current future-file reader');mode('Read');wheel('main/preview',7)
    wait(lambda s:s['writes'].get('error') is not None and 'Unsupported' in s['writes']['error'],'Current writer reports unsupported version');error=state()['writes']['error'];memory=settle('main/preview');assert memory['offset'][1]>100
    shot('03-future-file-protected-memory-position-usable');stop();assert future.read_bytes()==future_bytes
    result=dict(passed=True,new_binary_sha256=hashlib.sha256(new_exe.read_bytes()).hexdigest(),legacy_binary_sha256=hashlib.sha256(old_exe.read_bytes()).hexdigest(),legacy_default_path=str(old_path),new_default_path=str(new_path),legacy_schema=2,new_schema=3,original_offset=old_position['offset'],migrated_offset=restored['offset'],migration_did_not_modify_legacy=True,old_writer_executed_without_touching_schema3=True,future_file_sha256_unchanged=hashlib.sha256(future_bytes).hexdigest(),future_writer_error=error,future_memory_offset=memory['offset'],boundary='Default paths isolate incompatible old writers, including after new runtime closes. Explicitly forcing an older unpatched executable to the new path with MDCARD_READING_STATE_PATH is unsupported; the new app cannot alter released old code. Current writer refuses existing unreadable/unsupported files and preserves memory and exact original bytes.')
    (OUT/'result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
except Exception as error:
    try:
        if app is not None and app.poll() is None:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
