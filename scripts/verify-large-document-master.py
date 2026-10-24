#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent native acceptance: exact source, history and global selection."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get('MDCARD_MASTER_LARGE_OUT', ROOT / 'evidence/performance/master-review/interactions'))
OUT.mkdir(parents=True, exist_ok=True)
SOURCE_EXE = Path(os.environ.get('MDCARD_MASTER_LARGE_EXE', ROOT / 'target/release/mdcard'))
EXE = Path('/tmp') / f'mdcard-master-binary-{uuid.uuid4()}'
shutil.copy2(SOURCE_EXE, EXE)
BINARY_SHA256 = hashlib.file_digest(EXE.open('rb'), 'sha256').hexdigest()
STATE = Path('/tmp') / f'mdcard-master-{uuid.uuid4()}.json'
ENV = dict(os.environ, DISPLAY=':138', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2',
           MDCARD_SETTINGS_PATH=str(OUT / 'settings.json'), MDCARD_PERFORMANCE_REPORT=str(STATE),
           XDG_RUNTIME_DIR='/tmp/mdcard-master-large')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
(OUT / 'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', gpu_acceleration=True,
                                                  reduce_motion=True, font_size=15)))
children, cases = [], []
app = None
window = None


def run(*args, **kwargs):
    return subprocess.run(args, env=ENV, check=True, **kwargs)


def state():
    try:
        return json.loads(STATE.read_text())
    except (OSError, ValueError):
        return {}


def wait(test, label, seconds=10, read=state):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        s = read()
        if test(s):
            return s
        if app and app.poll() is not None:
            raise AssertionError(f'Application exited: {label}, code={app.returncode}')
        time.sleep(.01)
    raise AssertionError(f'{label}: {str(state())[:1200]}')


def keys(*args):
    run('xdotool', 'key', '--clearmodifiers', '--delay', '30', *args)
    time.sleep(.12)


def launch(name, content, extension='.md', search_report=False, wrap=None):
    global app, window
    close()
    STATE.unlink(missing_ok=True)
    path = OUT / (name + extension)
    path.write_bytes(content.encode('utf-8'))
    (OUT / 'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', gpu_acceleration=True,
                                                      reduce_motion=True, font_size=15, source_wrap=wrap)))
    launch_env = dict(ENV)
    if search_report:
        (OUT / 'search-state.json').unlink(missing_ok=True)
        launch_env['MDCARD_SEARCH_REPORT'] = str(OUT / 'search-state.json')
    app = subprocess.Popen([str(EXE), str(path)], env=launch_env,
                           stdout=(OUT / (name + '.log')).open('wb'), stderr=subprocess.STDOUT)
    wait(lambda s: s.get('active') and s.get('source_rect'), 'source ready')
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        mapped = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(app.pid), '--name', 'MDcard'],
                                env=ENV, capture_output=True)
        if mapped.returncode == 0:
            window = mapped.stdout.decode().splitlines()[0]
            break
        time.sleep(.02)
    else:
        raise AssertionError('Native window was not mapped')
    run('xdotool', 'windowsize', window, '1420', '950', 'windowfocus', window)
    time.sleep(.15)
    r = state()['source_rect']
    run('xdotool', 'mousemove', '--window', window, str(round(r[0] + 50)), str(round(r[1] + 5)), 'click', '1')
    time.sleep(.1)
    return path


def close():
    global app
    if app and app.poll() is None:
        app.terminate()
        app.wait(timeout=10)
    app = None


def clip():
    return run('xclip', '-selection', 'clipboard', '-o', capture_output=True).stdout.decode()


def save_exact(path, expected):
    keys('ctrl+s')
    deadline = time.monotonic() + 4
    while time.monotonic() < deadline:
        if path.read_bytes() == expected.encode('utf-8'):
            return
        time.sleep(.02)
    actual = path.read_bytes()
    raise AssertionError(f'Save differs: expected {len(expected.encode())} bytes, actual {len(actual)} bytes; '
                         f'actual tail={actual[-100:]!r}')


def case(name, fn):
    selected = os.environ.get('MDCARD_MASTER_LARGE_CASE', '')
    if selected and selected not in name:
        return
    started = time.monotonic()
    try:
        details = fn() or {}
        cases.append(dict(name=name, passed=True, **details))
    except Exception as exc:
        cases.append(dict(name=name, passed=False, error=str(exc)))
        if window:
            subprocess.run(['import', '-window', window, str(OUT / (name + '-failure.png'))], env=ENV)
    finally:
        cases[-1]['seconds'] = time.monotonic() - started
        (OUT / (name + '-state.json')).write_text(json.dumps(state(), indent=2))
        close()


