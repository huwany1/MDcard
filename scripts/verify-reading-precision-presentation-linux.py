# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native GUI smoke for revision-cached chrome, original guards and preview/TOC refresh."""
from pathlib import Path
common = Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'presentation-native-helpers', 'exec'))
WORK = OUT/'workspace';WORK.mkdir(exist_ok=True)
original = '# Cache smoke\n\n中文 é 👩🏽‍💻 original'
other = '# Secondary source\n\nIndependent buffer'
(WORK/'ordinary.md').write_text(original)
(WORK/'other.md').write_text(other)
measurements=[]

def paste(text):
    subprocess.run(['xclip','-selection','clipboard'],input=text.encode(),env=ENV,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    key('ctrl+v')

def focus(name):
    rect=view(name)['viewport_rect'];click((rect[0]+rect[2])/2,(rect[1]+rect[3])/2)

def record(name,expected,dirty,secondary=False):
    wait(lambda s:s.get('secondary',{}).get('text')==expected if secondary else s.get('text')==expected,'Actual edited source', 'search')
    if secondary:
        wait(lambda s:s.get('secondary',{}).get('dirty')==dirty,'Secondary dirty display','search')
    else:
        wait(lambda s:s.get('dirty')==dirty,'Primary dirty display','ui')
    shot(name)
    measurements.append(dict(name=name,secondary=secondary,dirty=dirty,expected_logical_lines=len(expected.splitlines()),expected_unicode_scalars=len(expected),text_sha256=hashlib.sha256(expected.encode()).hexdigest(),screenshot=name+'.png'))

try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(OUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT));time.sleep(.6)
    start('ordinary.md');mode('Write');settle('main/source')
    record('01-warmed-clean',original,False)
    focus('main/source');key('ctrl+End');paste('\nAdded row🙂')
    edited=original+'\nAdded row🙂';record('02-main-edit',edited,True)
    key('ctrl+z');record('03-main-undo-clean',original,False)
    key('ctrl+shift+z');record('04-main-redo',edited,True)
    key('ctrl+s');record('05-main-save-clean',edited,False)
    assert (WORK/'ordinary.md').read_text()==edited
    control('navigation:Outline')
    updated='# Changed heading\n\nUpdated preview 中文 é 👩🏽‍💻'
    focus('main/source');key('ctrl+a');paste(updated)
    record('06-heading-edit',updated,True)
    mode('Read');precise('main/preview');time.sleep(.7);shot('07-preview-outline-updated')
    preview=view('main/preview');g=preview['geometry'];assert updated.encode()[g['source_byte']:g['source_end']].decode()==g['source_text']
    mode('Write');key('ctrl+s');record('08-heading-save',updated,False)
    quick('other.md',True);wait(lambda s:s.get('secondary') is not None,'Secondary opened','search');settle('secondary/source')
    record('09-secondary-clean',other,False,True)
    focus('secondary/source');key('ctrl+End');paste('\nSecond row')
    second=other+'\nSecond row';record('10-secondary-edit',second,True,True)
    key('ctrl+s');record('11-secondary-worker-save',second,False,True)
    assert (WORK/'other.md').read_text()==second
    assert state('search')['text']==updated
    stop()
    result=dict(passed=True,binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),cases=measurements,main_saved_sha256=hashlib.sha256((WORK/'ordinary.md').read_bytes()).hexdigest(),secondary_saved_sha256=hashlib.sha256((WORK/'other.md').read_bytes()).hexdigest(),method='Actual native source paste/undo/redo/save, outline/Read switch, secondary worker save. Source and UI dirty flags independently checked. Line/scalar expectations accompany actual screenshots; screenshots require visual review, not reported as OCR automation. This is not a performance sample.')
    (OUT/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False,indent=2))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (OUT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=measurements),ensure_ascii=False,indent=2));raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for service in reversed(services):
        service.terminate()
        try:service.wait(timeout=5)
        except subprocess.TimeoutExpired:service.kill();service.wait(timeout=5)
