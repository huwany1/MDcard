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
#[path = "../../../src/text_files.rs"]
mod text_files;

#[cfg(test)]
mod faults {
    use super::*;
    use serde_json::{Value, json};
    use std::{fs, path::Path, sync::Mutex};
    static SERIAL: Mutex<()> = Mutex::new(());
    fn state_case(version: u32, anchor: Option<Value>, check: impl FnOnce(&mut reading_state::ReadingState, &mut document::Document, &Path)) {
        let _serial = SERIAL.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("source.md");
        let text = "# A\n\noriginal 中文 e\u{301} 👩🏽‍💻 text\n";
        fs::write(&file, text).unwrap();
        let path = reading_state::path_key(&file);
        let mut position = json!({"offset":[17.0,80.0],"viewport":[500.0,300.0],"anchor":{"byte":5},"layout":"test-layout","fingerprint":reading_precision::fingerprint(text)});
        if let Some(anchor) = anchor { position["text_anchor"] = anchor; }
        let state = root.path().join("state.json");
        fs::write(&state, serde_json::to_vec(&json!({"version":version,"records":{path.clone():{"id":77,"path":path,"views":{"main/preview":position}}}})).unwrap()).unwrap();
        let previous = std::env::var_os("MDCARD_READING_STATE_PATH");
        // These implementation-only cases run under SERIAL and do not share the
        // environment with a GUI or a product thread that reads this variable.
        unsafe { std::env::set_var("MDCARD_READING_STATE_PATH", &state); }
        let mut book = reading_state::ReadingState::persistent();
        let mut doc = document::Document::open(&file).unwrap();
        book.bind(&mut doc, "main", 1);
        check(&mut book, &mut doc, &state);
        drop(book);
        unsafe { if let Some(value) = previous {std::env::set_var("MDCARD_READING_STATE_PATH", value);} else {std::env::remove_var("MDCARD_READING_STATE_PATH");} }
        assert_eq!(fs::read_to_string(file).unwrap(), text);
    }
    #[test]
    fn old_schema_migrates_without_inventing_a_text_anchor() {
        state_case(1, None, |book, doc, _| {
            let ctx = egui::Context::default();
            let restored = book.plan(doc,"main/preview",1,"test-layout",&ctx,egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(500.,300.)),false).unwrap();
            assert_eq!(restored.position.offset,[17.,80.]);
            assert!(restored.position.text_anchor.is_none());
            assert_eq!(book.report(doc,None)["schema"],3);
        });
    }
    #[test]
    fn future_schema_is_inspectable_and_does_not_replace_the_original_state_file() {
        state_case(900, None, |book, doc, path| {
            let before = fs::read(path).unwrap();
            assert!(book.report(doc,None)["writes"]["error"].is_string());
            book.flush();
            assert_eq!(fs::read(path).unwrap(),before);
        });
    }
    #[test]
    fn oversized_optional_context_is_rejected_safely() {
        let mut anchor = serde_json::to_value(reading_state::TextAnchor {source:reading_precision::capture("# A\n\noriginal 中文 e\u{301} 👩🏽‍💻 text\n",8),..Default::default()}).unwrap();
        anchor["source"]["before"]=json!("X".repeat(5000));
        state_case(2,Some(anchor),|book,doc,_|assert!(book.report(doc,None)["writes"]["error"].is_string()));
    }
    #[test]
    fn invalid_optional_target_coordinates_degrade_without_losing_manuscript_bytes() {
        let mut anchor=serde_json::to_value(reading_state::TextAnchor{source:reading_precision::capture("# A\n\noriginal 中文 e\u{301} 👩🏽‍💻 text\n",8),..Default::default()}).unwrap();
        anchor["source"]["source_byte"]=json!(usize::MAX);
        state_case(2,Some(anchor),|book,doc,_|assert!(book.report(doc,None)["writes"]["error"].is_string()));
    }
    #[test]
    fn pathological_grapheme_does_not_persist_an_entire_long_manuscript() {
        let text = format!("e{}", "\u{301}".repeat(200000));
        let anchor = reading_precision::capture(&text, text.len()/2);
        assert_eq!(anchor.grapheme_range,0..text.len());
        assert!(serde_json::to_vec(&anchor).unwrap().len()<4096);
        assert!(!reading_precision::resolve(&text,&anchor).exact);
    }
    #[test]
    fn a_virtual_window_cannot_claim_to_paint_a_missing_grapheme_prefix() {
        let source = "prefix 👩🏽‍💻 suffix";
        let source_start = "prefix 👩".len();
        let ctx = egui::Context::default();
        let mut cache = reading_geometry::GeometryCache::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let galley=ui.fonts_mut(|f|f.layout_no_wrap(source[source_start..].to_owned(),egui::FontId::proportional(15.),egui::Color32::WHITE));
                let mapped=cache.linear(galley,egui::Pos2::ZERO,source_start);
                let first=mapped.target(source_start,source,egui::Rect::EVERYTHING,1);
                assert!(first.is_none_or(|g|g.source_byte>=source_start),"a painted partial ZWJ sequence must not claim the missing woman glyph");
            });
        });
    }
}