corpus = ''.join(f'# Heading {i:05d}\n\n中文 🦀 source-{i:05d}; precise text and selection.\n\n' for i in range(3000))


def copy_all():
    launch('copy', corpus)
    keys('ctrl+Home', 'ctrl+a', 'ctrl+c')
    assert clip() == corpus, 'Ctrl+A/C differs from exact document'
    keys('ctrl+End', 'ctrl+shift+Home', 'ctrl+c')
    assert clip() == corpus, 'Reverse whole-document selection differs'


def append_history():
    path = launch('append-history', corpus)
    keys('ctrl+End')
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=b'APPEND',
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    keys('ctrl+v')
    save_exact(path, corpus + 'APPEND')
    time.sleep(.5)
    keys('ctrl+z')
    save_exact(path, corpus)
    keys('ctrl+shift+z')
    save_exact(path, corpus + 'APPEND')


def cross_selection(from_end):
    launch('selection-' + str(from_end), corpus)
    keys('ctrl+End' if from_end else 'ctrl+Home')
    run('xdotool', 'key', '--clearmodifiers', '--repeat', '180', '--repeat-delay', '12',
        'shift+Up' if from_end else 'shift+Down')
    time.sleep(.2)
    s = state()
    anchor = len(corpus) if from_end else 0
    assert s['selection'][1] == anchor, f'Global anchor lost: {s["selection"]}, expected secondary={anchor}'
    a, b = sorted(s['selection'])
    assert corpus[a:b].count('\n') >= 175, f'Selection stopped at virtual boundary: {s["selection"]}'
    keys('ctrl+c')
    assert clip() == corpus[a:b], 'Clipboard differs from complete cross-screen selection'
    return dict(selection=s['selection'], source_window=s['source_window'])


