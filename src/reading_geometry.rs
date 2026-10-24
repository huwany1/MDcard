// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Source identity projected onto galleys produced by the current UI pass.
use crate::{egui, reading_precision};
use serde::Serialize;
use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, Weak},
};

#[derive(Clone, Debug)]
pub struct Segment {
    pub rendered: Range<usize>,
    pub source: Range<usize>,
    pub linear: bool,
}
#[derive(Clone)]
pub struct MappedGalley {
    pub galley: Arc<egui::Galley>,
    pub pos: egui::Pos2,
    pub segments: Vec<Segment>,
    characters: Arc<CharacterIndex>,
}
struct CachedCharacters {
    galley: Weak<egui::Galley>,
    positions: Arc<CharacterIndex>,
    used: u64,
}
struct CharacterIndex {
    positions: Vec<u32>,
    stride: usize,
    characters: usize,
}
impl CharacterIndex {
    fn byte(&self, text: &str, character: usize) -> usize {
        let character = character.min(self.characters);
        let slot = character / self.stride;
        let start = self
            .positions
            .get(slot)
            .copied()
            .unwrap_or(text.len() as u32) as usize;
        text[start..]
            .char_indices()
            .nth(character - slot * self.stride)
            .map_or(text.len(), |(b, _)| start + b)
    }
    fn character(&self, text: &str, byte: usize) -> usize {
        let slot = self
            .positions
            .partition_point(|b| (*b as usize) <= byte)
            .saturating_sub(1);
        slot * self.stride + text[self.positions[slot] as usize..byte].chars().count()
    }
}
#[derive(Default)]
pub struct GeometryCache {
    entries: HashMap<usize, CachedCharacters>,
    clock: u64,
    bytes: usize,
    frame_work_ms: f64,
    index_ms: std::collections::VecDeque<f64>,
}
impl GeometryCache {
    fn characters(&mut self, galley: &Arc<egui::Galley>) -> Arc<CharacterIndex> {
        self.clock = self.clock.wrapping_add(1);
        let key = Arc::as_ptr(galley) as usize;
        if let Some(entry) = self.entries.get_mut(&key)
            && entry
                .galley
                .upgrade()
                .is_some_and(|g| Arc::ptr_eq(&g, galley))
        {
            entry.used = self.clock;
            return entry.positions.clone();
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.positions.positions.len() * 4);
        }
        let started = std::time::Instant::now();
        // Sparse checkpoints bound even a pathological multi-megabyte galley.
        // A conversion scans at most stride-1 local Unicode scalars.
        let stride = galley.text().len().div_ceil(262144).max(1);
        let mut positions = Vec::new();
        let mut characters = 0;
        for (i, (b, _)) in galley.text().char_indices().enumerate() {
            if i % stride == 0 {
                positions.push(b as u32);
            }
            characters = i + 1;
        }
        if characters % stride == 0 {
            positions.push(galley.text().len() as u32);
        }
        let positions = Arc::new(CharacterIndex {
            positions,
            stride,
            characters,
        });
        let bytes = positions.positions.len() * 4;
        while !self.entries.is_empty()
            && (self.entries.len() >= 256 || self.bytes + bytes > 2 * 1024 * 1024)
        {
            let key = *self.entries.iter().min_by_key(|(_, e)| e.used).unwrap().0;
            if let Some(old) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(old.positions.positions.len() * 4);
            }
        }
        if bytes <= 2 * 1024 * 1024 {
            self.entries.insert(
                key,
                CachedCharacters {
                    galley: Arc::downgrade(galley),
                    positions: positions.clone(),
                    used: self.clock,
                },
            );
            self.bytes += bytes;
        }
        let ms = started.elapsed().as_secs_f64() * 1000.;
        self.frame_work_ms += ms;
        if self.index_ms.len() >= 2000 {
            self.index_ms.pop_front();
        }
        self.index_ms.push_back(ms);
        positions
    }
    pub fn take_frame_ms(&mut self) -> f64 {
        std::mem::take(&mut self.frame_work_ms)
    }
    pub fn index_timings(&self) -> &std::collections::VecDeque<f64> {
        &self.index_ms
    }
    pub fn linear(
        &mut self,
        galley: Arc<egui::Galley>,
        pos: egui::Pos2,
        source_start: usize,
    ) -> MappedGalley {
        let started = std::time::Instant::now();
        let previous = self.frame_work_ms;
        let segments = vec![Segment {
            rendered: 0..galley.text().len(),
            source: source_start..source_start + galley.text().len(),
            linear: true,
        }];
        let characters = self.characters(&galley);
        self.frame_work_ms = previous + started.elapsed().as_secs_f64() * 1000.;
        MappedGalley {
            galley,
            pos,
            segments,
            characters,
        }
    }
    pub fn markdown(
        &mut self,
        painted: egui_commonmark::PaintedText,
        raw: &str,
        project: &impl Fn(Range<usize>) -> Option<Range<usize>>,
    ) -> MappedGalley {
        let started = std::time::Instant::now();
        let previous = self.frame_work_ms;
        let mut segments = Vec::new();
        for span in painted.spans {
            let Some(raw_text) = raw.get(span.source.clone()) else {
                continue;
            };
            let Some(value) = painted.galley.text().get(span.text.clone()) else {
                continue;
            };
            for (rendered, source) in fragment_map(raw_text, value) {
                let local = span.source.start + source.start..span.source.start + source.end;
                if let Some(source) = project(local) {
                    let rendered = span.text.start + rendered.start..span.text.start + rendered.end;
                    let linear = rendered.len() == source.len();
                    segments.push(Segment {
                        rendered,
                        source,
                        linear,
                    });
                }
            }
        }
        let characters = self.characters(&painted.galley);
        self.frame_work_ms = previous + started.elapsed().as_secs_f64() * 1000.;
        MappedGalley {
            galley: painted.galley,
            pos: painted.pos,
            segments,
            characters,
        }
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

// This verifies the complete decoded fragment before accepting a non-linear
// mapping. It never guesses the first matching word in the manuscript.
pub(crate) fn fragment_map(raw: &str, value: &str) -> Vec<(Range<usize>, Range<usize>)> {
    if raw == value {
        return vec![(0..value.len(), 0..raw.len())];
    }
    if !value.is_empty() && !raw.starts_with('&') {
        let mut found = raw.match_indices(value);
        if let Some((at, _)) = found.next()
            && found.next().is_none()
        {
            return vec![(0..value.len(), at..at + value.len())];
        }
    }
    let mut decoded = String::new();
    let mut result = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let start = at;
        let ch = raw[at..].chars().next().unwrap();
        if ch == '\\'
            && let Some(next) = raw[at + 1..]
                .chars()
                .next()
                .filter(char::is_ascii_punctuation)
        {
            at += 1 + next.len_utf8();
            let before = decoded.len();
            decoded.push(next);
            result.push((before..decoded.len(), start + 1..at));
            continue;
        }
        if ch == '&'
            && let Some(end) = raw[at..].find(';').filter(|n| *n < 40)
        {
            let end = at + end + 1;
            let entity = &raw[at..end];
            let value = pulldown_cmark::Parser::new(entity)
                .filter_map(|e| {
                    if let pulldown_cmark::Event::Text(t) = e {
                        Some(t.to_string())
                    } else {
                        None
                    }
                })
                .collect::<String>();
            if value != entity && !value.is_empty() {
                let before = decoded.len();
                decoded.push_str(&value);
                result.push((before..decoded.len(), at..end));
                at = end;
                continue;
            }
        }
        at += ch.len_utf8();
        let before = decoded.len();
        decoded.push(ch);
        result.push((before..decoded.len(), start..at));
    }
    if decoded == value { result } else { Vec::new() }
}

