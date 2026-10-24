# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Real single-clear/interleaving GUI; capacity setup uses a public-API CLI."""
from pathlib import Path
common = Path('/work/scripts/verify-reading-precision-worker-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'capacity-clear-native-helpers', 'exec'))
ROOTOUT = OUT
shared = ROOTOUT/'shared-state.json'
instances = {}
helper = ROOT/'tests/reading-store-acceptance/target/debug/fill_reading_capacity'
assert helper.is_file()
body = ''.join(f'Paragraph {i:04} 中文 é 👩🏽‍💻. ' + 'Independent source text. '*9 + '\n\n' for i in range(180))
(WORK/'heavy.md').write_text('# Document A\n\n'+body)
(WORK/'tiny.md').write_text('# Document B\n\n'+body)
original_hashes = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in WORK.glob('*.md')}

def choose(name):
    global OUT, ENV, app, window
    OUT, ENV, app, window = instances[name]

def launch(name, filename):
    global OUT, ENV, app, window
    OUT = ROOTOUT/name
    OUT.mkdir(exist_ok=True)
    ENV = dict(ENV, MDCARD_READING_STATE_PATH=str(shared), MDCARD_READING_REPORT=str(OUT/'reading.json'),
               MDCARD_UI_REPORT=str(OUT/'ui.json'), MDCARD_SETTINGS_PATH=str(OUT/'settings.json'),
               MDCARD_SEARCH_REPORT=str(OUT/'search.json'))
    (OUT/'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', reduce_motion=True, font_size=15)))
    start(filename)
    instances[name] = OUT, ENV, app, window

disk_seen_successfully = False

def disk():
    global disk_seen_successfully
    current = json.loads(shared.read_text())
    disk_seen_successfully = True
    return current

disk_observation_errors = []
single_clear_input_qualification = None

def wait_disk(predicate, message):
    until = time.monotonic()+8
    while time.monotonic() < until:
        try:
            current = disk()
        except (FileNotFoundError, json.JSONDecodeError) as error:
            disk_observation_errors.append(dict(monotonic_ns=time.perf_counter_ns(),error=repr(error),previous_successful_read=disk_seen_successfully,exists_after_error=shared.exists()))
            time.sleep(.02)
            continue
        if predicate(current):return current
        time.sleep(.02)
    raise AssertionError(message)

