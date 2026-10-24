# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Visible end-user downgrade summary and real clear using current Linux GUI."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'native-summary-helpers', 'exec'))
import shutil
source=Path(os.environ.get('MDCARD_READING_SUMMARY_SOURCE',str(ROOT/'evidence/reading-precision/implementation/stage-b-r6-relocation/md-empty')))
WORK=source/'workspace';shutil.copyfile(source/'reading-state.json',OUT/'reading-state.json')
try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('sample.md');wait(lambda s:any(r.get('restore_status') for r in s['records']),'Persisted bounded user summary restored');key('ctrl+comma');time.sleep(.8)
    # Modal insertion can change its ScrollArea bounds during the first layout.
    # Wait for actual native coordinates to settle before pressing the header.
    previous=None
    for _ in range(12):
        ui=state('ui');r=next(c['rect'] for c in ui['controls'] if c['name']=='reading-restore-status')
        if previous==r:break
        previous=r;time.sleep(.2)
    (OUT/'before-summary-click-ui.json').write_text(json.dumps(state('ui'),indent=2))
    run('xdotool','mousemove','--window',window,str(round((r[0]+r[2])/2)),str(round((r[1]+r[3])/2)));time.sleep(.12)
    run('xdotool','click','1');time.sleep(.4);shot('01-visible-latest-downgrade')
    record=next(r for r in state()['records'] if r['id']==state()['main_id']);summary=record['restore_status'];assert summary['main/preview']['tier']=='invalid'
    control("Clear this document's reading positions");time.sleep(1.2);assert all(not r.get('restore_status') for r in state()['records']);shot('02-cleared-summary-remains-cleared');key('Escape');stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),visible_summary=summary,clear_survives_modal_idle=True,manuscript_empty_unchanged=(WORK/'sample.md').read_bytes()==b'')
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