#[derive(Clone, Debug, Serialize)]
pub struct Glyph {
    pub source_byte: usize,
    pub source_end: usize,
    pub grapheme_range: Range<usize>,
    pub token_range: Range<usize>,
    pub text: String,
    pub source_text: String,
    pub rect: [f32; 4],
    pub viewport_y: f32,
    pub visual_row: usize,
    pub logical_line: usize,
    pub frame: u64,
    pub precision: &'static str,
    pub origin: &'static str,
    pub clamped: bool,
    pub clamp_reason: Option<String>,
    pub compensations: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct VisibleText {
    pub text: String,
    pub source_text: String,
    pub source_start: usize,
    pub source_end: usize,
    pub rect: [f32; 4],
    pub frame: u64,
    pub origin: &'static str,
}
impl MappedGalley {
    fn source_for(&self, rendered: Range<usize>) -> Option<Range<usize>> {
        let mut result: Option<Range<usize>> = None;
        let mut covered = rendered.start;
        for segment in &self.segments {
            let start = rendered.start.max(segment.rendered.start);
            let end = rendered.end.min(segment.rendered.end);
            if start >= end {
                continue;
            }
            if start > covered {
                return None;
            }
            let source = if segment.linear {
                segment.source.start + start - segment.rendered.start
                    ..segment.source.start + end - segment.rendered.start
            } else {
                segment.source.clone()
            };
            if let Some(prior) = &mut result {
                if source.start > prior.end {
                    return None;
                }
                prior.end = prior.end.max(source.end);
            } else {
                result = Some(source);
            }
            covered = covered.max(end);
        }
        (covered >= rendered.end).then_some(result).flatten()
    }
    fn rendered_for(&self, source: usize) -> Option<usize> {
        self.segments
            .iter()
            .find(|s| s.source.contains(&source))
            .map(|s| {
                if s.linear {
                    s.rendered.start + source - s.source.start
                } else {
                    s.rendered.start
                }
            })
    }
    fn glyph(
        &self,
        text: &str,
        rendered: usize,
        viewport: egui::Rect,
        frame: u64,
    ) -> Option<Glyph> {
        let range = reading_precision::grapheme_range(self.galley.text(), rendered);
        if range.is_empty() || range.len() > 1024 {
            return None;
        }
        let value = &self.galley.text()[range.clone()];
        if value.trim().is_empty() || value.chars().all(char::is_control) {
            return None;
        }
        let source = self.source_for(range.clone())?;
        text.get(source.clone())?;
        let grapheme = reading_precision::grapheme_range(text, source.start);
        if grapheme.len() > 1024 || source.start != grapheme.start || source.end < grapheme.end {
            return None;
        }
        let start = self.characters.character(self.galley.text(), range.start);
        let end = self.characters.character(self.galley.text(), range.end);
        let mut first = egui::text::CCursor::new(start);
        first.prefer_next_row = true;
        let mut last = egui::text::CCursor::new(end);
        last.prefer_next_row = false;
        let a = self.galley.pos_from_cursor(first);
        let b = self.galley.pos_from_cursor(last);
        let rect = a.union(b).translate(self.pos.to_vec2());
        if rect.width() <= 0.01 || rect.height() <= 0.01 {
            return None;
        }
        let visual_row = self
            .galley
            .rows
            .partition_point(|r| r.pos.y + self.pos.y < rect.min.y - 0.1)
            .min(self.galley.rows.len().saturating_sub(1));
        Some(Glyph {
            source_byte: grapheme.start,
            source_end: grapheme.end,
            token_range: source.clone(),
            grapheme_range: grapheme.clone(),
            text: value.to_owned(),
            source_text: text[grapheme].to_owned(),
            rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
            viewport_y: rect.min.y - viewport.min.y,
            visual_row,
            logical_line: 0,
            frame,
            precision: "text",
            origin: "galley-current-pass",
            clamped: false,
            clamp_reason: None,
            compensations: 0,
        })
    }
    pub fn target(
        &self,
        source: usize,
        text: &str,
        viewport: egui::Rect,
        frame: u64,
    ) -> Option<Glyph> {
        self.glyph(text, self.rendered_for(source)?, viewport, frame)
    }
    pub fn visible_glyphs(&self, text: &str, viewport: egui::Rect, frame: u64) -> Vec<Glyph> {
        let mut result = Vec::new();
        for run in self.visible_runs(text, viewport, frame) {
            let mut at = run.source_start;
            while at < run.source_end && result.len() < 1024 {
                if let Some(g) = self.target(at, text, viewport, frame) {
                    let rect = egui::Rect::from_min_max(
                        egui::pos2(g.rect[0], g.rect[1]),
                        egui::pos2(g.rect[2], g.rect[3]),
                    );
                    if rect.intersects(viewport)
                        && result
                            .last()
                            .is_none_or(|prior: &Glyph| prior.source_byte != g.source_byte)
                    {
                        result.push(g);
                    }
                }
                let next = reading_precision::grapheme_range(text, at).end;
                if next <= at {
                    break;
                }
                at = next;
            }
            if result.len() >= 1024 {
                break;
            }
        }
        result
    }
    pub fn visible_runs(&self, text: &str, viewport: egui::Rect, frame: u64) -> Vec<VisibleText> {
        let mut result = Vec::new();
        let first = self
            .galley
            .rows
            .partition_point(|r| r.pos.y + self.pos.y + r.height() < viewport.top());
        for row in self.galley.rows.iter().skip(first) {
            if row.pos.y + self.pos.y > viewport.bottom() {
                break;
            }
            let y = row.rect().center().y;
            let start = self
                .galley
                .cursor_from_pos(egui::vec2((viewport.left() - self.pos.x).max(row.pos.x), y));
            let end = self.galley.cursor_from_pos(egui::vec2(
                (viewport.right() - self.pos.x).min(row.pos.x + row.size.x),
                y,
            ));
            let range = self.characters.byte(self.galley.text(), start.index)
                ..self.characters.byte(self.galley.text(), end.index);
            for s in &self.segments {
                let a = range.start.max(s.rendered.start);
                let b = range.end.min(s.rendered.end);
                if a >= b {
                    continue;
                }
                let Some(source) = self.source_for(a..b) else {
                    continue;
                };
                let Some(original) = text.get(source.clone()) else {
                    continue;
                };
                let mut ca =
                    egui::text::CCursor::new(self.characters.character(self.galley.text(), a));
                ca.prefer_next_row = true;
                let mut cb =
                    egui::text::CCursor::new(self.characters.character(self.galley.text(), b));
                cb.prefer_next_row = false;
                let rect = self
                    .galley
                    .pos_from_cursor(ca)
                    .union(self.galley.pos_from_cursor(cb))
                    .translate(self.pos.to_vec2());
                result.push(VisibleText {
                    text: self.galley.text()[a..b].to_owned(),
                    source_text: original.to_owned(),
                    source_start: source.start,
                    source_end: source.end,
                    rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
                    frame,
                    origin: "galley-current-pass",
                });
                if result.len() >= 256 {
                    return result;
                }
            }
        }
        result
    }
    pub fn first_visible(&self, text: &str, viewport: egui::Rect, frame: u64) -> Option<Glyph> {
        if !self
            .galley
            .rect
            .translate(self.pos.to_vec2())
            .intersects(viewport)
        {
            return None;
        }
        let row = self
            .galley
            .rows
            .partition_point(|r| r.pos.y + self.pos.y < viewport.top() - 0.1)
            .min(self.galley.rows.len().saturating_sub(1));
        let at_y = self.galley.rows.get(row)?.rect().center().y;
        let cursor = self
            .galley
            .cursor_from_pos(egui::vec2((viewport.left() - self.pos.x).max(0.), at_y));
        let mut byte = self.characters.byte(self.galley.text(), cursor.index);
        let bottom = self.galley.cursor_from_pos(egui::vec2(
            viewport.right() - self.pos.x,
            viewport.bottom() - self.pos.y,
        ));
        let last_visible = self.characters.byte(self.galley.text(), bottom.index);
        while byte <= last_visible {
            if byte >= self.galley.text().len() {
                return None;
            }
            let range = reading_precision::grapheme_range(self.galley.text(), byte);
            if let Some(glyph) = self.glyph(text, byte, viewport, frame) {
                let rect = egui::Rect::from_min_max(
                    egui::pos2(glyph.rect[0], glyph.rect[1]),
                    egui::pos2(glyph.rect[2], glyph.rect[3]),
                );
                if rect.top() > viewport.bottom() {
                    return None;
                }
                if rect.intersects(viewport)
                    && rect.left() >= viewport.left() - 0.1
                    && rect.top() >= viewport.top() - 0.1
                {
                    return Some(glyph);
                }
            }
            byte = range.end;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fenced_code_and_entities_keep_original_graphemes() {
        let source = "A &amp; B\n\n```rust\nlet fenced_code = 3;\n```\n";
        let ctx = egui::Context::default();
        let mut cache = egui_commonmark::CommonMarkCache::default();
        let mut geometry = GeometryCache::default();
        let mut got = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for actual in egui_commonmark::CommonMarkViewer::new()
                    .show_with_geometry(ui, &mut cache, source)
                    .1
                {
                    let mapped = geometry.markdown(actual, source, &|r| Some(r));
                    for needle in ["&amp;", "fenced_code"] {
                        if let Some(g) =
                            mapped.target(source.find(needle).unwrap(), source, ui.max_rect(), 1)
                        {
                            got.push(g);
                        }
                    }
                }
            });
        });
        assert_eq!(got.len(), 2);
        let entity = got.iter().find(|g| g.text == "&").unwrap();
        assert_eq!(entity.source_end - entity.source_byte, 1);
        assert_eq!(entity.token_range.len(), 5);
    }
    #[test]
    fn markdown_text_is_mapped_from_actual_painted_shapes() {
        let source = "# Heading\n\nOne **bold 中文 e\u{301}** [label](https://invalid.test/other) and \\*escaped\\* &amp; 👩🏽‍💻.\n\n> quote\n\n- item\n\n| left | right |\n| --- | --- |\n| x | y |\n";
        let ctx = egui::Context::default();
        let mut cache = egui_commonmark::CommonMarkCache::default();
        let mut mapping = GeometryCache::default();
        let mut observed = 0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_width(460.);
                let (_, painted) = egui_commonmark::CommonMarkViewer::new()
                    .show_with_geometry(ui, &mut cache, source);
                for text in painted {
                    let actual = mapping.markdown(text, source, &|range| Some(range));
                    for needle in [
                        "Heading",
                        "bold",
                        "中文",
                        "label",
                        "escaped",
                        "&amp;",
                        "👩🏽‍💻",
                        "quote",
                        "item",
                        "left",
                        "right",
                    ] {
                        let byte = source.find(needle).unwrap();
                        if let Some(glyph) = actual.target(byte, source, ui.max_rect(), 1) {
                            assert_eq!(glyph.source_byte, byte, "{needle}");
                            assert_eq!(glyph.origin, "galley-current-pass");
                            assert!(glyph.rect[1].is_finite());
                            observed += 1;
                        }
                    }
                }
            });
        });
        assert!(observed >= 11, "only {observed} source runs mapped");
    }
}
