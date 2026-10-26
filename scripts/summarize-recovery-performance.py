# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Summarize the raw native recovery performance arrays without dropping samples."""
import argparse
import json
import math
from pathlib import Path
import statistics

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory',type=Path)
args=parser.parse_args()
def stats(values):
    values=sorted(float(v) for v in values)
    return {'n':len(values),'median_ms':statistics.median(values),'p95_ms':values[math.ceil(len(values)*.95)-1],'max_ms':max(values)} if values else {'n':0}
rows=[]
for m in json.loads((args.directory/'measurements.json').read_text(encoding='utf-8-sig')):
    workers=[c['worker_ms'] for c in m['commits']]+[m['after_manual_save']['worker_ms']]
    rows.append({'mib':m['requested_mib'],'source_bytes':m['source_bytes'],'checkpoint_worker':stats(workers),'all_update_cpu':stats(m['update_cpu_ms']),'input_update_cpu':stats(m['input_event_to_update_finish_ms']),'peak_sampled_working_set_mib':m['peak_sampled_working_set']/1048576,'native_peak_working_set_mib':m.get('native_peak_working_set',0)/1048576,'input_samples_with_worker':m.get('input_samples_with_worker',0),'observed_max_writers':m['observed_max_writers'],'observed_max_pending_versions':m['observed_max_pending_versions'],'manual_save_observed_during_worker':m['manual_save_observed_during_worker'],'diagnostics':'UI, performance, rich/structure metadata; search manuscript report disabled','note':'Update CPU begins inside the app; excludes OS queue, GPU presentation and physical scanout. All recorded samples are included.'})
(args.directory/'summary.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(rows,ensure_ascii=False,indent=2))
