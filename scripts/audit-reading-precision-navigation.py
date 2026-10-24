# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Read-only independent audit of functional and native navigation evidence."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--manifest-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    manifest_path = (ROOT / args.manifest).resolve()
    output = (ROOT / args.output).resolve()
    assert output.is_relative_to(ROOT / 'evidence/reading-precision/master')
    assert not output.exists(), 'Retain previous audits'
    assert digest(manifest_path) == args.manifest_sha256
    manifest = read(manifest_path)
    source_path = ROOT / manifest['source_manifest']
    assert digest(source_path) == manifest['source_manifest_sha256']
    source = read(source_path)
    binary = source['binaries']['linux']
    assert digest(ROOT / binary['path']) == binary['sha256']
    checks = []
    for case in manifest['linux_functional_groups']:
        path = ROOT / case['result']
        result = read(path)
        assert result['passed'] and result['binary_sha256'].lower() == binary['sha256']
        assert digest(path) == case['result_sha256']
        checks.append(dict(name=case['name'], path=case['result'], sha256=digest(path)))
    helper = ROOT / 'scripts/verify-reading-precision-cancellation-linux.py'
    helper_text = helper.read_text(encoding='utf-8')
    prefix = helper_text[:helper_text.index('\ntry:\n    services.append')]
    navigation = []
    for case in manifest['navigation_priority_groups']:
        for name, expected_sha in case['evidence_sha256'].items():
            assert digest(ROOT / name) == expected_sha, name
        folder = (ROOT / case['result']).parent
        result = read(folder / 'result.json')
        assert result['passed'] and result['binary_sha256'].lower() == binary['sha256']
        assert digest(folder / 'driver.py') == result['driver_sha256']
        assert digest(helper) == result['imported_cancellation_helper_sha256']
        assert (folder / 'imported-cancellation-prefix.py').read_text(encoding='utf-8') == prefix
        before = read(folder / 'before-navigation.json')
        after = read(folder / '02-stable-after-layout.json')
        raw = (folder / 'workspace/heavy.md').read_bytes()
        assert hashlib.sha256(raw).hexdigest() == result['changed_sha256']
        expected = result['expected_source_byte']
        end = result['expected_source_end']
        view = after['views']['main/source']
        glyph = view['geometry']
        assert glyph['source_byte'] == expected and glyph['source_end'] == end
        assert raw[expected:end].decode('utf-8') == glyph['source_text'] == result['actual_glyph']['source_text']
        line = raw[:expected].count(b'\n')
        assert glyph['logical_line'] == result['expected_logical_line'] == line
        assert abs(glyph['rect'][1] - view['viewport_rect'][1] - glyph['viewport_y']) < .001
        assert view['pending'] is None
        old_task, new_task = result['before_task'], result['after_task']
        assert old_task['running'] and old_task['progress_checkpoints'] > 0
        assert old_task['cancel_requested_monotonic_ns'] == 0
        assert new_task['task_id'] == old_task['task_id']
        assert new_task['metadata'] == old_task['metadata']
        assert new_task['cancel_requested_while_running'] and not new_task['running']
        assert new_task['cancelled_phase'] == 'relocation-match'
        assert new_task['started_monotonic_ns'] < new_task['cancel_requested_monotonic_ns'] <= new_task['finished_monotonic_ns']
        for name, report_name in [('document_id', 'main_id'), ('buffer_id', 'main_buffer_id'), ('revision', 'main_revision')]:
            assert old_task['metadata'][name] == before[report_name]
        pending = before['views']['main/source']['pending']
        assert pending is not None
        old = pending['position']['text_anchor']['source']['source_byte']
        assert old == result['old_history_source_byte'] and old != expected
        tail = result['tail']
        duration = (tail[-1]['monotonic_ns'] - tail[0]['monotonic_ns']) / 1e6
        assert len(tail) >= 20 and duration >= 2000
        assert all(sample['glyph']['source_byte'] == expected for sample in tail)
        assert max(abs(sample['glyph']['viewport_y'] - tail[0]['glyph']['viewport_y']) for sample in tail) <= 2
        assert max(abs(sample['offset'][1] - tail[0]['offset'][1]) for sample in tail) <= 2
        navigation.append(dict(kind=result['kind'], source_byte=expected,
            independently_counted_logical_line=line, active_geometry_logical_line=glyph['logical_line'],
            visible_text_logical_line_default_not_used=result['actual_glyph']['logical_line'],
            old_pending_source_byte=old, cancelled_while_running=True, task_id=new_task['task_id'],
            tail_duration_ms=duration, driver_sha256=result['driver_sha256'],
            imported_helper_sha256=result['imported_cancellation_helper_sha256'],
            screenshot_sha256=digest(folder / '02-stable-after-layout.png')))
        checks.append(dict(name=result['kind'] + '-navigation-race', path=case['result'], sha256=digest(folder / 'result.json')))
    report = dict(passed=True, created_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
        product_commit=manifest['product_commit'], manifest=str(manifest_path.relative_to(ROOT)),
        manifest_sha256=digest(manifest_path), current_binary_sha256=binary['sha256'],
        functional_groups=checks, navigation_independent_checks=navigation,
        boundary='Read-only original UTF-8, actual current Galley coordinates, active logical line and native cancellation evidence. Visible-text diagnostic default logical_line is not used. Separate from exclusive performance and extracted-package acceptance.')
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, functional_groups=len(checks), navigation=navigation)))


if __name__ == '__main__':
    main()
