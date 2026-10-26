// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! User-facing search, preview, history and a second editable document pane.
use crate::{
    Action, Document, Editor, Mode, egui,
    search::{self, Command, Output},
};
use egui::{Color32, FontId, RichText, Vec2};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
fn error_text(language: crate::localization::Language, error: &str) -> String {
    for key in [
        "Save or close the second document before opening another.",
        "This document is already open in the main pane.",
        "Stale search result: refresh the search",
        "Stale diff: an open document changed; refresh the preview",
    ] {
        if key == error {
            return language.text(key).to_owned();
        }
    }
    error.to_owned()
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    #[test]
    fn snapshot_escape_closes_only_top_window_and_returns_to_find_query() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "# Main\n\nselection-token");
        app.mode = Mode::Edit;
        let mut frame = eframe::Frame::_new_kittest();
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280., 800.),
            )),
            ..Default::default()
        };
        let source = egui::Id::new(("source-editor", app.document_generation));
        let _ = ctx.run(input(), |ctx| {
            ctx.memory_mut(|m| m.request_focus(source));
            eframe::App::update(&mut app, ctx, &mut frame);
        });
        app.open_global_search(&ctx, true);
        for _ in 0..3 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(egui::Id::new("global-query"))
        );
        app.global.history_open = true;
        for _ in 0..3 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        let escape = || {
            let mut raw = input();
            raw.events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
            raw
        };
        let _ = ctx.run(escape(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame)
        });
        for _ in 0..4 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        assert!(
            !app.global.history_open && app.global.open,
            "Escape closed the parent Find window"
        );
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(egui::Id::new("global-query")),
            "History close did not return to its parent query"
        );
        let _ = ctx.run(escape(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame)
        });
        for _ in 0..4 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        assert!(!app.global.windows_open());
        assert_eq!(ctx.memory(|m| m.focused()), Some(source));
    }

    #[test]
    fn cancelled_dirty_quick_navigation_keeps_original_focus_bookmark() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("main.md");
        let target = dir.path().join("target.md");
        std::fs::write(&original, "# Main\n\noriginal source").unwrap();
        std::fs::write(&target, "# Target\n\nother source").unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, "");
        app.doc = Document::open(&original).unwrap();
        app.doc.text = "# Main\n\nunsaved selection-token".into();
        app.doc.mark_text_changed();
        app.mode = Mode::Edit;
        let source = egui::Id::new(("source-editor", app.document_generation));
        let mut frame = eframe::Frame::_new_kittest();
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280., 800.),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input(), |ctx| {
            ctx.memory_mut(|m| m.request_focus(source));
            eframe::App::update(&mut app, ctx, &mut frame);
        });
        app.open_search(&ctx);
        for _ in 0..3 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        app.search_open_target(target, None, false, &ctx);
        assert!(app.pending.is_some());
        let _ = ctx.run(input(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame)
        });
        let escape = || {
            let mut raw = input();
            raw.events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
            raw
        };
        let _ = ctx.run(escape(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame)
        });
        for _ in 0..4 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        assert!(app.pending.is_none());
        assert!(
            app.global.quick_open,
            "Cancel must leave the quick switcher open"
        );
        let _ = ctx.run(escape(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame)
        });
        for _ in 0..4 {
            let _ = ctx.run(input(), |ctx| {
                eframe::App::update(&mut app, ctx, &mut frame)
            });
        }
        assert!(!app.global.quick_open);
        assert_eq!(ctx.memory(|m| m.focused()), Some(source));
        assert_eq!(app.doc.text, "# Main\n\nunsaved selection-token");
        assert_eq!(
            std::fs::read_to_string(original).unwrap(),
            "# Main\n\noriginal source"
        );
    }

    #[test]
    fn quick_navigation_keeps_focus_on_the_new_primary_or_secondary_target() {
        for secondary in [false, true] {
            let ctx = egui::Context::default();
            let dir = tempfile::tempdir().unwrap();
            let original = dir.path().join("main.md");
            let target = dir.path().join("target.md");
            std::fs::write(&original, "# Main\n\noriginal source").unwrap();
            std::fs::write(&target, "# Target\n\nnew selection destination").unwrap();
            // Document::open canonicalizes regular paths (Windows adds \\?\).
            let target = std::fs::canonicalize(target).unwrap();
            let mut app = crate::preview_layout_tests::editor(&ctx, "# Main\n\noriginal source");
            app.doc = Document::open(&original).unwrap();
            app.mode = Mode::Edit;
            let mut frame = eframe::Frame::_new_kittest();
            let input = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280., 800.),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input(), |ctx| {
                ctx.memory_mut(|m| {
                    m.request_focus(egui::Id::new(("source-editor", app.document_generation)))
                });
                eframe::App::update(&mut app, ctx, &mut frame);
            });
            app.open_search(&ctx);
            for _ in 0..3 {
                let _ = ctx.run(input(), |ctx| {
                    eframe::App::update(&mut app, ctx, &mut frame)
                });
            }
            app.search_open_target(target.clone(), None, secondary, &ctx);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let _ = ctx.run(input(), |ctx| {
                    eframe::App::update(&mut app, ctx, &mut frame)
                });
                let loaded = if secondary {
                    app.global
                        .secondary
                        .as_ref()
                        .is_some_and(|d| d.path.as_ref() == Some(&target))
                } else {
                    app.doc.path.as_ref() == Some(&target)
                };
                if loaded && !app.global.quick_open && !app.global.busy {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "Quick navigation did not load target"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            for _ in 0..4 {
                let _ = ctx.run(input(), |ctx| {
                    eframe::App::update(&mut app, ctx, &mut frame)
                });
            }
            let wanted = if secondary {
                egui::Id::new(("second-editor", app.global.secondary_generation))
            } else {
                egui::Id::new(("source-editor", app.document_generation))
            };
            assert_eq!(
                ctx.memory(|m| m.focused()),
                Some(wanted),
                "Old cancellation bookmark stole navigation focus: secondary={secondary}"
            );
            assert_eq!(
                std::fs::read_to_string(&original).unwrap(),
                "# Main\n\noriginal source"
            );
        }
    }

    #[test]
    fn truncated_results_have_a_visible_warning_in_the_search_window() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            crate::preview_layout_tests::editor(&ctx, &"x ".repeat(search::MAX_RESULTS + 1));
        let index = search::Index::new(dir.path()).unwrap();
        let guard = search::Guard::new(
            Arc::new(std::sync::atomic::AtomicU64::new(1)),
            1,
            Duration::from_secs(5),
        );
        app.global.options.query = "x".into();
        app.global.single = true;
        app.global.results = index
            .search(&app.global.options, &[app.doc.clone()], true, &guard)
            .unwrap();
        app.global.open = true;
        // Let egui settle the window's first-pass placement before inspecting paint.
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| app.full_search_ui(ctx));
        }
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600., 1100.),
                )),
                ..Default::default()
            },
            |ctx| app.full_search_ui(ctx),
        );
        assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == "Partial results · match limit reached")));
    }

    #[test]
    fn current_document_replace_tracks_the_active_secondary_pane_and_its_history() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.md");
        let second = dir.path().join("second.md");
        std::fs::write(&main, "cat-main").unwrap();
        std::fs::write(&second, "cat-secondary").unwrap();
        let mut app = crate::preview_layout_tests::editor(&ctx, "cat-main");
        app.doc = Document::open(&main).unwrap();
        app.global.secondary = Some(Document::open(&second).unwrap());
        app.last_secondary = true;
        app.open_global_search(&ctx, true);
        let options = search::Options {
            query: "cat".into(),
            replacement: "dog".into(),
            ..Default::default()
        };
        let index = search::Index::new(dir.path()).unwrap();
        let guard = search::Guard::new(
            Arc::new(std::sync::atomic::AtomicU64::new(1)),
            1,
            Duration::from_secs(5),
        );
        let preview = index
            .preview(&options, &app.search_query_overlays(), true, &guard)
            .unwrap();
        assert_eq!(
            preview.files[0].doc.path,
            app.global.secondary.as_ref().unwrap().path
        );
        app.global.preview = Some(preview);
        app.global.preview_generation = app.document_generation;
        app.global.preview_secondary_generation = app.global.secondary_generation;
        app.apply_search_preview(&ctx);
        assert_eq!(app.doc.text, "cat-main");
        assert!(!app.doc.dirty());
        assert_eq!(app.global.secondary.as_ref().unwrap().text, "dog-secondary");
        assert!(app.global.secondary.as_ref().unwrap().dirty());
        assert_eq!(std::fs::read_to_string(&main).unwrap(), "cat-main");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "cat-secondary");
        let id = egui::Id::new(("second-editor", app.global.secondary_identity()));
        let state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        let current = (
            state.cursor.char_range().unwrap(),
            "dog-secondary".to_owned(),
        );
        let mut undoer = state.undoer();
        let undo = undoer.undo(&current).unwrap().clone();
        assert_eq!(undo.1, "cat-secondary");
        assert_eq!(undoer.redo(&undo).unwrap().1, "dog-secondary");
    }
}

