// SPDX-License-Identifier: AGPL-3.0-only
//! One bounded, versioned two-pane scroll session. Only consumed native vertical
//! input acquires ownership; applying a target never generates another intent.
use crate::{egui, reading_geometry::MappedGalley};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Pane {
    Source,
    Preview,
}

impl Pane {
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub byte: usize,
    pub fraction: f32,
    pub viewport_y: f32,
    pub text: bool,
}
#[derive(Default)]
pub(crate) struct View {
    pub(crate) rect: Option<egui::Rect>,
    pub(crate) scrollbar: Option<egui::Rect>,
    pub(crate) offset: f32,
    pub(crate) max: f32,
    pub(crate) galleys: Vec<MappedGalley>,
    pub(crate) revision: Option<u64>,
}
#[derive(Clone, Copy)]
struct MeasuredAnchor {
    requested: usize,
    resolved: usize,
    content_y: f32,
}
#[derive(Clone, Copy, Debug)]
struct TableProgress {
    start: usize,
    end: usize,
    fraction: f32,
    end_after_row: bool,
}
#[derive(Default)]
pub struct SplitScroll {
    session: Option<(u64, u64)>,
    revision: u64,
    epoch: u64,
    pub(crate) current: bool,
    pub(crate) driver: Option<Pane>,
    keyboard_pane: Option<Pane>,
    key_target: Option<(Pane, f32)>,
    bar_drag: Option<Pane>,
    pub(crate) intent: Option<Pane>,
    pending_capture: Option<(Pane, u64, f32)>,
    confirmed_native: Option<(Pane, u64)>,
    anchor_sequence: Option<u64>,
    target_sequences: [Option<u64>; 2],
    settled_sequences: [Option<u64>; 2],
    anchor_top_reference: bool,
    recent_positions: [std::collections::VecDeque<MeasuredAnchor>; 2],
    geometry_stamps: [Option<u64>; 2],
    table_edges: std::collections::VecDeque<(usize, usize, bool)>,
    table_progress: Option<TableProgress>,
    pub(crate) anchor: Option<Anchor>,
    pub(crate) views: [View; 2],
    reduce_motion: bool,
    last_step: [f64; 2],
    layouts: [Option<(u32, u32, bool)>; 2],
    layout_compensation: [bool; 2],
    targets: [Option<f32>; 2],
    native_offsets: [f32; 2],
    pub sequence: u64,
    pub trace: std::collections::VecDeque<serde_json::Value>,
    reporter: Option<std::sync::mpsc::SyncSender<Vec<u8>>>,
    #[cfg(test)]
    pub(crate) repaint_requests: u64,
}
impl SplitScroll {
    pub fn session(
        &mut self,
        identity: Option<(u64, u64)>,
        revision: u64,
        current: bool,
        epoch: u64,
        reduce: bool,
    ) {
        if self.session != identity {
            *self = Self::default();
            self.session = identity;
        }
        // An edit invalidates a settled source coordinate. A new native scroll
        // during the waiting period may acquire ownership without coordinates;
        // that single latest intent is recaptured from its viewport on commit.
        if self.current && !current {
            self.driver = None;
        }
        if self.revision != revision || self.epoch != epoch || !current {
            self.anchor = None;
            self.settled_sequences = [None; 2];
            self.table_edges.clear();
            self.table_progress = None;
            for positions in &mut self.recent_positions {
                positions.clear();
            }
            self.pending_capture = None;
            self.confirmed_native = None;
            self.anchor_sequence = None;
            self.anchor_top_reference = false;
            for view in &mut self.views {
                view.galleys.clear();
                view.revision = None;
            }
        }
        self.revision = revision;
        self.epoch = epoch;
        self.current = current;
        self.reduce_motion = reduce;
    }
    pub fn enabled(&self) -> bool {
        self.session.is_some()
    }
    pub fn navigating(&mut self) {
        self.pending_capture = None;
        self.confirmed_native = None;
        self.anchor_sequence = None;
        self.target_sequences = [None; 2];
        self.settled_sequences = [None; 2];
        self.anchor_top_reference = false;
        self.driver = None;
        self.table_progress = None;
        self.intent = None;
        self.anchor = None;
    }
    pub fn input(&mut self, ctx: &egui::Context) {
        self.intent = None;
        self.key_target = None;
        if !self.enabled() {
            return;
        }
        if let Some(pane) = ctx.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
                .and_then(|p| {
                    self.views
                        .iter()
                        .position(|v| v.rect.is_some_and(|r| r.contains(p)))
                })
                .map(|n| if n == 0 { Pane::Source } else { Pane::Preview })
        }) {
            self.keyboard_pane = Some(pane);
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.bar_drag = None;
        }
        if ctx.input(|i| i.pointer.primary_pressed()) {
            self.bar_drag = ctx.input(|i| {
                i.pointer
                    .interact_pos()
                    .and_then(|p| {
                        self.views
                            .iter()
                            .position(|v| v.scrollbar.is_some_and(|r| r.expand(3.).contains(p)))
                    })
                    .map(|n| if n == 0 { Pane::Source } else { Pane::Preview })
            });
        }
        // Only the follower adds animation. Deliver native vertical wheel input
        // directly to the hovered ScrollArea, including nested consumers, so a
        // framework-wide smoothing remainder cannot migrate between panes.
        {
            let vertical = ctx.input(|i| {
                let hover = i.pointer.hover_pos().is_some_and(|p| {
                    self.views
                        .iter()
                        .any(|v| v.rect.is_some_and(|r| r.contains(p)))
                });
                (hover && !i.modifiers.ctrl && !i.modifiers.command).then_some(i.raw_scroll_delta.y)
            });
            if let Some(delta) = vertical {
                ctx.input_mut(|i| i.smooth_scroll_delta.y = delta);
            }
        }
        let candidate = self.bar_drag.or_else(|| {
            ctx.input(|i| {
                self.views.iter().enumerate().find_map(|(n, v)| {
                    let p = i.pointer.hover_pos()?;
                    let wheel = v.rect.is_some_and(|r| r.contains(p))
                        && (i.raw_scroll_delta.y.abs() > 0.01
                            || i.smooth_scroll_delta.y.abs() > 0.01);
                    let bar = i.pointer.primary_down()
                        && v.scrollbar.is_some_and(|r| r.expand(3.).contains(p));
                    (wheel || bar).then_some(if n == 0 { Pane::Source } else { Pane::Preview })
                })
            })
        });
        if let Some(pane) = candidate {
            self.intent = Some(pane);
            self.layout_compensation[pane.index()] = false;
            // Cancel before drawing the formerly following pane, even when its
            // native event later proves to have been consumed by a nested area.
            // Keep the current content identity through a takeover. Intent
            // immediately cancels the old follower before either pane draws.
        }
        let source_id = egui::Id::new(("source-editor", self.session.unwrap().1));
        let focused = ctx.memory(|m| m.focused());
        if focused == Some(source_id)
            && ctx.input(|i| {
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key {
                            key: egui::Key::ArrowUp
                                | egui::Key::ArrowDown
                                | egui::Key::ArrowLeft
                                | egui::Key::ArrowRight
                                | egui::Key::Home
                                | egui::Key::End,
                            pressed: true,
                            ..
                        }
                    )
                })
            })
        {
            self.navigating();
            self.keyboard_pane = Some(Pane::Source);
        }
        if focused.is_none() || focused == Some(source_id) {
            let key = ctx.input(|i| {
                (!i.modifiers.alt && !i.modifiers.command && !i.modifiers.shift)
                    .then(|| {
                        if i.key_pressed(egui::Key::PageDown) {
                            Some((egui::Key::PageDown, 1.))
                        } else if i.key_pressed(egui::Key::PageUp) {
                            Some((egui::Key::PageUp, -1.))
                        } else {
                            None
                        }
                    })
                    .flatten()
            });
            if let Some((key, delta)) = key {
                let pane = self.keyboard_pane.or(self.driver).unwrap_or(Pane::Source);
                if pane == Pane::Preview || focused == Some(source_id) {
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
                    let v = &self.views[pane.index()];
                    let y = (v.offset + delta * v.rect.map_or(400., |r| r.height() * 0.9))
                        .clamp(0., v.max);
                    self.key_target = Some((pane, y));
                    self.intent = Some(pane);
                    self.anchor = None;
                }
            }
        }
    }
    pub fn keyboard(&mut self, ui: &egui::Ui, pane: Pane, focused: bool) -> Option<f32> {
        if self.key_target.is_some_and(|(p, _)| p == pane) {
            return self.key_target.take().map(|(_, y)| y);
        }
        if !self.enabled() || !focused || self.keyboard_pane.is_some_and(|p| p != pane) {
            return None;
        }
        let delta = ui.input(|i| {
            if i.modifiers.alt || i.modifiers.command || i.modifiers.shift {
                return None;
            }
            if i.key_pressed(egui::Key::PageDown) {
                Some(1.)
            } else if i.key_pressed(egui::Key::PageUp) {
                Some(-1.)
            } else {
                None
            }
        })?;
        ui.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::NONE,
                if delta > 0. {
                    egui::Key::PageDown
                } else {
                    egui::Key::PageUp
                },
            );
        });
        self.intent = Some(pane);
        self.anchor = None;
        let v = &self.views[pane.index()];
        Some((v.offset + delta * v.rect.map_or(400., |r| r.height() * 0.9)).clamp(0., v.max))
    }
    pub fn following(&self, pane: Pane) -> bool {
        self.enabled()
            && self.current
            && self.anchor.is_some()
            && (self.driver != Some(pane) || self.layout_compensation[pane.index()])
            && self.intent != Some(pane)
    }
    pub fn keyboard_preview(&self) -> bool {
        self.keyboard_pane == Some(Pane::Preview)
    }
    pub fn operating(&self, pane: Pane) -> bool {
        self.intent == Some(pane)
    }
    pub fn preview_first(&self, ctx: &egui::Context) -> bool {
        self.enabled()
            && self.current
            && (self.intent == Some(Pane::Preview)
                || (self.intent.is_none()
                    && self
                        .pending_capture
                        .is_some_and(|(owner, _, _)| owner == Pane::Preview)))
            && !ctx.input(|i| {
                i.events.iter().any(|event| match event {
                    egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Cut
                    | egui::Event::Ime(_) => true,
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => !matches!(key, egui::Key::PageUp | egui::Key::PageDown),
                    _ => false,
                })
            })
    }
    pub fn layout(&mut self, pane: Pane, width: f32, font: f32, wrap: bool) {
        let key = (width.to_bits(), font.to_bits(), wrap);
        let n = pane.index();
        if self.layouts[n] != Some(key) {
            self.recent_positions[n].clear();
            self.targets[n] = None;
            self.target_sequences[n] = None;
            self.settled_sequences[n] = None;
            if self
                .pending_capture
                .is_some_and(|(owner, _, _)| owner == pane)
            {
                self.pending_capture = None;
                self.anchor_top_reference = false;
            }
            self.views[n].galleys.clear();
            self.views[n].revision = None;
            self.layouts[n] = Some(key);
            self.layout_compensation[n] = self.anchor.is_some();
        }
    }
    pub fn geometry_stamp(&mut self, pane: Pane, stamp: u64) {
        let n = pane.index();
        if self.geometry_stamps[n] != Some(stamp) {
            self.geometry_stamps[n] = Some(stamp);
            self.recent_positions[n].clear();
            self.targets[n] = None;
            self.target_sequences[n] = None;
            self.settled_sequences[n] = None;
            self.views[n].galleys.clear();
            self.views[n].revision = None;
            if self
                .pending_capture
                .is_some_and(|(owner, _, _)| owner == pane)
            {
                self.pending_capture = None;
            }
        }
    }
    pub(crate) fn precise(
        &self,
        pane: Pane,
        anchor: Anchor,
        text: &str,
        frame: u64,
    ) -> Option<f32> {
        let v = &self.views[pane.index()];
        if v.revision != Some(self.revision) || !anchor.text {
            return None;
        }
        let mapped = Self::mapped_glyph(v, anchor.byte, text, v.rect?, frame)
            .map(|g| (v.offset + g.viewport_y - anchor.viewport_y).clamp(0., v.max));
        #[cfg(test)]
        if mapped.is_none() && std::env::var_os("MDCARD_SCROLL_TEST_TRACE").is_some() {
            eprintln!(
                "CACHE_MISS frame={frame} pane={pane:?} requested={} stamp={:?} offset={} max={} entries={:?}",
                anchor.byte,
                self.geometry_stamps[pane.index()],
                v.offset,
                v.max,
                self.recent_positions[pane.index()]
                    .iter()
                    .map(|e| (e.requested, e.resolved, e.content_y))
                    .collect::<Vec<_>>()
            );
        }
        mapped.or_else(|| {
            self.recent_positions[pane.index()]
                .iter()
                .rev()
                .find(|entry| entry.requested == anchor.byte || entry.resolved == anchor.byte)
                .map(|entry| (entry.content_y - anchor.viewport_y).clamp(0., v.max))
        })
    }
    fn mapped_glyph(
        view: &View,
        byte: usize,
        text: &str,
        rect: egui::Rect,
        frame: u64,
    ) -> Option<crate::reading_geometry::Glyph> {
        if let Some(g) = view
            .galleys
            .iter()
            .find_map(|g| g.target(byte, text, rect, frame))
        {
            return Some(g);
        }
        // Hidden syntax falls back to a mapped neighbor on the same source
        // line, never to a matching string in another repeated paragraph.
        let byte = byte.min(text.len());
        let low = byte.saturating_sub(512);
        let high = (byte + 512).min(text.len());
        let bytes = text.as_bytes();
        let start = bytes[low..byte]
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(low, |n| low + n + 1);
        let end = bytes[byte..high]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(high, |n| byte + n);
        view.galleys
            .iter()
            .flat_map(|g| {
                g.segments.iter().filter_map(move |segment| {
                    let a = segment.source.start.max(start);
                    let b = segment.source.end.min(end);
                    if a >= b {
                        return None;
                    }
                    let mut probe = byte.clamp(a, b - 1);
                    while probe > a && !text.is_char_boundary(probe) {
                        probe -= 1;
                    }
                    g.target(probe, text, rect, frame)
                        .map(|glyph| (probe.abs_diff(byte), glyph))
                })
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, g)| g)
    }
    fn drawn_row_y(&self, pane: Pane, byte: usize, text: &str, frame: u64) -> Option<f32> {
        let v = &self.views[pane.index()];
        if pane == Pane::Preview
            && let Some(y) = v
                .galleys
                .iter()
                .filter_map(|g| g.table_row.as_ref())
                .find_map(|row| {
                    if row.source.start == byte {
                        Some(row.rect.top())
                    } else if row.source.end == byte {
                        Some(row.rect.bottom())
                    } else {
                        None
                    }
                })
        {
            return Some(v.offset + y - v.rect?.top());
        }
        let after = pane == Pane::Source
            && self
                .table_edges
                .iter()
                .any(|&(_, end, after)| end == byte && after);
        Self::mapped_glyph(
            v,
            byte.saturating_sub(usize::from(after)),
            text,
            v.rect?,
            frame,
        )
        .map(|g| v.offset + g.viewport_y + if after { g.rect[3] - g.rect[1] } else { 0. })
    }
    fn row_y(&self, pane: Pane, byte: usize, text: &str, frame: u64) -> Option<f32> {
        self.drawn_row_y(pane, byte, text, frame).or_else(|| {
            self.recent_positions[pane.index()]
                .iter()
                .rev()
                .find(|e| e.requested == byte || e.resolved == byte)
                .map(|e| e.content_y)
        })
    }
    fn drawn_target(&self, pane: Pane, text: &str, frame: u64) -> Option<f32> {
        let a = self.anchor?;
        if let Some(row) = self.table_progress {
            let start = self.drawn_row_y(pane, row.start, text, frame)?;
            let end = self.drawn_row_y(pane, row.end, text, frame)?;
            Some(start + (end - start) * row.fraction)
        } else {
            let view = &self.views[pane.index()];
            let glyph = Self::mapped_glyph(view, a.byte, text, view.rect?, frame)?;
            Some((view.offset + glyph.viewport_y - a.viewport_y).clamp(0., view.max))
        }
    }
    fn target_position(&self, pane: Pane, a: Anchor, text: &str, frame: u64) -> Option<f32> {
        #[cfg(test)]
        if std::env::var_os("MDCARD_TABLE_TRACE").is_some() {
            eprintln!(
                "ROW_TARGET frame={frame} pane={pane:?} row={:?} anchor={a:?} bounds={:?}",
                self.table_progress,
                self.table_progress.map(|r| (
                    self.row_y(pane, r.start, text, frame),
                    self.row_y(pane, r.end, text, frame)
                ))
            );
        }
        if let Some(row) = self.table_progress {
            let start = self.row_y(pane, row.start, text, frame)?;
            let end = self.row_y(pane, row.end, text, frame)?;
            return Some(start + (end - start) * row.fraction);
        }
        self.precise(pane, a, text, frame)
    }
    pub fn target(
        &mut self,
        pane: Pane,
        text: &str,
        ctx: &egui::Context,
        fallback: impl Fn(Anchor) -> Option<f32>,
    ) -> Option<f32> {
        if !self.following(pane) {
            return None;
        }
        let a = self.anchor?;
        let v = &self.views[pane.index()];
        let measured_target = self.target_position(pane, a, text, ctx.cumulative_frame_nr());
        let target = measured_target
            .or_else(|| {
                if let Some(row) = self.table_progress {
                    // A geometry invalidation may require mounting a distant
                    // row again. Bootstrap the same semantic interval rather
                    // than jumping to the next visible cell's glyph anchor.
                    let edge = |byte, fraction| Anchor {
                        byte,
                        viewport_y: 0.,
                        fraction,
                        text: true,
                    };
                    let start = fallback(edge(row.start, 0.))?;
                    let end = fallback(edge(row.end, if row.end_after_row { 1. } else { 0. }))?;
                    Some(start + (end - start) * row.fraction)
                } else {
                    fallback(a)
                }
            })?
            .clamp(0., v.max);
        self.targets[pane.index()] = Some(target);
        self.target_sequences[pane.index()] = self.anchor_sequence;
        let now = ctx.input(|i| i.time);
        let dt = (now - self.last_step[pane.index()]).clamp(1. / 240., 0.05) as f32;
        self.last_step[pane.index()] = now;
        let distance = target - v.offset;
        // A table row has different physical heights in each pane. Dropping
        // every <=1px remainder at each ownership handover compounds that
        // remainder in the other row's scale. Finish the newest actually
        // confirmed intent exactly once, then retain the idle deadband.
        let final_snap = self.table_progress.is_some()
            && measured_target.is_some()
            && self.anchor_sequence == Some(self.sequence)
            && self.target_sequences[pane.index()] == Some(self.sequence)
            && self
                .confirmed_native
                .is_some_and(|(owner, s)| Some(owner) == self.driver && s == self.sequence)
            && self.pending_capture.is_none()
            && self.settled_sequences[pane.index()] != Some(self.sequence);
        // Do not rewrite subpixel offsets on later proof/persistence wakeups.
        // Such writes make the reading store dirty again and can sustain an
        // otherwise idle loop even without requesting a repaint here.
        if distance.abs() <= 1. {
            if final_snap {
                self.settled_sequences[pane.index()] = Some(self.sequence);
                return (distance.abs() > 0.01).then_some(target);
            }
            return None;
        }
        let height = v.rect.map_or(600., |r| r.height());
        let mut next = if self.reduce_motion || distance.abs() > height * 1.5 || distance.abs() < 1.
        {
            target
        } else {
            v.offset + distance * (1. - (-dt / 0.018).exp())
        };
        if final_snap && (target - next).abs() <= 1. {
            self.settled_sequences[pane.index()] = Some(self.sequence);
            next = target;
        }
        if (target - next).abs() > 1. {
            #[cfg(test)]
            {
                self.repaint_requests += 1;
            }
            ctx.request_repaint();
        }
        ((next - v.offset).abs() > 0.01).then_some(next)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        pane: Pane,
        ui: &egui::Ui,
        id: egui::Id,
        rect: egui::Rect,
        offset: f32,
        native_next_offset: f32,
        max: f32,
        galleys: Vec<MappedGalley>,
        text: &str,
        revision: u64,
        valid: bool,
        internal: bool,
        fallback: Anchor,
    ) {
        if !self.enabled() {
            return;
        }
        let n = pane.index();
        let changed = (native_next_offset - self.native_offsets[n]).abs() > 0.01;
        self.native_offsets[n] = native_next_offset;
        let user = self.intent == Some(pane) && changed && !internal;
        let height_stable = (self.views[n].max - max.max(0.)).abs() <= 1.;
        let displayed_delta = offset - self.views[n].offset;
        let previous_driver = self.driver;
        self.views[n] = View {
            rect: Some(rect),
            scrollbar: ui.ctx().read_response(id.with(1usize)).map(|r| r.rect),
            offset,
            max: max.max(0.),
            galleys,
            revision: (valid && revision == self.revision).then_some(revision),
        };
        if !height_stable {
            self.recent_positions[n].clear();
        }
        if valid && revision == self.revision {
            if pane == Pane::Preview {
                let mut found = Vec::new();
                for g in &self.views[n].galleys {
                    if let Some(row) = &g.table_row {
                        // Only retain a bounded neighborhood of real painted
                        // rows; distant virtual rows never become fake anchors.
                        if row.rect.intersects(rect.expand(rect.height())) {
                            found.push((
                                row.source.clone(),
                                row.end_after_row,
                                offset + row.rect.top() - rect.top(),
                                offset + row.rect.bottom() - rect.top(),
                            ));
                        }
                    }
                }
                for (range, after, start_y, end_y) in found {
                    let pair = (range.start, range.end, after);
                    if let Some(i) = self.table_edges.iter().position(|x| *x == pair) {
                        self.table_edges.remove(i);
                    }
                    if self.table_edges.len() >= 128 {
                        self.table_edges.pop_front();
                    }
                    self.table_edges.push_back(pair);
                    for (start, y) in [(range.start, start_y), (range.end, end_y)] {
                        let entries = &mut self.recent_positions[n];
                        if let Some(i) = entries.iter().position(|e| e.requested == start) {
                            entries.remove(i);
                        }
                        if entries.len() >= 64 {
                            entries.pop_front();
                        }
                        entries.push_back(MeasuredAnchor {
                            requested: start,
                            resolved: start,
                            content_y: y,
                        });
                    }
                }
            } else {
                let rows: Vec<_> = self
                    .table_edges
                    .iter()
                    .flat_map(|&(a, b, _)| [a, b])
                    .collect();
                for start in rows {
                    if let Some(y) = self.row_y(pane, start, text, ui.ctx().cumulative_frame_nr()) {
                        let entries = &mut self.recent_positions[n];
                        if let Some(i) = entries.iter().position(|e| e.requested == start) {
                            entries.remove(i);
                        }
                        if entries.len() >= 64 {
                            entries.pop_front();
                        }
                        entries.push_back(MeasuredAnchor {
                            requested: start,
                            resolved: start,
                            content_y: y,
                        });
                    }
                }
            }
        }
        let previous_anchor = self.anchor;
        let arrived = !internal
            && valid
            && revision == self.revision
            && self.current
            && self.driver == Some(pane)
            && self
                .pending_capture
                .is_some_and(|(owner, sequence, wanted)| {
                    owner == pane && sequence == self.sequence && (offset - wanted).abs() <= 1.
                });
        if arrived
            || (internal
                && self
                    .pending_capture
                    .is_some_and(|(owner, _, _)| owner == pane))
        {
            self.pending_capture = None;
        }
        if arrived {
            self.confirmed_native = Some((pane, self.sequence));
        }
        if user {
            self.driver = Some(pane);
            self.keyboard_pane = Some(pane);
            self.sequence = self.sequence.wrapping_add(1);
            self.pending_capture = (valid && (native_next_offset - offset).abs() > 0.01)
                .then_some((pane, self.sequence, native_next_offset));
            if self.pending_capture.is_none() && valid && revision == self.revision {
                self.confirmed_native = Some((pane, self.sequence));
            } else if self
                .confirmed_native
                .is_some_and(|(owner, _)| owner != pane)
            {
                self.confirmed_native = None;
            }
            if self.pending_capture.is_some() {
                ui.ctx().request_repaint();
            }
        }
        if self.driver == Some(pane)
            && valid
            && revision == self.revision
            && !internal
            && (user || arrived || displayed_delta.abs() > 0.01 || self.anchor.is_none())
        {
            let view = &self.views[n];
            self.anchor_sequence = self
                .confirmed_native
                .filter(|(owner, _)| *owner == pane)
                .map(|(_, sequence)| sequence);
            // Both identity and viewport position come from this painted pass.
            // Never project old visible rows into the future native viewport.
            let mapped = previous_anchor.filter(|a| a.text).and_then(|a| {
                Self::mapped_glyph(view, a.byte, text, rect, ui.ctx().cumulative_frame_nr())
                    .map(|g| (a, g))
            });
            let canonical = mapped
                .as_ref()
                .filter(|(a, g)| {
                    let y = a.viewport_y
                        - if previous_driver == Some(pane) {
                            displayed_delta
                        } else {
                            0.
                        };
                    let line = g.rect[3] - g.rect[1];
                    let in_view = if self.anchor_top_reference {
                        y.abs() <= line
                    } else {
                        y >= 0. && y < rect.height()
                    };
                    height_stable
                        && !self.layout_compensation[n]
                        && in_view
                        && (g.viewport_y - y).abs() <= 1.
                })
                .map(|(a, _)| Anchor {
                    viewport_y: a.viewport_y
                        - if previous_driver == Some(pane) {
                            displayed_delta
                        } else {
                            0.
                        },
                    ..*a
                });
            let retained = mapped
                .filter(|(_, g)| {
                    let line = g.rect[3] - g.rect[1];
                    g.viewport_y >= -line
                        && if self.anchor_top_reference {
                            g.viewport_y <= line
                        } else {
                            g.viewport_y < rect.height()
                        }
                })
                .map(|(_, g)| Anchor {
                    byte: g.source_byte,
                    fraction: 0.,
                    viewport_y: g.viewport_y,
                    text: true,
                });
            self.anchor = canonical.or(retained);
            if self.anchor.is_none() {
                self.anchor = view
                    .galleys
                    .iter()
                    .filter_map(|g| g.first_visible(text, rect, ui.ctx().cumulative_frame_nr()))
                    .min_by(|a, b| a.viewport_y.total_cmp(&b.viewport_y))
                    .filter(|g| {
                        pane == Pane::Source
                            || g.viewport_y
                                <= (3. * (g.rect[3] - g.rect[1])).min(rect.height() * 0.25)
                    })
                    .map(|g| Anchor {
                        byte: g.source_byte,
                        fraction: 0.,
                        viewport_y: g.viewport_y,
                        text: true,
                    })
                    .or(Some(fallback));
                self.anchor_top_reference = true;
            }
            self.table_progress = None;
            for &(row_start, row_end, end_after_row) in &self.table_edges {
                if let (Some(start), Some(end)) = (
                    self.row_y(pane, row_start, text, ui.ctx().cumulative_frame_nr()),
                    self.row_y(pane, row_end, text, ui.ctx().cumulative_frame_nr()),
                ) && end > start
                    && start <= offset
                    && offset < end
                {
                    self.table_progress = Some(TableProgress {
                        start: row_start,
                        end: row_end,
                        fraction: ((offset - start) / (end - start)).clamp(0., 1.),
                        end_after_row,
                    });
                    break;
                }
            }
            #[cfg(test)]
            if std::env::var_os("MDCARD_TABLE_TRACE").is_some() {
                eprintln!(
                    "ROW_CAPTURE frame={} pane={pane:?} offset={offset} pending={:?} row={:?} pairs={:?}",
                    ui.ctx().cumulative_frame_nr(),
                    self.pending_capture,
                    self.table_progress,
                    self.table_edges
                        .iter()
                        .filter_map(|&(a, b, _)| Some((
                            a,
                            b,
                            self.row_y(pane, a, text, ui.ctx().cumulative_frame_nr())?,
                            self.row_y(pane, b, text, ui.ctx().cumulative_frame_nr())?
                        )))
                        .filter(|(_, _, a, b)| *a < offset + 100. && *b > offset - 100.)
                        .collect::<Vec<_>>()
                );
            }
            #[cfg(test)]
            {
                self.repaint_requests += 1;
            }
            ui.ctx().request_repaint();
        }
        if valid
            && revision == self.revision
            && let Some(a) = self.anchor.filter(|a| a.text)
            && let Some(g) = Self::mapped_glyph(
                &self.views[n],
                a.byte,
                text,
                rect,
                ui.ctx().cumulative_frame_nr(),
            )
        {
            let mut keys = vec![a.byte];
            let low = a.byte.saturating_sub(65);
            let line_start = text.as_bytes().get(low..a.byte).and_then(|tail| {
                tail.iter()
                    .rposition(|b| *b == b'\n')
                    .map(|at| low + at + 1)
                    .or((low == 0).then_some(0))
            });
            if let Some(line_start) = line_start
                && let Some(gap) = text.get(line_start..a.byte).filter(|gap| {
                    !gap.is_empty()
                        && gap.len() <= 64
                        && gap.chars().all(|c| {
                            c.is_whitespace()
                                || matches!(c, '|' | '#' | '>' | '*' | '_' | '~' | '`')
                        })
                })
            {
                let _ = gap;
                if Self::mapped_glyph(
                    &self.views[n],
                    line_start,
                    text,
                    rect,
                    ui.ctx().cumulative_frame_nr(),
                )
                .is_some_and(|alias| {
                    alias.source_byte == g.source_byte
                        && (alias.viewport_y - g.viewport_y).abs() < 0.1
                }) {
                    keys.push(line_start);
                }
            }
            for requested in keys {
                let entry = MeasuredAnchor {
                    requested,
                    resolved: g.source_byte,
                    content_y: offset + g.viewport_y,
                };
                let entries = &mut self.recent_positions[n];
                if let Some(index) = entries.iter().position(|old| {
                    old.requested == entry.requested && old.resolved == entry.resolved
                }) {
                    entries.remove(index);
                }
                if entries.len() >= 64 {
                    entries.pop_front();
                }
                entries.push_back(entry);
            }
        }
        if self.following(pane)
            && let Some(a) = self.anchor
            && let Some(target) =
                self.target_position(pane, a, text, ui.ctx().cumulative_frame_nr())
        {
            self.targets[n] = Some(target);
            self.target_sequences[n] = self.anchor_sequence;
            if (target - offset).abs() > 1. {
                #[cfg(test)]
                {
                    self.repaint_requests += 1;
                }
                ui.ctx().request_repaint();
            }
        }
        if self.layout_compensation[n]
            && let Some(a) = self.anchor
            && let Some(target) =
                self.target_position(pane, a, text, ui.ctx().cumulative_frame_nr())
            && (target - offset).abs() <= 1.
        {
            self.layout_compensation[n] = false;
        }
        if std::env::var_os("MDCARD_SCROLL_REPORT").is_some() {
            if self.trace.len() >= 512 {
                self.trace.pop_front();
            }
            let actual_glyph = self.anchor.and_then(|a| {
                Self::mapped_glyph(
                    &self.views[n],
                    a.byte,
                    text,
                    rect,
                    ui.ctx().cumulative_frame_nr(),
                )
            });
            let drawn_target = self.drawn_target(pane, text, ui.ctx().cumulative_frame_nr());
            self.trace.push_back(serde_json::json!({"frame":ui.ctx().cumulative_frame_nr(),"clock_ms":crate::rich_ui::preview_clock_ms(),"pane":pane,"offset":offset,"drawn_offset":offset,"native_next_offset":native_next_offset,"max":max,"user":user,"sequence":self.sequence,"confirmed_sequence":self.confirmed_native.map(|(_,sequence)|sequence),"confirmed_pane":self.confirmed_native.map(|(owner,_)|owner),"anchor_sequence":self.anchor_sequence,"target_sequence":self.target_sequences[n],"drawn_target":drawn_target,"drawn_target_residual":drawn_target.map(|t|t-offset),"pending_sequence":self.pending_capture.map(|(_,sequence,_)|sequence),"pending_pane":self.pending_capture.map(|(owner,_,_)|owner),"pending_offset":self.pending_capture.map(|(_,_,offset)|offset),"actual_line_height":actual_glyph.as_ref().map(|g|g.rect[3]-g.rect[1]),"actual_glyph_origin":actual_glyph.as_ref().map(|g|g.origin),"actual_glyph_viewport_y":actual_glyph.as_ref().map(|g|g.viewport_y),"driver":self.driver,"current":valid,"revision":revision,"epoch":self.epoch,"session":self.session,"internal":internal,"target":self.targets[n],"rect":[rect.left(),rect.top(),rect.right(),rect.bottom()],"scrollbar":self.views[n].scrollbar.map(|r|[r.left(),r.top(),r.right(),r.bottom()]),"anchor":self.anchor.map(|a|serde_json::json!({"byte":a.byte,"viewport_y":a.viewport_y,"text":a.text}))}));
        }
    }
    pub fn report(&mut self) {
        let Some(path) = std::env::var_os("MDCARD_SCROLL_REPORT") else {
            return;
        };
        if self.reporter.is_none() {
            let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(1);
            std::thread::spawn(move || {
                let mut sink = crate::diagnostic_sink::DiagnosticSinks::default();
                while let Ok(bytes) = rx.recv() {
                    let _ = sink.write(crate::diagnostic_sink::Kind::Scroll, &path, &bytes);
                }
            });
            self.reporter = Some(tx);
        }
        if let Ok(bytes) = serde_json::to_vec(&self.trace)
            && let Some(tx) = &self.reporter
        {
            let _ = tx.try_send(bytes);
        }
    }
}