def shrink_history():
    path = launch('shrink', corpus)
    keys('ctrl+Home', 'ctrl+a')
    run('xclip', '-selection', 'clipboard', '-loops', '1', input='tiny 中文 🦀\n'.encode(),
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    keys('ctrl+v')
    save_exact(path, 'tiny 中文 🦀\n')
    time.sleep(.6)
    keys('ctrl+z')
    save_exact(path, corpus)
    keys('ctrl+shift+z')
    save_exact(path, 'tiny 中文 🦀\n')


def giant_line(wrap=False):
    text = '中' * 100000 + 'TAILMARK'
    path = launch('wrapped-giant-line' if wrap else 'giant-line', text, '.txt', wrap=wrap)
    keys('ctrl+End')
    s = state()
    assert s['selection'][0] == len(text), 'Ctrl+End did not select the true end'
    a, b = s['source_window']
    assert a <= len(text) <= b, f'End caret remains unmounted: window={a,b}, end={len(text)}'
    run('xdotool', 'type', '--clearmodifiers', 'Z')
    save_exact(path, text + 'Z')


def drag_selection():
    launch('drag-selection', corpus)
    keys('ctrl+Home')
    r = state()['source_rect']
    x, y = round(r[0] + 3), round(r[1] + 5)
    run('xdotool', 'mousemove', '--window', window, str(x), str(y), 'mousedown', '1')
    time.sleep(.08)
    anchor = state()['selection'][1]
    for i in range(75):
        run('xdotool', 'mousemove', '--window', window, str(x + i % 2), str(round(r[3] + 25 + i % 2 * 5)))
        time.sleep(.025)
    run('xdotool', 'mouseup', '1')
    time.sleep(.15)
    s = state()
    assert s['selection'][1] == anchor, f'Drag anchor changed: {s["selection"]}, expected {anchor}'
    a, b = sorted(s['selection'])
    assert corpus[a:b].count('\n') >= 60, f'Drag did not auto-scroll across screens: {s["selection"]}'
    keys('ctrl+c')
    assert clip() == corpus[a:b], 'Dragged cross-screen clipboard differs'
    return dict(selection=s['selection'], source_offset=s['source_offset'])


def synchronized_navigation():
    launch('synchronized-navigation', corpus)
    keys('ctrl+End')
    wait(lambda s: s.get('preview_range', [0, 0])[1] >= s.get('preview_blocks', 1)
         and s.get('preview_offset', 0) > 1000, 'End reaches both source and preview')
    keys('ctrl+Home')
    wait(lambda s: s.get('source_offset', 100) < 1 and s.get('preview_offset', 100) < 1
         and s.get('source_anchor', 100) < 100 and s.get('preview_anchor', 100) < 100,
         'Home returns both panes and their source anchors to the beginning')
    r = state()['source_rect']
    run('xdotool', 'mousemove', '--window', window, str(round(r[2] + 120)), str(round(r[1] + 100)))
    run('xdotool', 'click', '--repeat', '28', '--delay', '18', '5')
    s = wait(lambda s: s.get('preview_offset', 0) > 1500 and s.get('source_offset', 0) > 1000,
             'Preview wheel synchronizes source')
    time.sleep(.25)
    s = state()
    assert abs(s['source_anchor'] - s['preview_anchor']) < 256, f'Semantic anchors drifted: {s["source_anchor"], s["preview_anchor"]}'
    source_char = len(corpus.encode('utf-8')[:s['source_anchor']].decode('utf-8', errors='ignore'))
    assert s['source_window'][0] <= source_char <= s['source_window'][1], 'Reported source anchor is outside the mounted window'
    idle_frame = s['frame']
    idle_samples = []
    for _ in range(16):
        time.sleep(.05)
        sample = state()
        idle_samples.append({k: sample.get(k) for k in ('frame', 'source_offset', 'preview_offset', 'source_anchor', 'preview_anchor')})
    (OUT / 'idle-samples.json').write_text(json.dumps(idle_samples, indent=2) + '\n')
    idle_frames = idle_samples[-1]['frame'] - idle_frame
    assert idle_frames < 40, f'Peer synchronization kept repainting while idle: {idle_frames} frames in 0.8s'
    return dict(source_anchor=s['source_anchor'], preview_anchor=s['preview_anchor'],
                source_offset=s['source_offset'], preview_offset=s['preview_offset'], idle_frames_800ms=idle_frames)


def precise_search():
    launch('search-jump', corpus, search_report=True)

    def search_state():
        try:
            return json.loads((OUT / 'search-state.json').read_text())
        except (OSError, ValueError):
            return {}

    def control(name):
        s = wait(lambda s: any(c['name'] == name for c in s.get('controls', [])),
                 'Search control ' + name, read=search_state)
        r = next(c['rect'] for c in s['controls'] if c['name'] == name)
        run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)),
            str(round((r[1]+r[3])/2)), 'click', '1')
        time.sleep(.1)

    keys('ctrl+f')
    control('find-query')
    keys('ctrl+a')
    marker = 'source-02975'
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=marker.encode(),
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    keys('ctrl+v')
    wait(lambda s: s.get('results') == 1 and not s.get('query_pending') and not s.get('error'),
         'Unique distant full-document search result', read=search_state)
    control('result-0-0')
    a = corpus.index(marker)
    b = a + len(marker)
    s = wait(lambda s: sorted(s.get('selection', [0, 0])) == [a, b]
             and s.get('source_window', [0, 0])[0] <= a <= s.get('source_window', [0, 0])[1],
             'Exact Unicode-aware search selection mounted near document end')
    keys('ctrl+c')
    assert clip() == marker, 'Distant highlighted search copied the wrong source'
    run('import', '-window', window, str(OUT / 'precise-search.png'))
    return dict(selection=s['selection'], source_window=s['source_window'], source_offset=s['source_offset'])


def precise_toc():
    launch('toc-jump', corpus)
    s = wait(lambda s: s.get('toc_scrollbar') and s.get('toc_controls'), 'Real TOC controls')
    r = s['toc_scrollbar']
    run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)), str(round(r[3]-3)), 'click', '1')
    s = wait(lambda s: any(c['byte'] > len(corpus.encode('utf-8'))*.9 for c in s.get('toc_controls', [])),
             'TOC virtual scroll mounts headings near document end')
    target = s['toc_controls'][len(s['toc_controls'])//2]
    r = target['rect']
    run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)),
        str(round((r[1]+r[3])/2)), 'click', '1')
    target_char = len(corpus.encode('utf-8')[:target['byte']].decode('utf-8'))
    target_block = int(target['title'].split()[-1])*2
    s = wait(lambda s: s.get('selection', [0])[0] == target_char
             and s.get('source_window', [0, 0])[0] <= target_char <= s.get('source_window', [0, 0])[1]
             and s.get('preview_range', [0, 0])[0] <= target_block < s.get('preview_range', [0, 0])[1],
             'TOC exact source caret and preview block are both mounted')
    run('import', '-window', window, str(OUT / 'precise-toc.png'))
    return dict(heading=target['title'], byte=target['byte'], source_char=target_char,
                selection=s['selection'], preview_range=s['preview_range'])


