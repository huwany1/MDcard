// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Master-authored contracts use literal, independent Unicode boundaries.
#![allow(dead_code)]
pub use egui;
#[path = "../../../src/reading_geometry.rs"]
mod reading_geometry;
#[path = "../../../src/reading_precision.rs"]
mod reading_precision;

#[cfg(test)]
mod independent {
    use super::reading_precision::{self as precision, Anchor};

    #[test]
    fn literal_unicode_graphemes_have_independent_expected_byte_ranges() {
        // Expected clusters are literal fixtures, not segmented by the same
        // library used by the implementation to manufacture its own answer.
        let cases = [
            "中",
            "🙂",
            "e\u{301}",
            "👩🏽\u{200d}💻",
            "👍🏿",
            "🇨🇳",
            "☀\u{fe0f}",
            "1\u{fe0f}\u{20e3}",
            "\u{1100}\u{1161}\u{11a8}",
            "\r\n",
        ];
        for cluster in cases {
            let prefix = "前言 ASCII: ";
            let text = format!("{prefix}{cluster} 后文");
            let start = prefix.len();
            for byte in start..start + cluster.len() {
                let anchor = precision::capture(&text, byte);
                assert_eq!(anchor.source_byte, start, "{cluster:?} at {byte}");
                assert_eq!(anchor.grapheme_range, start..start + cluster.len());
                assert_eq!(anchor.grapheme, cluster);
                let resolved = precision::resolve(&text, &anchor);
                assert!(resolved.exact);
                assert_eq!(resolved.source_byte, start);
                assert_eq!(resolved.grapheme_range, start..start + cluster.len());
            }
        }
    }

    #[test]
    fn empty_eof_and_out_of_range_anchors_are_safe_without_false_empty_exactness() {
        for text in ["", "中🙂e\u{301}", "a\r\nb"] {
            for byte in [0, text.len(), text.len() + 1, usize::MAX] {
                let anchor = precision::capture(text, byte);
                let result = precision::resolve(text, &anchor);
                assert!(result.source_byte <= text.len());
                assert!(text.is_char_boundary(result.source_byte));
                assert!(result.grapheme_range.end <= text.len());
                assert!(result.grapheme_range.start <= result.grapheme_range.end);
                if text.is_empty() {
                    assert!(
                        !result.exact,
                        "No visible text exists in an empty manuscript"
                    );
                }
            }
        }
    }

    #[test]
    fn state_roundtrip_preserves_target_and_large_document_anchor_is_bounded() {
        let text = format!(
            "{}目标👩🏽\u{200d}💻保存位置后文",
            "普通上下文。".repeat(800_000)
        );
        let target = text.len() - "目标👩🏽\u{200d}💻保存位置后文".len() + "目标".len();
        let anchor = precision::capture(&text, target);
        let bytes = serde_json::to_vec(&anchor).unwrap();
        assert!(
            bytes.len() < 8192,
            "Persist bounded fragments rather than the manuscript"
        );
        let restored: Anchor = serde_json::from_slice(&bytes).unwrap();
        let result = precision::resolve(&text, &restored);
        assert!(result.exact);
        assert_eq!(result.source_byte, target);
        assert_eq!(&text[result.grapheme_range], "👩🏽\u{200d}💻");
    }

    #[test]
    fn malformed_optional_anchor_fields_cannot_panic_or_invent_text_exactness() {
        let text = "正文中🙂需要保守恢复";
        let valid = precision::capture(text, "正文".len());
        for (start, end, byte) in [(usize::MAX, 0, usize::MAX), (5, 2, 5), (0, usize::MAX, 0)] {
            let mut json = serde_json::to_value(&valid).unwrap();
            json["source_byte"] = serde_json::json!(byte);
            json["grapheme_range"] = serde_json::json!({"start": start, "end": end});
            json["grapheme"] = serde_json::json!("不存在的目标");
            let anchor: Anchor = serde_json::from_value(json).unwrap();
            let result = precision::resolve(text, &anchor);
            assert!(!result.exact);
            assert!(result.source_byte <= text.len());
            assert!(text.is_char_boundary(result.source_byte));
        }
    }

    #[test]
    fn decoded_utf8_coordinates_are_independent_of_utf16_file_storage_positions() {
        let decoded = "第一行\r\n中文👩🏽\u{200d}💻e\u{301}末尾";
        let prefix = "第一行\r\n中文";
        let utf8_byte = prefix.len();
        let utf16_byte = 2 + prefix.encode_utf16().count() * 2;
        assert_ne!(utf8_byte, utf16_byte);
        let anchor = precision::capture(decoded, utf8_byte);
        assert_eq!(anchor.source_byte, utf8_byte);
        assert_eq!(anchor.grapheme, "👩🏽\u{200d}💻");
        let resolution = precision::resolve(decoded, &anchor);
        assert_eq!(resolution.source_byte, utf8_byte);
        assert!(resolution.exact);
    }

