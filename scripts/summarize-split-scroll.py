# SPDX-License-Identifier: AGPL-3.0-only
"""Preserve all attempts and separate UI commands from desktop observations."""
import argparse
import json
import math
import hashlib
from pathlib import Path
import re

def stats(values):
    values=sorted(values)
    return dict(n=len(values),p50=values[math.ceil(len(values)*.5)-1],p95=values[math.ceil(len(values)*.95)-1],maximum=values[-1]) if values else dict(n=0,p50=None,p95=None,maximum=None)

def main():
    p=argparse.ArgumentParser();p.add_argument('directory');a=p.parse_args();root=Path(a.directory)
    frames=json.loads((root/'frames.json').read_text(encoding='utf-8'))
    actions=json.loads((root/'actions.json').read_text(encoding='utf-8'))
    env=json.loads((root/'environment.json').read_text(encoding='utf-8'))
    pooled={}
    retained=[row for action in actions for row in action.get('scroll_trace',[])]+[row for frame in frames for row in frame.get('ui_tail',[])]
    if (root/'scroll.json').exists():retained+=json.loads((root/'scroll.json').read_text(encoding='utf-8'))
    for row in retained:
        identity=(tuple(row.get('session') or []),row.get('epoch'),row.get('revision'),row.get('frame'),row.get('pane'))
        if row.get('clock_ms') is not None and (identity not in pooled or pooled[identity]['clock_ms']<row['clock_ms']):pooled[identity]=row
    pooled=sorted(pooled.values(),key=lambda row:row['clock_ms'])
    samples=[]
    for action_index,action in enumerate(actions):
        if 'side' not in action:continue
        side=action['side'];driver=['source','preview'].index(side);follower=1-driver;t=action['at']
        keys=['main/source','main/preview'];names=['Source','Preview']
        before=action['before']['views'];after=action['after']['views']
        before_offset=[before[k]['presented_offset'][1] for k in keys]
        after_offset=[after[k]['presented_offset'][1] for k in keys]
        consumed=abs(after_offset[driver]-before_offset[driver])>.5
        prior=[f for f in frames if f['capture_finished']<=t]
        baseline=prior[-1] if prior else None
        observed=next((f for f in frames if baseline and f['capture_started']>=t and f['capture_started']<t+.32 and f['hashes'][follower]!=baseline['hashes'][follower]),None)
        until=actions[action_index+1]['at'] if action_index+1<len(actions) else t+1.
        tail=[row for row in pooled if t<=row['clock_ms']/1000<until]
        active=tail
        current=bool(active and all(r.get('current') for r in active))
        user=any(r['pane']==names[driver] and r.get('user') for r in active)
        valid=consumed and action.get('expected_body_input',True) and action.get('accepted_stable_input',True) and (env.get('baseline') or (current and user))
        ui=next((r for r in tail if r['pane']==names[follower] and r.get('clock_ms',0)/1000>=t and abs(r['offset']-before_offset[follower])>.5),None)
        native_rows=[r for r in tail if r['pane']==names[driver] and r.get('user')]
        latest_native=max(native_rows,key=lambda r:r['clock_ms'],default=None)
        last_native=latest_native['clock_ms']/1000 if latest_native else None
        latest_sequence=latest_native.get('sequence') if latest_native else None
        def same_intent(row):
            return latest_native and all(row.get(k)==latest_native.get(k) for k in ['session','epoch','revision']) and row.get('sequence')==latest_sequence
        confirmed=next((r for r in tail if same_intent(r) and r['pane']==names[driver] and r['clock_ms']/1000>=last_native and r.get('confirmed_sequence')==latest_sequence and r.get('anchor_sequence')==latest_sequence and r.get('pending_sequence')!=latest_sequence),None)
        driver_drawn=confirmed['clock_ms']/1000 if confirmed else None
        geometry=after[keys[follower]].get('geometry') or {};rect=geometry.get('rect')
        line_height=rect[3]-rect[1] if rect else None
        def current_drawn_target(row):
            return row.get('drawn_target') if 'drawn_target' in row else row.get('target')
        converged=next((r for r in tail if driver_drawn is not None and same_intent(r) and r['pane']==names[follower] and r['clock_ms']/1000>=driver_drawn and all(r.get(k)==latest_sequence for k in ['confirmed_sequence','anchor_sequence','target_sequence']) and r.get('pending_sequence')!=latest_sequence and current_drawn_target(r) is not None and abs(r['drawn_offset']-current_drawn_target(r))<=1.),None)
        if converged and converged.get('actual_line_height') is not None:line_height=converged['actual_line_height']
        ending=[frame for frame in frames if driver_drawn is not None and frame['capture_started']>=driver_drawn and frame['capture_finished']<until]
        streak=[]
        if ending:
            final_hash=ending[-1]['hashes'][follower]
            for frame in reversed(ending):
                if frame['hashes'][follower]!=final_hash:break
                streak.append(frame)
        desktop_converged=streak[-1] if len(streak)>=2 else None
        # Independent literal paragraph labels in actual painted text runs.
        labels=[]
        for key in keys:
            v=after[key];found={}
            for run in v.get('visible_runs',[]):
                for match in re.finditer(r'Paragraph (\d{6})',run.get('text','')):
                    found[int(match.group(1))]=run['rect'][1]-v['viewport_rect'][1]
            labels.append(found)
        shared=sorted(set(labels[0])&set(labels[1]))
        first=shared[0] if shared else None
        samples.append(dict(side=side,at=t,valid_input=valid,consumed=consumed,user_observed=user,before=before_offset,after=after_offset,
            desktop_ms=(observed['capture_finished']-t)*1000 if observed else None,
            desktop_frame=observed['frame'] if observed else None,
            ui_commands_ms=ui['clock_ms']-t*1000 if ui else None,
            convergence_after_last_native_ms=(converged['clock_ms']/1000-last_native)*1000 if converged else None,
            convergence_after_input_ms=converged['clock_ms']-t*1000 if converged else None,
            convergence_after_driver_drawn_ms=(converged['clock_ms']/1000-driver_drawn)*1000 if converged else None,
            desktop_convergence_after_last_native_ms=(desktop_converged['capture_finished']-last_native)*1000 if desktop_converged else None,
            last_native=last_native,latest_sequence=latest_sequence,confirmed_driver_drawn=driver_drawn,converged_clock_ms=converged['clock_ms'] if converged else None,observation_until=until,target_line_height_px=line_height,convergence_tolerance_px=1.,
            paragraph=first,paragraph_y=[x.get(first) for x in labels] if first is not None else None,
            paragraph_error_px=abs(labels[0][first]-labels[1][first]) if first is not None else None,
            mapping_current=current))
    result=dict(environment=env,capture_interval_ms=stats([(b['capture_finished']-a['capture_finished'])*1000 for a,b in zip(frames,frames[1:])]),directions={},samples=samples,
        summarizer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        ui_trace_observation='Original rings merged by session/epoch/revision/frame/pane, retaining last pass; next user action is the observation cutoff. This repairs async report snapshot truncation without inventing timestamps.',
        measurement='UI command production and independent desktop capture completion upper bounds; neither is physical panel latency. No timeout/invalid attempt removed from raw samples.')
    for side in ['source','preview']:
        group=[s for s in samples if s['side']==side];valid=[s for s in group if s['valid_input']]
        result['directions'][side]=dict(total=len(group),valid=len(valid),invalid=len(group)-len(valid),
            mapping_stale=sum(not s['mapping_current'] for s in group),no_user_record=sum(not s['user_observed'] for s in group),no_consumption=sum(not s['consumed'] for s in group),
            desktop_missing=sum(s['desktop_ms'] is None for s in valid),ui_missing=sum(s['ui_commands_ms'] is None for s in valid),
            desktop_ms=stats([s['desktop_ms'] for s in valid if s['desktop_ms'] is not None]),
            ui_commands_ms=stats([s['ui_commands_ms'] for s in valid if s['ui_commands_ms'] is not None]),
            convergence_ms=stats([s['convergence_after_last_native_ms'] for s in valid if s['convergence_after_last_native_ms'] is not None]),
            convergence_after_input_ms=stats([s['convergence_after_input_ms'] for s in valid if s['convergence_after_input_ms'] is not None]),
            convergence_after_driver_drawn_ms=stats([s['convergence_after_driver_drawn_ms'] for s in valid if s['convergence_after_driver_drawn_ms'] is not None]),
            convergence_missing=sum(s['convergence_after_last_native_ms'] is None for s in valid),
            desktop_convergence_ms=stats([s['desktop_convergence_after_last_native_ms'] for s in valid if s['desktop_convergence_after_last_native_ms'] is not None]),
            desktop_convergence_missing=sum(s['desktop_convergence_after_last_native_ms'] is None for s in valid),
            desktop_convergence_threshold_failures=sum(s['desktop_convergence_after_last_native_ms'] is not None and s['desktop_convergence_after_last_native_ms']>(250 if env['bytes']>=16777216 else 150) for s in valid),
            desktop_threshold_failures=sum(s['desktop_ms'] is not None and s['desktop_ms']>(100 if env['bytes']>=16777216 else 50) for s in valid),
            ui_threshold_failures=sum(s['ui_commands_ms'] is not None and s['ui_commands_ms']>(100 if env['bytes']>=16777216 else 50) for s in valid),
            convergence_threshold_failures=sum(s['convergence_after_last_native_ms'] is not None and s['convergence_after_last_native_ms']>(250 if env['bytes']>=16777216 else 150) for s in valid),
            convergence_after_input_threshold_failures=sum(s['convergence_after_input_ms'] is not None and s['convergence_after_input_ms']>(250 if env['bytes']>=16777216 else 150) for s in valid),
            paragraph_alignment_px=stats([s['paragraph_error_px'] for s in valid if s['paragraph_error_px'] is not None]),
            paragraph_alignment_missing=sum(s['paragraph_error_px'] is None for s in valid),
            paragraph_alignment_failures=sum(s['paragraph_error_px'] is not None and s['paragraph_error_px']>22.5 for s in valid))
    idle=json.loads((root/'idle.json').read_text(encoding='utf-8'))
    result['idle_application_frames']=[idle.get('performance_before',{}).get('frame'),idle.get('performance_after',{}).get('frame')]
    result['idle_repaint_causes']=[idle.get('performance_before',{}).get('repaint_causes'),idle.get('performance_after',{}).get('repaint_causes')]
    if isinstance(idle['before'],list) and isinstance(idle['after'],list) and idle['before'] and idle['after']:
        result['idle_frames']=[idle['before'][-1]['frame'],idle['after'][-1]['frame']]
        result['idle_drift']={name:abs(next(x for x in reversed(idle['after']) if x['pane']==name)['offset']-next(x for x in reversed(idle['before']) if x['pane']==name)['offset']) for name in ['Source','Preview']}
    (root/'summary.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps({k:v for k,v in result.items() if k!='samples'},ensure_ascii=False,indent=2))

if __name__=='__main__':main()