def lazy_code_highlighting():
    body = ''.join(f'let value_{i:05d} = "literal $x$ 中文"; // line {i:05d}\n' for i in range(12000))
    launch('lazy-long-code', '# Long code\n\n```rust\n' + body + '```\n')
    initial = wait(lambda s: s.get('cache', {}).get('code', {}).get('submissions', 0) > 0
                   and s['cache']['code'].get('pending') == 0, 'Visible code pages highlighted asynchronously')
    first_count = initial['cache']['code']['submissions']
    assert first_count < 8 and first_count < initial['preview_blocks']/4, 'Code highlighter eagerly processed offscreen pages'
    r = initial['preview_scrollbar']
    x = round((r[0]+r[2])/2)
    run('xdotool', 'mousemove', '--window', window, str(x), str(round(r[3]-3)), 'click', '1')
    end = wait(lambda s: s.get('cache', {}).get('code', {}).get('submissions', 0) > first_count
               and s['cache']['code'].get('pending') == 0, 'New distant code pages highlighted')
    end_count = end['cache']['code']['submissions']
    assert end_count < end['preview_blocks']/3, 'Jumping to the bottom highlighted all intermediate UI pages'
    r = end['preview_scrollbar']
    run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)), str(round(r[1]+3)), 'click', '1')
    final = wait(lambda s: s.get('preview_offset', 100) < 1 and s.get('cache', {}).get('code', {}).get('pending') == 0,
                 'Return to cached first code pages')
    time.sleep(.2)
    final = state()
    assert final['cache']['code']['submissions'] == end_count, 'Returning to cached code pages repeated highlighting'
    assert final['cache']['code']['bytes'] <= final['cache']['code']['byte_limit'], 'Code cache exceeded byte budget'
    assert final['preview_mounted'] < 10, 'Code preview mounted too many pages'
    run('import', '-window', window, str(OUT / 'lazy-code-highlighted.png'))
    return dict(total_pages=final['preview_blocks'], initial_submissions=first_count,
                after_jump_submissions=end_count, cached_return_submissions=final['cache']['code']['submissions'],
                mounted=final['preview_mounted'], cache_bytes=final['cache']['code']['bytes'])