    fn painted_targets(
        source: &str,
        width: f32,
        needles: &[&str],
    ) -> Vec<Option<super::reading_geometry::Glyph>> {
        let ctx = egui::Context::default();
        let mut commonmark = egui_commonmark::CommonMarkCache::default();
        let mut geometry = super::reading_geometry::GeometryCache::default();
        let mut result = vec![None; needles.len()];
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_width(width);
                let (_, painted) = egui_commonmark::CommonMarkViewer::new().show_with_geometry(
                    ui,
                    &mut commonmark,
                    source,
                );
                for shape in painted {
                    let mapped = geometry.markdown(shape, source, &Some, None);
                    for (index, needle) in needles.iter().enumerate() {
                        let expected_byte = source.find(needle).unwrap();
                        if let Some(glyph) = mapped.target(expected_byte, source, ui.max_rect(), 1)
                        {
                            assert!(
                                result[index].is_none(),
                                "Multiple painted targets for one original range"
                            );
                            result[index] = Some(glyph);
                        }
                    }
                }
            });
        });
        result
    }

    #[test]
    fn markdown_semantic_landmarks_map_to_actual_galleys_and_original_ranges() {
        let source = concat!(
            "# 标题🙂\n\n普通正文 e\u{301} 中文。\n\n",
            "**强调内容** [可见标签](https://invalid.test/hidden-destination) ",
            "`inline_code` \\*转义正文\\*。\n\n",
            "> 引用文字\n\n- 列表文字\n\n",
            "| 表头文字 | 数值 |\n| --- | --- |\n| 单元内容 | 17 |\n\n",
            "```text\nfenced_code_中文\nsecond line\n```\n"
        );
        let expected = [
            ("标题", "标"),
            ("普通正文", "普"),
            ("e\u{301}", "e\u{301}"),
            ("强调内容", "强"),
            ("可见标签", "可"),
            ("inline_code", "i"),
            ("转义正文", "转"),
            ("引用文字", "引"),
            ("列表文字", "列"),
            ("表头文字", "表"),
            ("单元内容", "单"),
            ("fenced_code", "f"),
        ];
        let needles = expected
            .iter()
            .map(|(needle, _)| *needle)
            .collect::<Vec<_>>();
        for width in [280., 510., 850.] {
            let mapped = painted_targets(source, width, &needles);
            for ((needle, cluster), point) in expected.iter().zip(mapped) {
                let point = point.unwrap_or_else(|| {
                    panic!("Missing actual painted source mapping for {needle} width {width}")
                });
                let start = source.find(needle).unwrap();
                assert_eq!(point.source_byte, start);
                assert_eq!(point.source_end, start + cluster.len());
                assert_eq!(point.text, *cluster);
                assert_eq!(point.source_text, *cluster);
                assert_eq!(point.grapheme_range, start..start + cluster.len());
                assert_eq!(point.origin, "galley-current-pass");
                assert!(point.rect.iter().all(|coordinate| coordinate.is_finite()));
            }
        }
    }

    #[test]
    fn entity_token_ranges_are_not_mislabeled_as_single_original_graphemes() {
        let source = "Before &amp; after &#x4E2D; last.\n";
        let expected = [("&amp;", "&"), ("&#x4E2D;", "中")];
        let targets = painted_targets(source, 460., &["&amp;", "&#x4E2D;"]);
        for ((token, displayed), point) in expected.into_iter().zip(targets) {
            let point = point.expect("An entity must have an actual painted source mapping");
            let start = source.find(token).unwrap();
            assert_eq!(point.text, displayed);
            assert_eq!(point.source_text, "&");
            assert_eq!(point.token_range, start..start + token.len());
            assert_eq!(point.source_byte, start);
            assert_eq!(point.source_end, start + 1);
            assert_eq!(
                point.grapheme_range,
                start..start + 1,
                "A whole Markdown entity token is not one original grapheme"
            );
        }
    }

    #[test]
    fn a_partial_virtual_galley_cannot_claim_to_paint_an_entire_original_grapheme() {
        for (cluster, split) in [("e\u{301}", 1), ("👩🏽\u{200d}💻", 4), ("🇨🇳", 4), ("👍🏿", 4)]
        {
            let prefix = "p: ";
            let source = format!("{prefix}{cluster} suffix");
            let start = prefix.len();
            for window in [start + split..start + cluster.len(), start..start + split] {
                let ctx = egui::Context::default();
                let mut cache = super::reading_geometry::GeometryCache::default();
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let galley = ui.fonts_mut(|fonts| {
                            fonts.layout(
                                source[window.clone()].to_owned(),
                                egui::FontId::monospace(16.),
                                egui::Color32::WHITE,
                                400.,
                            )
                        });
                        let pos = ui.cursor().min;
                        ui.add(egui::Label::new(galley.clone()));
                        let mapped = cache.linear(galley, pos, window.start);
                        if let Some(point) = mapped.target(window.start, &source, ui.max_rect(), 1)
                        {
                            assert_ne!(
                                point.precision,
                                "text",
                                "Only {:?} was painted, not the original full {cluster:?}",
                                &source[window.clone()]
                            );
                        }
                    });
                });
            }
        }
    }
}
