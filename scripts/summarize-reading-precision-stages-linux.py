# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Derive six wall-clock phases from completed, unchanged raw measurements.

No sampling or product execution occurs here. Thread CPU counters were not taken.
"""
import argparse
import hashlib
import json
from pathlib import Path


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def distribution(values):
    ordered=sorted(values)
    return dict(n=len(ordered),p50_ms=ordered[len(ordered)//2] if ordered else None,
                p95_ms=ordered[min(len(ordered)-1,int(len(ordered)*.95))] if ordered else None,
                max_ms=max(ordered) if ordered else None)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--root',type=Path,default=Path('/work'))
    parser.add_argument('--cohort',required=True)
    parser.add_argument('--completed-stages',type=Path,required=True)
    parser.add_argument('--completed-manifest',type=Path,default=Path('artifacts/reading-precision/completed-stages-bench/build-manifest.json'))
    parser.add_argument('--completed-binary',type=Path,default=Path('target-completed-stages-bench/release/precision-completed-stages-bench'))
    parser.add_argument('--source-manifest',type=Path,default=Path('artifacts/reading-precision/stage-b-r21-source.json'))
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    root=args.root.resolve()
    completed_path=(root/args.completed_stages).resolve()
    completed=read(completed_path)
    manifest_path=(root/args.source_manifest).resolve()
    manifest=read(manifest_path)
    bench_manifest_path=(root/args.completed_manifest).resolve()
    bench_manifest=read(bench_manifest_path)
    pure_path=root/'src/reading_precision.rs'
    pure_sha=hashlib.sha256(pure_path.read_bytes()).hexdigest()
    assert bench_manifest['api_sha256']==pure_sha==manifest['source_sha256']['src/reading_precision.rs'], 'Completed benchmark/current pure source mismatch'
    bench_binary=(root/args.completed_binary).resolve()
    assert hashlib.sha256(bench_binary.read_bytes()).hexdigest()==bench_manifest['binary_sha256'], 'Completed benchmark binary drift'
    expected_binary=manifest['binaries']['linux']
    assert hashlib.sha256((root/expected_binary['path']).read_bytes()).hexdigest()==expected_binary['sha256'], 'Current GUI candidate binary drift'
    rows=[]
    sources={completed_path.relative_to(root).as_posix():hashlib.sha256(completed_path.read_bytes()).hexdigest()}
    for path in [manifest_path,bench_manifest_path,pure_path]:
        sources[path.relative_to(root).as_posix()]=hashlib.sha256(path.read_bytes()).hexdigest()
    for size,file in [('ordinary','ordinary.md'),('one-mib','one-mib.md'),('ten-mib','ten-mib.md')]:
        folder=root/f'evidence/reading-precision/implementation/{args.cohort}-on-candidate-{size}'
        result_path=folder/'result.json'
        result=read(result_path)
        assert result['passed']
        assert result['binary_sha256']==expected_binary['sha256'], ('Result binary does not match R21',size)
        sources[result_path.relative_to(root).as_posix()]=hashlib.sha256(result_path.read_bytes()).hexdigest()
        metrics_path=folder/'first-scroll-metrics.json'
        observations_path=folder/'external-observations.json'
        for path in [metrics_path,observations_path]:
            sources[path.relative_to(root).as_posix()]=hashlib.sha256(path.read_bytes()).hexdigest()
        md_metrics=read(metrics_path)
        txt_metrics=result['other_scroll'][0]['metrics']
        observations=read(observations_path)
        matching=next(row for row in completed['results'] if row['file']==file)
        for app,format,metrics in [('a','Markdown',md_metrics),('b','plain text',txt_metrics)]:
            phases=dict(
                capture=distribution(metrics.get('position_capture_cpu_ms',[])),
                completed_source_index=distribution(metrics.get('index_build_ms',[])),
                completed_candidate_match=distribution(matching['completed_candidate_matching']['raw_wall_ms']),
                actual_layout_by_kind={name:distribution(values) for name,values in metrics.get('layout_wall_ms',{}).items()},
                main_thread_record_submit=distribution(metrics.get('submit_cpu_ms',[])),
                observed_successful_disk_transactions=distribution([v['duration_ms'] for v in observations if v['kind']=='successful-disk-transaction' and v['name']==app]),
            )
            rows.append(dict(size=size,format=format,binary_sha256=result['binary_sha256'],phases=phases,
                hot_frame_wall_ms=distribution(metrics.get('hot_frame_wall_ms',[])),
                actual_scroll_completed_match=distribution(metrics.get('match_wall_ms',[])),
                separate_completed_enrichment=distribution(matching['completed_anchor_enrichment']['raw_wall_ms'])))
    output=(root/args.output).resolve()
    assert output.is_relative_to(root/'artifacts/reading-precision') and not output.exists()
    report=dict(
        cohort=args.cohort,source_sha256=sources,rows=rows,
        component_binding=dict(current_pure_sha256=pure_sha,completed_api_sha256=bench_manifest['api_sha256'],
                               completed_binary_sha256=bench_manifest['binary_sha256'],gui_binary_sha256=expected_binary['sha256']),
        measurement='Function wall intervals from Instant.elapsed. Linux quantiles floor(n*p), capped at n-1. Thread CPU counters unmeasured.',
        boundary='Capture/source-index/layout/submit/disk come from actual diagnosticON GUI calls; source-index counts include only completed jobs. '
                 'Candidate match and extra enrichment are n10 completed calls per size on one dedicated background thread, with exact independent prefix expected; '
                 'the same pure matching batch is displayed beside both formats and must not be counted twice. Normal scroll matching n0 remains0. '
                 'Disk rows are externally observed successful transaction samples, not all attempted writes or failed lock samples. '
                 'Actual layout kinds are kept separate; application layout calls can include font-cache lookup. '
                 'These phases do not replace complete App::update OFF/ON paired budget comparisons and do not claim physical display latency/FPS.',
    )
    output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(dict(rows=len(rows),output=str(output))))


if __name__=='__main__':
    main()
