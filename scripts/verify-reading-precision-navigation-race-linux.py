# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real Search/Outline navigation during a natural version restore.

Reports are observation only. Fixture transformations and navigation targets are
computed independently from manuscript text. No application command injection.
"""
from pathlib import Path

common = Path('/work/scripts/verify-reading-precision-cancellation-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'navigation-race-native-helpers', 'exec'))
DRIVER_SHA = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
HELPER_SHA = hashlib.sha256(Path('/work/scripts/verify-reading-precision-cancellation-linux.py').read_bytes()).hexdigest()
(OUT / 'driver.py').write_bytes(Path(__file__).read_bytes())
(OUT / 'imported-cancellation-prefix.py').write_text(common[:common.index('\ntry:\n    services.append')])

KIND = os.environ.get('MDCARD_NAVIGATION_KIND', 'search')
assert KIND in ('search', 'outline')
WORK = OUT / 'workspace'
WORK.mkdir(exist_ok=True)
ENV['MDCARD_READING_GEOMETRY_REPORT'] = '1'
for directory, suffix in [('XDG_STATE_HOME', 'state'), ('XDG_CONFIG_HOME', 'config'),
                          ('XDG_DATA_HOME', 'data'), ('XDG_CACHE_HOME', 'cache')]:
    ENV[directory] = str(OUT / suffix)
if KIND == 'outline':
    ENV.pop('MDCARD_SEARCH_REPORT', None)
original = ''.join((f'## Navigation section {i // 100:05}\n\n' if i % 100 == 0 else '')
                   + f'NAVHIT-{i:05} original record {i:05}: '
                   + ('Immutable 中文 e\u0301 👩🏽‍💻 source context for this logical line. ' * 4)
                   + '\n' for i in range(30000))
prefix = ('An independently inserted introduction with unrelated material. ' * (len(original) // 165 + 1)) + '\n\n'
changed = prefix + original
paste_at = original.index('\n')
paste_text = '\n\n' + prefix[:65536] + '\n\n'
if KIND == 'outline':
    # The first heading stays at source byte zero. Its already-painted TOC
    # control can therefore be used while a real in-app version match runs.
    changed = original[:paste_at] + paste_text + original[paste_at:]
heavy = WORK / 'heavy.md'
heavy.write_text(original)
(WORK / 'tiny.md').write_text('# Separate initial manuscript\n\nA small independent document.\n')
assert len(original.encode()) < 16 * 1024 * 1024
assert len(changed.encode()) < 16 * 1024 * 1024
query = 'NAVHIT-25000'
target_text = query if KIND == 'search' else '## Navigation section 00000'
expected = changed.encode().index(target_text.encode())
lib.XWarpPointer.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong,
                            ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint,
                            ctypes.c_int, ctypes.c_int]
lib.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
lib.XKeysymToKeycode.restype = ctypes.c_uint
xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
observations = []
qualification_samples = []


def center(rect):
    return [(rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2]


def move_to(rect):
    x, y = center(rect)
    lib.XWarpPointer(display, 0, int(window), 0, 0, 0, 0, round(x), round(y))
    lib.XSync(display, 0)


def native_click(rect=None, hold=.001):
    if rect is not None:
        move_to(rect)
    begin = time.perf_counter_ns()
    xtst.XTestFakeButtonEvent(display, 1, 1, 0)
    lib.XFlush(display)
    time.sleep(hold)
    xtst.XTestFakeButtonEvent(display, 1, 0, 0)
    lib.XSync(display, 0)
    return begin, time.perf_counter_ns()


def rect_for(name, report='ui'):
    current = wait(lambda s: any(c['name'] == name for c in s['controls']), 'Missing ' + name, report)
    return next(c['rect'] for c in reversed(current['controls']) if c['name'] == name)


def proof_ready(s):
    v = s['views'].get('main/source', {})
    anchor = ((v.get('position') or {}).get('text_anchor') or {})
    return anchor.get('source', {}).get('proof') is not None and v.get('proof_pending') is None and v.get('pending') is None


def running_match():
    until = time.perf_counter_ns() + 20_000_000_000
    while time.perf_counter_ns() < until:
        s = state()
        v = s['views'].get('main/source', {})
        qualification_samples.append(dict(monotonic_ns=time.perf_counter_ns(), revision=s['main_revision'],
            pending=v.get('pending') is not None, match_pending=v.get('match_pending'),
            offset=(v.get('position') or {}).get('offset')))
        for task in reversed(s.get('background_tasks', [])):
            if task['kind'] == 'relocation-match' and task['metadata']['view'] == 'main/source' and task['running'] and task['progress_checkpoints'] >= 10 and task['cancel_requested_monotonic_ns'] == 0 and v.get('pending') is not None:
                return s, task, time.perf_counter_ns()
        time.sleep(.001)
    raise AssertionError('No naturally running match and pending source restore')


def expected_glyph(s):
    v = s['views'].get('main/source', {})
    rect = v.get('viewport_rect')
    for g in v.get('visible_text', []):
        if g['source_byte'] == expected and g['source_text'] == target_text[0] and rect and g['rect'][3] > rect[1] and g['rect'][1] < rect[3] and g['rect'][2] > rect[0] and g['rect'][0] < rect[2]:
            return g
    return None


try:
    services.append(subprocess.Popen(['Xvfb', ENV['DISPLAY'], '-screen', '0', '1600x1100x24'], stdout=(OUT / 'xvfb.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.6)
    services.append(subprocess.Popen(['openbox'], env=ENV, stdout=(OUT / 'openbox.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.6)
    display = lib.XOpenDisplay(ENV['DISPLAY'].encode())
    assert display
    start('tiny.md')
    mode('Write')
    prior = state()['main_id']
    control('tree:heavy.md')
    wait(lambda s: s['main_id'] != prior and s['views'].get('main/source', {}).get('position') is not None, 'Initial heavy document')
    if KIND == 'outline':
        control('navigation:Outline')
        navigation_rect = rect_for('outline:0')
        source_rect = state()['views']['main/source']['viewport_rect']
        native_click(source_rect, hold=.06)
        key('ctrl+Home')
        key('End')
    wheel('main/source', 18)
    before = wait(proof_ready, 'Original source proof completed')
    old_position = before['views']['main/source']['position']
    assert old_position['offset'][1] > 100
    (OUT / 'original-position.json').write_text(json.dumps(before, ensure_ascii=False, indent=2))
    if KIND == 'search':
        stop()
        heavy.write_text(changed)
    if KIND == 'search':
        start('tiny.md')
        mode('Write')
    if KIND == 'search':
        key('ctrl+shift+f')
        control('find-query', 'search')
        key('ctrl+a')
        run('xdotool', 'type', '--clearmodifiers', '--delay', '1', query)
        ready = wait(lambda s: s.get('results') == 1 and any(c['name'] == 'result-0-0' for c in s['controls']), 'Independent unique search hit', 'search')
        navigation_rect = next(c['rect'] for c in ready['controls'] if c['name'] == 'result-0-0')
    if KIND == 'outline':
        # Hover does not change TextEdit focus. Paste is a real clipboard
        # input at the independent head caret while its viewport stays far
        # below; the old saved reading target must remain nonzero afterwards.
        move_to(navigation_rect)
        run('xclip', '-selection', 'clipboard', '-loops', '1', input=paste_text.encode())
        run('xdotool', 'key', '--clearmodifiers', 'ctrl+v')
    else:
        tiny_id = state()['main_id']
        native_click(rect_for('tree:heavy.md'), hold=.06)
        # Keep the pointer on the opening control until its release has been
        # processed, before moving it onto a different navigation control.
        until = time.monotonic() + 10
        while state()['main_id'] == tiny_id and time.monotonic() < until:
            time.sleep(.001)
        assert state()['main_id'] != tiny_id, 'Native heavy open did not commit'
        move_to(navigation_rect)
    pending, task, observed = running_match()
    pending_view = pending['views']['main/source']
    pending_history = (pending_view.get('pending') or {}).get('position') or pending_view.get('position')
    assert pending_history is not None, 'No saved history in the observed pending restore'
    if KIND == 'outline':
        assert pending['main_id'] == before['main_id']
        assert pending['main_revision'] > before['main_revision']
        assert pending_history['text_anchor']['source']['source_byte'] > 0, 'Paste reset the old reading target to the TOC target'
        assert task['metadata']['revision'] == pending['main_revision']
    injection, injected = native_click()
    # Persist evidence only after native input has been dispatched.
    (OUT / 'before-navigation.json').write_text(json.dumps(pending, ensure_ascii=False, indent=2))
    task_id = task['task_id']
    until = time.monotonic() + 25
    done = None
    while time.monotonic() < until:
        s = state()
        current_task = next((t for t in s.get('background_tasks', []) if t['task_id'] == task_id), None)
        glyph = expected_glyph(s)
        v = s['views'].get('main/source', {})
        observations.append(dict(monotonic_ns=time.perf_counter_ns(), pending=v.get('pending') is not None,
                                 reason=v.get('reason'), task=current_task,
                                 glyph=glyph, offset=(v.get('position') or {}).get('offset')))
        if glyph and current_task and not current_task['running'] and current_task['finished_monotonic_ns'] > 0 and v.get('pending') is None:
            done = s
            break
        time.sleep(.005)
    assert done is not None, 'Explicit navigation target was not actually drawn after the old job finished'
    final_task = next(t for t in done['background_tasks'] if t['task_id'] == task_id)
    assert final_task['cancel_requested_while_running'], 'Actual processing did not cancel a still-running matching task; pre-input running alone is insufficient'
    assert final_task['metadata']['buffer_id'] == task['metadata']['buffer_id']
    assert done['main_id'] == pending['main_id']
    if KIND == 'outline':
        key('ctrl+s')
        wait(lambda s: heavy.read_text() == changed, 'Real pasted manuscript saved')
    assert heavy.read_text() == changed
    glyph = expected_glyph(done)
    assert glyph['grapheme_range'] == dict(start=expected, end=expected + 1)
    shot('01-navigation-wins')
    # A genuine later layout change must not reapply the canceled history.
    run('xdotool', 'windowsize', window, '1100', '820')
    wait(lambda s: expected_glyph(s) is not None and s['views']['main/source'].get('pending') is None, 'Navigation survives a later width layout')
    time.sleep(.3)
    tail = []
    for _ in range(23):
        s = state()
        g = expected_glyph(s)
        v = s['views']['main/source']
        assert g is not None, 'Late history pulled the navigated target out of view'
        assert v.get('pending') is None
        tail.append(dict(monotonic_ns=time.perf_counter_ns(), glyph=g, offset=v['position']['offset'], reason=v['reason']))
        time.sleep(.1)
    assert max(abs(x['glyph']['viewport_y'] - tail[0]['glyph']['viewport_y']) for x in tail) <= 2
    assert max(abs(x['offset'][1] - tail[0]['offset'][1]) for x in tail) <= 2
    shot('02-stable-after-layout')
    stop()
    result = dict(passed=True, kind=KIND, binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),
                  driver_sha256=DRIVER_SHA, imported_cancellation_helper_sha256=HELPER_SHA,
                  original_sha256=hashlib.sha256(original.encode()).hexdigest(), changed_sha256=hashlib.sha256(changed.encode()).hexdigest(),
                  original_bytes=len(original.encode()), changed_bytes=len(changed.encode()),
                  expected_source_byte=expected, expected_source_end=expected + 1, expected_logical_line=changed.encode()[:expected].count(b'\n'),
                  old_history_source_byte=old_position['text_anchor']['source']['source_byte'],
                  native_navigation_control_rect=navigation_rect,
                  pending_history_source_byte=pending_history['text_anchor']['source']['source_byte'],
                  pending_restore_target=pending_view.get('target'),
                  edit_method='Native clipboard paste after unchanged first heading' if KIND == 'outline' else 'Closed-process external prefix insertion',
                  observed_running_monotonic_ns=observed, native_input_begin_ns=injection, native_input_end_ns=injected,
                  observation_to_input_ms=(injection-observed)/1e6,
                  before_task=task, after_task=final_task, actual_glyph=glyph,
                  processing_qualification='cancel_requested_while_running proves the real native navigation canceled a matching computation still running when processed',
                  stability_seconds=2.3, tail=tail, observations=observations,
                  qualification_samples=qualification_samples,
                  boundary='Correctness observation with synchronous diagnostic reports; not the separate 100ms cancellation performance cohort or display latency.')
    (OUT / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps({k:v for k,v in result.items() if k not in ('tail', 'observations')}, ensure_ascii=False))
except Exception as error:
    screenshot_error = None
    try:
        if app is not None and app.poll() is None and window is not None:
            shot('failure-observation')
    except Exception as capture_error:
        screenshot_error = repr(capture_error)
    (OUT / 'failure.json').write_text(json.dumps(dict(error=repr(error), kind=KIND, driver_sha256=DRIVER_SHA,
                imported_cancellation_helper_sha256=HELPER_SHA, observations=observations,
                screenshot_error=screenshot_error, qualification_samples=qualification_samples), ensure_ascii=False, indent=2))
    raise
finally:
    if display:
        lib.XCloseDisplay(display)
    if app is not None and app.poll() is None:
        app.kill()
        app.wait(timeout=5)
    for process in reversed(services):
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