pub struct Jump {
    pub path: Option<PathBuf>,
    pub start: usize,
    pub end: usize,
    pub expected: String,
}
#[derive(Default)]
pub struct SearchUi {
    pub worker: Option<search::Worker>,
    pub open: bool,
    pub quick_open: bool,
    pub focus: bool,
    pub changed: Option<Instant>,
    pub busy: bool,
    pub single: bool,
    single_secondary: bool,
    pub secondary: Option<Document>,
    pub secondary_large: crate::large_document::LargeDocument,
    pub jump: Option<Jump>,
    secondary_jump: Option<(usize, usize)>,
    pending_hit: Option<search::Hit>,
    open_secondary: bool,
    options: search::Options,
    query: String,
    selected: usize,
    quick: Vec<search::QuickItem>,
    results: search::Results,
    preview: Option<search::Preview>,
    histories: Vec<search::HistoryItem>,
    history_open: bool,
    error: Option<String>,
    message: Option<String>,
    status: search::IndexStatus,
    generation: u64,
    roundtrip: f64,
    queue_ms: f64,
    processing_ms: f64,
    poll_ms: f64,
    shown: usize,
    pub(crate) secondary_close: bool,
    pub(crate) secondary_save_close: bool,
    pub(crate) secondary_generation: u64,
    preview_generation: u64,
    preview_secondary_generation: u64,
    waiting: bool,
    input_at: Option<Instant>,
    request_input_at: Option<Instant>,
    visible_at: Option<Instant>,
    visible_ms: f64,
    controls: Vec<serde_json::Value>,
    reporter: Option<std::sync::mpsc::SyncSender<Vec<u8>>>,
}
impl SearchUi {
    pub(crate) fn invalidate_open(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.generation = u64::MAX;
        self.waiting = false;
        self.busy = false;
    }
    pub fn workspace_changed(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.worker = None;
        self.status = Default::default();
        self.results = Default::default();
        self.preview = None;
        self.quick.clear();
        self.histories.clear();
        self.history_open = false;
        self.error = None;
        self.message = None;
        self.waiting = false;
        self.generation = u64::MAX;
        self.changed = Some(Instant::now());
    }
    pub fn history_is_open(&self) -> bool {
        self.history_open
    }
    pub fn windows_open(&self) -> bool {
        self.quick_open || self.open || self.history_open
    }
    pub fn begin_frame(&mut self) {
        self.controls.clear();
    }
    pub fn secondary_focused(&self, ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.has_focus(egui::Id::new(("second-editor", self.secondary_generation))))
    }
    pub fn secondary_identity(&self) -> u64 {
        self.secondary_generation
    }
    pub fn buffer_changed(&mut self) {
        if self.open && self.preview.is_none() {
            self.changed = Some(Instant::now());
        }
    }
}

