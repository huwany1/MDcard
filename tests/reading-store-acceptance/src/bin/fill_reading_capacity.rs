// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Public API setup for capacity GUI QA; this is not an application input hook.
use reading_store_acceptance::{Document, ReadingState};
use std::{fs, path::PathBuf};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(args.next().expect("new fixture directory"));
    let protected = PathBuf::from(args.next().expect("existing protected document"));
    assert!(!directory.exists(), "Use a fresh setup directory");
    fs::create_dir_all(&directory).unwrap();
    let state_path = PathBuf::from(std::env::var_os("MDCARD_READING_STATE_PATH").unwrap());
    {
        let mut state = ReadingState::persistent();
        let mut keep = Document::open(&protected).unwrap();
        state.bind(&mut keep, "secondary", 1);
        for index in 0..512 {
            let file = directory.join(format!("capacity-{index:04}.md"));
            fs::write(
                &file,
                format!("# Independent capacity {index}\n\nBody.\n\n").repeat(40),
            )
            .unwrap();
            let mut doc = Document::open(&file).unwrap();
            state.bind(&mut doc, "main", index + 1);
            state.observe(
                &doc,
                "main/preview",
                "capacity".into(),
                [0., 450.],
                [800., 600.],
                100,
                0.,
                0.,
                true,
            );
        }
        state.flush();
    }
    let bytes = fs::read(&state_path).unwrap();
    let disk: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(disk["records"].as_object().unwrap().len() <= 512);
    assert!(bytes.len() <= 4 * 1024 * 1024);
    println!(
        "{}",
        serde_json::json!({"passed":true,"actual_public_api_opens":512,"records":disk["records"].as_object().unwrap().len(),"bytes":bytes.len(),"clear_serial":disk["clear_serial"],"latest_clear_ns":disk["latest_clear_ns"],"method":"CLI-only capacity setup through the real persistent public API. No GUI or input claim."})
    );
}
