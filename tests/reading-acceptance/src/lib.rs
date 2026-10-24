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

#[cfg(test)]
mod independent {
    use super::{document::Document, egui, reading_state::*};

    #[test]
    fn independent_cached_clean_display_cannot_skip_save_bytes_or_external_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cached-clean.txt");
        let original = "原稿🙂\r\n";
        let changed = "修改🙂\r\n";
        assert_eq!(original.len(), changed.len());
        std::fs::write(&path, original.as_bytes()).unwrap();
        let mut document = Document::open(&path).unwrap();
        assert!(!document.presentation_dirty());
        let revision = document.revision();

        // Deliberately bypass the display invalidation protocol. Authoritative
        // save protection must still inspect the real source and disk baseline.
        document.text = changed.into();
        assert_eq!(document.revision(), revision);
        assert!(document.dirty());
        document.save_to(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), changed.as_bytes());
        assert!(!document.presentation_dirty());
        assert!(!document.dirty());

        document.text = original.into();
        std::fs::write(&path, b"external change\n").unwrap();
        assert!(document.dirty());
        assert!(document.save_to(&path).is_err());
        assert_eq!(document.text, original);
        assert_eq!(std::fs::read(&path).unwrap(), b"external change\n");
        document.mark_text_changed();
        assert!(document.presentation_dirty());
        assert!(document.dirty());
    }

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0., 0.), egui::vec2(800., 600.))
    }

    fn observe(state: &mut ReadingState, doc: &Document, key: &str, byte: usize, y: f32) {
        state.observe(
            doc,
            key,
            "layout-a".into(),
            [17., y],
            [800., 600.],
            byte,
            0.25,
            -4.,
            true,
        );
    }

    #[test]
    fn independent_utf8_boundaries_match_standard_library_golden_coordinates() {
        let source = "A中🙂e\u{301}\r\n🦀尾";
        let mut golden = source.char_indices().map(|(b, _)| b).collect::<Vec<_>>();
        golden.push(source.len());
        for (character, byte) in golden.iter().enumerate() {
            assert_eq!(char_to_byte(source, character), *byte);
            assert_eq!(byte_to_char(source, *byte), character);
        }
        for byte in 0..=source.len() + 8 {
            let expected = *golden.iter().filter(|v| **v <= byte).last().unwrap();
            assert_eq!(byte_floor(source, byte), expected);
        }
        assert_eq!(char_to_byte(source, usize::MAX), source.len());
    }

    #[test]
    fn independent_last_position_is_separate_from_furthest_and_read_status() {
        let mut doc = Document::from_text(&"中文🙂 reading ".repeat(160));
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        observe(&mut state, &doc, "main/preview", 2000, 800.);
        observe(&mut state, &doc, "main/preview", 100, 40.);
        let report = state.report(&doc, None);
        let record = &report["records"][0];
        assert_eq!(record["views"]["main/preview"]["offset"][1], 40.);
        assert!(record["furthest"].as_f64().unwrap() > 0.4);
        assert_eq!(record["read"], false);
    }

    #[test]
    fn independent_document_id_canonical_path_and_draft_isolation() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        let a = dir.path().join("same.md");
        let b = dir.path().join("nested/same.md");
        std::fs::write(&a, "same source").unwrap();
        std::fs::write(&b, "same source").unwrap();
        let mut state = ReadingState::default();
        let mut first = Document::open(&a).unwrap();
        state.bind(&mut first, "main", 1);
        let relative = a.strip_prefix(std::env::current_dir().unwrap()).unwrap();
        assert_eq!(path_key(relative), path_key(&a));
        let mut reopened = Document::open(relative).unwrap();
        state.bind(&mut reopened, "main", 99);
        assert_eq!(first.id, reopened.id);
        let mut other = Document::open(&b).unwrap();
        state.bind(&mut other, "main", 100);
        assert_ne!(first.id, other.id);
        let mut draft_a = Document::default();
        let mut draft_b = Document::default();
        state.bind(&mut draft_a, "main", 101);
        state.bind(&mut draft_b, "main", 102);
        assert_ne!(draft_a.id, draft_b.id);
    }

    #[test]
    fn independent_view_and_selection_records_do_not_replace_each_other() {
        let mut doc = Document::from_text(&"ab中文🙂\n".repeat(100));
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        observe(&mut state, &doc, "main/source", 100, 120.);
        observe(&mut state, &doc, "main/preview", 500, 700.);
        state.store_selection(&doc, "main/source", [0, 4]);
        state.bind(&mut doc, "main", 2);
        assert_eq!(state.selection(&doc, "main/source"), Some([0, 4]));
        let ctx = egui::Context::default();
        let source = state
            .plan(&doc, "main/source", 2, "layout-a", &ctx, viewport(), false)
            .unwrap();
        let preview = state
            .plan(&doc, "main/preview", 2, "layout-a", &ctx, viewport(), false)
            .unwrap();
        assert_eq!(source.position.offset, [17., 120.]);
        assert_eq!(preview.position.offset, [17., 700.]);
        assert!(source.exact && preview.exact);
    }

    #[test]
    fn independent_open_generation_rejects_late_restoration() {
        let mut doc = Document::from_text("source paragraph");
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        observe(&mut state, &doc, "main/preview", 0, 120.);
        state.bind(&mut doc, "main", 3);
        let ctx = egui::Context::default();
        assert!(
            state
                .plan(&doc, "main/preview", 2, "layout-a", &ctx, viewport(), false)
                .is_none()
        );
        assert!(
            state
                .plan(&doc, "main/preview", 3, "layout-a", &ctx, viewport(), false)
                .is_some()
        );
    }

    #[test]
    fn independent_layout_change_preserves_content_anchor_and_navigation_cancels_restore() {
        let mut doc = Document::from_text("# 标题\n\nparagraph 中文🙂\n");
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        let byte = doc.text.find("paragraph").unwrap();
        observe(&mut state, &doc, "main/preview", byte, 900.);
        let ctx = egui::Context::default();
        let restore = state
            .plan(&doc, "main/preview", 1, "layout-b", &ctx, viewport(), false)
            .unwrap();
        assert_eq!(restore.position.anchor.byte, byte);
        assert_eq!(restore.tier, "layout-anchor");
        assert!(!restore.exact);
        assert!(
            state
                .plan(&doc, "main/preview", 1, "layout-b", &ctx, viewport(), true)
                .is_none()
        );
        assert_eq!(
            state.report(&doc, None)["views"]["main/preview"]["reason"],
            "navigation"
        );
    }

    #[test]
    fn independent_user_wheel_cancels_pending_automatic_restoration() {
        let mut doc = Document::from_text("source paragraph");
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        observe(&mut state, &doc, "main/preview", 0, 120.);
        state.bind(&mut doc, "main", 2);
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(viewport()),
            events: vec![
                egui::Event::PointerMoved(egui::pos2(50., 50.)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -120.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        });
        assert!(
            state
                .plan(&doc, "main/preview", 2, "layout-a", &ctx, viewport(), false)
                .is_none()
        );
        assert_eq!(
            state.report(&doc, None)["views"]["main/preview"]["reason"],
            "user-scroll"
        );
        let _ = ctx.end_pass();
    }

    #[test]
    fn independent_content_insertion_relocates_unique_context_and_empty_is_safe() {
        let mut doc = Document::from_text("# 标题\n\nfirst\n\nunique paragraph 中文🙂\n\nlast\n");
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        let original_byte = doc.text.find("unique").unwrap();
        observe(&mut state, &doc, "main/preview", original_byte, 900.);
        let prefix = "新加前言🙂\n\n";
        doc.text.insert_str(0, prefix);
        doc.mark_text_changed();
        state.bind(&mut doc, "main", 2);
        let ctx = egui::Context::default();
        let restore = state
            .plan(&doc, "main/preview", 2, "layout-a", &ctx, viewport(), false)
            .unwrap();
        assert_eq!(restore.position.anchor.byte, original_byte + prefix.len());
        assert_eq!(restore.tier, "context");
        assert!(!restore.exact);
        doc.text.clear();
        doc.mark_text_changed();
        state.bind(&mut doc, "main", 3);
        let empty = state
            .plan(&doc, "main/preview", 3, "layout-a", &ctx, viewport(), false)
            .unwrap();
        assert_eq!(empty.position.anchor.byte, 0);
        assert_eq!(empty.tier, "empty");
    }

    #[test]
    fn independent_ambiguous_repeated_context_falls_back_instead_of_arbitrary_match() {
        let repeated = "重复🙂abcdefghijklmnopqrstuvwxyz ".repeat(40);
        let text = format!("# Duplicate\n\n{repeated}\n\n{repeated}\n\n{repeated}");
        let mut doc = Document::from_text(&text);
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        observe(
            &mut state,
            &doc,
            "main/preview",
            byte_floor(&doc.text, 500),
            900.,
        );
        doc.text.insert_str(0, "Prefix\n\n");
        doc.mark_text_changed();
        state.bind(&mut doc, "main", 2);
        let ctx = egui::Context::default();
        let restore = state
            .plan(&doc, "main/preview", 2, "layout-a", &ctx, viewport(), false)
            .unwrap();
        assert!(
            matches!(restore.tier.as_str(), "nearby-heading" | "approximate"),
            "Ambiguity was not reported: {}",
            restore.tier
        );
        assert!(!restore.exact);
        assert!(doc.text.is_char_boundary(restore.position.anchor.byte));
    }

    #[test]
    fn independent_save_as_copies_main_history_and_retains_other_pane_identity() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.md");
        let copied = dir.path().join("copied.md");
        std::fs::write(&original, "# Source\n\nbody 中文🙂\n").unwrap();
        let mut doc = Document::open(&original).unwrap();
        let mut secondary = doc.clone();
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        state.bind(&mut secondary, "secondary", 5);
        observe(&mut state, &doc, "main/preview", 10, 400.);
        observe(&mut state, &secondary, "secondary/source", 15, 700.);
        let original_id = doc.id;
        doc.save_to(&copied).unwrap();
        state.copied_path(&mut doc, original_id, "main");
        assert_ne!(doc.id, original_id);
        assert_eq!(secondary.id, original_id);
        let report = state.report(&doc, Some(&secondary));
        assert_eq!(
            report["views"]["secondary/source"]["document_id"],
            serde_json::to_value(original_id).unwrap()
        );
        assert_eq!(
            report["views"]["main/preview"]["document_id"],
            serde_json::to_value(doc.id).unwrap()
        );
        let records = report["records"].as_array().unwrap();
        let original_record = records
            .iter()
            .find(|r| r["id"] == serde_json::to_value(original_id).unwrap())
            .unwrap();
        let copied_record = records
            .iter()
            .find(|r| r["id"] == serde_json::to_value(doc.id).unwrap())
            .unwrap();
        assert_eq!(original_record["path"], path_key(&original));
        assert_eq!(copied_record["path"], path_key(&copied));
        assert_eq!(
            original_record["views"]["main/preview"]["offset"],
            copied_record["views"]["main/preview"]["offset"]
        );
        assert_eq!(
            std::fs::read(original).unwrap(),
            std::fs::read(copied).unwrap()
        );
    }

    #[test]
    fn independent_internal_rename_preserves_identity_and_rejects_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.md");
        let existing = dir.path().join("existing.md");
        std::fs::write(&original, "原文🙂").unwrap();
        std::fs::write(&existing, "PROTECTED").unwrap();
        let mut doc = Document::open(&original).unwrap();
        let mut state = ReadingState::default();
        state.bind(&mut doc, "main", 1);
        let id = doc.id;
        observe(&mut state, &doc, "main/preview", 0, 400.);
        assert!(doc.rename_in_place("existing.md").is_err());
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "PROTECTED");
        assert!(original.is_file());
        doc.rename_in_place("renamed.md").unwrap();
        state.migrated_path(&doc, &original);
        assert!(!original.exists());
        let mut reopened = Document::open(&dir.path().join("renamed.md")).unwrap();
        state.bind(&mut reopened, "main", 2);
        assert_eq!(reopened.id, id);
        let restore = state
            .plan(
                &reopened,
                "main/preview",
                2,
                "layout-a",
                &egui::Context::default(),
                viewport(),
                false,
            )
            .unwrap();
        assert_eq!(restore.position.offset, [17., 400.]);
    }
}

