// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Independent corpus adapter: calls the same pure source module as the app.
//! Never reads golden answers or chooses a candidate on the product's behalf.
#![allow(dead_code)]
#[path = "../../../src/reading_precision.rs"]
mod reading_precision;

use serde::Deserialize;
use std::io::{BufRead, Write};
use std::time::Instant;

#[derive(Deserialize)]
struct Case {
    case_id: String,
    old: String,
    new: String,
    old_byte: usize,
}

fn main() {
    let arguments = std::env::args().collect::<Vec<_>>();
    assert_eq!(arguments.len(), 3, "Use INPUTS.jsonl OUTPUT.jsonl");
    let inputs = std::io::BufReader::new(std::fs::File::open(&arguments[1]).unwrap());
    let mut output = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&arguments[2])
            .unwrap(),
    );
    let mut completed = 0;
    for line in inputs.lines() {
        let case: Case = serde_json::from_str(&line.unwrap()).unwrap();
        let started = Instant::now();
        let outcome = std::panic::catch_unwind(|| {
            let capture_start = Instant::now();
            let anchor = reading_precision::capture(&case.old, case.old_byte);
            let capture_ms = capture_start.elapsed().as_secs_f64() * 1000.;
            let resolve_start = Instant::now();
            let resolution = reading_precision::resolve(&case.new, &anchor);
            let resolve_ms = resolve_start.elapsed().as_secs_f64() * 1000.;
            serde_json::json!({
                "case_id": case.case_id, "anchor": anchor, "resolution": resolution,
                "capture_wall_ms": capture_ms, "resolve_wall_ms": resolve_ms,
                "status": "completed"
            })
        });
        let value = outcome.unwrap_or_else(|_| {
            serde_json::json!({
                "case_id": case.case_id, "status": "panic",
                "elapsed_wall_ms": started.elapsed().as_secs_f64() * 1000.
            })
        });
        serde_json::to_writer(&mut output, &value).unwrap();
        writeln!(&mut output).unwrap();
        output.flush().unwrap();
        completed += 1;
    }
    eprintln!("Independent corpus adapter completed {completed} cases");
}
