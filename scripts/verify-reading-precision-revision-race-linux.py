# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native typing and view changes during naturally running version matching.

Only read-only reports observe tasks. Expected manuscript bytes are constructed
independently, and checked through an actual native Save; no position/job input.
"""
from pathlib import Path
common = Path('/work/scripts/verify-reading-precision-cancellation-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'revision-race-native-helpers', 'exec'))

WORK = OUT / 'workspace'
WORK.mkdir(exist_ok=True)
ENV.pop('MDCARD_READING_GEOMETRY_REPORT', None)
ENV.pop('MDCARD_SEARCH_REPORT', None)  # Avoid serializing a full manuscript during the race.
original = ''.join(f'Paragraph record {i:05}: ' + ('Original 中文 e\u0301 👩🏽‍💻 immutable source information. ' * 7) + '\n\n' for i in range(20000))
prefix = ('A substantial independently inserted introduction without copied body information. ' * (len(original) // 220 + 1)) + '\n\n'
changed = prefix + original
assert 9 * 1024 * 1024 < len(original.encode()) < 16 * 1024 * 1024
assert len(changed.encode()) < 16 * 1024 * 1024
heavy = WORK / 'heavy.md'
heavy.write_text(original)
(WORK / 'tiny.md').write_text('# Independent small initial document\n\nInitial source.\n')
measurements = []
lib.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
lib.XKeysymToKeycode.restype = ctypes.c_uint
xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
lib.XWarpPointer.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_int, ctypes.c_int]


def launch_fast():
    global app, window
    app = subprocess.Popen([str(EXE), str(heavy)], env=ENV, stdout=(OUT / 'app.log').open('ab'), stderr=subprocess.STDOUT)
    for _ in range(1200):
        found = subprocess.run(['xdotool', 'search', '--all', '--onlyvisible', '--pid', str(app.pid), '--name', 'MDcard'], env=ENV, capture_output=True)
        if found.returncode == 0:
            window = found.stdout.decode().splitlines()[0]
            break
        time.sleep(.01)
    else:
        raise AssertionError('No mapped native window')
    run('xdotool', 'windowsize', window, '1240', '820', 'windowactivate', '--sync', window)


def running_source_match():
    until = time.perf_counter_ns() + 15_000_000_000
    while time.perf_counter_ns() < until:
        before = state()
        for task in reversed(before.get('background_tasks', [])):
            if task['kind'] == 'relocation-match' and task['metadata']['view'] == 'main/source' and task['running'] and task['progress_checkpoints'] >= 100 and task['cancel_requested_monotonic_ns'] == 0:
                return before, task, time.perf_counter_ns()
        time.sleep(.001)
    raise AssertionError('No naturally running source match with actual computation checkpoints')


def proof_for(s, name):
    view = s['views'].get(name, {})
    return ((view.get('position') or {}).get('text_anchor') or {}).get('source', {}).get('proof') is not None and view.get('proof_pending') is None and view.get('pending') is None


def proof_ready(s):
    return proof_for(s, 'main/source')


def input_key(code):
    xtst.XTestFakeKeyEvent(display, code, 1, 0)
    xtst.XTestFakeKeyEvent(display, code, 0, 0)
    lib.XSync(display, 0)


try:
    services.append(subprocess.Popen(['Xvfb', ENV['DISPLAY'], '-screen', '0', '1600x1100x24'], stdout=(OUT / 'xvfb.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.6)
    services.append(subprocess.Popen(['openbox'], env=ENV, stdout=(OUT / 'openbox.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.6)
    display = lib.XOpenDisplay(ENV['DISPLAY'].encode())
    assert display
    code = lib.XKeysymToKeycode(display, ord('k'))
    assert code

    # Open heavy from a different already-Write document, so heavy has no prior
    # preview history. Source/preview therefore have distinguishable targets.
    start('tiny.md')
    mode('Write')
    prior_id = state()['main_id']
    control('tree:heavy.md')
    wait(lambda s: s['main_id'] != prior_id and s['views'].get('main/source', {}).get('position') is not None, 'Heavy source opened through native tree')
    rect = view('main/source')['viewport_rect']
    click((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2)
    key('ctrl+Home')
    wait(lambda s: s['selection'] == [0, 0], 'Actual source caret at head', 'ui')
    wheel('main/source', 12)
    wait(proof_ready, 'Original source evidence ready')
    assert view('main/source')['position']['offset'][1] > 100
    assert state('ui')['selection'] == [0, 0]
    assert heavy.read_text() == original
    assert state('ui')['focused'] != 'None', 'Actual source focus required before native Paste'
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=prefix.encode(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run('xdotool', 'key', '--clearmodifiers', 'ctrl+v')
    before, task, observed = running_source_match()
    assert task['metadata']['revision'] > 0, 'Native Paste must have created a new content revision'
    injection = time.perf_counter_ns()
    input_key(code)  # Continue editing the actually focused source, without a cancelling mouse click.
    input_end = time.perf_counter_ns()
    (OUT / '01-edit-running-before.json').write_text(json.dumps(before, ensure_ascii=False, indent=2))
    until = injection + 2_000_000_000
    finished = None
    while time.perf_counter_ns() < until:
        current = state()
        done = next((t for t in current['background_tasks'] if t['task_id'] == task['task_id']), None)
        if current['main_revision'] > task['metadata']['revision'] and done and done['finished_monotonic_ns']:
            finished = done
            observed_end = time.perf_counter_ns()
            break
        time.sleep(.001)
    assert finished and finished['cancel_requested_while_running'], ('Typing did not invalidate a still-running old revision task', task, finished)
    assert current['main_id'] == before['main_id']
    assert current['main_buffer_id'] == before['main_buffer_id']
    assert finished['cancel_to_worker_exit_wall_ms'] <= 100
    edit_revision = current['main_revision']
    key('ctrl+s')
    wait(lambda s: not s['dirty'], 'Actual native Save complete', 'ui')
    expected = prefix + 'k' + original
    assert heavy.read_text() == expected, 'Actual saved manuscript differs from independently expected post-paste insertion'
    wait(proof_ready, 'New revision/current buffer source evidence ready')
    stable = state()
    position = stable['views']['main/source']['presented_offset']
    tail = []
    for _ in range(22):
        s = state()
        v = s['views']['main/source']
        assert s['main_id'] == before['main_id'] and s['main_revision'] == edit_revision
        assert s['main_buffer_id'] == stable['main_buffer_id'] and v['buffer_id'] == s['main_buffer_id'] and v['revision'] == s['main_revision']
        assert max(abs(v['presented_offset'][i] - position[i]) for i in range(2)) <= 2
        tail.append(dict(revision=s['main_revision'], buffer_id=s['main_buffer_id'], actual_offset=v['presented_offset']))
        time.sleep(.1)
    measurements.append(dict(event='actual source k key while running match', before_task=task, finished_task=finished, pending_to_injection_ms=(injection-observed)/1e6, native_call_ms=(input_end-injection)/1e6, native_to_observed_old_worker_exit_upper_bound_ms=(observed_end-injection)/1e6, new_revision=edit_revision, buffer_before_save=before['main_buffer_id'], buffer_after_save=stable['main_buffer_id'], saved_expected_sha256=hashlib.sha256(expected.encode()).hexdigest(), stable_tail_seconds=2.2, tail=tail))
    shot('02-after-real-edit-and-save')

    # Establish current-version Read history through the actual UI at the head,
    # then keep a different source history. The next external change is append
    # only, so Read has an independent exact expected zero while source is back.
    mode('Read')
    settle('main/preview')
    assert max(abs(n) for n in view('main/preview')['presented_offset']) <= 2, 'Current-version Read head precondition not met'
    wait(lambda s: proof_for(s, 'main/preview'), 'Current-version Read head evidence ready')
    mode('Write')
    wheel('main/source', 12)
    wait(proof_ready, 'Distinct source history ready for view change')
    assert view('main/source')['position']['offset'][1] > 100
    read_control = next(c for c in state('ui')['controls'] if c['name'] == 'Read')['rect']
    stop()
    final_text = expected + '\n\nAn independent external appendix changes the version again.\n'
    heavy.write_text(final_text)
    launch_fast()
    before, task, observed = running_source_match()
    assert before['views']['main/source']['pending'] is not None, 'Actual source restore pending at native view request'
    saved_source_offset = before['views']['main/source']['pending']['position']['offset']
    lib.XWarpPointer(display, 0, int(window), 0, 0, 0, 0, round((read_control[0] + read_control[2]) / 2), round((read_control[1] + read_control[3]) / 2))
    lib.XSync(display, 0)
    injection = time.perf_counter_ns()
    xtst.XTestFakeButtonEvent(display, 1, 1, 0)
    lib.XFlush(display)
    time.sleep(.02)
    xtst.XTestFakeButtonEvent(display, 1, 0, 0)
    lib.XSync(display, 0)
    (OUT / '03-view-running-before.json').write_text(json.dumps(before, ensure_ascii=False, indent=2))
    wait(lambda s: s['mode'] == 'Read', 'Actual native view change', 'ui')
    first = wait(lambda s: s['views'].get('main/preview', {}).get('presented_offset') is not None, 'Actual Read view presented')
    old_task = next(t for t in first['background_tasks'] if t['task_id'] == task['task_id'])
    # Current-version preview history was explicitly established at the head;
    # only an appendix changed. The hidden source has a different saved offset.
    assert max(abs(n) for n in first['views']['main/preview']['presented_offset']) <= 2
    wait(lambda s: any(t['task_id'] == task['task_id'] and t['finished_monotonic_ns'] for t in s['background_tasks']), 'Hidden source task actually finished')
    wait(lambda s: s['views']['main/preview']['pending'] is None, 'Current Read restoration settled')
    tail = []
    for _ in range(22):
        s = state()
        actual = s['views']['main/preview']['presented_offset']
        assert max(abs(n) for n in actual) <= 2, 'Hidden source task pulled the current Read view'
        assert s['main_id'] == before['main_id'] and s['main_buffer_id'] == before['main_buffer_id'] and s['main_revision'] == before['main_revision']
        tail.append(dict(actual_offset=actual, revision=s['main_revision'], buffer_id=s['main_buffer_id']))
        time.sleep(.1)
    assert state('ui')['mode'] == 'Read'
    measurements.append(dict(event='actual Read click requested while source restore pending and match observed running', before_task=task, task_at_first_read_frame=old_task, pending_to_injection_ms=(injection-observed)/1e6, source_saved_offset=saved_source_offset, preview_expected_offset=[0,0], stable_tail_seconds=2.2, tail=tail, boundary='View isolation/late-result correctness. The first painted Read report can already contain a completed hidden-source task; this case does not claim CPU cancellation at mode handling.'))
    shot('04-hidden-source-result-did-not-pull-read-view')
    stop()
    assert heavy.read_text() == final_text
    result = dict(passed=True, binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(), original_bytes=len(original.encode()), external_changed_bytes=len(changed.encode()), measurements=measurements, method='Natural computation, no sleeps in product; real native source key/Save and view click. Read-only running/checkpoint/version/buffer observations. Independent exact saved bytes and 2.2-second actual-offset tails. This is GUI correctness, not a throughput or physical-display benchmark.')
    (OUT / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
except Exception as error:
    if app is not None and app.poll() is None:
        try:
            shot('failure')
        except Exception:
            pass
    (OUT / 'failure.json').write_text(json.dumps(dict(error=repr(error), measurements=measurements), ensure_ascii=False, indent=2))
    raise
finally:
    if display:
        lib.XCloseDisplay(display)
    if app is not None and app.poll() is None:
        app.kill()
        app.wait(timeout=5)
    for service in reversed(services):
        service.terminate()
        try:
            service.wait(timeout=5)
        except subprocess.TimeoutExpired:
            service.kill()
            service.wait(timeout=5)
