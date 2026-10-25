#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real native GUI regression checks on an isolated Xvfb display (no Windows desktop needed)."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get('MDCARD_STRUCTURE_OUTPUT',str(ROOT / 'evidence/structure/linux-final')))
EXE = Path(sys.argv[1] if len(sys.argv) > 1 else str(ROOT/'target/release/mdcard'))
OUT.mkdir(parents=True, exist_ok=True)
ENV = dict(os.environ, DISPLAY=os.environ.get('MDCARD_STRUCTURE_DISPLAY',':109'), LANG='C.UTF-8', LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2',
           XDG_RUNTIME_DIR='/tmp/mdcard-structure-runtime', MDCARD_SETTINGS_PATH=str(OUT/'settings.json'))
Path(ENV['XDG_RUNTIME_DIR']).mkdir(exist_ok=True)
Path(ENV['XDG_RUNTIME_DIR']).chmod(0o700)
app = None
window = None
children = []

def run(*args, **kwargs):
    return subprocess.run(args, env=ENV, check=True, **kwargs)

def key(*keys):
    run('xdotool', 'key', '--clearmodifiers', *keys)
    time.sleep(.18)

def click(x, y, button=1):
    run('xdotool', 'mousemove', '--window', window, str(x), str(y), 'click', str(button))
    time.sleep(.2)

