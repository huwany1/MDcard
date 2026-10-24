#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Observe six localized native export dialogs through real X11 keyboard input."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

root = Path('/work')
out = root / 'artifacts/export-ui/linux'
out.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, DISPLAY=':117', LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1',
           LP_NUM_THREADS='2', XDG_RUNTIME_DIR='/tmp/mdcard-export-ui-runtime')
Path(env['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(env['XDG_RUNTIME_DIR']).chmod(0o700)
services = []
def run(*args, **kw):
    return subprocess.run(args, env=env, check=True, **kw)
def observe(report):
    for _ in range(30):
        try:
            return json.loads(report.read_text())
        except (ValueError, FileNotFoundError):
            time.sleep(.03)
    raise RuntimeError('No complete observation report')
if subprocess.run(['xdpyinfo', '-display', env['DISPLAY']], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
    services.append(subprocess.Popen(['Xvfb', env['DISPLAY'], '-screen', '0', '1400x1000x24'], env=env, stdout=(out/'xvfb.log').open('w'), stderr=subprocess.STDOUT))
    time.sleep(.5)
services.append(subprocess.Popen(['openbox'], env=env, stdout=(out/'openbox.log').open('w'), stderr=subprocess.STDOUT))
exe = root / 'target/release/mdcard'
fixture = out/'source.md'
fixture.write_text('# Export source\n\n中文 한국어 日本語 Русский.\n')
results = []
for language in ['en', 'zh-CN', 'zh-TW', 'ko', 'ja', 'ru']:
    settings = out/f'{language}-settings.json'
    settings.write_text(json.dumps(dict(language=language, appearance='Dark', gpu_acceleration=False, reduce_motion=True, font_size=16)))
    report = out/f'{language}-export.json'
    appenv = dict(env, MDCARD_SETTINGS_PATH=str(settings), MDCARD_EXPORT_REPORT=str(report), MDCARD_RENDERER_REPORT=str(out/f'{language}-renderer.json'))
    app = subprocess.Popen([str(exe), str(fixture)], env=appenv, stdout=(out/f'{language}.log').open('w'), stderr=subprocess.STDOUT)
    try:
        time.sleep(1.8)
        window = run('xdotool', 'search', '--pid', str(app.pid), '--name', 'MDcard', capture_output=True).stdout.decode().splitlines()[0]
        run('xdotool', 'windowsize', window, '1000', '760', 'windowfocus', window, 'key', '--clearmodifiers', 'ctrl+alt+e')
        time.sleep(.5)
        run('import', '-window', window, str(out/f'{language}-export.png'))
        normal = observe(report)
        assert normal['open'] and 'export' in normal['controls']
        run('xdotool', 'windowsize', window, '620', '420')
        time.sleep(.5)
        run('import', '-window', window, str(out/f'{language}-compact.png'))
        compact = observe(report)
        button = compact['controls']['export']
        assert button['y'] >= 0 and button['y']+button['h'] <= 420
        results.append(dict(language=language, normal=normal['controls']['export'], compact=button, clean_exit=True))
        run('xdotool', 'key', '--clearmodifiers', 'ctrl+q')
        app.wait(timeout=10)
        assert app.returncode == 0
    finally:
        if app.poll() is None:
            app.terminate()
            app.wait(timeout=10)
for service in services:
    service.terminate()
    service.wait(timeout=10)
(out/'result.json').write_text(json.dumps(dict(environment='Debian 12 / Xvfb X11 / Openbox / CPU Vulkan software rendering', executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(), languages=results), indent=2))
print(json.dumps(results))