try:
    services.append(subprocess.Popen(['Xvfb', ENV['DISPLAY'], '-screen', '0', '1600x1100x24'], stdout=(ROOTOUT/'xvfb.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.6)
    services.append(subprocess.Popen(['openbox'], env=ENV, stdout=(ROOTOUT/'openbox.log').open('wb'), stderr=subprocess.STDOUT))
    time.sleep(.6)
    display = lib.XOpenDisplay(ENV['DISPLAY'].encode())
    assert display
    launch('old', 'heavy.md');mode('Read');precise('main/preview');wheel('main/preview', 8)
    old_a_view = precise('main/preview'); old_a = old_a_view['position']['offset']
    old_id = state()['main_id']
    quick('tiny.md');precise('main/preview');wheel('main/preview', 12)
    old_b_view = precise('main/preview'); old_b = old_b_view['position']['offset']
    time.sleep(.8)
    quick('heavy.md');precise('main/preview')
    a_key = str((WORK/'heavy.md').resolve())
    b_key = str((WORK/'tiny.md').resolve())
    def seeded(d):
        for key, observed in [(a_key, old_a_view), (b_key, old_b_view)]:
            position = d.get('records', {}).get(key, {}).get('views', {}).get('main/preview')
            if position is None or max(abs(position['offset'][i]-observed['position']['offset'][i]) for i in range(2)) > 2:
                return False
            if position['text_anchor']['source']['source_byte'] != observed['position']['text_anchor']['source']['source_byte']:
                return False
        return True
    seeded_disk = wait_disk(seeded, 'Both exact real histories persisted before clear within original8s')
    (ROOTOUT/'disk-before-clear-seed.json').write_text(json.dumps(seeded_disk, indent=2))
    shot('01-before-external-single-clear')
    launch('clearer', 'heavy.md');key('ctrl+comma');time.sleep(.6)
    for _ in range(8):
        controls = [c['rect'] for c in state('ui')['controls'] if c['name']=="Clear this document's reading positions"]
        if controls and 20 < controls[-1][1] < controls[-1][3] < 790:break
        run('xdotool', 'mousemove', '--window', window, '800', '590', 'click', '--repeat', '8', '--delay', '15', '5');time.sleep(.2)
    else:raise AssertionError('Single clear control is not actually visible')
    for _ in range(8):
        current = state('ui')
        rect = next(c['rect'] for c in reversed(current['controls']) if c['name']=="Clear this document's reading positions")
        run('xdotool','mousemove','--window',window,str(round((rect[0]+rect[2])/2)),str(round((rect[1]+rect[3])/2)))
        time.sleep(.15)
        current = state('ui')
        button = next(c for c in reversed(current['controls']) if c['name']=="Clear this document's reading positions")
        if button['enabled'] and button['hovered'] and max(abs(button['rect'][i]-rect[i]) for i in range(4)) <= 1:
            break
    else: raise AssertionError('Actual single-clear control never stabilized under native pointer')
    def shell_fields(*arguments):
        return dict(line.split('=', 1) for line in subprocess.check_output(
            ['xdotool', *arguments], env=ENV, text=True, timeout=3).splitlines() if '=' in line)
    pointer = shell_fields('getmouselocation', '--shell')
    placement = shell_fields('getwindowgeometry', '--shell', window)
    owner = int(subprocess.check_output(['xdotool', 'getwindowpid', window], env=ENV, text=True, timeout=3))
    active_window = int(subprocess.check_output(['xdotool', 'getactivewindow'], env=ENV, text=True, timeout=3))
    ppp = current['pixels_per_point']
    local = [(int(pointer['X'])-int(placement['X']))/ppp,
             (int(pointer['Y'])-int(placement['Y']))/ppp]
    assert owner == app.pid and app.poll() is None and active_window == int(window)
    assert button['rect'][0] <= local[0] <= button['rect'][2] and button['rect'][1] <= local[1] <= button['rect'][3]
    reading_at_click = state()
    single_clear_input_qualification = dict(process_id=app.pid, window=window,
        owner_pid=owner, active_window=active_window, pointer=pointer, window_geometry=placement,
        pointer_logical=local, pixels_per_point=ppp, control=button,
        geometry_frame=(reading_at_click.get('views', {}).get('main/preview', {}).get('geometry') or {}).get('frame'),
        observed_monotonic_ns=time.perf_counter_ns())
    (ROOTOUT/'single-clear-before-native-input.json').write_text(json.dumps(single_clear_input_qualification, indent=2))
    (ROOTOUT/'single-clear-before-native-ui.json').write_text(json.dumps(current, indent=2))
    run('xdotool','mousedown','1')
    time.sleep(.08);run('xdotool', 'mouseup', '1')
    wait(lambda s:all(not r['views'] for r in s['records'] if r['id']==old_id), 'Clearer memory is empty')
    cleared = wait_disk(lambda d:a_key in d['records'] and not d['records'][a_key]['views'], 'Explicit single clear reached disk')
    assert abs(cleared['records'][b_key]['views']['main/preview']['offset'][1]-old_b[1]) <= 2
    shot('02-real-single-clear')
    setup = subprocess.run([str(helper), str(ROOTOUT/'capacity-documents'), str(WORK/'tiny.md')], env=ENV, check=True, capture_output=True, text=True)
    (ROOTOUT/'capacity-setup.json').write_text(setup.stdout)
    evicted = disk()
    assert a_key not in evicted['records'], 'Actual merged capacity setup must evict the tombstone'
    assert len(evicted['records']) <= 512 and shared.stat().st_size <= 4*1024*1024
    assert abs(evicted['records'][b_key]['views']['main/preview']['offset'][1]-old_b[1]) <= 2
    (ROOTOUT/'disk-after-actual-tombstone-eviction.json').write_text(json.dumps(evicted, indent=2))
    choose('old');run('xdotool', 'windowactivate', '--sync', window)
    mode('Split');mode('Read');time.sleep(1.3)
    assert a_key not in disk()['records'], 'Old metadata-only GUI activity resurrected history'
    shot('03-stale-metadata-without-resurrection')
    quick('tiny.md');restored_b = precise('main/preview')['position']['offset']
    assert abs(restored_b[1]-old_b[1]) <= 2
    wheel('main/preview', 1);wheel('main/preview', -1)
    quick('heavy.md');empty_a = settle('main/preview')['offset']
    assert abs(empty_a[1]) <= 2 and state()['main_id']==old_id
    shot('04-stale-reopen-from-top')
    wheel('main/preview', 8);new_a = precise('main/preview')['position']['offset'];assert new_a[1] > 100
    wait_disk(lambda d:a_key in d['records'] and abs(d['records'][a_key]['views']['main/preview']['offset'][1]-new_a[1]) <= 2, 'Genuine later input persisted')
    shot('05-genuine-new-reading')
    stop();launch('restart', 'heavy.md');mode('Read');restart_a = precise('main/preview')['position']['offset']
    assert max(abs(restart_a[i]-new_a[i]) for i in range(2)) <= 2
    final = disk()
    assert len(final['records']) <= 512 and shared.stat().st_size <= 4*1024*1024
    assert b_key in final['records'] and abs(final['records'][b_key]['views']['main/preview']['offset'][1]-old_b[1]) <= 2
    assert original_hashes == {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in WORK.glob('*.md')}
    shot('06-restart-new-position');stop();choose('clearer');key('Escape');stop()
    result = dict(passed=True, binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(), helper_sha256=hashlib.sha256(helper.read_bytes()).hexdigest(),
                  old_a=old_a, old_b=old_b, reopened_a=empty_a, new_a=new_a, restart_a=restart_a, b_retained=restored_b,
                  records=len(final['records']), state_bytes=shared.stat().st_size, manuscript_hashes=original_hashes,
                  method='Two actual GUI instances; native single clear, metadata-only mode buttons, B to A, genuine scroll and restart. CLI capacity setup performs 512 real public-API opens/observations and merged eviction, not 512 GUI opens or position injection into an app.')
    result['disk_observation_errors'] = disk_observation_errors
    result['single_clear_control'] = button
    result['single_clear_input_qualification'] = single_clear_input_qualification
    result['seed_positions'] = dict(a=old_a_view['position'],b=old_b_view['position'])
    result['state_filesystem'] = dict(path=str(shared),stat_filesystem=subprocess.check_output(['stat','-f','-c','%T',str(ROOTOUT)],text=True).strip(),boundary='Application state is explicitly on the /work host workspace mount. Errors are observations, not a diagnosed filesystem root cause; the original deadline and final conditions are unchanged.')
    (ROOTOUT/'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps(result, indent=2))
except Exception as error:
    try:shot('failure')
    except Exception:pass
    (ROOTOUT/'failure.json').write_text(json.dumps(dict(error=repr(error), single_clear_input_qualification=single_clear_input_qualification, disk_observation_errors=disk_observation_errors, disk_seen_successfully=disk_seen_successfully, state_path=str(shared), state_exists=shared.exists()), indent=2));raise
finally:
    if display:lib.XCloseDisplay(display)
    for _, _, process, _ in instances.values():
        if process.poll() is None:process.kill();process.wait(timeout=5)
    for service in reversed(services):
        service.terminate()
        try:service.wait(timeout=5)
        except subprocess.TimeoutExpired:service.kill();service.wait(timeout=5)
