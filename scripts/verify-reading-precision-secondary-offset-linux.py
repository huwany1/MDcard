# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native large-source reflow/reopen exact-offset and measured-prefix regression."""
from pathlib import Path
common=Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')],'secondary-offset-native-helpers','exec'))
WORK=OUT/'workspace';WORK.mkdir(exist_ok=True)
SCALE=float(os.environ.get('MDCARD_SECONDARY_OFFSET_DPI_SCALE','1'))
if SCALE!=1:
    ENV['WINIT_X11_SCALE_FACTOR']=str(SCALE)
    original_run=run
    def run(*args,**kwargs):
        args=list(args)
        if args and args[0]=='xdotool':
            for at,value in enumerate(args):
                if value=='mousemove' and args[at+1]=='--window':
                    args[at+3]=str(round(float(args[at+3])*SCALE));args[at+4]=str(round(float(args[at+4])*SCALE))
                if value=='windowsize':
                    args[at+2]=str(round(float(args[at+2])*SCALE));args[at+3]=str(round(float(args[at+3])*SCALE))
        return original_run(*args,**kwargs)
    (OUT/'settings.json').write_text(json.dumps(dict(language='en',appearance='Light',reduce_motion=True,font_size=20.8)))
source=''.join(f'Logical source {i:05} identifier-{i:05}：'+('中文🙂 é 👩🏽‍💻 👍🏿 literal **source** [label](url) a long logical line wraps into different visual rows. '*4)+'\n' for i in range(int(os.environ.get('MDCARD_SECONDARY_OFFSET_LINES','1600'))))
if os.environ.get('MDCARD_SECONDARY_OFFSET_PUBLIC_SOURCE')=='1':
    source=json.loads((ROOT/'evidence/reading-precision/master/precision-b-r19-secondary-critical-lifecycle-write/search.json').read_text(encoding='utf-8-sig'))['secondary']['text']
assert 64*1024<len(source.encode())<16*1024*1024
(WORK/'large.txt').write_text(source);(WORK/'ordinary.md').write_text('# Primary remains independent\n\nBody');(WORK/'other.md').write_text('# Other secondary\n\nBody')

def calibration(reading):
    record=next(record for record in reading['records'] if record['id']==reading['secondary_id'])
    return record['calibration'].get('secondary/source',{})

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('ordinary.md');mode('Write');control('source-wrap');control('source-wrap-on');quick('large.txt',True);precise('secondary/source')
    if SCALE!=1:
        r=view('secondary/source')['viewport_rect'];difference=(r[2]-r[0])-405.4;x=r[0]-16;y=(r[1]+r[3])/2
        run('xdotool','mousemove','--window',window,str(x),str(y),'mousedown','1');time.sleep(.08);run('xdotool','mousemove','--window',window,str(x+difference),str(y));time.sleep(.12);run('xdotool','mouseup','1');time.sleep(.4)
    for _ in range(20):
        g=precise('secondary/source')['geometry']
        if g['logical_line']==1:break
        wheel('secondary/source',1 if g['logical_line']<1 else -1)
    assert g['logical_line']==1,'Native logical-line1 target was not reached'
    font_precise('secondary/source');before=precise('secondary/source');shot('01-after-font-before-switch')
    before_calibration=calibration(state());
    if SCALE!=1:
        assert abs((before['viewport_rect'][2]-before['viewport_rect'][0])-405.4)<.5,before['viewport_rect']
        assert ':14.6:' in before['position']['layout'],before['position']['layout']
    assert not any(line==0 for line,height in before_calibration['heights']),'Fixture must leave first logical-line height unmeasured after real font reflow'
    quick('other.md',True);settle('secondary/source');quick('large.txt',True);after=precise('secondary/source');shot('02-after-reopen')
    error=[abs(before['position']['offset'][i]-after['position']['offset'][i]) for i in range(2)]
    exact=dict(same_layout=before['position']['layout']==after['position']['layout'],same_fingerprint=before['position']['fingerprint']==after['position']['fingerprint'],before_offset=before['position']['offset'],after_offset=after['position']['offset'],offset_error_logical_px=error,same_original_byte=before['geometry']['source_byte']==after['geometry']['source_byte'],viewport_y_error=abs(before['geometry']['viewport_y']-after['geometry']['viewport_y']),before_calibration=before_calibration,after_calibration=calibration(state()))
    (OUT/'comparison.json').write_text(json.dumps(exact,ensure_ascii=False,indent=2)+'\n')
    assert exact['same_layout'] and exact['same_fingerprint'];assert max(error)<=2,exact
    assert exact['same_original_byte'] and exact['viewport_y_error']<=2,exact
    stop();(OUT/'result.json').write_text(json.dumps(dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),source_sha256=hashlib.sha256(source.encode()).hexdigest(),source_bytes=len(source.encode()),dpi_scale=SCALE,comparison=exact,method='Native source wrap/font/wheel and secondary A-other-A. Actual original Galley/y and identical-layout raw xy are independently checked; calibration is pure observation.'),ensure_ascii=False,indent=2)+'\n')
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error)),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for service in reversed(services):
        service.terminate()
        try:service.wait(timeout=5)
        except subprocess.TimeoutExpired:service.kill();service.wait(timeout=5)
