#![allow(dead_code)]
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
extern crate self as eframe;
pub use egui;
#[path = "../../../src/reading_state.rs"]
mod reading_state;
#[path = "../../../src/reading_precision.rs"]
mod reading_precision;
#[path = "../../../src/reading_geometry.rs"]
mod reading_geometry;
#[path = "../../../src/document.rs"]
mod document;
pub use document::Document;
#[path = "../../../src/encoding.rs"]
mod encoding;
#[path = "../../../src/large_document.rs"]
mod large_document;
#[path = "../../../src/localization.rs"]
mod localization;
#[path = "../../../src/rich_nodes.rs"]
mod rich_nodes;
#[path = "../../../src/text_files.rs"]
mod text_files;

#[cfg(test)]
mod independent {
    use super::*;
    use std::{
        ops::Range,
        time::{Duration, Instant},
    };

    fn settle<'a>(
        large: &'a mut large_document::LargeDocument,
        doc: &Document,
        generation: u64,
        ctx: &egui::Context,
    ) -> &'a large_document::Parsed {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            large.ensure(&doc.text, generation, doc.revision(), ctx);
            if large
                .parsed
                .as_ref()
                .is_some_and(|p| p.generation == generation && p.revision == large.revision)
            {
                return large.parsed.as_ref().unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "parser did not publish current generation/revision"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn full_blocks(node: tree_sitter::Node<'_>, out: &mut Vec<Range<usize>>) {
        if matches!(node.kind(), "document" | "section") {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                full_blocks(child, out);
            }
        } else if node.end_byte() > node.start_byte() {
            out.push(node.byte_range());
        }
    }

    fn compare_fresh(parsed: &large_document::Parsed, text: &str) {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_md::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        let mut expected = Vec::new();
        full_blocks(tree.root_node(), &mut expected);
        let actual = parsed
            .blocks
            .iter()
            .map(|b| b.source.clone())
            .collect::<Vec<_>>();
        let mut index = 0;
        for block in expected {
            let mut at = block.start;
            while at < block.end {
                let part = actual.get(index).expect("fresh grammar block was omitted");
                assert_eq!(
                    part.start, at,
                    "virtual partitions must cover grammar blocks without gaps"
                );
                assert!(
                    part.end > at && part.end <= block.end,
                    "virtual partition crosses a grammar boundary"
                );
                at = part.end;
                index += 1;
            }
        }
        assert_eq!(
            index,
            actual.len(),
            "unexpected virtual partitions after grammar tree end"
        );
        let expected_nodes = rich_nodes::parse(text).1;
        let signature = |nodes: &[rich_nodes::Node]| {
            nodes
                .iter()
                .map(|n| {
                    (
                        n.kind,
                        n.range.clone(),
                        n.body.clone(),
                        n.source.clone(),
                        n.raw.clone(),
                        n.prefix.clone(),
                        n.prefixes.clone(),
                        n.endings.clone(),
                        n.inline,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            signature(&parsed.nodes),
            signature(&expected_nodes),
            "partition nodes differ from whole-source parsing"
        );
    }

    fn corpus(prefix: &str) -> String {
        format!(
            "{prefix}\n\n{}",
            "Ordinary 中文 paragraph keeps grammar boundaries stable.\n\n".repeat(1800)
        )
    }

    #[test]
    fn local_and_fallback_edits_match_fresh_tree_and_rich_offsets() {
        let ctx = egui::Context::default();
        let mut large = large_document::LargeDocument::default();
        let mut doc = Document::from_text(&corpus(
            "# Title\n\nTarget $x$ and $y$ plain text.\n\n```mermaid\nflowchart LR\nA-->B\n```",
        ));
        compare_fresh(settle(&mut large, &doc, 1, &ctx), &doc.text);
        for (old, new) in [
            ("plain text", "plain updated text"),
            ("updated", "中🦀"),
            ("# Title", "# Title with more words"),
            ("$x$", "$x+1$"),
            ("Target", "> Target"),
            ("A-->B", "A-->C"),
            ("```mermaid", "``mermaid"),
            ("``mermaid", "```mermaid"),
            ("> Target", "Target\r\n\r\nNew paragraph"),
        ] {
            let at = doc.text.find(old).unwrap();
            doc.text.replace_range(at..at + old.len(), new);
            doc.mark_text_changed();
            compare_fresh(settle(&mut large, &doc, 1, &ctx), &doc.text);
        }
    }

    #[test]
    fn coalesced_mailbox_edits_and_generation_switch_do_not_publish_stale_ranges() {
        let ctx = egui::Context::default();
        let mut large = large_document::LargeDocument::default();
        let mut doc = Document::from_text(&corpus("# Initial\n\nTarget $a$"));
        settle(&mut large, &doc, 10, &ctx);
        for i in 0..60 {
            let at = doc.text.find("Target").unwrap() + "Target".len();
            doc.text.insert_str(at, if i % 3 == 0 { "中" } else { "x" });
            doc.mark_text_changed();
            large.ensure(&doc.text, 10, doc.revision(), &ctx);
        }
        compare_fresh(settle(&mut large, &doc, 10, &ctx), &doc.text);
        doc = Document::from_text(&corpus("# Replacement\n\n$new$"));
        large.ensure(&doc.text, 11, doc.revision(), &ctx);
        doc.text.insert_str(2, "Changed ");
        doc.mark_text_changed();
        compare_fresh(settle(&mut large, &doc, 11, &ctx), &doc.text);
    }

    #[test]
    fn outline_matches_commonmark_for_setext_inline_formatting_and_escaped_hashes() {
        let ctx = egui::Context::default();
        let mut large = large_document::LargeDocument::default();
        let doc = Document::from_text(&corpus(
            "# *Formatted* title\n\nSecond level\n------------\n\n# Literal \\#\n\n```text\n# ignored\n```",
        ));
        let parsed = settle(&mut large, &doc, 99, &ctx);
        let actual = parsed
            .headings
            .iter()
            .map(|(level, title, _)| (*level, title.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            document::outline(&doc.text),
            "large-document TOC differs from CommonMark outline"
        );
    }

    #[test]
    fn giant_line_end_cursor_survives_idle_frames_and_remains_mounted() {
        giant_line_end_cursor(false);
    }

    #[test]
    fn wrapped_giant_line_end_cursor_survives_idle_frames_and_remains_mounted() {
        giant_line_end_cursor(true);
    }

    fn giant_line_end_cursor(wrap: bool) {
        let ctx = egui::Context::default();
        let mut large = large_document::LargeDocument::default();
        let mut doc = Document::from_text(&"中".repeat(100000));
        let id = egui::Id::new("independent-giant-source");
        for frame in 0..6 {
            large.ensure(&doc.text, 1, doc.revision(), &ctx);
            ctx.memory_mut(|m| m.request_focus(id));
            let mut raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700., 700.),
                )),
                time: Some(frame as f64 * 0.03),
                ..Default::default()
            };
            if frame == 1 {
                raw.events.push(egui::Event::Key {
                    key: egui::Key::End,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    },
                });
            }
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    large.show_source(ui, &mut doc, id, 15., wrap, false,&mut crate::reading_state::ReadingState::default(),"test/source",1);
                });
            });
            if frame >= 1 {
                let selection = egui::text_edit::TextEditState::load(&ctx, id)
                    .unwrap()
                    .cursor
                    .char_range()
                    .unwrap();
                assert_eq!(
                    selection.primary.index, 100000,
                    "end cursor lost on frame {frame}"
                );
                assert!(
                    large.visible_source.contains(&99999),
                    "end cursor unmounted on frame {frame}: {:?}",
                    large.visible_source
                );
            }
        }
    }

    #[test]
    fn long_code_pages_preserve_language_source_and_literal_math() {
        for (opening, closing) in [
            ("```rust", "```"),
            ("~~~~rust", "~~~~"),
            ("````rust", "````"),
        ] {
            let body = "let literal = \"$x$\"; // 中文 🦀\n".repeat(700);
            let source = corpus(&format!("# Code\n\n{opening}\n{body}{closing}\n"));
            let doc = Document::from_text(&source);
            let ctx = egui::Context::default();
            let mut large = large_document::LargeDocument::default();
            let parsed = settle(&mut large, &doc, 1, &ctx);
            compare_fresh(parsed, &source);
            assert!(
                parsed.nodes.is_empty(),
                "literal math inside code created media nodes"
            );
            let pages = parsed
                .blocks
                .iter()
                .filter_map(|b| b.code.as_ref())
                .collect::<Vec<_>>();
            assert!(pages.len() > 2, "long code was not paged");
            assert!(
                pages.iter().all(|p| p.language == "rust"),
                "code page lost its language"
            );
            let reconstructed = pages
                .iter()
                .map(|p| &p.source[p.bytes.clone()])
                .collect::<String>();
            assert_eq!(reconstructed, body, "paging changed code body bytes");
        }
    }

    #[test]
    fn giant_single_line_code_has_bounded_ui_pages() {
        let body = format!("let s = \"{}\";\n", "中".repeat(50000));
        let doc = Document::from_text(&corpus(&format!("```rust\n{body}```\n")));
        let ctx = egui::Context::default();
        let mut large = large_document::LargeDocument::default();
        let parsed = settle(&mut large, &doc, 1, &ctx);
        let pages = parsed
            .blocks
            .iter()
            .filter_map(|b| b.code.as_ref())
            .collect::<Vec<_>>();
        assert!(!pages.is_empty());
        assert!(
            pages.iter().all(|p| p.bytes.len() <= 8192),
            "a giant line reached the UI as an unbounded code page"
        );
        let reconstructed = pages
            .iter()
            .map(|p| &p.source[p.bytes.clone()])
            .collect::<String>();
        assert_eq!(reconstructed, body, "horizontal paging lost source bytes");
    }

    #[test]
    fn long_markdown_containers_keep_original_media_spans_and_edit_metadata() {
        let paragraph = format!(
            "**{}$x^2$ {} [linked text](https://example.com)**\n",
            "word 中文 ".repeat(1700),
            "continued ".repeat(1400)
        );
        let quote = format!(
            "> *start {}\n> $quoted$\n> {} end*\n",
            "quoted words ".repeat(1400),
            "quoted continuation ".repeat(1000)
        );
        let list = (1..700).map(|i|format!("{i}. List item **bold 中文** with $x_{{{i}}}$ and [link](https://example.com).\n")).collect::<String>();
        let nested_code = format!(
            "1. First\n\n   ```rust\n{}   ```\n\n2. Second with $outside$\n",
            "   let literal = \"$inside$\"; // multiline 中文\n".repeat(1000)
        );
        for content in [paragraph, quote, list, nested_code] {
            let source = corpus(&content);
            let ctx = egui::Context::default();
            let mut large = large_document::LargeDocument::default();
            let doc = Document::from_text(&source);
            compare_fresh(settle(&mut large, &doc, 1, &ctx), &source);
        }
    }
}