#[cfg(test)]
mod pending_capture_tests {
    use super::*;
    #[test]
    fn confirmed_table_intent_finishes_once_without_subpixel_idle_rewrites() {
        let ctx = egui::Context::default();
        let mut s = SplitScroll::default();
        s.session(Some((1, 1)), 0, true, 1, false);
        s.driver = Some(Pane::Preview);
        s.sequence = 7;
        s.confirmed_native = Some((Pane::Preview, 7));
        s.anchor_sequence = Some(7);
        s.anchor = Some(Anchor {
            byte: 0,
            fraction: 0.,
            viewport_y: 0.,
            text: true,
        });
        s.table_progress = Some(TableProgress {
            start: 0,
            end: 1,
            fraction: 0.,
            end_after_row: false,
        });
        s.views[0].offset = 200.;
        s.views[0].max = 1000.;
        s.recent_positions[0].push_back(MeasuredAnchor {
            requested: 0,
            resolved: 0,
            content_y: 200.8,
        });
        s.recent_positions[0].push_back(MeasuredAnchor {
            requested: 1,
            resolved: 1,
            content_y: 218.4,
        });
        let _ = ctx.run(Default::default(), |ctx| {
            s.anchor_sequence = Some(6);
            assert!(
                s.target(Pane::Source, "", ctx, |_| None).is_none(),
                "old drawn anchor cannot finish the newest input"
            );
            s.anchor_sequence = Some(7);
            s.confirmed_native = Some((Pane::Source, 7));
            assert!(
                s.target(Pane::Source, "", ctx, |_| None).is_none(),
                "another pane's confirmation cannot settle this owner"
            );
            s.confirmed_native = Some((Pane::Preview, 7));
            s.pending_capture = Some((Pane::Preview, 7, 240.));
            assert!(
                s.target(Pane::Source, "", ctx, |_| None).is_none(),
                "pending future viewport is not confirmed drawn geometry"
            );
            s.pending_capture = None;
            assert_eq!(s.target(Pane::Source, "", ctx, |_| None), Some(200.8));
            s.views[0].offset = 200.8;
            for y in [200.8, 201.2, 200.2, 200.8] {
                s.recent_positions[0][0].content_y = y;
                assert!(
                    s.target(Pane::Source, "", ctx, |_| None).is_none(),
                    "same confirmed sequence cannot rewrite a later quantized proof position"
                );
            }
        });
        assert_eq!(s.repaint_requests, 0);
        assert_eq!(s.sequence, 7);
    }
    #[test]
    fn latest_source_intent_precedes_an_older_pending_preview_capture() {
        let ctx = egui::Context::default();
        let mut s = SplitScroll::default();
        s.session(Some((1, 1)), 0, true, 1, false);
        s.pending_capture = Some((Pane::Preview, 1, 40.));
        let _ = ctx.run(Default::default(), |ctx| {
            s.intent = Some(Pane::Source);
            assert!(!s.preview_first(ctx));
            s.intent = None;
            assert!(s.preview_first(ctx));
        });
    }
    #[test]
    fn latest_native_recapture_rejects_stale_layout_and_isolates_owner() {
        let ctx = egui::Context::default();
        let mut s = SplitScroll::default();
        s.session(Some((1, 1)), 0, true, 1, false);
        s.layout(Pane::Source, 400., 15., true);
        s.layout(Pane::Preview, 400., 15., true);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400., 300.));
                let observe =
                    |s: &mut SplitScroll, pane, offset, next, valid, internal, revision, byte| {
                        s.observe(
                            pane,
                            ui,
                            egui::Id::new(pane.index()),
                            rect,
                            offset,
                            next,
                            500.,
                            vec![],
                            "",
                            revision,
                            valid,
                            internal,
                            Anchor {
                                byte,
                                fraction: 0.,
                                viewport_y: 0.,
                                text: false,
                            },
                        )
                    };
                s.intent = Some(Pane::Source);
                observe(&mut s, Pane::Source, 0., 40., true, false, 0, 1);
                let first = s.sequence;
                assert_eq!(
                    s.anchor_sequence, None,
                    "new pending input cannot tag the old drawn anchor as confirmed"
                );
                assert_eq!(s.pending_capture, Some((Pane::Source, first, 40.)));
                s.intent = None;
                observe(&mut s, Pane::Preview, 0., 0., true, true, 0, 2);
                assert_eq!(
                    s.pending_capture,
                    Some((Pane::Source, first, 40.)),
                    "follower compensation cannot consume owner pending"
                );
                observe(&mut s, Pane::Source, 40., 40., false, false, 1, 3);
                assert_eq!(
                    s.pending_capture,
                    Some((Pane::Source, first, 40.)),
                    "stale geometry cannot consume capture"
                );
                s.intent = Some(Pane::Preview);
                observe(&mut s, Pane::Preview, 0., 80., true, false, 0, 4);
                let latest = s.sequence;
                assert!(latest > first);
                assert_eq!(s.pending_capture, Some((Pane::Preview, latest, 80.)));
                assert_ne!(
                    s.anchor_sequence,
                    Some(latest),
                    "pending preview's future viewport is not painted yet"
                );
                s.intent = None;
                observe(&mut s, Pane::Source, 40., 40., true, false, 0, 5);
                assert_eq!(
                    s.pending_capture,
                    Some((Pane::Preview, latest, 80.)),
                    "old pane cannot replay pending"
                );
                observe(&mut s, Pane::Preview, 80., 80., true, false, 0, 7);
                assert!(s.pending_capture.is_none());
                assert_eq!(s.confirmed_native, Some((Pane::Preview, latest)));
                assert_eq!(s.anchor_sequence, Some(latest));
                assert_eq!(
                    s.anchor.unwrap().byte,
                    7,
                    "actual new painted viewport recaptured once"
                );
            });
        });
        s.pending_capture = Some((Pane::Source, s.sequence, 40.));
        s.layout(Pane::Preview, 420., 15., true);
        assert!(s.pending_capture.is_some());
        s.layout(Pane::Source, 420., 15., true);
        assert!(s.pending_capture.is_none());
        s.pending_capture = Some((Pane::Source, s.sequence, 40.));
        s.session(Some((1, 1)), 1, true, 1, false);
        assert!(s.pending_capture.is_none());
        s.pending_capture = Some((Pane::Source, s.sequence, 40.));
        s.navigating();
        assert!(s.pending_capture.is_none());
    }
    #[test]
    fn measured_cache_rejects_local_reflow_with_equal_total_height() {
        let mut s = SplitScroll::default();
        s.session(Some((1, 1)), 0, true, 1, false);
        s.geometry_stamp(Pane::Preview, 11);
        s.views[1].rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(300., 300.),
        ));
        s.views[1].revision = Some(0);
        s.views[1].max = 500.;
        s.recent_positions[1].push_back(MeasuredAnchor {
            requested: 3,
            resolved: 3,
            content_y: 240.,
        });
        let anchor = Anchor {
            byte: 3,
            fraction: 0.,
            viewport_y: 20.,
            text: true,
        };
        assert_eq!(s.precise(Pane::Preview, anchor, "abcx", 0), Some(220.));
        s.geometry_stamp(Pane::Preview, 12);
        assert_eq!(s.views[1].max, 500., "total height unchanged");
        assert!(s.recent_positions[1].is_empty());
        assert!(
            s.precise(Pane::Preview, anchor, "abcx", 0).is_none(),
            "local height stamp must reject old exact Y even if total cancels"
        );
    }
}
