#![allow(dead_code)]
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
extern crate self as eframe;
pub use egui;
#[path = "../../../src/document.rs"]
mod document;
#[path = "../../../src/encoding.rs"]
mod encoding;
#[path = "../../../src/localization.rs"]
mod localization;
#[path = "../../../src/reading_geometry.rs"]
mod reading_geometry;
#[path = "../../../src/reading_precision.rs"]
mod reading_precision;
#[path = "../../../src/reading_state.rs"]
mod reading_state;
#[path = "../../../src/rich_nodes.rs"]
mod rich_nodes;
#[path = "../../../src/text_files.rs"]
mod text_files;
pub use document::Document;
pub use reading_state::ReadingState;

#[cfg(test)]
mod store_contract {
    use super::{document::Document, reading_state::ReadingState};
    use serde_json::Value;
    use std::{
        fs,
        path::Path,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    static ENVIRONMENT: Mutex<()> = Mutex::new(());

    struct StateEnvironment(Option<std::ffi::OsString>);
    impl StateEnvironment {
        fn set(path: &Path) -> Self {
            let old = std::env::var_os("MDCARD_READING_STATE_PATH");
            // This test filter runs only the two serialized store contract tests.
            unsafe {
                std::env::set_var("MDCARD_READING_STATE_PATH", path);
            }
            Self(old)
        }
    }
    impl Drop for StateEnvironment {
        fn drop(&mut self) {
            unsafe {
                if let Some(old) = &self.0 {
                    std::env::set_var("MDCARD_READING_STATE_PATH", old);
                } else {
                    std::env::remove_var("MDCARD_READING_STATE_PATH");
                }
            }
        }
    }

    fn future_record_clear(all: bool, same_instance: bool) {
        let _serial = ENVIRONMENT.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reading.json");
        let file = temp.path().join("document.md");
        let source = "# Ordinary file\n\n中文🙂 independent reading paragraph.\n\n".repeat(30);
        fs::write(&file, &source).unwrap();
        let _environment = StateEnvironment::set(&path);
        {
            let mut doc = Document::open(&file).unwrap();
            let mut state = ReadingState::persistent();
            state.bind(&mut doc, "main", 1);
            state.observe(
                &doc,
                "main/preview",
                "unchanged".into(),
                [0., 800.],
                [800., 600.],
                100,
                0.,
                0.,
                true,
            );
            state.flush();
        }
        let mut disk: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000;
        for record in disk["records"].as_object_mut().unwrap().values_mut() {
            record["updated_ms"] = future.into();
            record["activity_ms"] = future.into();
            for position in record["views"].as_object_mut().unwrap().values_mut() {
                position["updated_ms"] = future.into();
            }
        }
        fs::write(&path, serde_json::to_vec(&disk).unwrap()).unwrap();
        {
            let mut doc = Document::open(&file).unwrap();
            let mut state = ReadingState::persistent();
            state.bind(&mut doc, "main", 1);
            let report = state.report(&doc, None);
            assert!(
                report["writes"]["error"].is_null(),
                "+1h is an accepted clock fixture"
            );
            assert!(!report["views"]["main/preview"]["pending"].is_null());
            state.clear(if all { None } else { Some(doc.id) });
            assert!(
                state.report(&doc, None)["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["views"].as_object().unwrap().is_empty())
            );
            if same_instance {
                let ctx = egui::Context::default();
                ctx.begin_pass(egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(egui::pos2(100., 100.)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0., -80.),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..egui::RawInput::default()
                });
                state.plan(
                    &doc,
                    "main/preview",
                    1,
                    "unchanged",
                    &ctx,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
                    false,
                );
                state.observe(
                    &doc,
                    "main/preview",
                    "unchanged".into(),
                    [0., 450.],
                    [800., 600.],
                    200,
                    0.,
                    0.,
                    true,
                );
                let _ = ctx.end_pass();
                state.flush();
            }
        }
        let mut doc = Document::open(&file).unwrap();
        let mut reopened = ReadingState::persistent();
        reopened.bind(&mut doc, "main", 1);
        let report = reopened.report(&doc, None);
        if same_instance {
            assert_eq!(
                report["views"]["main/preview"]["pending"]["position"]["offset"][1], 450.,
                "The same instance must persist genuine reading after its own clear"
            );
        } else {
            assert!(
                report["views"]["main/preview"]["pending"].is_null(),
                "An explicit {} clear must beat the accepted disk snapshot observed in its transaction",
                if all { "global" } else { "single-document" }
            );
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), source);
    }

    #[test]
    fn store_contract_single_clear_covers_valid_future_record() {
        future_record_clear(false, false);
    }

    #[test]
    fn store_contract_all_clear_covers_valid_future_record() {
        future_record_clear(true, false);
    }
    #[test]
    fn store_contract_same_instance_can_persist_new_reading_after_future_clear() {
        future_record_clear(false, true);
        future_record_clear(true, true);
    }

    #[test]
    #[ignore = "Explicit serial capacity/clear interleaving through the public API"]
    fn store_contract_single_clear_survives_tombstone_capacity_eviction() {
        let _serial = ENVIRONMENT.lock().unwrap_or_else(|e| e.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reading.json");
        let file = temp.path().join("original.md");
        let source = "# Original\n\nAn independent paragraph 中文🙂.\n\n".repeat(30);
        fs::write(&file, &source).unwrap();
        let key = super::reading_state::path_key(&file);
        let _environment = StateEnvironment::set(&path);
        // Keep an older instance alive. Its original real activity precedes
        // the other instance's explicit clear; subsequent mode metadata is not
        // new reading activity.
        let mut original = Document::open(&file).unwrap();
        let mut old = ReadingState::persistent();
        old.bind(&mut original, "main", 1);
        old.observe(
            &original,
            "main/preview",
            "same".into(),
            [0., 800.],
            [800., 600.],
            100,
            0.,
            0.,
            true,
        );
        old.flush();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if fs::read(&path)
                .ok()
                .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
                .is_some_and(|d| d["records"][&key]["views"]["main/preview"]["offset"][1] == 800.)
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        {
            let mut doc = Document::open(&file).unwrap();
            let mut clearer = ReadingState::persistent();
            clearer.bind(&mut doc, "main", 1);
            clearer.clear(Some(doc.id));
        }
        let cleared: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(
            cleared["records"][&key]["views"]
                .as_object()
                .unwrap()
                .is_empty()
        );
        assert!(cleared["clear_serial"].as_u64().unwrap() > 0);
        {
            let mut filler = ReadingState::persistent();
            for i in 0..512 {
                let other = temp.path().join(format!("other-{i:04}.md"));
                fs::write(&other, format!("# Distinct file {i}\n\n{source}")).unwrap();
                let mut doc = Document::open(&other).unwrap();
                filler.bind(&mut doc, "main", i + 1);
                filler.observe(
                    &doc,
                    "main/preview",
                    "same".into(),
                    [0., 450.],
                    [800., 600.],
                    100,
                    0.,
                    0.,
                    true,
                );
            }
            filler.flush();
        }
        let evicted: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(evicted["records"].as_object().unwrap().len() <= 512);
        assert!(
            evicted["records"].get(&key).is_none(),
            "Exercise actual tombstone eviction"
        );
        old.set_mode(&original, "Split");
        old.flush();
        drop(old);
        let merged: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(
            merged["records"]
                .get(&key)
                .is_none_or(|record| { record["views"].as_object().unwrap().is_empty() }),
            "A stale metadata-only snapshot cannot revive a single-cleared record after capacity eviction"
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), source);
    }
}
