# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent native X11 checks for pending drafts and active-pane replacement."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

ROOT = Path('/work')
OUT = ROOT / 'evidence/workbench-redesign-2026-10-22/master' / os.environ.get('MDCARD_WORKBENCH_RUN','linux-r2')
OUT.mkdir(parents=True, exist_ok=True)
(OUT / 'result.json').unlink(missing_ok=True)
EXE = Path(os.environ.get('MDCARD_AUDIT_EXE', '/work/artifacts/bug-audit/bin/mdcard'))
RENDERER = EXE.parent / 'renderer'
if not RENDERER.is_dir():
    RENDERER = ROOT / 'dist/linux/renderer'
ENV = dict(os.environ, DISPLAY=':146', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2',
           MDCARD_RENDERER_DIR=str(RENDERER), MDCARD_SETTINGS_PATH=str(OUT / 'settings.json'),
           MDCARD_RICH_REPORT=str(OUT / 'rich.json'), MDCARD_SEARCH_REPORT=str(OUT / 'state.json'),
           MDCARD_UI_REPORT=str(OUT / 'ui.json'),
           XDG_RUNTIME_DIR='/tmp/mdcard-ui-master-package-x11')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
app = None
services = []
window = None


def run(*args, **kwargs):
    return subprocess.run(args, env=ENV, check=True, **kwargs)


def key(*keys):
    run('xdotool', 'key', '--clearmodifiers', *keys)
    time.sleep(.15)


def state(filename='state.json'):
    for _ in range(30):
        try:
            return json.loads((OUT / filename).read_text())
        except (ValueError, OSError):
            time.sleep(.02)
    raise RuntimeError('No current report: ' + filename)


def await_state(predicate, message, filename='state.json'):
    until = time.monotonic() + 15
    while time.monotonic() < until:
        current = state(filename)
        if predicate(current):
            return current
        time.sleep(.05)
    raise AssertionError(message)


def click(x, y, pause=.2):
    run('xdotool', 'mousemove', '--window', window, str(round(x)), str(round(y)))
    time.sleep(.18)
    run('xdotool', 'click', '1')
    time.sleep(pause)


