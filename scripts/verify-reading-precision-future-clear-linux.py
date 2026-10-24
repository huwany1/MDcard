# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Native clear/restart QA with a valid +1h persisted-clock fixture.

Only closed-application metadata timestamps are seeded. Positions, commands,
matching, and UI state are never injected. Both failures remain in the result.
"""
from pathlib import Path
common = Path('/work/scripts/verify-reading-precision-linux.py').read_text()
exec(compile(common[:common.index('\ntry:\n    services.append')], 'native-future-clear-helpers', 'exec'))

ROOTOUT = OUT
WORK = ROOTOUT / 'workspace'
WORK.mkdir(exist_ok=True)
manuscript = '# Valid clock rollback QA\n\n' + ''.join(
    f'## Chapter {i:04d}\n\nParagraph {i:04d} 中文🙂 ' + 'ordinary visible text ' * 14 + '\n\n'
    for i in range(120)
)
(WORK / 'sample.md').write_text(manuscript)
(WORK / 'inactive.md').write_text(manuscript.replace('Valid clock rollback QA', 'Inactive clock rollback QA'))
original_sha = hashlib.sha256((WORK / 'sample.md').read_bytes()).hexdigest()
results = []


def configure(name):
    global OUT, ENV
    OUT = ROOTOUT / name
    OUT.mkdir(exist_ok=True)
    ENV = dict(ENV, MDCARD_READING_STATE_PATH=str(OUT/'reading-state.json'),
               MDCARD_READING_REPORT=str(OUT/'reading.json'),
               MDCARD_UI_REPORT=str(OUT/'ui.json'),
               MDCARD_SETTINGS_PATH=str(OUT/'settings.json'))
    ENV.pop('MDCARD_SEARCH_REPORT', None)
    (OUT/'settings.json').write_text(json.dumps(dict(language='en', appearance='Light', reduce_motion=True, font_size=15)))


def disk():
    return json.loads((OUT/'reading-state.json').read_text())


def click_clear(label):
    key('ctrl+comma')
    time.sleep(.7)
    for _ in range(8):
        current = state('ui')
        found = [c['rect'] for c in current['controls'] if c['name'] == label]
        if found and 20 < found[-1][1] < found[-1][3] < 790:
            r = found[-1]
            run('xdotool', 'mousemove', '--window', window, str(round((r[0]+r[2])/2)), str(round((r[1]+r[3])/2)))
            time.sleep(.15)
            run('xdotool', 'mousedown', '1')
            time.sleep(.08)
            run('xdotool', 'mouseup', '1')
            return
        run('xdotool', 'mousemove', '--window', window, '800', '590', 'click', '--repeat', '8', '--delay', '15', '5')
        time.sleep(.3)
    raise AssertionError('Clear control not actually visible: '+label)


try:
    services.append(subprocess.Popen(['Xvfb',ENV['DISPLAY'],'-screen','0','1600x1100x24'],stdout=(ROOTOUT/'xvfb.log').open('wb'),stderr=subprocess.STDOUT))
    time.sleep(.6)
    services.append(subprocess.Popen(['openbox'],env=ENV,stdout=(ROOTOUT/'openbox.log').open('wb'),stderr=subprocess.STDOUT))
    time.sleep(.6)
    for name, label in [('single', "Clear this document's reading positions"), ('all', 'Clear all reading positions')]:
        configure(name)
        start('sample.md')
        mode('Read')
        precise('main/preview')
        wheel('main/preview', 18)
        before = precise('main/preview')
        assert before['position']['offset'][1] > 500
        shot('01-real-scroll-before-seed')
        stop()
        inactive_before = None
        if name == 'all':
            start('inactive.md')
            mode('Read')
            precise('main/preview')
            wheel('main/preview', 16)
            inactive_before = precise('main/preview')['position']
            assert inactive_before['offset'][1] > 500
            shot('01b-real-inactive-document-scroll')
            stop()
        saved = disk()
        (OUT/'disk-before-clock-seed.json').write_text(json.dumps(saved, indent=2))
        future = int(time.time()*1000) + 3_600_000
        for record in saved['records'].values():
            record['updated_ms'] = future
            record['activity_ms'] = future
            for position in record.get('views', {}).values():
                position['updated_ms'] = future
            for summary in record.get('restore_status', {}).values():
                summary['updated_ms'] = future
        (OUT/'reading-state.json').write_text(json.dumps(saved))
        (OUT/'disk-valid-one-hour-ahead.json').write_text(json.dumps(saved, indent=2))
        start('sample.md')
        restored = precise('main/preview')
        assert abs(restored['position']['offset'][1]-before['position']['offset'][1]) <= 2
        assert state()['writes'].get('error') is None, state()['writes']
        shot('02-valid-future-history-restored')
        click_clear(label)
        wait(lambda s:all(not r['views'] for r in s['records']), 'Real clear emptied memory')
        time.sleep(1.2)
        shot('03-memory-cleared-modal-idle')
        key('Escape')
        immediate = None
        if os.environ.get('MDCARD_FUTURE_CLEAR_SAME_INSTANCE') == '1':
            wheel('main/preview', 8)
            immediate = precise('main/preview')['position']['offset']
            assert immediate[1] > 500
            shot('03b-same-instance-genuine-scroll-after-clear')
        stop()
        after_clear = disk()
        (OUT/'disk-after-real-clear.json').write_text(json.dumps(after_clear, indent=2))
        target_file = 'sample.md' if immediate else ('inactive.md' if name == 'all' else 'sample.md')
        start(target_file)
        reopened = settle('main/preview')
        time.sleep(.5)
        shot('04-restart-after-clear')
        actual = reopened['offset'][1]
        wheel('main/preview', 8)
        fresh = precise('main/preview')['position']
        assert fresh['offset'][1] > 300, fresh
        shot('05-genuine-reading-after-clear')
        stop()
        start(target_file)
        fresh_reopened = precise('main/preview')['position']
        assert abs(fresh_reopened['offset'][1]-fresh['offset'][1]) <= 2
        shot('06-new-reading-is-persistable')
        expected_y = immediate[1] if immediate else 0
        results.append(dict(name=name, passed=abs(actual-expected_y)<=2, old_offset=before['position']['offset'],
                            reopened_offset=reopened['offset'], expected_reopened_y=expected_y,
                            same_instance_new_activity=immediate,
                            new_activity_offset=fresh['offset'], new_activity_reopened=fresh_reopened['offset'],
                            clock_fixture_ms=future, disk_clear_ms=after_clear.get('cleared_ms'),
                            inactive_document_before=inactive_before,
                            disk_records_after_clear=after_clear['records'],
                            memory_clear_observed=True, modal_idle_seconds=1.2))
        stop()
    result = dict(passed=all(r['passed'] for r in results), binary_sha256=hashlib.sha256(EXE.read_bytes()).hexdigest(),
                  cases=results, manuscript_sha256=original_sha,
                  manuscript_unchanged=hashlib.sha256((WORK/'sample.md').read_bytes()).hexdigest()==original_sha,
                  method='Real scroll, close, seed only valid +1h timestamps, reopen accepted without error, native Preferences clear, idle, normal close/restart. No scroll/position injection.')
    (ROOTOUT/'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps(result, ensure_ascii=False, indent=2))
except Exception as error:
    try:
        if app is not None and app.poll() is None:shot('failure')
    except Exception:pass
    (ROOTOUT/'failure.json').write_text(json.dumps(dict(error=repr(error),cases=results),indent=2))
    raise
finally:
    if app is not None and app.poll() is None:app.kill();app.wait(timeout=5)
    for p in reversed(services):
        p.terminate()
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