impl Editor {
    fn ensure_search_worker(&mut self, ctx: &egui::Context) {
        if self.global.worker.is_none() {
            let repaint = ctx.clone();
            self.global.worker = Some(search::Worker::start(
                self.files.root.clone(),
                Arc::new(move || repaint.request_repaint()),
            ));
        }
    }
    pub fn open_global_search(&mut self, ctx: &egui::Context, single: bool) {
        if !self.flush_rich_edits(ctx) {
            return;
        }
        self.ensure_search_worker(ctx);
        self.global.open = true;
        self.global.quick_open = false;
        self.global.single = single;
        // Capture the pane before the search field takes focus. Changing the
        // search scope later keeps this same pane as "Current document".
        self.global.single_secondary = self.global.secondary.is_some()
            && (self.global.secondary_focused(ctx) || self.last_secondary);
        self.global.focus = true;
        self.global.changed = Some(Instant::now());
        self.global.preview = None;
        self.global.error = None;
    }
    fn search_overlays(&self) -> Vec<Document> {
        let mut docs = vec![self.doc.clone()];
        if let Some(doc) = &self.global.secondary {
            docs.push(doc.clone());
        }
        docs
    }
    fn search_query_overlays(&self) -> Vec<Document> {
        if self.global.single && self.global.single_secondary {
            self.global.secondary.iter().cloned().collect()
        } else {
            self.search_overlays()
        }
    }
    fn search_send(&mut self, command: Command) {
        self.global.waiting = matches!(
            command,
            Command::Query { .. } | Command::Quick { .. } | Command::Preview { .. }
        );
        if matches!(command, Command::Query { .. } | Command::Quick { .. }) {
            self.global.request_input_at =
                self.global.input_at.take().or_else(|| Some(Instant::now()));
        }
        if let Some(worker) = &self.global.worker {
            self.global.generation = worker.send(command);
        }
    }
    pub fn dispatch_search_open(&mut self, path: PathBuf, ctx: &egui::Context) {
        if !self.flush_rich_edits(ctx) {
            return;
        }
        self.ensure_search_worker(ctx);
        self.reading.flush();
        self.global.busy = true;
        self.search_send(Command::Open(path));
    }
    fn search_open_target(
        &mut self,
        path: PathBuf,
        hit: Option<search::Hit>,
        secondary: bool,
        ctx: &egui::Context,
    ) {
        self.global.pending_hit = hit;
        if path == self.doc.path.clone().unwrap_or_default() && !secondary {
            if self
                .global
                .pending_hit
                .as_ref()
                .is_some_and(|h| h.stamp != search::text_stamp(&self.doc.text))
            {
                self.global.error = Some("Stale search result: refresh the search".into());
                self.global.open = true;
                self.global.changed = Some(Instant::now());
                return;
            }
            if let Some(hit) = self.global.pending_hit.take() {
                self.mode = Mode::Edit;
                self.global.jump = Some(Jump {
                    path: self.doc.path.clone(),
                    start: hit.start,
                    end: hit.end,
                    expected: self.doc.text.clone(),
                });
            }
            crate::theme::forget_window_focus(ctx, "search");
            self.editor_focus = true;
            self.global.quick_open = false;
            return;
        }
        if self
            .global
            .secondary
            .as_ref()
            .is_some_and(|d| d.path.as_ref() == Some(&path))
        {
            if self.global.pending_hit.as_ref().is_some_and(|h| {
                Some(h.stamp)
                    != self
                        .global
                        .secondary
                        .as_ref()
                        .map(|d| search::text_stamp(&d.text))
            }) {
                self.global.error = Some("Stale search result: refresh the search".into());
                self.global.open = true;
                self.global.changed = Some(Instant::now());
                return;
            }
            if let Some(hit) = self.global.pending_hit.take() {
                self.global.secondary_jump = Some((hit.start, hit.end));
            }
            crate::theme::forget_window_focus(ctx, "search");
            self.last_secondary = true;
            ctx.memory_mut(|m| {
                m.request_focus(egui::Id::new((
                    "second-editor",
                    self.global.secondary_generation,
                )))
            });
            ctx.request_repaint();
            self.global.quick_open = false;
            return;
        }
        self.global.open_secondary = secondary;
        if secondary {
            if !self.saves.quiesce(true)
                || self.global.secondary.as_ref().is_some_and(|d| d.dirty())
            {
                self.global.error =
                    Some("Save or close the second document before opening another.".into());
                return;
            }
            if self.doc.path.as_ref() == Some(&path) {
                self.global.error = Some("This document is already open in the main pane.".into());
                return;
            }
            self.dispatch_search_open(path, ctx);
        } else {
            self.request(Action::SearchPath(path), ctx);
        }
    }
    fn update_committed_documents(&mut self, ctx: &egui::Context, documents: Vec<Document>) {
        for next in documents {
            if next.path == self.doc.path {
                let id = self.doc.id;
                self.apply_rich_source(ctx, next.text.clone());
                let revision = self.doc.revision();
                self.doc = next;
                self.doc.id = id;
                self.doc.continue_revision(revision);
                self.cache = Default::default();
            } else if let Some(doc) = &mut self.global.secondary
                && next.path == doc.path
            {
                let id = doc.id;
                let revision = doc.revision();
                *doc = next;
                doc.id = id;
                doc.continue_revision(revision);
                self.global.secondary_generation += 1;
                self.reading.rebind_after_commit(
                    doc,
                    "secondary",
                    self.global.secondary_generation,
                );
            }
        }
    }
    pub fn poll_global_search(&mut self, ctx: &egui::Context) {
        let mut responses = Vec::new();
        if let Some(worker) = &self.global.worker {
            while let Some(response) = worker.poll() {
                responses.push(response);
            }
        }
        for response in responses {
            if response.generation != 0 && response.generation != self.global.generation {
                continue;
            }
            if response.generation != 0 {
                self.global.waiting = false;
            }
            if response.generation != 0 {
                self.global.roundtrip = response.roundtrip_ms;
                self.global.queue_ms = response.queue_ms;
                self.global.processing_ms = response.processing_ms;
                self.global.poll_ms = response.completed_at.elapsed().as_secs_f64() * 1000.;
            }
            match response.output {
                Output::Status(status) => {
                    let changed = !self.global.status.ready || status.changed > 0;
                    self.global.status = status;
                    if changed {
                        self.global.changed = Some(Instant::now());
                    }
                }
                Output::Results(results) => {
                    self.global.results = results;
                    self.global.error = None;
                    self.global.shown = 100;
                    self.global.visible_at = self.global.request_input_at.take();
                }
                Output::Preview(preview) => {
                    self.global.preview = Some(preview);
                    self.global.error = None;
                }
                Output::Quick(items) => {
                    self.global.quick = items;
                    self.global.selected = 0;
                    self.global.error = None;
                    self.global.visible_at = self.global.request_input_at.take();
                }
                Output::Error(error) => {
                    self.global.error = Some(error);
                    self.global.busy = false;
                }
                Output::Committed(documents, id) => {
                    self.update_committed_documents(ctx, documents);
                    self.global.busy = false;
                    self.global.preview = None;
                    self.global.message =
                        Some(format!("{} {id}", self.language.text("Saved snapshot:")));
                    self.global.changed = Some(Instant::now());
                }
                Output::RolledBack(documents) => {
                    self.update_committed_documents(ctx, documents);
                    self.global.busy = false;
                    self.global.preview = None;
                    self.global.histories.clear();
                    self.global.message = Some(self.language.text("Snapshot restored.").into());
                    self.global.changed = Some(Instant::now());
                }
                Output::Created(path) => {
                    self.global.busy = false;
                    self.global.open_secondary = false;
                    self.request(Action::SearchPath(path), ctx);
                }
                Output::Opened(doc) => {
                    self.global.busy = false;
                    if self
                        .global
                        .pending_hit
                        .as_ref()
                        .is_some_and(|h| h.stamp != search::text_stamp(&doc.text))
                    {
                        self.global.error = Some("Stale search result: refresh the search".into());
                        self.global.open = true;
                        self.global.changed = Some(Instant::now());
                        continue;
                    }
                    crate::theme::forget_window_focus(ctx, "search");
                    let path = doc.path.clone();
                    if let Some(path) = &path {
                        self.recent.retain(|p| p != path);
                        self.recent.insert(0, path.clone());
                        self.recent.truncate(50);
                    }
                    if self.global.open_secondary {
                        self.commit_document(doc, true);
                        self.last_secondary = true;
                        ctx.memory_mut(|m| {
                            m.request_focus(egui::Id::new((
                                "second-editor",
                                self.global.secondary_generation,
                            )))
                        });
                        self.global.secondary_jump =
                            self.global.pending_hit.take().map(|h| (h.start, h.end));
                    } else {
                        let navigation_hit = self.global.pending_hit.is_some();
                        self.commit_document(doc, false);
                        self.cache = Default::default();
                        self.editor_focus = true;
                        if let Some(hit) = self.global.pending_hit.take()
                            && hit.end <= self.doc.text.len()
                            && self.doc.text.is_char_boundary(hit.start)
                            && self.doc.text.is_char_boundary(hit.end)
                        {
                            self.global.jump = Some(Jump {
                                path,
                                start: hit.start,
                                end: hit.end,
                                expected: self.doc.text.clone(),
                            });
                        }
                        if navigation_hit {
                            self.mode = Mode::Edit;
                        }
                        self.sync_file_root();
                    }
                    self.global.quick_open = false;
                }
                Output::History(items) => {
                    self.global.histories = items;
                    self.global.history_open = true;
                }
            }
        }
        self.dispatch_pending_query(ctx);
    }
    fn dispatch_pending_query(&mut self, ctx: &egui::Context) {
        // Reuse the fixed global root even after a nested document is opened.
        if (self.global.open || self.global.quick_open)
            && !self.global.busy
            && self.global.changed.is_some()
        {
            if self.global.status.ready {
                self.global.changed = None;
                if self.global.quick_open {
                    self.search_send(Command::Quick {
                        query: self.global.query.clone(),
                        recent: self.recent.clone(),
                    });
                } else {
                    self.search_send(Command::Query {
                        options: self.global.options.clone(),
                        overlays: self.search_query_overlays(),
                        single: self.global.single,
                    });
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(8));
            }
        }
    }
    pub fn global_search_windows(&mut self, ctx: &egui::Context) {
        if self.global.quick_open || self.global.open {
            self.ensure_search_worker(ctx);
        }
        if self.global.busy || self.pending.is_some() || self.error.is_some() {
            self.search_report();
            return;
        }
        if self.global.quick_open {
            self.quick_switcher_ui(ctx);
        }
        if self.global.open {
            self.full_search_ui(ctx);
        }
        if self.global.history_open {
            self.search_history_ui(ctx);
        }
        // Input is already coalesced by egui into one frame. Dispatch the newest
        // query in this same frame instead of adding another frame of debounce.
        self.dispatch_pending_query(ctx);
        if let Some(input) = self.global.visible_at.take() {
            self.global.visible_ms = input.elapsed().as_secs_f64() * 1000.;
        }
        self.search_report();
    }
    fn track_search(&mut self, key: &str, response: &egui::Response) {
        self.global.controls.push(serde_json::json!({"name":key,"rect":[response.rect.min.x,response.rect.min.y,response.rect.max.x,response.rect.max.y]}));
    }
    fn quick_switcher_ui(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let mut open = true;
        let mut chosen = None;
        let down = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        let split = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter));
        let enter =
            split || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let response = egui::Window::new(crate::theme::window_title(
            language.text("Quick Switcher · Ctrl+P"),
        ))
        .id(egui::Id::new("quick-switcher"))
        .open(&mut open)
        .collapsible(false)
        .default_width(650.)
        .min_width(260.)
        .max_size(crate::theme::window_bounds(ctx).size() - Vec2::new(16., 32.))
        .constrain_to(crate::theme::window_bounds(ctx))
        .vscroll(true)
        .default_pos(ctx.content_rect().center() - Vec2::new(325., 240.))
        .show(ctx, |ui| {
            let r = ui.add_sized(
                [ui.available_width(), 28.],
                egui::TextEdit::singleline(&mut self.global.query)
                    .id(egui::Id::new("quick-query"))
                    .hint_text(language.text("Filename, path, heading or alias…")),
            );
            self.track_search("quick-query", &r);
            if self.global.focus {
                r.request_focus();
                self.global.focus = false;
            }
            if r.changed() {
                if let Some(w) = &self.global.worker {
                    w.cancel();
                }
                self.global.generation = u64::MAX;
                self.global.waiting = false;
                self.global.changed = Some(Instant::now());
                self.global.input_at = self.global.changed;
                self.global.quick.clear();
                self.global.selected = 0;
            }
            ui.label(
                language.text("↑ ↓ select · Enter open · Shift+Enter open beside · Esc close"),
            );
            if !self.global.status.ready {
                ui.spinner();
                ui.label(language.text("Indexing text files…"));
            }
            if down {
                self.global.selected =
                    (self.global.selected + 1).min(self.global.quick.len().saturating_sub(1));
            }
            if up {
                self.global.selected = self.global.selected.saturating_sub(1);
            }
            egui::ScrollArea::vertical()
                .id_salt("quick-results")
                .max_height(320.)
                .show_rows(ui, 29., self.global.quick.len(), |ui, range| {
                    for row in range {
                        let item = &self.global.quick[row];
                        let label = if item.title.is_empty() {
                            item.label.clone()
                        } else {
                            format!("{}  ·  {}", item.label, item.title)
                        };
                        let path = item.path.clone();
                        let r = ui.add_sized(
                            [ui.available_width(), 29.],
                            egui::Button::new(label).selected(self.global.selected == row),
                        );
                        self.track_search(&format!("quick-result-{row}"), &r);
                        if self.global.selected == row && (up || down) {
                            r.scroll_to_me(None);
                        }
                        if r.clicked() {
                            chosen = Some((path, false));
                        }
                    }
                });
            if enter
                && !self.global.waiting
                && self.global.changed.is_none()
                && let Some(item) = self.global.quick.get(self.global.selected)
            {
                chosen = Some((item.path.clone(), split));
            }
            if self.global.status.ready
                && !self.global.waiting
                && self.global.quick.is_empty()
                && !self.global.query.trim().is_empty()
                && self.global.changed.is_none()
            {
                let r = ui.button(format!(
                    "{} {}",
                    language.text("Create Markdown document:"),
                    self.global.query.trim()
                ));
                self.track_search("quick-create", &r);
                if r.clicked() || enter {
                    let mut relative = PathBuf::from(self.global.query.trim());
                    if relative.extension().is_none() {
                        relative.set_extension("md");
                    }
                    self.global.busy = true;
                    self.search_send(Command::Create(relative));
                }
            }
            if let Some(error) = &self.global.error {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    format!(
                        "{} {}",
                        language.text("Search error:"),
                        error_text(language, error)
                    ),
                );
            }
        });
        if response
            .as_ref()
            .is_some_and(|r| crate::theme::window_escape(ctx, r.response.layer_id))
        {
            open = false;
        }
        self.global.quick_open = open;
        if let Some((path, split)) = chosen {
            self.search_open_target(path, None, split, ctx);
        }
    }
    fn full_search_ui(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let mut open = true;
        let mut navigate = None;
        let mut preview = false;
        let mut apply = false;
        let mut history = false;
        let response=egui::Window::new(crate::theme::window_title(language.text("Find and replace"))).id(egui::Id::new("global-search"))
            .open(&mut open).collapsible(false).resizable(true).default_width(760.).default_height(610.)
            .min_width(260.).max_size(crate::theme::window_bounds(ctx).size()-Vec2::new(16.,32.))
            .constrain_to(crate::theme::window_bounds(ctx)).vscroll(true)
            .default_pos(crate::theme::window_bounds(ctx).center()-Vec2::new(380.,300.)).show(ctx,|ui|{
                let old=self.global.options.clone();let old_single=self.global.single;
                ui.horizontal_wrapped(|ui|{
                    ui.selectable_value(&mut self.global.single,true,language.text("Current document"));
                    ui.selectable_value(&mut self.global.single,false,language.text("Workspace"));
                    let r=ui.small_button(language.text("Snapshots"));self.track_search("history",&r);history=r.clicked();
                });
                if let Some(root)=self.global.worker.as_ref().map(|w|w.root.display().to_string()) {
                        let name=std::path::Path::new(&root).file_name().unwrap_or_default().to_string_lossy();
                        ui.add(egui::Label::new(RichText::new(name).size(12.0).strong()).truncate()).on_hover_text(root);
                        if ui.small_button(language.text("Use current folder")).clicked(){
                            let repaint=ctx.clone();self.global.worker=Some(search::Worker::start(self.files.root.clone(),Arc::new(move||repaint.request_repaint())));self.global.status=Default::default();self.global.changed=Some(Instant::now());self.global.preview=None;
                        }
                }
                ui.horizontal(|ui|{
                    let r=ui.add_sized([(ui.available_width()-142.).max(80.),27.],egui::TextEdit::singleline(&mut self.global.options.query).id(egui::Id::new("global-query")).hint_text(language.text("Find…")));
                    self.track_search("find-query",&r);if self.global.focus{r.request_focus();self.global.focus=false;}
                    let r=ui.toggle_value(&mut self.global.options.case,"Aa").on_hover_text(language.text("Match case"));self.track_search("case",&r);
                    let r=ui.toggle_value(&mut self.global.options.word,"ab").on_hover_text(language.text("Whole word"));self.track_search("word",&r);
                    let r=ui.toggle_value(&mut self.global.options.regex,".*").on_hover_text(language.text("Regular expression"));self.track_search("regex",&r);
                });
                ui.horizontal(|ui|{
                    ui.label(language.text("Replace with"));let r=ui.add_sized([ui.available_width(),40.],egui::TextEdit::multiline(&mut self.global.options.replacement).desired_rows(2).id(egui::Id::new("replace-query")).hint_text("$1 · $2 · $<name>"));self.track_search("replace-query",&r);
                });
                if !self.global.single {
                    ui.horizontal(|ui|{ui.label(language.text("Include glob"));let r=ui.text_edit_singleline(&mut self.global.options.include);self.track_search("include",&r);});
                    ui.horizontal(|ui|{ui.label(language.text("Exclude glob"));let r=ui.text_edit_singleline(&mut self.global.options.exclude);self.track_search("exclude",&r);});
                    ui.label(RichText::new(language.text("Separate globs with ; · **/*.md · notes/**")).small());
                }
                if old!=self.global.options||old_single!=self.global.single {
                    if let Some(worker)=&self.global.worker{worker.cancel();}self.global.generation=u64::MAX;self.global.waiting=false;
                    self.global.changed=Some(Instant::now());self.global.preview=None;self.global.error=None;self.global.results=Default::default();
                    self.global.input_at=self.global.changed;
                }
                ui.horizontal_wrapped(|ui|{
                    let r=ui.button(language.text("Preview changes"));self.track_search("preview",&r);preview=r.clicked();
                    if ui.button(language.text("Refresh")).clicked(){self.search_send(Command::Refresh);self.global.changed=Some(Instant::now());self.global.preview=None;}
                    if ui.button(language.text("Cancel search")).clicked(){if let Some(w)=&self.global.worker{w.cancel();}self.global.changed=None;}
                    if !self.global.status.ready {ui.spinner();}
                    ui.label(format!("{} · {:.1} ms",self.global.status.files,self.global.roundtrip)).on_hover_text(format!("{} bytes · {:.1} ms",self.global.status.bytes,self.global.results.elapsed_ms));
                });
                if self.global.status.limited||self.global.status.errors>0 {ui.colored_label(self.palette.accent,format!("{} ({})",language.text("Partial index · scan limit reached"),self.global.status.errors));}
                if self.global.results.limited {ui.colored_label(self.palette.accent,language.text("Partial results · match limit reached"));}
                if let Some(error)=&self.global.error {ui.colored_label(Color32::from_rgb(210,65,70),format!("{} {}",language.text("Search error:"),error_text(language,error)));}
                if let Some(message)=&self.global.message {ui.label(message);}
                ui.separator();
                if let Some(diff)=&mut self.global.preview {
                    if diff.options!=self.global.options {self.global.error=Some("Stale diff: an open document changed; refresh the preview".into());}
                    ui.label(language.text("Review the diff before applying. Workspace replacement saves selected dirty buffers."));
                    ui.horizontal_wrapped(|ui|{
                        let r=ui.button(language.text("Apply selected"));
                        self.global.controls.push(serde_json::json!({"name":"apply","rect":[r.rect.min.x,r.rect.min.y,r.rect.max.x,r.rect.max.y]}));
                        apply=r.clicked();
                        if ui.button(language.text("Select all")).clicked(){for f in &mut diff.files{for e in &mut f.edits{e.selected=true;}}}
                        if ui.button(language.text("Select none")).clicked(){for f in &mut diff.files{for e in &mut f.edits{e.selected=false;}}}
                    });
                    let mut apply_file=None;let mut apply_one=None;
                    egui::ScrollArea::vertical().id_salt("diff-results").max_height(360.).show(ui,|ui|{
                        for (n,file) in diff.files.iter_mut().enumerate() {
                            egui::CollapsingHeader::new(format!("{} · {}",file.relative.display(),file.edits.len())).id_salt(("diff-file",n)).default_open(n==0).show(ui,|ui|{
                                if ui.small_button(language.text("Replace this file")).clicked(){apply_file=Some(n);}
                                for (i,e) in file.edits.iter_mut().enumerate().take(200) {
                                    let h=search::hit(&file.doc.text,e.start,e.end);
                                    ui.horizontal(|ui|{
                                        ui.checkbox(&mut e.selected,format!("{} {}",language.text("Line"),h.line));
                                        if ui.small_button(language.text("Replace this match")).clicked(){apply_one=Some((n,i));}
                                    });
                                    let context_start=e.start-h.highlight[0];
                                    let context_end=file.doc.text[e.end..].find('\n').map_or(file.doc.text.len(),|n|e.end+n).min((context_start+h.snippet.len()).max(e.end));
                    ui.colored_label(ui.visuals().error_fg_color,format!("− {}",&file.doc.text[context_start..context_end]));
                                    let prefix=&file.doc.text[context_start..e.start];
                                    let suffix=&file.doc.text[e.end..context_end];
                                    ui.colored_label(if self.palette.dark{Color32::from_rgb(118,210,161)}else{Color32::from_rgb(27,113,68)},format!("+ {prefix}{}{suffix}",e.replacement));
                                }
                                if file.edits.len()>200 {ui.label(language.text("Only the first 200 matches are shown; use filters to narrow the preview."));}
                            });
                        }
                    });
                    if let Some(n)=apply_file {for (f,file) in diff.files.iter_mut().enumerate(){for e in &mut file.edits{e.selected=f==n;}}apply=true;}
                    if let Some((n,i))=apply_one {for (f,file) in diff.files.iter_mut().enumerate(){for (m,e) in file.edits.iter_mut().enumerate(){e.selected=f==n&&m==i;}}apply=true;}
                } else {
                    ui.label(format!("{} {}",self.global.results.count,language.text("matches")));
                    egui::ScrollArea::vertical().id_salt("global-results").max_height(350.).show(ui,|ui|{
                        for (g,group) in self.global.results.groups.iter().take(self.global.shown.max(100)).enumerate() {
                            egui::CollapsingHeader::new(format!("{} ({})",group.relative,group.hits.len())).id_salt(("result-group",g)).default_open(g<5).show(ui,|ui|{
                                for (n,h) in group.hits.iter().enumerate().take(200) {
                                    let mut job=egui::text::LayoutJob::default();
                                    job.append(&format!("{}: ",h.line),0.,egui::TextFormat{color:self.palette.muted,..Default::default()});
                                    let [a,b]=h.highlight;
                                    for (s,highlight) in [(&h.snippet[..a],false),(&h.snippet[a..b],true),(&h.snippet[b..],false)] {
                                        job.append(s,0.,egui::TextFormat{font_id:FontId::monospace(12.),color:self.palette.ink,background:if highlight{self.palette.selected}else{Color32::TRANSPARENT},..Default::default()});
                                    }
                                    let r=ui.add(egui::Button::new(job).frame(false).wrap());
                                    self.global.controls.push(serde_json::json!({"name":format!("result-{g}-{n}"),"rect":[r.rect.min.x,r.rect.min.y,r.rect.max.x,r.rect.max.y]}));
                                    if r.clicked(){navigate=Some((group.path.clone(),h.clone()));}
                                }
                            });
                        }
                        if self.global.results.groups.len()>self.global.shown.max(100)&&ui.button(language.text("Show more files")).clicked(){self.global.shown=self.global.shown.max(100)+100;}
                    });
                }
            });
        if response
            .as_ref()
            .is_some_and(|r| crate::theme::window_escape(ctx, r.response.layer_id))
        {
            open = false;
        }
        self.global.open = open;
        if preview {
            self.global.changed = None;
            self.global.preview_generation = self.document_generation;
            self.global.preview_secondary_generation = self.global.secondary_generation;
            self.search_send(Command::Preview {
                options: self.global.options.clone(),
                overlays: self.search_query_overlays(),
                single: self.global.single,
            });
        }
        if apply {
            self.apply_search_preview(ctx);
        }
        if history {
            self.search_send(Command::History);
        }
        if let Some((path, hit)) = navigate {
            self.global.open = false;
            self.search_open_target(path, Some(hit), false, ctx);
        }
    }
    fn apply_search_preview(&mut self, ctx: &egui::Context) {
        let Some(preview) = self.global.preview.take() else {
            return;
        };
        if self.global.preview_generation != self.document_generation
            || self.global.preview_secondary_generation != self.global.secondary_generation
        {
            self.global.error =
                Some("Stale diff: an open document changed; refresh the preview".into());
            self.global.preview = Some(preview);
            return;
        }
        let target = if self.global.single_secondary {
            self.global.secondary.as_ref()
        } else {
            Some(&self.doc)
        };
        if preview.single
            && (target.is_none()
                || preview.files.first().is_some_and(|f| {
                    target.is_none_or(|d| f.doc.path != d.path || f.doc.text != d.text)
                }))
        {
            self.global.error =
                Some("Stale diff: an open document changed; refresh the preview".into());
            self.global.preview = Some(preview);
            return;
        }
        if !self.saves.quiesce(false) || !self.saves.quiesce(true) {
            self.global.error = Some(
                self.language
                    .text("A save is finishing. Please wait.")
                    .into(),
            );
            self.global.preview = Some(preview);
            return;
        }
        let overlays = self.search_overlays();
        for file in &preview.files {
            if let Some(current) = overlays.iter().find(|d| d.path == file.doc.path)
                && (current.text != file.doc.text
                    || current.encoding != file.doc.encoding
                    || current.bom != file.doc.bom
                    || current.disk_snapshot() != file.doc.disk_snapshot())
            {
                self.global.error =
                    Some("Stale diff: an open document changed; refresh the preview".into());
                self.global.preview = Some(preview);
                return;
            }
        }
        if preview.single {
            if let Some(file) = preview.files.first() {
                if self.global.single_secondary {
                    let id = egui::Id::new(("second-editor", self.global.secondary_generation));
                    let doc = self.global.secondary.as_mut().unwrap();
                    let mut state =
                        egui::text_edit::TextEditState::load(ctx, id).unwrap_or_else(|| {
                            crate::structure_ui::bounded_source_state(doc.text.len())
                        });
                    let cursor = state.cursor.char_range().unwrap_or_default();
                    let next = file.after();
                    let chars = next.chars().count();
                    crate::structure_ui::commit_change(
                        &mut state,
                        &mut doc.text,
                        crate::structure::Change {
                            text: next,
                            selection: [
                                cursor.secondary.index.min(chars),
                                cursor.primary.index.min(chars),
                            ],
                            block_anchor: None,
                        },
                        ctx.input(|i| i.time),
                    );
                    state.store(ctx, id);
                    doc.mark_text_changed();
                    self.global.buffer_changed();
                    self.last_secondary = true;
                    ctx.memory_mut(|m| m.request_focus(id));
                } else {
                    self.apply_rich_source(ctx, file.after());
                    self.mode = Mode::Edit;
                    self.editor_focus = true;
                }
                self.global.preview = None;
                self.global.changed = Some(Instant::now());
                self.global.open = false;
            }
        } else {
            self.global.busy = true;
            self.global.changed = None;
            self.search_send(Command::Apply(preview));
        }
    }
    fn search_history_ui(&mut self, ctx: &egui::Context) {
        let language = self.language;
        // Snapshots belongs to Find. Returning to its query is a stable
        // keyboard destination even when the mouse-trigger button had no focus.
        crate::theme::set_window_return_focus(ctx, "history", egui::Id::new("global-query"));
        let mut open = true;
        let mut rollback = None;
        let response=egui::Window::new(crate::theme::window_title(language.text("Replacement snapshots"))).id(egui::Id::new("search-history")).open(&mut open).default_width(600.)
        .min_width(260.).max_size(crate::theme::window_bounds(ctx).size()-Vec2::new(16.,32.))
        .constrain_to(crate::theme::window_bounds(ctx)).vscroll(true).show(ctx,|ui|{
            ui.label(language.text("Rollback checks current disk and open buffers before restoring original bytes."));
            if self.global.histories.is_empty(){ui.label(language.text("No replacement snapshots."));}
            for (n,item) in self.global.histories.iter().enumerate(){ui.horizontal_wrapped(|ui|{
                ui.label(format!("{} · {} · {}",item.id,item.state,item.files));
                if ["prepared","committed","recovery_required"].contains(&item.state.as_str()) {
                    let r=ui.button(language.text("Rollback"));
                    self.global.controls.push(serde_json::json!({"name":format!("rollback-{n}"),"rect":[r.rect.min.x,r.rect.min.y,r.rect.max.x,r.rect.max.y]}));
                    if r.clicked(){rollback=Some(item.id.clone());}
                }
            });}
            if let Some(error)=&self.global.error{ui.colored_label(Color32::LIGHT_RED,error_text(language,error));}
        });
        if response
            .as_ref()
            .is_some_and(|r| crate::theme::window_escape(ctx, r.response.layer_id))
        {
            open = false;
        }
        self.global.history_open = open;
        if let Some(id) = rollback
            && self.saves.quiesce(false)
            && self.saves.quiesce(true)
        {
            self.global.busy = true;
            self.global.changed = None;
            self.search_send(Command::Rollback {
                id,
                overlays: self.search_overlays(),
            });
        }
    }
    pub fn secondary_panel(&mut self, ctx: &egui::Context) {
        if self.global.secondary.is_none() {
            return;
        }
        let language = self.language;
        let mut save = false;
        let mut close = false;
        let mut edited = false;
        egui::SidePanel::right("second-document")
            .default_width(440.).min_width(240.).resizable(true)
            .frame(egui::Frame::new().fill(self.palette.paper).inner_margin(egui::Margin::symmetric(16,8)).stroke(egui::Stroke::new(0.5,self.palette.line)))
            .show(ctx, |ui| {
                let pressed=ctx.input(|i|i.pointer.any_pressed().then(||i.pointer.interact_pos()).flatten());
                if pressed.is_some_and(|pos|ui.max_rect().contains(pos)&&ctx.layer_id_at(pos)==Some(ui.layer_id())) { self.last_secondary=true;ctx.memory_mut(|m|m.surrender_focus(egui::Id::new(("source-editor",self.document_generation)))); }
                let doc = self.global.secondary.as_ref().unwrap();
                let id = egui::Id::new(("second-editor", self.global.secondary_generation));
                let path=doc.path.as_ref().map(|p|p.display().to_string()).unwrap_or_default();
                let state=format!("{}{}",language.text(if self.last_secondary {"Active"} else {"Second document"}),if doc.presentation_dirty() {format!(" · {}",language.text("Unsaved changes"))} else {String::new()});
                crate::theme::document_header(ui,&doc.name(language),&path,&state,(self.last_secondary,doc.presentation_dirty()),self.palette);
                ui.horizontal(|ui| {
                    ui.set_height(32.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                        close = ui.small_button(language.text("Close")).clicked();
                        save = ui.small_button(language.text("Save")).clicked();
                        ui.add(egui::Label::new(RichText::new(language.text(if crate::text_files::markdown(doc.path.as_deref()) {"MARKDOWN"} else {"PLAIN TEXT"})).size(11.0).color(self.palette.muted)).truncate());
                    });
                });
                self.autosave_status(ui, true);
                self.recovery_status(ui, true);
                ui.add_space(8.0);
                let doc = self.global.secondary.as_mut().unwrap();
                let large = &mut self.global.secondary_large;
                large.ensure(&doc.text, self.global.secondary_generation, doc.revision(), ctx);
                if large.active {
                    let mut state = egui::text_edit::TextEditState::load(ctx, id)
                        .unwrap_or_else(|| crate::structure_ui::bounded_source_state(doc.text.len()));
                    if ui.memory(|m| m.has_focus(id)) {
                        let history = ui.input_mut(|i| {
                            if i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) { Some(true) }
                            else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) { Some(false) }
                            else { None }
                        });
                        if let Some(redo) = history {
                            let current = (state.cursor.char_range().unwrap_or_default(), doc.text.clone());
                            let mut candidate = current.clone();
                            let mut undoer = state.undoer();
                            let result = loop {
                                let next = if redo { undoer.redo(&candidate) } else { undoer.undo(&candidate) }.cloned();
                                match next { Some(next) if next.1 == current.1 => candidate = next, other => break other }
                            };
                            if let Some((cursor, text)) = result {
                                doc.text = text;
                                doc.mark_text_changed();
                                large.ensure(&doc.text, self.global.secondary_generation, doc.revision(), ctx);
                                large.jump = Some(large.rope.char_to_byte(cursor.primary.index.min(large.rope.len_chars())));
                                state.cursor.set_char_range(Some(cursor));
                                edited = true;
                            }
                            state.set_undoer(undoer);
                        }
                    }
                    if let Some((start, end)) = self.global.secondary_jump.take()
                        && end <= doc.text.len() && doc.text.is_char_boundary(start) && doc.text.is_char_boundary(end) {
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(large.rope.byte_to_char(start)),
                            egui::text::CCursor::new(large.rope.byte_to_char(end)))));
                        large.jump = Some(start);
                        ui.memory_mut(|m| m.request_focus(id));
                    }
                    state.store(ctx, id);
                    let wrap = self.preferences.source_wrap.unwrap_or(doc.text.len() <= 256 * 1024);
                    let (response, changed) = large.show_source(ui, doc, id, self.font_size, wrap, false,&mut self.reading,"secondary/source",self.global.secondary_generation);
                    edited |= changed;
                    let rect = large.source_rect.unwrap_or(response.rect);
                    self.global.controls.push(serde_json::json!({"name":"secondary-editor","rect":[rect.min.x,rect.min.y,rect.max.x,rect.max.y]}));
                } else {
                    let reading_key="secondary/source";
                    self.reading.layer(reading_key,ui.layer_id());
                    if let Some(selection)=self.reading.selection(doc,reading_key){let mut state=egui::text_edit::TextEditState::load(ctx,id).unwrap_or_default();state.cursor.set_char_range(Some(crate::reading_state::cursor(selection)));state.store(ctx,id);}
                    let wrap=self.preferences.source_wrap.unwrap_or(true);
                    let source_width=ui.available_width();
                    let source_font=FontId::monospace(self.font_size);
                    let source_color=ui.visuals().text_color();
                    let layout_metrics=self.reading.layout_metrics.clone();
                    let mut layouter=|ui:&egui::Ui,text:&dyn egui::TextBuffer,_width:f32| {
                        let started=Instant::now();
                        let galley=ui.fonts_mut(|fonts|fonts.layout(text.as_str().to_owned(),source_font.clone(),source_color,if wrap {source_width} else {f32::INFINITY}));
                        crate::reading_state::layout_sample(&layout_metrics,"secondary-source-font-layout",started);
                        galley
                    };
                    let reading_layout=format!("secondary-source:{:.1}:{}:{}",source_width,self.font_size,wrap);
                    let restore=self.reading.plan(doc,reading_key,self.global.secondary_generation,&reading_layout,ctx,ui.available_rect_before_wrap(),self.global.secondary_jump.is_some());
                    let mut scroll=egui::ScrollArea::both().id_salt(("second-scroll",doc.id,self.global.secondary_generation)).auto_shrink([false;2]);
                    if let Some(restore)=&restore{let target=self.reading.scroll_target(reading_key,restore.position.offset);scroll=scroll.horizontal_scroll_offset(target[0]).vertical_scroll_offset(target[1]);}
                    let mut target=None;
                    let scrolled=scroll.show_viewport(ui, |ui,_viewport| {
                        egui::TextEdit::multiline(&mut doc.text).id(id).font(FontId::monospace(self.font_size))
                            .layouter(&mut layouter).desired_width(source_width).desired_rows(40).show(ui)
                    });
                    let output=scrolled.inner;
                    edited |= output.response.changed();
                    if output.response.changed() { doc.mark_text_changed(); }
                    let cursor=output.galley.cursor_from_pos(egui::vec2(0.,scrolled.state.offset.y));
                    let anchor=(self.reading.byte_at_char(doc,cursor.index),output.galley.pos_from_cursor(cursor).top()-scrolled.state.offset.y);
                    if let Some(restore)=&restore{target=Some(if restore.exact{restore.position.offset}else{[restore.position.offset[0],(output.galley.pos_from_cursor(egui::text::CCursor::new(self.reading.char_at_byte(doc,restore.position.anchor.byte))).top()-restore.position.anchor.viewport_y).max(0.)]});}
                    self.reading.viewport(reading_key,scrolled.inner_rect);
                    let mapped=self.reading.geometry_cache.linear(output.galley.clone(),output.galley_pos,0);
                    let precise=self.reading.geometry_frame(doc,reading_key,&[mapped],scrolled.inner_rect,[scrolled.state.offset.x,scrolled.state.offset.y],[(scrolled.content_size.x-scrolled.inner_rect.width()).max(0.),(scrolled.content_size.y-scrolled.inner_rect.height()).max(0.)],ctx);
                    if precise.is_some(){target=precise;}
                    let mut scroll_state=scrolled.state;
                    if let Some(target)=target{let started=Instant::now();scroll_state.offset=egui::vec2(target[0],target[1]);scroll_state.store(ctx,scrolled.id);self.reading.target(reading_key,target,ctx);self.reading.commit_sample(started);}
                    if target.is_none()||!self.reading.restoring(reading_key){self.reading.observe(doc,reading_key,reading_layout,[scroll_state.offset.x,scroll_state.offset.y],[scrolled.inner_rect.width(),scrolled.inner_rect.height()],anchor.0,0.,anchor.1,scrolled.inner_rect.contains(ctx.pointer_hover_pos().unwrap_or_default()));}
                    let selection=egui::text_edit::TextEditState::load(ctx,id).and_then(|state|state.cursor.char_range()).unwrap_or_default();self.reading.store_selection(doc,reading_key,[selection.primary.index,selection.secondary.index]);
                    self.global.controls.push(serde_json::json!({"name":"secondary-editor","rect":[output.response.rect.min.x,output.response.rect.min.y,output.response.rect.max.x,output.response.rect.max.y]}));
                    if let Some((start, end)) = self.global.secondary_jump.take()
                        && end <= doc.text.len() && doc.text.is_char_boundary(start) && doc.text.is_char_boundary(end) {
                        let mut state = output.state;
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(doc.text[..start].chars().count()),
                            egui::text::CCursor::new(doc.text[..end].chars().count()))));
                        state.store(ctx, id);
                        output.response.request_focus();
                    }
                }
                if ui.memory(|m|m.has_focus(id)) && ctx.input_mut(|i|i.consume_key(egui::Modifiers::COMMAND,egui::Key::S)) { save = true; }
            });
        if edited {
            self.global.buffer_changed();
        }
        if save {
            self.save_secondary();
        }
        if close {
            self.request_secondary_close();
        }
        if self.global.secondary_close {
            egui::Window::new(crate::theme::window_title(
                language.text("Unsaved second document"),
            ))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(language.text("Save your edits before closing?"));
                if self.saves.active(true) {
                    ui.label(
                        language.text(
                            "A save is finishing. Changes already written cannot be discarded.",
                        ),
                    );
                } else if self.global.secondary.as_ref().is_some_and(|d| !d.dirty()) {
                    ui.label(language.text("The latest changes have already been saved to disk."));
                }
                if ui.button(language.text("Save")).clicked() {
                    self.global.secondary_save_close = true;
                    self.save_secondary();
                }
                if ui
                    .add_enabled(
                        !self.saves.active(true),
                        egui::Button::new(language.text("Discard")),
                    )
                    .clicked()
                {
                    if !self.recovery.invalidate(true) {
                        self.error = Some(language.text("Could not discard recovery copy. Resolve the storage error and retry.").into());
                        return;
                    }
                    self.global.secondary = None;
                    self.global.secondary_close = false;
                }
                if ui.button(language.text("Cancel")).clicked() {
                    self.cancel_secondary_close();
                }
            });
        }
    }
    pub(crate) fn request_secondary_close(&mut self) {
        self.global.secondary_save_close = false;
        if !self.saves.quiesce(true) || self.global.secondary.as_ref().is_some_and(|d| d.dirty()) {
            self.global.secondary_close = true;
        } else {
            if !self.recovery.invalidate(true) {
                self.error = Some(
                    self.language
                        .text(
                            "Could not discard recovery copy. Resolve the storage error and retry.",
                        )
                        .into(),
                );
                return;
            }
            self.global.secondary = None;
        }
    }
    pub(crate) fn cancel_secondary_close(&mut self) {
        self.global.secondary_close = false;
        self.global.secondary_save_close = false;
    }
    pub fn save_secondary(&mut self) -> bool {
        let ctx = self.saves.context.clone().unwrap_or_default();
        let Some(path) = self
            .global
            .secondary
            .as_ref()
            .and_then(|doc| doc.path.clone())
        else {
            return true;
        };
        self.start_save(true, path, false, &ctx);
        false
    }
    fn search_report(&mut self) {
        if let Some(path) = std::env::var_os("MDCARD_SEARCH_REPORT") {
            let mut report = serde_json::json!({"controls":self.global.controls,"open":self.global.open,"quick_open":self.global.quick_open,"busy":self.global.busy,"index":{"ready":self.global.status.ready,"files":self.global.status.files},"results":self.global.results.count,"groups":self.global.results.groups.iter().take(200).map(|g|serde_json::json!({"path":g.path,"hits":g.hits.iter().take(200).map(|h|serde_json::json!({"line":h.line,"start":h.start,"end":h.end})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"quick":self.global.quick.iter().map(|q|serde_json::json!({"path":q.path,"label":q.label,"score":q.score})).collect::<Vec<_>>(),"error":self.global.error,"preview_files":self.global.preview.as_ref().map_or(0,|p|p.files.len()),"path":self.doc.path,"text":diagnostic_document_text(&self.doc),"secondary":self.global.secondary.as_ref().map(|d|serde_json::json!({"path":d.path,"text":diagnostic_document_text(d),"dirty":d.presentation_dirty()})),"generation":self.document_generation});
            report["secondary_virtual"] = serde_json::json!({"active":self.global.secondary_large.active,"source_window":[self.global.secondary_large.visible_source.start,self.global.secondary_large.visible_source.end],"laid_out_lines":self.global.secondary_large.laid_out_lines,"selection":self.global.secondary_large.selection});
            report["results_limited"] = serde_json::json!(self.global.results.limited);
            report["index"]["full_verifications"] =
                serde_json::json!(self.global.status.full_verifications);
            report["index"]["refresh_ms"] = serde_json::json!(self.global.status.refresh_ms);
            report["queue_ms"] = serde_json::json!(self.global.queue_ms);
            report["processing_ms"] = serde_json::json!(self.global.processing_ms);
            report["poll_ms"] = serde_json::json!(self.global.poll_ms);
            report["query"] = serde_json::json!(self.global.options.query);
            report["quick_query"] = serde_json::json!(self.global.query);
            report["query_pending"] =
                serde_json::json!(self.global.waiting || self.global.changed.is_some());
            report["visible_ms"] = serde_json::json!(self.global.visible_ms);
            report["worker_ms"] = serde_json::json!(self.global.roundtrip);
            report["engine_ms"] = serde_json::json!(self.global.results.elapsed_ms);
            if self.global.reporter.is_none() {
                let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(1);
                let path = PathBuf::from(path);
                std::thread::Builder::new()
                    .name("mdcard-search-report".into())
                    .spawn(move || {
                        while let Ok(mut bytes) = rx.recv() {
                            while let Ok(new) = rx.try_recv() {
                                bytes = new;
                            }
                            let _ = crate::document::atomic_write(&path, &bytes);
                        }
                    })
                    .expect("search report worker");
                self.global.reporter = Some(tx);
            }
            if let Some(tx) = &self.global.reporter {
                let _ = tx.try_send(serde_json::to_vec(&report).unwrap_or_default());
            }
        }
    }
}

fn diagnostic_document_text(doc: &Document) -> Option<&str> {
    (!doc.is_recovery_copy()).then_some(doc.text.as_str())
}

#[cfg(test)]
mod reading_replace_tests {
    use super::*;
    #[test]
    fn recovery_source_is_not_exported_to_search_diagnostics_even_after_save() {
        let mut doc = Document::recovered(
            "RECOVERY PRIVATE TEXT".into(),
            crate::encoding::Encoding::Utf8,
            false,
        );
        assert!(doc.is_recovery_copy());
        let dir = tempfile::tempdir().unwrap();
        doc.save_to(&dir.path().join("explicit.md")).unwrap();
        assert!(doc.is_recovery_copy());
        let value = serde_json::json!({"text":diagnostic_document_text(&doc),"bytes":doc.text.len(),"digest":doc.diagnostic_digest()});
        assert!(value["text"].is_null());
        assert!(!value.to_string().contains("RECOVERY PRIVATE TEXT"));
    }
    #[test]
    fn ordinary_secondary_maps_real_graphemes_and_keeps_them_through_font_and_wrap_changes() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "independent primary");
        let text = format!(
            "# Side select\n\n{}",
            (0..180)
                .map(|i| format!(
                    "SIDE-{i:04} 中文 é 👩🏽‍💻 independently distinguishable source row.\n"
                ))
                .collect::<String>()
        );
        assert!(text.len() < crate::large_document::THRESHOLD);
        let mut doc = Document::from_text(&text);
        app.global.secondary_generation = 7;
        app.reading.bind(&mut doc, "secondary", 7);
        let id = doc.id;
        let buffer = doc.buffer_id();
        app.global.secondary = Some(doc);
        let frame_time = std::cell::Cell::new(0.0);
        let frame = |app: &mut Editor, events| {
            frame_time.set(frame_time.get() + 1.0 / 60.0);
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 820.),
                    )),
                    time: Some(frame_time.get()),
                    events,
                    ..Default::default()
                },
                |ctx| app.secondary_panel(ctx),
            );
        };
        for _ in 0..3 {
            frame(&mut app, vec![]);
        }
        let report = app.reading.report(&app.doc, app.global.secondary.as_ref());
        let rect = &report["views"]["secondary/source"]["viewport_rect"];
        let pointer = egui::pos2(
            (rect[0].as_f64().unwrap() + rect[2].as_f64().unwrap()) as f32 * 0.5,
            (rect[1].as_f64().unwrap() + rect[3].as_f64().unwrap()) as f32 * 0.5,
        );
        // Use the actual editor input path to select the first source line.
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut app,
            vec![egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(
            &mut app,
            vec![key(
                egui::Key::Home,
                egui::Modifiers::COMMAND | egui::Modifiers::CTRL,
            )],
        );
        frame(&mut app, vec![key(egui::Key::End, egui::Modifiers::SHIFT)]);
        assert_eq!(
            app.reading.report(&app.doc, app.global.secondary.as_ref())["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"].as_u64() == Some(id.0))
                .unwrap()["selections"]["secondary/source"],
            serde_json::json!([13, 0])
        );
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -960.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..120 {
            frame(&mut app, vec![]);
        }
        assert_eq!(ctx.input(|i| i.smooth_scroll_delta), egui::Vec2::ZERO);
        let report = app.reading.report(&app.doc, app.global.secondary.as_ref());
        let before = &report["views"]["secondary/source"];
        assert!(before["position"]["offset"][1].as_f64().unwrap() > 100.);
        let byte = before["geometry"]["source_byte"]
            .as_u64()
            .expect("ordinary secondary needs actual source geometry") as usize;
        let y = before["geometry"]["viewport_y"].as_f64().unwrap();
        let selection = report["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"].as_u64() == Some(id.0))
            .unwrap()["selections"]["secondary/source"]
            .clone();
        let outside = egui::pos2(pointer.x, 30.);
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(outside),
                egui::Event::PointerButton {
                    pos: outside,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut app,
            vec![egui::Event::PointerButton {
                pos: outside,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!ctx.memory(|m| m.has_focus(egui::Id::new(("second-editor", 7)))));
        for (font, wrap) in [(23., true), (23., false), (15., true)] {
            app.font_size = font;
            app.preferences.source_wrap = Some(wrap);
            for _ in 0..16 {
                frame(&mut app, vec![]);
            }
            let report = app.reading.report(&app.doc, app.global.secondary.as_ref());
            let view = &report["views"]["secondary/source"];
            let g = &view["geometry"];
            assert_eq!(
                g["source_byte"].as_u64(),
                Some(byte as u64),
                "{font}/{wrap}: {view}"
            );
            assert!(
                (g["viewport_y"].as_f64().unwrap() - y).abs() <= 2.,
                "{font}/{wrap}: {view}"
            );
            let end = g["source_end"].as_u64().unwrap() as usize;
            assert_eq!(&text[byte..end], g["source_text"].as_str().unwrap());
            assert_eq!(app.global.secondary.as_ref().unwrap().buffer_id(), buffer);
            assert_eq!(
                report["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"].as_u64() == Some(id.0))
                    .unwrap()["selections"]["secondary/source"],
                selection
            );
        }
        let original = app.global.secondary.take().unwrap();
        app.commit_document(Document::from_text("Other independent secondary"), true);
        for _ in 0..4 {
            frame(&mut app, vec![]);
        }
        app.commit_document(original, true);
        for _ in 0..8 {
            frame(&mut app, vec![]);
        }
        let reopened = app.reading.report(&app.doc, app.global.secondary.as_ref());
        assert_eq!(
            reopened["records"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"].as_u64() == Some(id.0))
                .unwrap()["selections"]["secondary/source"],
            serde_json::json!([13, 0])
        );
        assert_eq!(app.doc.text, "independent primary");
        assert_eq!(app.global.secondary.as_ref().unwrap().text, text);
    }
    #[test]
    fn secondary_replacement_preserves_document_id_and_rebinds_current_open_generation() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "main stays here");
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("second.md");
        std::fs::write(
            &path,
            "# Section\n\nfirst paragraph\n\nTARGET retained paragraph 中文🙂\n",
        )
        .unwrap();
        let mut doc = Document::open(&path).unwrap();
        app.global.secondary_generation = 7;
        app.reading.bind(&mut doc, "secondary", 7);
        let id = doc.id;
        let revision = doc.revision();
        assert!(!doc.presentation_dirty());
        let old_statistics = doc.text_statistics();
        assert_eq!(app.doc.text_statistics(), (1, 15));
        let byte = doc.text.find("TARGET").unwrap();
        app.reading.observe(
            &doc,
            "secondary/source",
            "old-layout".into(),
            [12., 777.],
            [400., 600.],
            byte,
            0.,
            0.,
            true,
        );
        app.reading
            .store_selection(&doc, "secondary/source", [12, 0]);
        app.global.secondary = Some(doc);
        std::fs::write(&path,"# New heading\n\nInserted paragraph.\n\n# Section\n\nfirst paragraph\n\nTARGET retained paragraph 中文🙂\n").unwrap();
        let next = Document::open(&path).unwrap();
        app.update_committed_documents(&ctx, vec![next]);
        let doc = app.global.secondary.as_ref().unwrap();
        assert_eq!(doc.id, id);
        assert!(doc.revision() > revision);
        assert!(!doc.presentation_dirty());
        assert_ne!(doc.text_statistics(), old_statistics);
        assert_eq!(
            doc.text_statistics(),
            (doc.text.lines().count(), doc.text.chars().count())
        );
        assert_eq!(app.doc.text_statistics(), (1, 15));
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400., 600.));
        assert!(
            app.reading
                .plan(
                    doc,
                    "secondary/source",
                    7,
                    "new-layout",
                    &ctx,
                    viewport,
                    false
                )
                .is_none()
        );
        let restore = app
            .reading
            .plan(
                doc,
                "secondary/source",
                8,
                "new-layout",
                &ctx,
                viewport,
                false,
            )
            .expect("current rebuilt pane must accept its restore task");
        assert_eq!(
            restore.position.anchor.byte,
            doc.text.find("TARGET").unwrap()
        );
        assert_eq!(
            app.reading.selection(doc, "secondary/source"),
            Some([12, 0])
        );
        assert_eq!(app.doc.text, "main stays here");
    }
}