def control(name, filename='state.json'):
    current = await_state(lambda s: any(c['name'] == name for c in s['controls']), 'Missing ' + name, filename)
    rect = next(c['rect'] for c in reversed(current['controls']) if c['name'] == name)
    click((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2)


def clipboard(text):
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=text.encode(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def paste(text):
    clipboard(text)
    key('ctrl+v')


def fill(name, text):
    control(name)
    key('ctrl+a')
    paste(text)


def shot(name):
    time.sleep(.3)
    run('import', '-window', window, str(OUT / (name + '.png')))


def rapid_command(value, action):
    rect=next(c['rect'] for c in state('ui.json')['controls'] if c['name']==action)
    control('floating-editor', 'rich.json')
    key('ctrl+a')
    clipboard(value)
    start = time.monotonic()
    run('xdotool', 'key', '--clearmodifiers', 'ctrl+v')
    time.sleep(.04)
    run('xdotool','mousemove','--window',window,str(round((rect[0]+rect[2])/2)),str(round((rect[1]+rect[3])/2)))
    time.sleep(.06)
    run('xdotool','click','1')
    elapsed = (time.monotonic() - start) * 1000
    time.sleep(.6)
    assert elapsed < 250, ('Input missed debounce window', elapsed)
    return elapsed


try:
    services.append(subprocess.Popen(['Xvfb', ENV['DISPLAY'], '-screen', '0', '1600x1100x24'], stdout=(OUT / 'xvfb.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.7)
    services.append(subprocess.Popen(['openbox'], env=ENV, stdout=(OUT / 'openbox.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.7)
    (OUT / 'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', gpu_acceleration=True, reduce_motion=True, font_size=15)))
    workspace = OUT / 'workspace'
    workspace.mkdir(exist_ok=True)
    fixture = workspace / 'main.md'
    second = workspace / 'second.md'
    source = '# Draft safety\n\nBefore $x^2$ after.\ncat-main\n'
    other = '# Second\ncat-secondary\n'
    fixture.write_text(source)
    second.write_text(other)
    app = subprocess.Popen([str(EXE), str(fixture)], env=ENV, stdout=(OUT / 'app.log').open('wb'), stderr=subprocess.STDOUT)
    for _ in range(100):
        time.sleep(.1)
        found = subprocess.run(['xdotool', 'search', '--pid', str(app.pid), '--name', 'MDcard'], env=ENV, capture_output=True)
        if found.returncode == 0:
            window = found.stdout.decode().splitlines()[0]
            break
    assert window, 'No native application window'
    run('xdotool', 'windowsize', window, '1240', '820', 'windowactivate', '--sync', window)
    time.sleep(2)
    await_state(lambda s: s['counters']['completed'] > 0, 'Formula did not render', 'rich.json')
    control('node-0', 'rich.json')
    save_ms = rapid_command('z^3', 'Save')
    assert fixture.read_text() == source.replace('x^2', 'z^3'), 'Toolbar Save omitted formula'
    shot('01-pending-formula-saved')
    new_ms = rapid_command('q^4', '+  New')
    pending = state()
    assert pending['text'] == source.replace('x^2', 'q^4') and pending['path'].endswith('main.md'), 'New discarded pending formula'
    shot('02-new-keeps-dirty-formula')
    key('Escape', 'ctrl+s', 'Escape')
    main_saved = fixture.read_bytes()
    assert b'$q^4$' in main_saved
    key('ctrl+p')
    fill('quick-query', 'second.md')
    await_state(lambda s: len(s['quick']) == 1 and not s['query_pending'] and s['quick_query'] == 'second.md', 'Second file query not settled')
    key('shift+Return')
    await_state(lambda s: bool(s['secondary']), 'No secondary pane')
    control('secondary-editor')
    key('ctrl+h')
    fill('find-query', 'cat')
    fill('replace-query', 'dog')
    found = await_state(lambda s: s['results'] == 1 and not s['error'], 'Secondary search failed')
    assert found['groups'][0]['path'].endswith('second.md'), 'Search targeted wrong pane'
    control('preview')
    await_state(lambda s: s['preview_files'] == 1, 'No replacement diff')
    shot('03-secondary-diff')
    control('apply')
    replaced = await_state(lambda s: not s['open'] and not s['busy'], 'Replacement did not complete')
    assert replaced['secondary']['text'] == other.replace('cat', 'dog') and replaced['text'].encode() == main_saved
    key('ctrl+z')
    await_state(lambda s: s['secondary']['text'] == other, 'One undo did not restore secondary')
    key('ctrl+shift+z')
    await_state(lambda s: s['secondary']['text'] == other.replace('cat', 'dog'), 'Redo did not restore replacement')
    key('ctrl+s')
    await_state(lambda s: not s['busy'] and not s['secondary']['dirty'], 'Secondary save failed')
    assert second.read_text() == other.replace('cat', 'dog') and fixture.read_bytes() == main_saved
    shot('04-secondary-redo-saved')
    key('ctrl+q')
    app.wait(timeout=8)
    assert app.returncode == 0
    digest = hashlib.sha256()
    with EXE.open('rb') as binary:
        for chunk in iter(lambda: binary.read(1024 * 1024), b''):
            digest.update(chunk)
    result = dict(verification_id=str(uuid.uuid4()), passed=True, binary_sha256=digest.hexdigest(),
                  exe=str(EXE), renderer_dir=str(RENDERER),
                  platform='Debian 12 / X11 / Xvfb / Openbox / Mesa software OpenGL',
                  save_click_ms=save_ms, new_click_ms=new_ms, pending_save=True, pending_new_guard=True,
                  secondary_search_replace=True, secondary_undo_redo=True, main_unchanged=True, exit_code=app.returncode)
    (OUT / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
finally:
    if app and app.poll() is None:
        app.kill()
        app.wait()
    for process in reversed(services):
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
