# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Independent real-X11 export cancel/close/retry acceptance with owned PID identities."""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import uuid

parser = argparse.ArgumentParser()
parser.add_argument('--exe', required=True, type=Path)
parser.add_argument('--renderer', required=True, type=Path)
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
(out / 'result.json').unlink(missing_ok=True)
release = threading.Event()
requested = threading.Event()
PNG = bytes.fromhex('89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c63f8cfc0f01f00050001ff89993d1d0000000049454e44ae426082')


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        requested.set()
        if not release.wait(30):
            return
        try:
            self.send_response(200)
            self.send_header('Content-Type', 'image/png')
            self.send_header('Content-Length', str(len(PNG)))
            self.end_headers()
            self.wfile.write(PNG)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *_):
        pass


server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
server.daemon_threads = True
threading.Thread(target=server.serve_forever, daemon=True).start()
env = dict(os.environ, DISPLAY=':133', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1',
           LP_NUM_THREADS='2', XDG_RUNTIME_DIR='/tmp/mdcard-audit-followup-export',
           MDCARD_RENDERER_DIR=str(args.renderer.resolve()), MDCARD_EXPORT_REPORT=str(out / 'state.json'),
           MDCARD_SETTINGS_PATH=str(out / 'settings.json'))
Path(env['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(env['XDG_RUNTIME_DIR']).chmod(0o700)
services = []
app = None
window = None
unrelated = subprocess.Popen(['/bin/sleep', '120'])


def run(*a, **kw):
    return subprocess.run(a, env=env, check=True, **kw)


def state():
    try:
        return json.loads((out / 'state.json').read_text())
    except (ValueError, OSError):
        return {}


def wait(predicate, label, timeout=20):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        s = state()
        if predicate(s):
            return s
        if app.poll() is not None:
            raise AssertionError(f'App exited at {label}: {app.returncode}')
        time.sleep(.03)
    raise AssertionError(f'{label}: {state()}')


def key(*keys):
    run('xdotool', 'key', '--clearmodifiers', *keys)
    time.sleep(.12)


def control(name):
    s = wait(lambda s: name in s.get('controls', {}), 'Control ' + name)
    r = s['controls'][name]
    run('xdotool', 'mousemove', '--window', window, str(round(r['x'] + r['w'] / 2)),
        str(round(r['y'] + r['h'] / 2)), 'click', '1')
    time.sleep(.16)


def identities():
    records = {}
    for p in Path('/proc').glob('[0-9]*/stat'):
        try:
            raw = p.read_text()
            fields = raw[raw.rfind(')') + 2:].split()
            records[int(p.parent.name)] = dict(pid=int(p.parent.name), state=fields[0],
                parent=int(fields[1]), starttime=int(fields[19]), command=(p.parent / 'cmdline').read_bytes().replace(b'\0', b' ').decode(errors='replace'))
        except (ValueError, OSError, IndexError):
            pass
    return records


def descendants(parent):
    records = identities()
    owned = {parent}
    while True:
        expanded = owned | {p for p, r in records.items() if r['parent'] in owned}
        if expanded == owned:
            return [r for p, r in records.items() if p in owned and p != parent]
        owned = expanded


def gone(owned):
    now = identities()
    return all(r['pid'] not in now or now[r['pid']]['starttime'] != r['starttime'] or
               now[r['pid']]['state'] in ('Z', 'X') for r in owned)


def observed_owned(owned):
    now = identities()
    return {r['pid']: now.get(r['pid']) for r in owned}


def launch(source):
    global app, window
    (out / 'state.json').unlink(missing_ok=True)
    app = subprocess.Popen([str(args.exe.resolve()), str(source)], env=env,
                           stdout=(out / f'app-{uuid.uuid4()}.log').open('wb'), stderr=subprocess.STDOUT)
    for _ in range(100):
        found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(app.pid), '--name', 'MDcard'], env=env, capture_output=True)
        if found.returncode == 0:
            window = found.stdout.decode().splitlines()[0]
            break
        time.sleep(.05)
    else:
        raise AssertionError('No native window')
    run('xdotool', 'windowsize', window, '1240', '860', 'windowactivate', '--sync', window)
    key('ctrl+alt+e')
    wait(lambda s: s.get('open'), 'Export dialog')
    control('format')
    control('format-0')


