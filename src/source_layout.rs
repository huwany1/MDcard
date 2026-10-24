// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Source layout which shares unchanged paragraph glyphs and meshes.
//! egui's general Galley::concat clones the final row of every paragraph to set
//! its newline bit. Cache that one-time adjustment and compose placed rows only.
use eframe::egui;
use egui::emath::GuiRounding as _;
use egui::text::{Galley, LayoutJob};
use egui::{Color32, FontId, Ui};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};
type Paragraph = (Arc<Galley>, Arc<Galley>, Arc<Galley>, bool);
#[derive(Default)]
pub struct SourceLayout {
    paragraphs: HashMap<(usize, bool), Paragraph>,
    pass: u64,
    frame_lines: HashMap<(u64, u64), Arc<Galley>>,
    last_children: Vec<Arc<Galley>>,
    last: Option<Arc<Galley>>,
    last_style: u64,
}
impl SourceLayout {
    pub fn layout(
        &mut self,
        ui: &Ui,
        text: &str,
        font: FontId,
        color: Color32,
        width: f32,
    ) -> Arc<Galley> {
        let pass = ui.ctx().cumulative_pass_nr();
        if self.pass != pass {
            self.pass = pass;
            self.frame_lines.clear();
        }
        let mut style_hash = std::collections::hash_map::DefaultHasher::new();
        font.hash(&mut style_hash);
        color.hash(&mut style_hash);
        width.to_bits().hash(&mut style_hash);
        ui.pixels_per_point().to_bits().hash(&mut style_hash);
        let style = style_hash.finish();
        let mut children = Vec::new();
        let mut metadata = Vec::new();
        let last_index = text.bytes().filter(|b| *b == b'\n').count();
        ui.fonts_mut(|fonts| {
            for (index, line) in text.split('\n').enumerate() {
                let newline = index < last_index;
                let mut line_hash = std::collections::hash_map::DefaultHasher::new();
                line.hash(&mut line_hash);
                let line_key = (style, line_hash.finish());
                let original = if let Some(cached) =
                    self.frame_lines.get(&line_key).filter(|g| g.text() == line)
                {
                    cached.clone()
                } else {
                    let g = fonts.layout(line.to_owned(), font.clone(), color, width);
                    self.frame_lines.insert(line_key, g.clone());
                    g
                };
                let key = (Arc::as_ptr(&original) as usize, newline);
                let entry = self.paragraphs.entry(key).or_insert_with(|| {
                    let mut prepared = (*original).clone();
                    if newline && let Some(last) = prepared.rows.last_mut() {
                        Arc::make_mut(&mut last.row).ends_with_newline = true;
                    }
                    let mut meta = prepared.clone();
                    meta.rows.clear();
                    (original.clone(), Arc::new(prepared), Arc::new(meta), false)
                });
                // The same paragraph can occur both with and without a following newline.
                let child = if !newline && entry.1.rows.last().is_some_and(|r| r.ends_with_newline)
                {
                    original
                } else {
                    entry.1.clone()
                };
                metadata.push(entry.2.clone());
                entry.3 = true;
                children.push(child);
            }
        });
        self.paragraphs.retain(|_, entry| {
            let used = entry.3;
            entry.3 = false;
            used
        });
        if self.last_style == style
            && self.last.as_ref().is_some_and(|g| g.text() == text)
            && children.len() == self.last_children.len()
            && children
                .iter()
                .zip(&self.last_children)
                .all(|(a, b)| Arc::ptr_eq(a, b))
        {
            return self.last.as_ref().unwrap().clone();
        }
        self.last_style = style;
        self.last_children = children.clone();
        let job = Arc::new(LayoutJob::simple(text.to_owned(), font, color, width));
        // Preserve the public Galley contract (including intrinsic_size) using cheap
        // metadata copies, then install shared rows without mutating their glyphs.

        let mut merged = Galley::concat(job, &metadata, ui.pixels_per_point());
        merged.rect = egui::Rect::ZERO;
        merged.mesh_bounds = egui::Rect::NOTHING;
        merged.rows.clear();
        for child in children {
            let offset = merged.rect.height();
            for row in &child.rows {
                let pos = (row.pos + egui::Vec2::Y * offset).round_to_pixels(ui.pixels_per_point());
                merged.mesh_bounds |= row.visuals.mesh_bounds.translate(pos.to_vec2());
                merged.rect |= egui::Rect::from_min_size(pos, row.size);
                merged.rows.push(egui::epaint::text::PlacedRow {
                    pos,
                    row: row.row.clone(),
                });
            }
        }
        // Match egui's final GUI rounding and narrow-glyph overflow contract.
        if merged.job.round_output_to_gui {
            let exceeds = merged.rect.width() > width + 1.;
            merged.rect = merged.rect.round_ui();
            if !exceeds {
                merged.rect.max.x = merged.rect.max.x.min(merged.rect.min.x + width).floor_ui();
            }
        }
        let merged = Arc::new(merged);
        self.last = Some(merged.clone());
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_native_layout_and_cursor_boundaries() {
        let ctx = egui::Context::default();
        let mut cache = SourceLayout::default();
        for text in [
            "",
            "one",
            "one\n",
            "\n\n",
            "same\nsame",
            "中😀a\r\n\r\n日本語\r",
            "a\nb\n\n",
            "long line with spaces and unicode αβγ😀中",
        ]
        .into_iter()
        .chain(["one\ntwo", "one\nmodified two", "one\ntwo\n"])
        {
            for width in [f32::INFINITY, 70., 300.] {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let font = FontId::monospace(15.);
                        let color = Color32::WHITE;
                        let expected =
                            ui.fonts_mut(|f| f.layout(text.to_owned(), font.clone(), color, width));
                        let actual = cache.layout(ui, text, font, color, width);
                        assert_eq!(actual.text(), expected.text());
                        assert_eq!(actual.rows.len(), expected.rows.len(), "{text:?}");
                        assert_eq!(actual.rect, expected.rect, "{text:?}");
                        assert_eq!(
                            actual.intrinsic_size(),
                            expected.intrinsic_size(),
                            "{text:?}"
                        );
                        for (a, b) in actual.rows.iter().zip(&expected.rows) {
                            assert_eq!(a, b, "{text:?}");
                        }
                        for index in 0..=text.chars().count() {
                            let cursor = egui::text::CCursor::new(index);
                            assert_eq!(
                                actual.pos_from_cursor(cursor),
                                expected.pos_from_cursor(cursor)
                            );
                            assert_eq!(
                                actual.layout_from_cursor(cursor),
                                expected.layout_from_cursor(cursor)
                            );
                        }
                    });
                });
            }
        }
    }
}