#[cfg(test)]
mod persistence_independent {
    use super::{document::Document, egui, reading_state::*};
    use serde_json::Value;
    use std::{
        path::Path,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    struct Environment(Option<std::ffi::OsString>);
    impl Drop for Environment {
        fn drop(&mut self) {
            // This test is explicitly run alone with one test thread. Every
            // ReadingState writer is dropped before this environment guard.
            unsafe {
                match &self.0 {
                    Some(value) => std::env::set_var("MDCARD_READING_STATE_PATH", value),
                    None => std::env::remove_var("MDCARD_READING_STATE_PATH"),
                }
            }
        }
    }
    fn disk(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
    fn empty_record(value: &Value, key: &str) -> bool {
        let record = &value["records"][key];
        record.is_null()
            || record["views"]
                .as_object()
                .is_some_and(|views| views.is_empty())
    }
    fn save_position(state: &mut ReadingState, doc: &Document, y: f32) {
        state.observe(
            doc,
            "main/preview",
            "independent-store-layout".into(),
            [0., y],
            [800., 600.],
            200,
            0.,
            0.,
            true,
        );
    }
    fn wait_cleared(state: &mut ReadingState, path: &Path, a: &str, b: &str, all: bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let ctx = egui::Context::default();
        loop {
            state.tick(&ctx);
            let value = disk(path);
            if empty_record(&value, a) && (!all || empty_record(&value, b)) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "An acknowledged explicit clear left original positions on disk (all={all})"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "Changes process environment: run explicitly alone with --ignored --test-threads=1"]
    fn independent_real_rename_preserves_identity_history_selection_after_restart() {
        let _environment = Environment(std::env::var_os("MDCARD_READING_STATE_PATH"));
        for clear_other_first in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("reading.json");
            let old = directory.path().join("original.md");
            let next = directory.path().join("renamed.md");
            let other = directory.path().join("other.md");
            let source = "中文🙂 independent rename source\n".repeat(100);
            std::fs::write(&old, source.as_bytes()).unwrap();
            std::fs::write(&other, source.as_bytes()).unwrap();
            unsafe { std::env::set_var("MDCARD_READING_STATE_PATH", &path) };
            let mut moving = ReadingState::persistent();
            let mut document = Document::open(&old).unwrap();
            moving.bind(&mut document, "main", 1);
            let identity = document.id;
            save_position(&mut moving, &document, 700.);
            moving.store_selection(&document, "main/source", [2, 7]);
            let mut neighbour = Document::open(&other).unwrap();
            moving.bind(&mut neighbour, "secondary", 1);
            if clear_other_first {
                moving.clear(Some(neighbour.id));
            }
            moving.flush();
            document.rename_in_place("renamed.md").unwrap();
            moving.migrated_path(&document, &old);
            moving.flush();
            drop(moving);
            assert!(!old.exists());
            assert_eq!(std::fs::read(&next).unwrap(), source.as_bytes());
            let persisted = disk(&path);
            let key = path_key(&next);
            assert_eq!(
                persisted["records"][&key]["id"].as_u64(),
                Some(identity.0),
                "Migrating a path must not be rejected by its own clear barrier (clear_other_first={clear_other_first})"
            );
            assert_eq!(persisted["records"][&key]["views"]["main/preview"]["offset"][1], 700.);
            assert!(empty_record(&persisted, &path_key(&old)));
            let mut reopened_state = ReadingState::persistent();
            let mut reopened = Document::open(&next).unwrap();
            reopened_state.bind(&mut reopened, "main", 99);
            assert_eq!(reopened.id, identity);
            assert_eq!(reopened_state.selection(&reopened, "main/source"), Some([2, 7]));
            let report = reopened_state.report(&reopened, None);
            let record = report["records"].as_array().unwrap().iter()
                .find(|record| record["id"].as_u64() == Some(identity.0)).unwrap();
            assert_eq!(record["views"]["main/preview"]["offset"][1], 700.);
            drop(reopened_state);
        }
    }

    #[test]
    #[ignore = "Changes process environment: run explicitly alone with --ignored --test-threads=1"]
    fn independent_future_clear_is_durable_stale_snapshots_cannot_revive_and_fresh_reads_can_save()
    {
        let _environment = Environment(std::env::var_os("MDCARD_READING_STATE_PATH"));
        for all in [false, true] {
            for future_activity_and_positions in [false, true] {
                for skew_ms in [1_u64, 3_600_000] {
                    let temp = tempfile::tempdir().unwrap();
                    let path = temp.path().join("reading-state.json");
                    let a = temp.path().join("a.md");
                    let b = temp.path().join("b.md");
                    let original = "# Independent persistence contract\n\n中文🙂 e\u{301} original paragraph.\n\n".repeat(60);
                    std::fs::write(&a, &original).unwrap();
                    std::fs::write(&b, &original).unwrap();
                    unsafe { std::env::set_var("MDCARD_READING_STATE_PATH", &path) };
                    {
                        let mut state = ReadingState::persistent();
                        let mut doc = Document::open(&a).unwrap();
                        state.bind(&mut doc, "main", 1);
                        save_position(&mut state, &doc, 700.);
                        let mut other = Document::open(&b).unwrap();
                        state.bind(&mut other, "main", 2);
                        save_position(&mut state, &other, 900.);
                        state.flush();
                    }
                    let mut seeded = disk(&path);
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                        + skew_ms;
                    for record in seeded["records"].as_object_mut().unwrap().values_mut() {
                        record["updated_ms"] = timestamp.into();
                        if future_activity_and_positions {
                            record["activity_ms"] = timestamp.into();
                            for position in record["views"].as_object_mut().unwrap().values_mut() {
                                position["updated_ms"] = timestamp.into();
                            }
                        }
                    }
                    // Seed only supported timestamp fields. Original offsets
                    // came through public capture APIs, not fabricated history.
                    std::fs::write(&path, serde_json::to_vec(&seeded).unwrap()).unwrap();
                    let a_key = path_key(&a);
                    let b_key = path_key(&b);
                    let mut stale = ReadingState::persistent();
                    let mut stale_doc = Document::open(&a).unwrap();
                    stale.bind(&mut stale_doc, "main", 1);
                    let mut clearer = ReadingState::persistent();
                    let mut clearing_doc = Document::open(&a).unwrap();
                    clearer.bind(&mut clearing_doc, "main", 1);
                    assert!(
                        clearer.report(&clearing_doc, None)["writes"]["error"].is_null(),
                        "The bounded future fixture must be accepted before clear"
                    );
                    clearer.clear(if all { None } else { Some(clearing_doc.id) });
                    clearer.flush();
                    wait_cleared(&mut clearer, &path, &a_key, &b_key, all);
                    // Ordinary metadata and the inactive B snapshot have no
                    // authority to reverse a clear from another instance.
                    stale.set_mode(&stale_doc, "Split");
                    stale.flush();
                    drop(stale);
                    assert!(empty_record(&disk(&path), &a_key));
                    if all {
                        assert!(empty_record(&disk(&path), &b_key));
                    } else {
                        assert_eq!(
                            disk(&path)["records"][&b_key]["views"]["main/preview"]["offset"][1],
                            900.
                        );
                    }
                    std::thread::sleep(Duration::from_millis(10));
                    {
                        let mut fresh = ReadingState::persistent();
                        let mut doc = Document::open(&a).unwrap();
                        fresh.bind(&mut doc, "main", 1);
                        save_position(&mut fresh, &doc, 450.);
                        fresh.flush();
                    }
                    assert_eq!(
                        disk(&path)["records"][&a_key]["views"]["main/preview"]["offset"][1],
                        450.,
                        "True reading after clear must remain persistable"
                    );
                    // The old clearer still owns a deleted in-memory record.
                    // Updating its mode is not a new user request to clear.
                    clearer.set_mode(&clearing_doc, "Write");
                    clearer.flush();
                    drop(clearer);
                    assert_eq!(
                        disk(&path)["records"][&a_key]["views"]["main/preview"]["offset"][1],
                        450.,
                        "A late tombstone snapshot deleted newer real reading"
                    );
                    if all {
                        assert!(empty_record(&disk(&path), &b_key));
                    }
                    assert_eq!(std::fs::read_to_string(&a).unwrap(), original);
                    assert_eq!(std::fs::read_to_string(&b).unwrap(), original);
                }
            }
        }
    }

    #[test]
    #[ignore = "Changes process environment: run explicitly alone with --ignored --test-threads=1"]
    fn independent_future_clear_same_instance_raw_wheel_can_persist_again() {
        let _environment = Environment(std::env::var_os("MDCARD_READING_STATE_PATH"));
        for all in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("state.json");
            let a = temp.path().join("a.md");
            std::fs::write(
                &a,
                "# Independent same-instance reading\n\n中文🙂 original body.\n\n".repeat(80),
            )
            .unwrap();
            unsafe { std::env::set_var("MDCARD_READING_STATE_PATH", &path) };
            {
                let mut state = ReadingState::persistent();
                let mut doc = Document::open(&a).unwrap();
                state.bind(&mut doc, "main", 1);
                save_position(&mut state, &doc, 700.);
                state.flush();
            }
            let mut seeded = disk(&path);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000;
            for record in seeded["records"].as_object_mut().unwrap().values_mut() {
                record["updated_ms"] = timestamp.into();
            }
            std::fs::write(&path, serde_json::to_vec(&seeded).unwrap()).unwrap();
            let mut state = ReadingState::persistent();
            let mut doc = Document::open(&a).unwrap();
            state.bind(&mut doc, "main", 1);
            state.clear(if all { None } else { Some(doc.id) });
            wait_cleared(&mut state, &path, &path_key(&a), &path_key(&a), all);
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
            state.viewport("main/preview", rect);
            let ctx = egui::Context::default();
            ctx.begin_pass(egui::RawInput {
                screen_rect: Some(rect),
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(40., 40.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., -12.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            });
            let _ = state.plan(
                &doc,
                "main/preview",
                1,
                "independent-store-layout",
                &ctx,
                rect,
                false,
            );
            save_position(&mut state, &doc, 450.);
            state.flush();
            let _ = ctx.end_pass();
            drop(state);
            assert_eq!(
                disk(&path)["records"][path_key(&a)]["views"]["main/preview"]["offset"][1],
                450.,
                "A current-process real-input event was discarded by an older sent timestamp (all={all})"
            );
        }
    }

    #[test]
    #[ignore = "Changes process environment: run explicitly alone with --ignored --test-threads=1"]
    fn independent_single_clear_survives_tombstone_capacity_eviction_and_stale_reopen() {
        let _environment = Environment(std::env::var_os("MDCARD_READING_STATE_PATH"));
        for stale_reopens in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("state.json");
            let a = temp.path().join("a.md");
            let b = temp.path().join("b.md");
            let source =
                "# Capacity and explicit clear\n\nOriginal 中文🙂 independent text.\n\n".repeat(30);
            std::fs::write(&a, &source).unwrap();
            std::fs::write(&b, &source).unwrap();
            unsafe { std::env::set_var("MDCARD_READING_STATE_PATH", &path) };
            {
                let mut seed = ReadingState::persistent();
                let mut doc = Document::open(&a).unwrap();
                seed.bind(&mut doc, "main", 1);
                save_position(&mut seed, &doc, 700.);
                let mut other = Document::open(&b).unwrap();
                seed.bind(&mut other, "main", 2);
                save_position(&mut seed, &other, 900.);
                seed.flush();
            }
            let a_key = path_key(&a);
            let b_key = path_key(&b);
            let mut stale = ReadingState::persistent();
            let mut old_a = Document::open(&a).unwrap();
            stale.bind(&mut old_a, "main", 1);
            let mut old_b = Document::open(&b).unwrap();
            stale.bind(&mut old_b, "main", 2);
            {
                let mut clearer = ReadingState::persistent();
                let mut doc = Document::open(&a).unwrap();
                clearer.bind(&mut doc, "main", 1);
                clearer.clear(Some(doc.id));
                wait_cleared(&mut clearer, &path, &a_key, &b_key, false);
            }
            std::thread::sleep(Duration::from_millis(10));
            {
                let mut churn = ReadingState::persistent();
                for index in 0..512 {
                    let file = temp.path().join(format!("later-{index:04}.md"));
                    std::fs::write(&file, &source).unwrap();
                    let mut doc = Document::open(&file).unwrap();
                    churn.bind(&mut doc, "main", index + 1);
                    save_position(&mut churn, &doc, 100.);
                }
                // Keep an uncleared existing file genuinely active. A missing
                // cleared key must not turn into authority to erase this B.
                let mut other = Document::open(&b).unwrap();
                churn.bind(&mut other, "main", 600);
                save_position(&mut churn, &other, 900.);
                churn.flush();
            }
            let after_eviction = disk(&path);
            assert!(
                after_eviction["records"][&a_key].is_null(),
                "The real capacity path must evict A's tombstone before the stale-instance checks"
            );
            assert!(after_eviction["records"].as_object().unwrap().len() <= 512);
            assert!(std::fs::metadata(&path).unwrap().len() <= 4 * 1024 * 1024);
            if stale_reopens {
                stale.bind(&mut old_a, "main", 3);
                let report = stale.report(&old_a, None);
                let record = report["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|record| record["id"].as_u64() == Some(old_a.id.0))
                    .unwrap();
                assert!(
                    record["views"].as_object().unwrap().is_empty(),
                    "A fresh explicit open must observe the missing key's clear barrier before adopting an old in-memory reading position"
                );
                assert!(report["views"]["main/preview"]["pending"].is_null());
                save_position(&mut stale, &old_a, 450.);
                stale.flush();
                drop(stale);
            } else {
                stale.set_mode(&old_a, "Write");
                stale.flush();
                drop(stale);
                assert!(
                    empty_record(&disk(&path), &a_key),
                    "An old metadata-only snapshot resurrected a single-cleared document after capacity eviction"
                );
                let mut fresh = ReadingState::persistent();
                let mut doc = Document::open(&a).unwrap();
                fresh.bind(&mut doc, "main", 1);
                save_position(&mut fresh, &doc, 450.);
                fresh.flush();
                drop(fresh);
            }
            let final_disk = disk(&path);
            assert_eq!(
                final_disk["records"][&a_key]["views"]["main/preview"]["offset"][1],
                450.
            );
            assert_eq!(
                final_disk["records"][&b_key]["views"]["main/preview"]["offset"][1],
                900.
            );
            assert_eq!(std::fs::read_to_string(&a).unwrap(), source);
            assert_eq!(std::fs::read_to_string(&b).unwrap(), source);
        }
    }
}
