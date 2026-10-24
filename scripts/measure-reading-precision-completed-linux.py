# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Completed pure-source stages on one dedicated background thread, not GUI timing."""
import hashlib,json,os,subprocess
from pathlib import Path
root=Path('/work');out=root/'artifacts/reading-precision/completed-stages-bench';out.mkdir(exist_ok=True)
(out/'src').mkdir(exist_ok=True)
(out/'Cargo.toml').write_text('''[package]
name="precision-completed-stages-bench"
version="0.0.0"
edition="2024"
[dependencies]
serde={version="1",features=["derive"]}
serde_json="1"
pulldown-cmark="=0.13.4"
unicode-segmentation="=1.13.3"
''')
(out/'src/main.rs').write_text(r'''
#[path="/work/src/reading_precision.rs"] mod reading_precision;
use std::{time::Instant,fs};
use serde_json::json;
fn summary(v:&[f64])->serde_json::Value { let mut a=v.to_vec();a.sort_by(f64::total_cmp); json!({"n":a.len(),"p50_ms":a[(a.len()-1)/2],"p95_ms":a[((a.len() as f64*0.95).ceil() as usize).saturating_sub(1)],"max_ms":a[a.len()-1],"raw_wall_ms":v}) }
fn main(){
 let handle=std::thread::Builder::new().name("completed-precision-benchmark".into()).spawn(||{
  let mut results=Vec::new();
  for file in ["ordinary.md","one-mib.md","ten-mib.md"] {
   let old=fs::read_to_string(format!("/work/artifacts/reading-precision/performance-inputs/{file}")).unwrap();
   let mut middle=old.len()/2;while !old.is_char_boundary(middle){middle-=1}let target=old[middle..].find("中文").map(|x|middle+x).unwrap_or_else(|| {let mut b=old.len()/2;while !old.is_char_boundary(b){b-=1}b});
   let mut index=Vec::new(); for _ in 0..10 {let t=Instant::now();std::hint::black_box(reading_precision::fingerprint(&old));index.push(t.elapsed().as_secs_f64()*1000.);}
   let fp=reading_precision::fingerprint(&old);
   let mut capture=Vec::new();for _ in 0..100 {let t=Instant::now();std::hint::black_box(reading_precision::capture_with_fingerprint(&old,target,fp));capture.push(t.elapsed().as_secs_f64()*1000.);}
   let mut enrichment=Vec::new();let mut anchor=None;for _ in 0..10 {let t=Instant::now();let a=reading_precision::enrich_with_fingerprint(&old,target,fp);enrichment.push(t.elapsed().as_secs_f64()*1000.);assert!(a.proof.is_some());anchor=Some(a);}
   let a=anchor.unwrap(); let prefix="An independently inserted introduction with entirely new information. ".repeat(old.len()/140+1)+"\n\n";
   let changed=prefix.clone()+&old;let expected=target+prefix.len();
   let mut matching=Vec::new();let mut decisions=Vec::new();for _ in 0..10 {let t=Instant::now();let r=reading_precision::resolve(&changed,&a);matching.push(t.elapsed().as_secs_f64()*1000.);assert!(r.exact,"{}",r.evidence);assert_eq!(r.source_byte,expected);decisions.push(json!({"source_byte":r.source_byte,"exact":r.exact,"tier":r.tier}));}
   results.push(json!({"file":file,"old_bytes":old.len(),"new_bytes":changed.len(),"source_byte":target,"expected_byte":expected,"proof_bytes":a.estimated_proof_bytes(),"fingerprint":summary(&index),"bounded_capture":summary(&capture),"completed_anchor_enrichment":summary(&enrichment),"completed_candidate_matching":summary(&matching),"decisions":decisions}));
  }results
 }).unwrap();
 println!("{}",serde_json::to_string_pretty(&json!({"results":handle.join().unwrap(),"method":"Sequential completed operations on one named background thread with current actual pure source module. 10 cold-new-call fingerprint/enrichment/matching samples per size, 100 bounded captures, no canceled sample. Fingerprinting separate; snapshots, file IO and output outside measured call. Function wall Instant elapsed, not thread CPU, GUI layout, input/display latency or FPS."})).unwrap());
}
''')
subprocess.run(['cargo','build','--release','--offline','--manifest-path',str(out/'Cargo.toml'),'--target-dir','/work/target-completed-stages-bench'],check=True)
manifest={'api_sha256':hashlib.sha256((root/'src/reading_precision.rs').read_bytes()).hexdigest(),'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'inputs_sha256':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in (root/'artifacts/reading-precision/performance-inputs').glob('*.md')},'binary_sha256':hashlib.sha256((root/'target-completed-stages-bench/release/precision-completed-stages-bench').read_bytes()).hexdigest()}
(out/'build-manifest.json').write_text(json.dumps(manifest,indent=2));print(json.dumps(manifest,indent=2))
if os.environ.get('MDCARD_BENCH_RUN'):
 result=subprocess.run([str(root/'target-completed-stages-bench/release/precision-completed-stages-bench')],check=True,capture_output=True,text=True);(root/'evidence/reading-precision/implementation'/os.environ['MDCARD_BENCH_RUN']).mkdir(exist_ok=True);(root/'evidence/reading-precision/implementation'/os.environ['MDCARD_BENCH_RUN']/'result.json').write_text(result.stdout)