def start_export(destination):
    requested.clear()
    control('export')
    # GTK's save chooser accepts an absolute path in its location entry.
    time.sleep(.5)
    key('ctrl+l', 'ctrl+a')
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=str(destination).encode(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    key('ctrl+v')
    key('Return')
    time.sleep(.3)
    run('import', '-window', 'root', str(out / 'chooser-after-return.png'))
    print('CHOOSER', subprocess.run(['xdotool', 'getactivewindow', 'getwindowname'], env=env, capture_output=True).stdout.decode(), flush=True)
    if not state().get('running'):
        key('alt+r')  # Confirm replacement of an existing target in the GTK chooser.
    wait(lambda s: s.get('running') and requested.is_set(), 'Live resource conversion', 90)
    assert state()['output'] == str(destination), ('Wrong chooser target', state())
    owned = descendants(app.pid)
    assert any('export.mjs' in r['command'] for r in owned), owned
    assert any('chrome' in r['command'].lower() for r in owned), owned
    return owned


try:
    services.append(subprocess.Popen(['Xvfb', env['DISPLAY'], '-screen', '0', '1600x1100x24'], stdout=(out / 'xvfb.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.5)
    services.append(subprocess.Popen(['openbox'], env=env, stdout=(out / 'openbox.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.5)
    (out / 'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', gpu_acceleration=True, reduce_motion=True, font_size=15)))
    workspace = out / ('workspace-' + str(uuid.uuid4()))
    workspace.mkdir(exist_ok=True)
    source = workspace / '稿件 中文.md'
    source.write_text(f'# Cancellation / 取消\n\n![controlled](http://127.0.0.1:{server.server_port}/slow.png)\n')
    original = source.read_bytes()
    destination = workspace / '旧目标.html'
    destination.write_text('OLD OUTPUT')
    launch(source)
    samples = []
    for n in range(3):
        owned = start_export(destination)
        run('import', '-window', window, str(out / f'cancel-{n}-running.png'))
        started = time.monotonic()
        control('cancel')
        final = wait(lambda s: not s.get('running') and s.get('last', {}).get('type') == 'cancelled', 'Cancellation finished')
        elapsed = (time.monotonic() - started) * 1000
        assert gone(owned), ('Live converter after UI finished', owned, observed_owned(owned))
        assert unrelated.poll() is None
        assert source.read_bytes() == original and destination.read_text() == 'OLD OUTPUT'
        assert not list(workspace.glob('.mdcard-task-*'))
        samples.append(dict(action='cancel', elapsed_ms=elapsed, before=owned, after=observed_owned(owned), state=final))
    # Retry the same frozen source and destination after cancellation, using real runtimes.
    release.set()
    control('export')
    time.sleep(.5)
    key('ctrl+l', 'ctrl+a')
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=str(destination).encode(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    key('ctrl+v')
    key('Return')
    time.sleep(.3)
    key('alt+r')
    final = wait(lambda s: not s.get('running') and s.get('last', {}).get('type') == 'complete', 'Retry completed', 60)
    assert source.read_bytes() == original
    assert 'data:image/png;base64,' in destination.read_text()
    assert not list(workspace.glob('.mdcard-task-*'))
    run('import', '-window', window, str(out / 'retry-complete.png'))
    key('ctrl+q')
    app.wait(timeout=10)
    assert app.returncode == 0
    app = None
    # A normal native close must also await cancellation and clear its workspace.
    release.clear()
    destination.write_text('OLD OUTPUT')
    launch(source)
    owned = start_export(destination)
    started = time.monotonic()
    key('alt+F4')
    app.wait(timeout=15)
    assert app.returncode == 0 and gone(owned) and unrelated.poll() is None
    assert source.read_bytes() == original and destination.read_text() == 'OLD OUTPUT'
    assert not list(workspace.glob('.mdcard-task-*'))
    samples.append(dict(action='native-close', elapsed_ms=(time.monotonic() - started) * 1000, before=owned, after=observed_owned(owned)))
    result = dict(verification_id=str(uuid.uuid4()), date=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
        executable=str(args.exe), exe_sha256=hashlib.sha256(args.exe.read_bytes()).hexdigest(), renderer=str(args.renderer),
        platform='Debian12/X11/Xvfb/Openbox/Mesa', cancelled=samples, retry=final,
        source_preserved=True, old_target_preserved=True, unrelated_process_preserved=True,
        no_live_owned_descendants_at_completion=True, task_directories_removed=True)
    (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ('cancelled', 'retry')}))
finally:
    release.set()
    server.shutdown()
    if app and app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()
            app.wait()
    unrelated.terminate()
    unrelated.wait()
    for p in reversed(services):
        p.terminate()
        p.wait(timeout=5)
