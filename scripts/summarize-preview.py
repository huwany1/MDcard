# SPDX-License-Identifier: AGPL-3.0-only
"""Summarize native preview observations without treating spinner motion as content."""
import argparse
import json
import math
from pathlib import Path

def quantiles(values):
    values=sorted(v for v in values if v is not None)
    def p(q):
        if not values:return None
        return values[max(0,math.ceil(len(values)*q)-1)]
    return dict(n=len(values),p50_ms=p(.5),p95_ms=p(.95),max_ms=p(1))

def summarize(path):
    environment=json.loads((path/'environment.json').read_text(encoding='utf-8'))
    actions=json.loads((path/'actions.json').read_text(encoding='utf-8'))
    frames=json.loads((path/'frames.json').read_text(encoding='utf-8'))
    samples=[json.loads(l) for l in (path/'samples.jsonl').read_text(encoding='utf-8').splitlines()]
    local=[a for a in actions if a['kind']=='local-edit']
    rows=[]
    for i,action in enumerate(local):
        at=action['at']; end=local[i+1]['at'] if i+1<len(local) else at+environment.get('latency_interval',.75)
        previous=[s for s in samples if s['t']<at]
        prev=previous[-1] if previous else {}
        old_stamp=prev.get('rich',{}).get('sourceStamp')
        old_revision=prev.get('ui',{}).get('revision',-1)
        expected=action.get('expected_revision',old_revision+1)
        during=[s for s in samples if at<=s['t']<end]
        input_seen=next((s for s in during if s.get('ui',{}).get('revision')==expected),None)
        def is_latest(sample):
            rich=sample.get('rich',{})
            mapped=rich.get('mappedRevision')
            return sample.get('ui',{}).get('revision')==expected and (
                (mapped is not None and mapped[1]==expected and rich.get('displayedRevision')==expected)
                or ('mappedRevision' not in rich and rich.get('sourceStamp') not in (None,old_stamp)))
        parsed_seen=next((s for s in during if is_latest(s)),None)
        possible=[f for f in frames if f['capture_started']>=at and f['capture_finished']<end-.06]
        screen=None; settled=None
        if len(possible)>=3 and parsed_seen:
            settled=possible[-1]
            associated=[s for s in during if s['t']<=settled['capture_started']]
            proven=bool(associated and is_latest(associated[-1]))
            stable=all(f['preview_pixel_sha256']==settled['preview_pixel_sha256'] for f in possible[-3:])
            before=[f for f in frames if f['capture_finished']<at]
            if proven and stable and before and settled['preview_pixel_sha256']!=before[-1]['preview_pixel_sha256']:
                screen=next((f for f in possible if f['preview_pixel_sha256']==settled['preview_pixel_sha256']
                    and f['preview_ink_pixels']>500),None)
        timings=next((r for s in during for r in s.get('rich',{}).get('previewTimings',[])
            if r.get('revision')==expected and r.get('current') and r.get('ui_draw_clock_ms',0)>=at*1000),None)
        rows.append(dict(index=i,action_at=at,expected_revision=expected,
            source_response_report_upper_ms=(input_seen['t']-at)*1000 if input_seen else None,
            parser_commit_report_upper_ms=(parsed_seen['t']-at)*1000 if parsed_seen else None,
            ui_commands_ms=timings['ui_draw_clock_ms']-at*1000 if timings else None,
            screen_observed_upper_ms=(screen['capture_finished']-at)*1000 if screen else None,
            first_latest_frame=screen['frame'] if screen else None,
            settled_frame=settled['frame'] if settled else None,
            missing_screen=screen is None,censor_window_ms=(end-at)*1000,
            latest_source_stamp=parsed_seen.get('rich',{}).get('sourceStamp') if parsed_seen else None,
            stages=timings))
    all_edits=[a for a in actions if a['kind']=='paste']
    continuity={}
    if all_edits:
        first=all_edits[0]['at'];last=all_edits[-1]['at']
        span=[f for f in frames if first+.5<=f['capture_started']<=last and 'preview_ink_pixels' in f]
        commits=[]; previous=None
        for sample in samples:
            if not first<=sample['t']<=last:continue
            stamp=sample.get('rich',{}).get('sourceStamp')
            if stamp is not None and stamp!=previous:
                commits.append(sample['t']);previous=stamp
        gaps=[(b-a)*1000 for a,b in zip(commits,commits[1:])]
        continuity=dict(continuous_action_span_seconds=last-first,screen_frames=len(span),
            blank_or_spinner_only_frames=sum(f['preview_ink_pixels']<500 for f in span),
            readable_frame_fraction=sum(f['preview_ink_pixels']>=500 for f in span)/max(1,len(span)),
            observed_parser_commits=len(commits),commit_gap_ms=quantiles(gaps),
            note='Ink threshold is supporting evidence; review video and sampled frames. Wall time uses capture/action timestamps, not nominal video duration.')
    performance=json.loads((path/'performance.json').read_text(encoding='utf-8'))
    rich=json.loads((path/'rich.json').read_text(encoding='utf-8'))
    ui=json.loads((path/'ui.json').read_text(encoding='utf-8'))
    source_gl=[]
    if (environment.get('measurement_bytes') or 0)>=64*1024:
        frequency=performance.get('paint',{}).get('qpc_frequency',0)
        completions=performance.get('paint',{}).get('input_completions',[])
        offset=ui.get('revision',0)-performance.get('revision',0)
        if frequency:
            for row in rows:
                match=next((c for c in completions if c['revision']==row['expected_revision']-offset
                    and c['qpc']/frequency>=row['action_at']),None)
                if match:
                    value=(match['qpc']/frequency-row['action_at'])*1000
                    row['source_gl_callback_ms']=value;source_gl.append(value)
    reading=json.loads((path/'reading-report.json').read_text(encoding='utf-8')) if (path/'reading-report.json').exists() else {}
    requested=environment.get('requested_local_samples',len(local))
    completed=sum(r['screen_observed_upper_ms'] is not None for r in rows)
    result=dict(environment=environment,samples=rows,
        requested_local_samples=requested,attempted_local_samples=len(local),completed_screen_samples=completed,
        missing_or_censored_screen_samples=requested-completed,
        source_response_report_upper=quantiles([r['source_response_report_upper_ms'] for r in rows]),
        parser_commit_report_upper=quantiles([r['parser_commit_report_upper_ms'] for r in rows]),
        ui_commands=quantiles([r['ui_commands_ms'] for r in rows]),
        screen_observed_upper=quantiles([r['screen_observed_upper_ms'] for r in rows]),
        input_update_wall=quantiles(performance.get('input_event_to_update_finish_ms',[])),
        input_gl_callback=quantiles(performance.get('paint',{}).get('input_to_complete_ms',[])),
        source_action_to_gl_callback=quantiles(source_gl),
        block_layout_wall={key:quantiles(values) for key,values in reading.get('metrics',{}).get('layout_wall_ms',{}).items()},
        continuity=continuity,media=rich.get('mediaTimings',[]),
        limitations=['Screen measure is desktop capture observation, not physical panel presentation.',
            'First latest frame must exactly match the settled preview pixels for that isolated edit; spinner motion never qualifies.',
            'Diagnostic publication and 20fps target sampling add overhead and observation delay.',
            'Missing/censored observations are reported separately and prevent declaring the full requested series passed; quantiles describe completed observations only.'])
    result['screen_target_500ms_pass']=(bool(local) and result['missing_or_censored_screen_samples']==0
        and result['screen_observed_upper']['p95_ms']<=500)
    (path/'summary.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    return {k:v for k,v in result.items() if k in ['source_response_report_upper','parser_commit_report_upper','ui_commands','screen_observed_upper','continuity']}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('paths',nargs='+');args=p.parse_args()
    for name in args.paths:
        try:print(name,json.dumps(summarize(Path(name)),ensure_ascii=False))
        except (OSError,ValueError) as error:print(name,str(error))
