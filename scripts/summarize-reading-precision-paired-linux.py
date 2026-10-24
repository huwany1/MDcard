# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Summarize every frozen Linux pair, retaining failed cohorts and distributions."""
from pathlib import Path
import hashlib
import json
import os

ROOT = Path('/work')
TAG = os.environ['MDCARD_PRECISION_PERF_RUN']
ARTIFACT = ROOT / 'artifacts/reading-precision' / TAG
EVIDENCE = ROOT / 'evidence/reading-precision/implementation'


def distribution(values):
    # Same upper order statistic as both pre-frozen Linux input drivers.
    ordered = sorted(values)
    return dict(count=len(values), p50=ordered[len(values) // 2] if values else None,
                p95=ordered[min(len(values) - 1, int(len(values) * .95))] if values else None,
                maximum=max(values) if values else None)


def full_update(path):
    report = json.loads(path.read_text())
    assert report['queue_dropped_samples'] == 0
    samples = report['samples']
    frames = {}
    wheel_frames = set()
    for sample in samples:
        frames[sample['frame']] = frames.get(sample['frame'], 0) + sample['elapsed_wall_ms']
        if sample['wheel_input']:
            wheel_frames.add(sample['frame'])
    return dict(all_invocations_wall_ms=distribution([s['elapsed_wall_ms'] for s in samples]),
                wheel_invocations_wall_ms=distribution([s['elapsed_wall_ms'] for s in samples if s['wheel_input']]),
                all_egui_frame_update_sums_wall_ms=distribution(list(frames.values())),
                wheel_egui_frame_update_sums_wall_ms=distribution([frames[f] for f in wheel_frames]),
                dropped_samples=0)


plan = json.loads((ARTIFACT / 'plan-before-first-measurement.json').read_text())
results = []
for mode in ['off', 'on']:
    for size in ['ordinary', 'one-mib', 'ten-mib']:
        row = dict(mode=mode, size=size, comparisons=[], source_runs={})
        cohorts = {}
        for variant in ['baseline', 'candidate']:
            folder = EVIDENCE / f'{TAG}-{mode}-{variant}-{size}'
            result_path = folder / 'result.json'
            if not result_path.exists():
                row['source_runs'][variant] = dict(passed=False, missing_result=True,
                                                 failure=json.loads((folder / 'failure.json').read_text()) if (folder / 'failure.json').exists() else None)
                continue
            source = json.loads(result_path.read_text())
            row['source_runs'][variant] = dict(passed=source['passed'], binary_sha256=source['binary_sha256'],
                                             result_sha256=hashlib.sha256(result_path.read_bytes()).hexdigest())
            cohorts[variant] = source['timing'] if mode == 'off' else {app: full_update(folder / f'{app}-full-update.json') for app in ['a', 'b']}
            if mode == 'on':
                row['source_runs'][variant]['phase_wall_ms'] = source['timing']
                row['source_runs'][variant]['peak_observed_app_rss_bytes'] = source['peak_observed_app_rss_bytes']
                row['source_runs'][variant]['continuous_scroll'] = source['continuous_scroll']
        if len(cohorts) == 2:
            for app in ['a', 'b']:
                for metric in ['all_invocations_wall_ms', 'wheel_invocations_wall_ms', 'all_egui_frame_update_sums_wall_ms', 'wheel_egui_frame_update_sums_wall_ms']:
                    old, new = cohorts['baseline'][app][metric], cohorts['candidate'][app][metric]
                    assert old['count'] and new['count']
                    allowed_increase = max(old['p95'] * .20, 2)
                    row['comparisons'].append(dict(app=app, file_type='Markdown' if app == 'a' else 'plain text',
                        metric=metric, baseline=old, candidate=new, p95_increase_ms=new['p95'] - old['p95'],
                        allowed_increase_ms=allowed_increase, passed=new['p95'] - old['p95'] <= allowed_increase))
        row['passed'] = len(row['comparisons']) == 8 and all(c['passed'] for c in row['comparisons'])
        results.append(row)

summary = dict(plan_sha256=hashlib.sha256((ARTIFACT / 'plan-before-first-measurement.json').read_bytes()).hexdigest(),
               quantiles='Upper sorted order index floor(n*p), capped at n-1, matching both pre-frozen Linux drivers; not Windows nearest-rank.',
               scope='Instant.elapsed function wall across the entire App::update entry to return, plus per-egui-frame sums; includes synchronous diagnostic I/O in ON. Observer queue/writer itself outside sample. No thread CPU, renderer/framework time, physical display latency or FPS claim.',
               normal_use_diag_off_passed=all(r['passed'] for r in results if r['mode'] == 'off'),
               diagnostic_on_passed=all(r['passed'] for r in results if r['mode'] == 'on'), pairs=results)
(ARTIFACT / 'summary.json').write_text(json.dumps(summary, indent=2))
print(json.dumps(dict(normal_use_diag_off_passed=summary['normal_use_diag_off_passed'],
                      diagnostic_on_passed=summary['diagnostic_on_passed'],
                      failures=[dict(mode=r['mode'], size=r['size'], comparisons=[c for c in r['comparisons'] if not c['passed']]) for r in results if not r['passed']]), indent=2))
