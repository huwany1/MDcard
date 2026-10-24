# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Run the pre-frozen OFF/ON cohorts sequentially with fresh paired old/new builds."""
from pathlib import Path
import hashlib,json,os,subprocess,time
root=Path('/work');tag=os.environ['MDCARD_PRECISION_PERF_RUN'];out=root/'artifacts/reading-precision'/tag;out.mkdir(exist_ok=False)
baseline=root/'artifacts/reading-precision/entry-baseline-264d42d/mdcard'
candidate=Path(os.environ['MDCARD_READING_EXE'])
drivers={'off':root/'scripts/measure-reading-precision-entry-visible-linux.py','on':root/'scripts/measure-reading-precision-linux.py'}
plan=[]
for mode in ['off','on']:
 for size in ['ordinary','one-mib','ten-mib']:
  for variant,exe in [('baseline',baseline),('candidate',candidate)]:
   name=f'{tag}-{mode}-{variant}-{size}'
   env=dict(os.environ,MDCARD_READING_EXE=str(exe))
   if mode=='off':env.update(MDCARD_ENTRY_RUN=name,MDCARD_ENTRY_FIRST=size+'.md',MDCARD_ENTRY_SECOND=size+'.txt')
   else:env.update(MDCARD_READING_MEASURE_RUN=name,MDCARD_READING_MEASURE_FIRST=size+'.md',MDCARD_READING_MEASURE_SECOND=size+'.txt')
   plan.append(dict(mode=mode,size=size,variant=variant,exe=str(exe),binary_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),run=name,driver=str(drivers[mode]),driver_sha256=hashlib.sha256(drivers[mode].read_bytes()).hexdigest(),environment={k:v for k,v in env.items() if k.startswith('MDCARD_')},env=env))
manifest=dict(frozen_before_measurement_unix_ns=time.time_ns(),budget=dict(main_hot_p95_ms=2,main_hot_max_ms=8,whole_update_p95_increase='max(20 percent baseline, 2 ms)'),baseline_commit='264d42de5f88fe910a60a0de573bcd9daff8dc2d',observer_sha256=hashlib.sha256((root/'src/update_probe.rs').read_bytes()).hexdigest(),api_sha256=hashlib.sha256((root/'src/reading_precision.rs').read_bytes()).hexdigest(),inputs_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (root/'artifacts/reading-precision/performance-inputs').glob('*') if p.suffix in ['.md','.txt']},plan=[{k:v for k,v in p.items() if k!='env'} for p in plan],protocol='Existing frozen cohort drivers unchanged. Same host/docs/window/font/flags/input per pair. OFF normal use primary; ON synchronous diagnostics separately retained. Previous historical measurements retained. No compilation/other heavy GUI during sampling.')
manifest['setup']={'off':'Previously verified visible-window wrapper; only native mapped-window discovery and bounded actual foreground polling differ from original frozen helper. Original wheel input driver SHA checked by wrapper.','on':'Original frozen ON driver with its existing 5-second actual foreground retry; no new helper.','comparison':'Each baseline/candidate pair uses the same mode helper, documents, flags, dimensions, native inputs, scope and unchanged budget.'}
(out/'plan-before-first-measurement.json').write_text(json.dumps(manifest,indent=2))
status=[]
for p in plan:
 begin=time.time_ns()
 with (out/(p['run']+'.log')).open('wb') as log:
  r=subprocess.run(['python3',p['driver']],env=p['env'],stdout=log,stderr=subprocess.STDOUT)
 status.append(dict(run=p['run'],exit_code=r.returncode,begin_unix_ns=begin,end_unix_ns=time.time_ns()))
 (out/'execution-status.json').write_text(json.dumps(status,indent=2))
 print(json.dumps(status[-1]),flush=True)
bench=root/'target-completed-stages-bench/release/precision-completed-stages-bench'
completed=root/'evidence/reading-precision/implementation'/(tag+'-completed-stages');completed.mkdir(exist_ok=False)
with (completed/'result.json').open('wb') as output, (completed/'stderr.log').open('wb') as errors:
 r=subprocess.run([str(bench)],stdout=output,stderr=errors)
(completed/'manifest.json').write_text(json.dumps(dict(exit_code=r.returncode,binary_sha256=hashlib.sha256(bench.read_bytes()).hexdigest(),api_sha256=manifest['api_sha256'],driver_sha256=hashlib.sha256((root/'scripts/measure-reading-precision-completed-linux.py').read_bytes()).hexdigest(),scope='Completed pure functions in dedicated named background thread. Separate from GUI whole update, renderer/font, main capture/submit, disk and canceled tasks.'),indent=2))
print(json.dumps(dict(completed_benchmark_exit_code=r.returncode)),flush=True)