def secondary_large_document():
    target = OUT / 'master-secondary-large-target.md'
    secondary_text = corpus.replace('source-', 'secondary-')
    target.write_text(secondary_text, encoding='utf-8')
    main_path = launch('secondary-main', corpus, search_report=True)

    def secondary_state():
        try:
            return json.loads((OUT / 'search-state.json').read_text())
        except (OSError, ValueError):
            return {}

    def control(name):
        s = wait(lambda s: any(c['name'] == name for c in s.get('controls', [])),
                 'Secondary control: ' + name, read=secondary_state)
        r = next(c['rect'] for c in s['controls'] if c['name'] == name)
        run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)),
            str(round((r[1]+r[3])/2)), 'click', '1')
        time.sleep(.1)

    def paste(value):
        run('xclip', '-selection', 'clipboard', '-loops', '1', input=value.encode(),
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        keys('ctrl+v')

    main_marker = '\nMAIN_DIRTY 中文 🦀\n'
    keys('ctrl+End')
    paste(main_marker)
    wait(lambda s: s.get('text') == corpus + main_marker, 'Main dirty buffer retained', read=secondary_state)
    keys('ctrl+p')
    control('quick-query')
    keys('ctrl+a')
    paste(target.stem)
    wait(lambda s: len(s.get('quick', [])) == 1 and s['quick'][0]['path'].endswith(target.name),
         'Unique large secondary quick-open match', read=secondary_state)
    keys('shift+Return')
    wait(lambda s: (s.get('secondary') or {}).get('path', '').endswith(target.name)
         and s.get('secondary_virtual', {}).get('active'),
         'Real secondary entry uses virtual source kernel', read=secondary_state)
    control('secondary-editor')
    keys('ctrl+a', 'ctrl+c')
    assert clip() == secondary_text, 'Secondary virtual copy differs from whole source'
    keys('ctrl+End')
    marker = '\nSECONDARY_EDIT 中文 🦀\n'
    paste(marker)
    keys('ctrl+z')
    wait(lambda s: (s.get('secondary') or {}).get('text') == secondary_text,
         'Secondary atomic undo', read=secondary_state)
    keys('ctrl+shift+z')
    expected = secondary_text + marker
    wait(lambda s: (s.get('secondary') or {}).get('text') == expected,
         'Secondary atomic redo', read=secondary_state)
    keys('ctrl+s')
    s = wait(lambda s: (s.get('secondary') or {}).get('text') == expected
             and not s['secondary']['dirty'] and not s.get('busy'),
             'Secondary save completes', read=secondary_state)
    assert target.read_bytes() == expected.encode(), 'Secondary Ctrl+S saved different bytes'
    assert main_path.read_bytes() == corpus.encode(), 'Secondary Ctrl+S touched main disk document'
    assert s['text'] == corpus + main_marker, 'Secondary edit/save changed dirty main buffer'
    v = s['secondary_virtual']
    assert 0 < v['laid_out_lines'] < 200, 'Secondary source laid out the whole long document'
    assert v['source_window'][0] <= len(expected) <= v['source_window'][1], 'Secondary end caret unmounted'

    r = state()['source_rect']
    run('xdotool', 'mousemove', '--window', window, str(round(r[0]+50)), str(round(r[1]+5)), 'click', '1')
    time.sleep(.1)
    keys('ctrl+z')
    wait(lambda s: s.get('text') == corpus and (s.get('secondary') or {}).get('text') == expected,
         'Main undo isolated from secondary history', read=secondary_state)
    control('secondary-editor')
    keys('ctrl+z')
    wait(lambda s: (s.get('secondary') or {}).get('text') == secondary_text and s.get('text') == corpus,
         'Secondary undo survives save and remains isolated', read=secondary_state)
    keys('ctrl+s')
    wait(lambda s: s.get('secondary') and not s['secondary']['dirty'] and not s.get('busy'),
         'Secondary restored source saved', read=secondary_state)
    assert target.read_bytes() == secondary_text.encode()
    run('import', '-window', window, str(OUT / 'secondary-large.png'))
    return dict(secondary_characters=len(secondary_text), laid_out_lines=v['laid_out_lines'],
                source_window=v['source_window'], exact_copy=True, save_history_and_main_isolation=True)


try:
    children.append(subprocess.Popen(['Xvfb', ENV['DISPLAY'], '-screen', '0', '1600x1100x24'],
                                     stdout=(OUT / 'xvfb.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.3)
    children.append(subprocess.Popen(['openbox'], env=ENV, stdout=(OUT / 'openbox.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.3)
    case('exact-whole-document-copy', copy_all)
    case('append-save-undo-redo', append_history)
    case('forward-cross-screen-selection', lambda: cross_selection(False))
    case('reverse-cross-screen-selection', lambda: cross_selection(True))
    case('cross-threshold-replacement-history', shrink_history)
    case('giant-line-end-caret', giant_line)
    case('wrapped-giant-line-end-caret', lambda: giant_line(True))
    case('drag-cross-screen-copy', drag_selection)
    case('bidirectional-synchronized-navigation', synchronized_navigation)
    case('precise-distant-search', precise_search)
    case('precise-distant-toc', precise_toc)
    case('lazy-long-code-highlighting', lazy_code_highlighting)
    case('secondary-large-document-isolation', secondary_large_document)
finally:
    close()
    for child in reversed(children):
        child.terminate()
        child.wait(timeout=5)
    STATE.unlink(missing_ok=True)
    result = dict(verification_id=str(uuid.uuid4()), binary_sha256=BINARY_SHA256, source_binary=str(SOURCE_EXE),
                  platform='Linux/X11/Xvfb/Openbox/llvmpipe', passed=bool(cases) and all(c['passed'] for c in cases), cases=cases)
    (OUT / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
    EXE.unlink(missing_ok=True)

raise SystemExit(0 if result['passed'] else 1)