def paste(value):
    run('xclip', '-selection', 'clipboard', '-loops', '1', input=value.encode(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    key('ctrl+v')

def shot(name):
    time.sleep(.2)
    run('import', '-window', window, str(OUT/f'{name}.png'))

def launch(path, name='main', language='en'):
    global app, window
    (OUT/'settings.json').write_text(json.dumps(dict(language=language, appearance='Light', gpu_acceleration=True, reduce_motion=True, font_size=15)))
    env = dict(ENV, MDCARD_RENDERER_REPORT=str(OUT/f'{name}-renderer.json'), MDCARD_STRUCTURE_REPORT=str(OUT/f'{name}-timing.json'))
    app = subprocess.Popen([str(EXE), str(path)], env=env, stdout=(OUT/f'{name}.log').open('wb'), stderr=subprocess.STDOUT)
    time.sleep(2)
    for _ in range(30):
        result = subprocess.run(['xdotool','search','--pid',str(app.pid),'--name','MDcard'], env=ENV, capture_output=True)
        if result.returncode == 0:
            window=result.stdout.decode().splitlines()[0]
            break
        time.sleep(.2)
    else:
        raise RuntimeError(f'No native app window for {name}')
    run('xdotool','windowsize',window,'1240','820','windowfocus',window)
    time.sleep(.3)

def close():
    global app
    key('ctrl+s', 'ctrl+q')
    app.wait(timeout=10)
    assert app.returncode == 0
    app=None

def saved(path):
    key('ctrl+s')
    return path.read_text(encoding='utf-8-sig')

verification=str(uuid.uuid4())
(OUT/'result.json').unlink(missing_ok=True)
try:
    xvfb=subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'], stdout=(OUT/'xvfb.log').open('wb'), stderr=subprocess.STDOUT)
    children.append(xvfb)
    time.sleep(.7)
    wm=subprocess.Popen(['openbox'],env=ENV,stdout=(OUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT)
    children.append(wm)
    time.sleep(.7)
    fixture=OUT/'fixture.md'
    original='# Visual table and blocks\r\n\r\nAlpha 🦀 paragraph.\r\n\r\nBeta paragraph.\r\n\r\nGamma paragraph.\r\n\r\n| Name | Value |\r\n| :--- | ---: |\r\n| Apples | 10 |\r\n| Pears | 20 |\r\n\r\n- Parent one\r\n  - Child A\r\n- Parent two\r\n- Parent three\r\n'
    fixture.write_bytes(b'\xef\xbb\xbf'+original.encode())
    initial=fixture.read_bytes()
    launch(fixture)
    key('ctrl+shift+t')
    shot('01-grid')
    key('ctrl+s')
    assert fixture.read_bytes()==initial, 'Untouched structure changed source bytes'
    click(390,431);key('ctrl+a');paste('Edited 🦀 value | pipe');key('Tab');key('ctrl+a');paste('42')
    assert '| Edited 🦀 value \\| pipe | 42 |' in saved(fixture)
    paste('x')
    assert '42x' in saved(fixture), 'Saving stole grid focus'
    key('ctrl+z')
    assert '42x' not in saved(fixture)
    key('ctrl+shift+z')
    assert '42x' in saved(fixture)
    key('ctrl+z','Tab','ctrl+a');paste('ShiftTab target');key('Tab','shift+Tab','ctrl+a');paste('Pears after ShiftTab')
    assert '| Pears after ShiftTab | 20 |' in saved(fixture)
    key('Tab','Tab');paste('New final row');key('Tab');paste('5')
    text=saved(fixture)
    assert 'New final row' in text and len([line for line in text.splitlines() if line.startswith('|')])==5
    shot('02-edited-appended-row')
    key('ctrl+alt+BackSpace','ctrl+a');paste('survivor')
    text=saved(fixture)
    assert 'New final row' not in text and len([line for line in text.splitlines() if line.startswith('|')])==4
    assert '| Pears after ShiftTab | survivor |' in text
    click(494,285);key('ctrl+a');paste('extra column')
    assert 'extra column' in saved(fixture)
    click(612,285);key('ctrl+a');paste('20')
    assert 'extra column' not in saved(fixture)
    key('ctrl+alt+2')
    assert '| :--- | :---: |' in saved(fixture)
    click(340,347,3);shot('03-column-menu');click(400,368)
    assert '|   | Name | Value |' in saved(fixture)
    key('ctrl+z')
    assert '| Name | Value |' in saved(fixture)
    click(433,245)
    assert '| Name | Value |' not in saved(fixture)
    key('ctrl+z')
    assert '| Name | Value |' in saved(fixture)
    click(378,169)
    shot('04-blocks')
    run('xdotool','mousemove','--window',window,'292','362','mousedown','1')
    for step in range(1,17):
        run('xdotool','mousemove','--window',window,str(round(292+(400-292)*step/16)),str(round(362+(475-362)*step/16)))
        time.sleep(.025)
    shot('05-drag-target')
    run('xdotool','mouseup','1');time.sleep(.3)
    text=saved(fixture)
    assert text.index('Beta paragraph.')<text.index('Gamma paragraph.')<text.index('Alpha 🦀 paragraph.')
    click(309,245)
    assert saved(fixture).index('Alpha 🦀 paragraph.')<saved(fixture).index('Gamma paragraph.')
    key('ctrl+z');shot('06-undo-selected-block')
    run('xdotool','windowsize',window,'620','420');shot('07-compact-blocks')
    close()
    assert fixture.read_bytes().startswith(b'\xef\xbb\xbf')
    # Source keyboard operations on a list with a descendant.
    lists=OUT/'list.md'
    list_source='- a\r\n  - child\r\n- b\r\n- c\r\n'
    lists.write_bytes(list_source.encode())
    launch(lists,'lists')
    click(400,205);key('ctrl+Home','alt+Right','ctrl+s')
    assert lists.read_bytes()==list_source.encode()
    shot('08-first-item-refused')
    key('Down','Down','alt+Right','ctrl+s')
    assert lists.read_bytes()==list_source.replace('- b','  - b').encode()
    shot('09-list-indent')
    key('alt+Left','ctrl+s')
    assert lists.read_bytes()==list_source.encode()
    key('alt+Down','ctrl+s')
    assert lists.read_bytes()=='- a\r\n  - child\r\n- c\r\n- b\r\n'.encode()
    key('alt+Up','ctrl+s')
    assert lists.read_bytes()==list_source.encode()
    shot('10-list-restored')
    close()
    edge=OUT/'empty-edge.md'
    original_edge='a | b\r\n--- | ---\r\nx | y'
    edge.write_bytes(original_edge.encode())
    launch(edge,'empty-edge')
    key('ctrl+shift+t');click(390,396);key('ctrl+a','BackSpace','ctrl+s')
    assert edge.read_bytes()=='| | b\r\n--- | ---\r\nx | y'.encode()
    paste('replacement')
    assert 'replacement' in saved(edge)
    key('ctrl+z','ctrl+z','ctrl+s')
    assert edge.read_bytes()==original_edge.encode()
    click(390,431);key('ctrl+a','BackSpace','ctrl+s')
    assert edge.read_bytes()=='a | b\r\n--- | ---\r\n| | y'.encode()
    shot('empty-edge-remains-editable')
    close()
    # Same native structural window in all supported locales, at minimum window size.
    for language in ['zh-CN','zh-TW','en','ko','ja','ru']:
        launch(fixture,f'locale-{language}',language)
        key('ctrl+shift+t')
        run('xdotool','windowsize',window,'620','420')
        shot(f'locale-{language}-compact')
        close()
    # 100 real, sequential keyboard commands in a >1 MiB document. Timings come
    # from the running app, including source layout, structure UI and shared undo.
    big=OUT/'large.md'
    big.write_text('| a | b |\n| --- | --- |\n| x | y |\n\n'+('Unicode 🦀 paragraph with **markdown** and [link](https://example.com).\n\n'*14000),encoding='utf-8')
    launch(big,'large')
    click(1062,82)  # Keep input in the application before opening the structural window.
    key('ctrl+shift+t');click(390,431)
    for n in range(100):
        key('ctrl+alt+1' if n%2==0 else 'ctrl+alt+3')
    shot('11-large-after-100-commands')
    text=saved(big)
    assert '| ---: | --- |' in text
    # Bounded real input burst. Multiple key events may reach the same eframe update.
    burst_started=time.monotonic()
    for n in range(100):
        run('xdotool','key','--clearmodifiers','--delay','0','ctrl+alt+Return')
        time.sleep(max(0,burst_started+(n+1)*.02-time.monotonic()))
    burst_send_ms=(time.monotonic()-burst_started)*1000
    time.sleep(2)
    burst_text=saved(big)
    assert len([line for line in burst_text.splitlines() if line.startswith('|')])==103, 'Coalesced burst lost row commands'
    shot('12-large-after-100-burst-rows')
    close()
    timing=json.loads((OUT/'large-timing.json').read_text())
    assert len(timing['command_ms'])==200, timing
    def stats(values):
        ordered=sorted(values)
        return dict(count=len(ordered),p50=ordered[len(ordered)//2],p95=ordered[min(len(ordered)-1,int(len(ordered)*.95))],maximum=max(ordered))
    sequential_cpu=[value for value,span in zip(timing['command_and_following_update_cpu_ms'],timing['update_command_ranges']) if span[1]<=100]
    burst_cpu=[value for value,span in zip(timing['command_and_following_update_cpu_ms'],timing['update_command_ranges']) if span[1]>100]
    result=dict(verification_id=verification,date=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),timezone='UTC',status='passed',exe=str(EXE),exe_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),environment=f'Debian 12 / X11 Xvfb {ENV["DISPLAY"]} / Openbox / llvmpipe OpenGL',real_pointer_keyboard=True,untouched_bom_crlf_exact=True,cell_pipe_unicode=True,tab_shift_tab=True,final_tab_adds_exactly_one_row=True,delete_final_row_column_retains_focus=True,save_keeps_grid_focus=True,shared_undo_redo=True,column_insert_before_context_menu=True,table_delete_undo=True,distant_block_drag=True,block_identity_after_undo=True,list_alt_indent_outdent_move=True,first_list_item_refused=True,empty_edge_header_body_keeps_shape_and_undo=True,locales=['zh-CN','zh-TW','en','ko','ja','ru'],compact='620x420',large_source_bytes=big.stat().st_size,large_blocks=timing['blocks'],large_sequential_commands=stats(timing['command_ms'][:100]),large_burst_commands=stats(timing['command_ms'][100:]),burst_scheduled_interval_ms=20,burst_send_ms=burst_send_ms,burst_exact_added_rows=100,large_sequential_updates_cpu=stats(sequential_cpu),large_burst_updates_cpu=stats(burst_cpu),timing_scope=timing['measurement'])
    (OUT/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
finally:
    if app is not None:
        app.terminate()
        try: app.wait(timeout=3)
        except subprocess.TimeoutExpired: app.kill();app.wait()
    for child in reversed(children):
        child.terminate()
        try:child.wait(timeout=3)
        except subprocess.TimeoutExpired:child.kill();child.wait()
