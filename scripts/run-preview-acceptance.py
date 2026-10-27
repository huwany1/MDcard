# SPDX-License-Identifier: AGPL-3.0-only
"""Sequential native GUI suite: never runs two desktop drivers concurrently."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

p = argparse.ArgumentParser()
p.add_argument('--output', required=True)
p.add_argument('--start', type=int, default=0)
p.add_argument('--candidate-exe', required=True, help='Candidate executable to verify')
p.add_argument('--baseline-exe', help='Original baseline executable for functional comparison')
p.add_argument('--measured-baseline-exe', help='Instrumented baseline executable for timing comparison')
p.add_argument('--latency-only', action='store_true')
p.add_argument('--candidate-only', action='store_true')
a = p.parse_args()
root=Path(__file__).resolve().parents[1];out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
jobs=[
    ('baseline-measured','latency-8192',['--scenario','latency','--bytes','8192','--samples','40']),
    ('candidate','latency-8192',['--scenario','latency','--bytes','8192','--samples','40']),
    ('baseline-measured','latency-65535',['--scenario','latency','--bytes','65535','--samples','40']),
    ('candidate','latency-65535',['--scenario','latency','--bytes','65535','--samples','40']),
    ('baseline-measured','latency-65537',['--scenario','latency','--bytes','65537','--samples','40']),
    ('candidate','latency-65537',['--scenario','latency','--bytes','65537','--samples','40']),
    ('baseline-measured','latency-1048575',['--scenario','latency','--bytes','1048575','--samples','40']),
    ('candidate','latency-1048575',['--scenario','latency','--bytes','1048575','--samples','40']),
    ('candidate','latency-1048577',['--scenario','latency','--bytes','1048577','--samples','12']),
    ('baseline','continuous-8192',['--seconds','32','--bytes','8192']),
    ('candidate','continuous-8192',['--seconds','32','--bytes','8192']),
    ('candidate','media',['--scenario','media','--seconds','32']),
    ('candidate','resources',['--scenario','resources']),
    ('candidate','ime',['--scenario','ime']),
    ('baseline','latency-8192-overhead',['--scenario','latency','--bytes','8192','--samples','40']),
]
if a.latency_only:
    jobs=[(build,f'fixed-slot-{size}', ['--scenario','latency','--bytes',str(size),'--samples','40'])
        for size in (8192,65535,65537,1048575,1048577) for build in ('baseline-measured','candidate')]
if a.candidate_only:jobs=[job for job in jobs if job[0]=='candidate']
executables = {
    'candidate': a.candidate_exe,
    'baseline': a.baseline_exe,
    'baseline-measured': a.measured_baseline_exe,
}
for build in {job[0] for job in jobs[a.start:]}:
    if not executables[build] or not Path(executables[build]).is_file():
        p.error(f'Provide an existing executable for {build}; extract archived baselines into an isolated test directory when needed')
results=[]
for index,(build,name,extra) in enumerate(jobs[a.start:],a.start):
    directory=out/f'{index:02d}-{build}-{name}'
    state=dict(current=index,name=directory.name,started=time.time(),completed=results)
    (out/'batch-state.json').write_text(json.dumps(state,indent=2),encoding='utf-8')
    command=[sys.executable,'-X','utf8',str(root/'scripts/verify-preview-windows.py'),'--exe',str(Path(executables[build]).resolve()),'--output',str(directory),*extra]
    with (out/f'{index:02d}.log').open('w',encoding='utf-8') as log:
        run=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT)
    results.append(dict(index=index,name=directory.name,exit_code=run.returncode,finished=time.time()))
    if run.returncode:
        state.update(status='failed',completed=results)
        (out/'batch-state.json').write_text(json.dumps(state,indent=2),encoding='utf-8');sys.exit(run.returncode)
(out/'batch-state.json').write_text(json.dumps(dict(status='complete',completed=results),indent=2),encoding='utf-8')
