# SPDX-License-Identifier: AGPL-3.0-only
"""Compose paired measured sub-stages; keep modeled network time explicit."""
import json,statistics
from pathlib import Path
root=Path(__file__).resolve().parent.parent;e=root/'evidence/release-015'
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def median(values):return statistics.median(values)
result={'method':'Pair trial indexes across separately measured install/extract and native GUI runs, then take median of complete per-trial sums. Network modeled separately; human deliberation and browser download clicks not timed. Warm OS cache and new app profile each trial.', 'platforms':{}}
for platform in ['windows','linux']:
    gui=read(e/('final-windows-gui-r3' if platform=='windows' else 'final-linux-gui')/'measurements.json')
    if platform=='windows':
        baseline=read(root/'artifacts/release-015/baseline-windows-final/baseline-measurements.json')
        install=read(e/'final-windows-protected-install/measurements.json')
        installer_bytes=install['installerBytes']
    else:
        baseline=read(e/'final-linux-install/baseline-linux.json');install=read(e/'final-linux-protected-install/installer-linux.json')
        installer_bytes=(root/'artifacts/release-015/final-packages/mdcard_0.1.5_amd64.deb').stat().st_size
    rows=[]
    for kind in ['baseline','installed']:
        for trial in range(1,4):
            g=next(x for x in gui if x['kind']==kind and x['trial']==trial)
            prepare=baseline['extractionSeconds'][trial-1] if kind=='baseline' else install['trials'][trial-1]['seconds']
            a=prepare+g['scenarioAProcessToSavedSeconds'];b=a+g['scenarioBAdditionalSeconds']
            rows.append(dict(kind=kind,trial=trial,preparationSeconds=prepare,startupSeconds=g['startupSeconds'],guiASeconds=g['scenarioAProcessToSavedSeconds'],guiBAdditionalSeconds=g['scenarioBAdditionalSeconds'],AWithoutNetworkSeconds=a,BWithoutNetworkSeconds=b))
    aggregates={k:{name:median([r[name] for r in rows if r['kind']==k]) for name in ['preparationSeconds','startupSeconds','guiASeconds','guiBAdditionalSeconds','AWithoutNetworkSeconds','BWithoutNetworkSeconds']} for k in ['baseline','installed']}
    saved=baseline['bytes']-installer_bytes;scenarios=[]
    for mbps in [20,50,100,500]:
        item={'fixedBandwidthMbps':mbps,'baselineDownloadEstimateSeconds':baseline['bytes']*8/(mbps*1e6),'installerDownloadEstimateSeconds':installer_bytes*8/(mbps*1e6)}
        for scene in ['A','B']:
            field=scene+'WithoutNetworkSeconds';item[scene+'ModeledSavingSeconds']=saved*8/(mbps*1e6)+aggregates['baseline'][field]-aggregates['installed'][field]
        scenarios.append(item)
    break_even={}
    for scene in ['A','B']:
        field=scene+'WithoutNetworkSeconds';penalty=aggregates['installed'][field]-aggregates['baseline'][field]
        break_even[scene]=saved*8/(penalty*1e6) if penalty>0 else None
    result['platforms'][platform]=dict(baselineBytes=baseline['bytes'],installerBytes=installer_bytes,downloadSavedBytes=saved,additionalApplicationDownloadsBytes=0,trials=rows,medians=aggregates,modeledNetworkScenarios=scenarios,breakEvenMbps=break_even)
(e/'final-cost-model.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(result,ensure_ascii=False,indent=2))
