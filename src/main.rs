#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors

mod branding;
mod diagnostic_sink;
mod document;
mod encoding;
mod export_ui;
mod large_document;
mod localization;
mod preferences;
#[cfg(target_os = "linux")]
mod process_tree;
mod reading_geometry;
mod reading_precision;
mod reading_state;
#[rustfmt::skip]
mod update_probe;
mod rendering;
mod rich_nodes;
mod rich_svg;
mod rich_ui;
mod search;
mod search_ui;
mod source_layout;
mod structure;
mod structure_ui;
mod syntax_cache;
mod text_files;
mod theme;
mod window_corners;

use document::{Document, outline};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, Vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use encoding::Encoding;
use std::{path::PathBuf, time::Instant};

use localization::Language;
use preferences::{Appearance, Preferences};
use theme::{Palette, button};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Edit,
    Split,
    Read,
}

#[cfg(test)]
mod presentation_cache_tests {
    use super::*;

    #[test]
    fn outline_cache_tracks_actual_buffer_revision_and_reuses_empty_result() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "# First\n\nbody");
        app.refresh_outline(&ctx);
        let pending_started = app.outline_changed;
        app.refresh_outline(&ctx);
        assert_eq!(app.outline_changed, pending_started);
        app.outline_changed = Some(Instant::now() - std::time::Duration::from_secs(1));
        app.refresh_outline(&ctx);
        assert_eq!(app.outline_cache, vec![(1, "First".into())]);
        let id = app.doc.id;
        let mut other = Document::from_text("# Other\n\nbody");
        other.id = id;
        assert_eq!(other.revision(), app.doc.revision());
        app.doc = other;
        app.refresh_outline(&ctx);
        assert!(app.outline_changed.is_some());
        app.outline_changed = Some(Instant::now() - std::time::Duration::from_secs(1));
        app.refresh_outline(&ctx);
        assert_eq!(app.outline_cache, vec![(1, "Other".into())]);
        app.doc.text = "no headings".into();
        app.doc.mark_text_changed();
        app.refresh_outline(&ctx);
        app.outline_changed = Some(Instant::now() - std::time::Duration::from_secs(1));
        app.refresh_outline(&ctx);
        assert!(app.outline_cache.is_empty());
        app.refresh_outline(&ctx);
        assert!(app.outline_changed.is_none());
    }
    #[test]
    fn native_source_edit_rich_patch_and_undo_refresh_presentation() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "baseline中文");
        app.mode = Mode::Edit;
        assert!(!app.doc.presentation_dirty());
        assert_eq!(app.doc.text_statistics(), (1, 10));
        let input = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280., 800.),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input(vec![]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.editor(ui));
        });
        let id = egui::Id::new(("source-editor", app.document_generation));
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(10),
            )));
        state.store(&ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
        let revision = app.doc.revision();
        let _ = ctx.run(input(vec![egui::Event::Text("\n🙂".into())]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.editor(ui));
        });
        assert_eq!(app.doc.text, "baseline中文\n🙂");
        assert!(app.doc.revision() > revision);
        assert!(app.doc.presentation_dirty());
        assert_eq!(app.doc.text_statistics(), (2, 12));
        let _ = ctx.run(
            input(vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }]),
            |ctx| app.structure_shortcuts(ctx),
        );
        assert_eq!(app.doc.text, "baseline中文");
        assert!(!app.doc.presentation_dirty());
        assert_eq!(app.doc.text_statistics(), (1, 10));
        app.apply_rich_source(&ctx, "rich\nchanged\n".into());
        assert!(app.doc.presentation_dirty());
        assert_eq!(app.doc.text_statistics(), (2, 13));
    }
}

#[derive(Clone)]
enum Action {
    New,
    Open,
    OpenPath(PathBuf),
    SearchPath(PathBuf),
    Reopen(PathBuf, Encoding),
    Welcome,
    Quit,
}

struct Editor {
    reading: reading_state::ReadingState,
    diagnostic_reports: diagnostic_sink::DiagnosticSinks,
    large: large_document::LargeDocument,
    last_secondary: bool,
    export: export_ui::ExportUi,
    global: search_ui::SearchUi,
    rich: rich_ui::RichUi,
    outline_source: String,
    outline_changed: Option<Instant>,
    outline_cache: Vec<(usize, String)>,
    structure: structure_ui::StructureUi,
    native_corners: window_corners::NativeCorners,
    native_icons: branding::NativeIcons,
    language: Language,
    preferences: Preferences,
    saved_preferences: Preferences,
    running_gpu: bool,
    requested_gpu_at_start: bool,
    automatic_compatibility: bool,
    renderer: String,
    palette: Palette,
    previous_palette: Palette,
    target_palette: Palette,
    theme_changed: Instant,
    theme_key: (bool, usize),
    settings_error: Option<String>,
    settings_open: bool,
    encoding_open: bool,
    encoding_choice: Encoding,
    encoding_bom: bool,
    encoding_target: Option<PathBuf>,
    failed_open: Option<PathBuf>,
    mode_transition: Option<(Mode, Instant)>,
    last_visible_mode: Mode,
    doc: Document,
    cache: CommonMarkCache,
    mode: Mode,
    sidebar: bool,
    navigation: u8,
    pending: Option<Action>,
    error: Option<String>,
    status: &'static str,
    recent: Vec<PathBuf>,
    editor_focus: bool,
    allow_close: bool,
    font_size: f32,
    document_generation: u64,
    resize_origin: Option<Vec2>,
    started: Instant,
    screenshot_requested: bool,
    screenshot_path: Option<PathBuf>,
    auto_quit: Option<f32>,
    startup_directory: PathBuf,
    files: text_files::FileIndex,
    tree: text_files::TreeView,
    search_open: bool,
    search_query: String,
    search_matches: Vec<usize>,
    search_revision: u64,
    search_focus: bool,
    search_selected: usize,
    rename_open: bool,
    rename_name: String,
}

impl Editor {
    /// Commit node drafts before an operation observes or replaces the source.
    /// A failed patch keeps its editor/error visible and blocks the operation.
    fn flush_rich_edits(&mut self, ctx: &egui::Context) -> bool {
        while let Some((expected, node, value)) = self.rich.take_pending(true) {
            match rich_nodes::patch(&self.doc.text, expected, &node, &value) {
                Ok(next) => {
                    self.apply_rich_source(ctx, next);
                    self.rich.applied(&self.doc.text, None);
                }
                Err(error) => {
                    self.rich
                        .applied(&self.doc.text, Some(self.language.text(error)));
                    self.error = Some(self.language.text(error).into());
                    return false;
                }
            }
        }
        if let Some(error) = self.rich.synchronization_error() {
            self.error = Some(error.into());
            return false;
        }
        true
    }
    fn open_export(&mut self, ctx: &egui::Context) {
        if !self.flush_rich_edits(ctx) {
            return;
        }
        let (path, generation) = if self.last_secondary {
            (
                self.global
                    .secondary
                    .as_ref()
                    .and_then(|d| d.path.as_deref()),
                self.global.secondary_identity(),
            )
        } else {
            (self.doc.path.as_deref(), self.document_generation)
        };
        self.export.open_for(self.last_secondary, path, generation);
    }
    fn sync_language(&mut self, ctx: &egui::Context) {
        let next = self
            .preferences
            .language
            .unwrap_or_else(localization::system_language);
        if next != self.language {
            if !self.flush_rich_edits(ctx) {
                return;
            }
            if localization::refresh_welcome(&mut self.doc, self.language, next) {
                self.document_generation += 1;
            }
            self.language = next;
            self.cache = CommonMarkCache::default();
            localization::install_fonts(ctx, next);
        }
    }

    fn animate_theme(&mut self, ctx: &egui::Context) {
        let dark = match self.preferences.appearance {
            Appearance::Light => false,
            Appearance::Dark => true,
            Appearance::System => ctx.system_theme() == Some(egui::Theme::Dark),
        };
        let key = (dark, self.preferences.accent);
        if key != self.theme_key {
            self.theme_key = key;
            self.previous_palette = self.palette;
            self.target_palette = Palette::new(dark, key.1);
            self.theme_changed = Instant::now();
            self.cache = CommonMarkCache::default();
        }
        let t = if self.preferences.reduce_motion {
            1.0
        } else {
            (self.theme_changed.elapsed().as_secs_f32() / 0.2).min(1.0)
        };
        self.palette = self
            .previous_palette
            .blend(self.target_palette, t * t * (3.0 - 2.0 * t));
        self.palette.apply(ctx, self.preferences.reduce_motion);
        if t < 1.0 {
            ctx.request_repaint();
        }
    }
    fn preferences_ui(&mut self, ui: &mut egui::Ui) {
        let language = self.language;
        let p = self.palette;
        ui.set_min_width(270.0);
        ui.label(RichText::new(format!("MDcard {APP_VERSION}")).strong());
        ui.separator();
        ui.label(theme::label(language.text("LANGUAGE"), p));
        egui::ComboBox::from_id_salt("language")
            .selected_text(
                self.preferences
                    .language
                    .map(Language::name)
                    .unwrap_or(language.text("Follow system")),
            )
            .width(250.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.preferences.language,
                    None,
                    language.text("Follow system"),
                );
                for value in Language::ALL {
                    ui.selectable_value(&mut self.preferences.language, Some(value), value.name());
                }
            });
        ui.label(
            RichText::new(language.text("Language changes apply immediately."))
                .small()
                .color(p.muted),
        );
        ui.separator();
        ui.label(theme::label(language.text("APPEARANCE"), p));
        ui.horizontal(|ui| {
            for (value, label) in [
                (Appearance::Light, language.text("Light")),
                (Appearance::Dark, language.text("Dark")),
                (Appearance::System, language.text("System")),
            ] {
                let response = ui.selectable_label(self.preferences.appearance == value, label);
                self.rich
                    .add_control(format!("appearance-{value:?}"), response.rect);
                if response.clicked() {
                    self.preferences.appearance = value;
                }
            }
        });
        ui.add_space(5.0);
        ui.label(theme::label(language.text("ACCENT COLOR"), p));
        ui.horizontal(|ui| {
            for (index, name) in theme::ACCENTS.iter().enumerate() {
                let c = Palette::new(p.dark, index).accent;
                let (r, response) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::click());
                let hover = ui.ctx().animate_bool_with_time(
                    response.id.with("swatch"),
                    response.hovered(),
                    ui.style().animation_time,
                );
                ui.painter().circle_filled(r.center(), 9.0 + hover, c);
                if index == self.preferences.accent {
                    ui.painter()
                        .circle_stroke(r.center(), 13.0, Stroke::new(1.5, p.ink));
                }
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        r,
                        5,
                        Stroke::new(1.5, p.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::RadioButton,
                        ui.is_enabled(),
                        index == self.preferences.accent,
                        language.text(name),
                    )
                });
                theme::record_control(ui, &response, language.text(name));
                if response.on_hover_text(language.text(name)).clicked() {
                    self.preferences.accent = index;
                }
            }
        });
        theme::toggle(
            ui,
            &mut self.preferences.reduce_motion,
            language.text("Reduce motion"),
        );
        ui.separator();
        ui.label(theme::label(language.text("TEXT SIZE"), p));
        let size = ui.add(egui::Slider::new(&mut self.font_size, 12.0..=23.0).suffix(" px"));
        theme::record_control(ui, &size, "reading-font-size");
        ui.separator();
        ui.label(theme::label(language.text("PERFORMANCE"), p));
        theme::toggle(
            ui,
            &mut self.preferences.gpu_acceleration,
            language.text("GPU acceleration"),
        );
        ui.label(
            RichText::new(language.text("Changes apply the next time you open MDcard."))
                .small()
                .color(p.muted),
        );
        if self.preferences.gpu_acceleration != self.requested_gpu_at_start {
            ui.label(
                RichText::new(language.text("Restart required · your document stays open"))
                    .small()
                    .color(p.accent),
            );
        }
        ui.add_space(4.0);
        if self.automatic_compatibility {
            ui.label(
                RichText::new(
                    language.text("Automatic graphics compatibility · GPU preference retained"),
                )
                .small()
                .color(p.muted),
            );
        }
        ui.label(
            RichText::new(if self.running_gpu {
                language.text("Current: OpenGL (acceleration requested)")
            } else {
                language.text("Current: CPU software rendering")
            })
            .small(),
        );
        ui.add(
            egui::Label::new(
                RichText::new(
                    self.renderer
                        .replace("Software", language.text("Software"))
                        .replace(
                            "Renderer unavailable",
                            language.text("Renderer unavailable"),
                        ),
                )
                .small()
                .color(p.muted),
            )
            .wrap(),
        );
        if let Some(error) = &self.settings_error {
            ui.separator();
            ui.label(localized_settings_error(language, error));
        }
        ui.separator();
        ui.label(theme::label(language.text("READING POSITIONS"), p));
        ui.label(
            RichText::new(reading_state::state_path().display().to_string())
                .small()
                .color(p.muted),
        );
        let main_restore = self.reading.restore_status(self.doc.id, "main").cloned();
        let secondary_restore = self
            .global
            .secondary
            .as_ref()
            .and_then(|pane| self.reading.restore_status(pane.id, "secondary"))
            .cloned();
        if main_restore.is_some() || secondary_restore.is_some() {
            let header = ui.collapsing(language.text("Recent reading restore"), |ui| {
                for (heading, status) in [
                    ("Primary document", main_restore),
                    ("Secondary document", secondary_restore),
                ] {
                    if let Some(status) = status {
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                language.text(heading),
                                language.text(status.tier_label())
                            ))
                            .small()
                            .strong(),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(language.text(status.reason_label()))
                                    .small()
                                    .color(p.muted),
                            )
                            .wrap(),
                        );
                    }
                }
            });
            theme::record_control(ui, &header.header_response, "reading-restore-status");
        }
        if let Some(error) = &self.reading.stats.error {
            ui.colored_label(
                p.accent,
                language.text("Reading positions could not be saved; they remain in memory."),
            );
            ui.label(error);
        }
        if button(ui, language.text("Clear this document's reading positions")).clicked() {
            self.reading.clear(Some(self.doc.id));
        }
        if button(ui, language.text("Clear all reading positions")).clicked() {
            self.reading.clear(None);
        }
    }
    fn persist_preferences(&mut self, ctx: &egui::Context) {
        self.preferences.font_size = self.font_size;
        if self.preferences != self.saved_preferences && !ctx.input(|i| i.pointer.any_down()) {
            match preferences::save(&self.preferences) {
                Ok(()) => {
                    self.saved_preferences = self.preferences.clone();
                    self.settings_error = None;
                }
                Err(e) => {
                    self.preferences = self.saved_preferences.clone();
                    self.font_size = self.preferences.font_size;
                    self.settings_error = Some(format!(
                        "{} {e}",
                        "Could not save settings; previous settings restored."
                    ));
                    self.error = self.settings_error.clone();
                }
            }
        }
    }
    #[cfg(test)]
    fn can_split(&self, width: f32) -> bool {
        width
            - if self.sidebar && width >= 850. {
                theme::SIDEBAR_WIDTH
            } else {
                0.
            }
            - if self.global.secondary.is_some() {
                440.
            } else {
                0.
            }
            - theme::CONTENT_MARGIN * 2.
            >= theme::SPLIT_CONTENT_MIN
    }
    fn can_split_ui(&self, ctx: &egui::Context) -> bool {
        let width = ctx.content_rect().width();
        let desired_sidebar = if self.sidebar && width >= 850. {
            theme::SIDEBAR_WIDTH
        } else {
            0.
        };
        let actual_sidebar = ctx
            .data(|d| d.get_temp::<f32>(egui::Id::new("ui-sidebar-width")))
            .unwrap_or(desired_sidebar);
        // During sidebar collapse keep the conservative last actual width;
        // do not enable two cramped columns in the middle of an animation.
        let sidebar = desired_sidebar.max(actual_sidebar);
        let secondary = if self.global.secondary.is_some() {
            egui::containers::panel::PanelState::load(ctx, egui::Id::new("second-document"))
                .map_or(440., |state| state.rect.width())
        } else {
            0.
        };
        width - sidebar - secondary - theme::CONTENT_MARGIN * 2. >= theme::SPLIT_CONTENT_MIN
    }
    fn refresh_outline(&mut self, ctx: &egui::Context) {
        let key = (self.doc.buffer_id(), self.doc.revision());
        let changed = ctx.data_mut(|data| {
            let id = egui::Id::new("outline-source-revision");
            if data.get_temp::<(u64, u64)>(id) == Some(key) {
                false
            } else {
                data.insert_temp(id, key);
                true
            }
        });
        if changed {
            self.outline_source.clone_from(&self.doc.text);
            self.outline_changed = Some(Instant::now());
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if self
            .outline_changed
            .is_some_and(|t| t.elapsed() >= std::time::Duration::from_millis(250))
        {
            self.outline_cache = outline(&self.outline_source);
            self.outline_changed = None;
        }
    }
    fn status_key(&self) -> &'static str {
        if self.doc.presentation_dirty() {
            "Unsaved changes"
        } else if self.status == "Unsaved changes" {
            "Ready when you are"
        } else {
            self.status
        }
    }
    fn new(
        cc: &eframe::CreationContext<'_>,
        path: Option<PathBuf>,
        preferences: Preferences,
        settings_error: Option<String>,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        let language = preferences
            .language
            .unwrap_or_else(localization::system_language);
        localization::install_fonts(ctx, language);
        egui_extras::install_image_loaders(ctx);
        ctx.add_image_loader(std::sync::Arc::new(rich_svg::SvgLoader::default()));
        let dark = preferences.appearance == Appearance::Dark
            || (preferences.appearance == Appearance::System
                && ctx.system_theme() == Some(egui::Theme::Dark));
        let palette = Palette::new(dark, preferences.accent);
        palette.apply(ctx, preferences.reduce_motion);
        let renderer = rendering::description(cc);
        let startup_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        // Document paths are canonical (including the Windows verbatim prefix).
        let startup_directory = startup_directory
            .canonicalize()
            .unwrap_or(startup_directory);
        let mut app = Self {
            diagnostic_reports: diagnostic_sink::DiagnosticSinks::default(),
            reading: if cfg!(test) {
                reading_state::ReadingState::default()
            } else {
                reading_state::ReadingState::persistent()
            },
            large: large_document::LargeDocument::default(),
            last_secondary: false,
            export: export_ui::ExportUi::default(),
            global: search_ui::SearchUi::default(),
            rich: rich_ui::RichUi::default(),
            outline_source: String::new(),
            outline_changed: None,
            outline_cache: vec![],
            structure: structure_ui::StructureUi::default(),
            native_corners: window_corners::NativeCorners::default(),
            native_icons: branding::NativeIcons::default(),
            language,
            running_gpu: preferences.gpu_acceleration && !rendering::automatic_compatibility(),
            requested_gpu_at_start: preferences.gpu_acceleration,
            automatic_compatibility: rendering::automatic_compatibility(),
            saved_preferences: preferences.clone(),
            font_size: preferences.font_size,
            theme_key: (dark, preferences.accent),
            preferences,
            renderer,
            palette,
            previous_palette: palette,
            target_palette: palette,
            theme_changed: Instant::now(),
            settings_error,
            settings_open: false,
            encoding_open: false,
            encoding_choice: Encoding::Utf8,
            encoding_bom: false,
            encoding_target: None,
            failed_open: None,
            mode_transition: None,
            last_visible_mode: Mode::Split,
            doc: Document::from_text(language.welcome()),
            cache: CommonMarkCache::default(),
            mode: Mode::Split,
            sidebar: true,
            navigation: 0,
            pending: None,
            error: None,
            status: "Ready when you are",
            recent: vec![],
            editor_focus: false,
            allow_close: false,

            document_generation: 0,
            resize_origin: None,
            started: Instant::now(),
            screenshot_requested: false,
            screenshot_path: std::env::var_os("MDCARD_SCREENSHOT").map(PathBuf::from),
            auto_quit: std::env::var("MDCARD_AUTO_QUIT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok()),
            files: text_files::FileIndex::new(startup_directory.clone()),
            tree: text_files::TreeView::default(),
            startup_directory,
            search_open: false,
            search_query: String::new(),
            search_matches: vec![],
            search_revision: u64::MAX,
            search_focus: false,
            search_selected: 0,
            rename_open: false,
            rename_name: String::new(),
        };
        app.reading
            .bind(&mut app.doc, "main", app.document_generation);
        if let Some(path) = path {
            // A command-line document establishes the initial workspace once.
            let root = text_files::base_directory(Some(&path), &app.startup_directory);
            app.files = text_files::FileIndex::new(root.canonicalize().unwrap_or(root));
            app.open_path(path);
            if app.doc.path.is_none() {
                app.files = text_files::FileIndex::new(app.startup_directory.clone());
            }
        }
        if !app.files.scanning {
            app.sync_file_root();
        }
        app
    }
    fn open_path(&mut self, path: PathBuf) {
        self.open_path_encoding(path, None);
    }
    fn open_path_encoding(&mut self, path: PathBuf, selected: Option<Encoding>) {
        self.global.invalidate_open();
        let result = if selected.is_none() {
            Document::open(&path)
        } else {
            Document::open_with_encoding(&path, selected)
        };
        match result {
            Ok(doc) => {
                let canonical = doc.path.clone().unwrap_or(path);
                self.recent.retain(|p| p != &canonical);
                self.recent.insert(0, canonical);
                self.recent.truncate(8);
                self.commit_document(doc, false);
                self.failed_open = None;
                self.sync_file_root();
                self.cache = CommonMarkCache::default();
                self.status = "Opened";
                self.editor_focus = true;
            }
            Err(e) => {
                self.failed_open = Some(path);
                self.error = Some(format!(
                    "{}\n{}",
                    "Could not open the document.",
                    localized_document_error(Language::English, &e)
                ))
            }
        }
    }
    /// Every successful file-open route commits here. Failure leaves the old
    /// buffer, identity, pending restore and native scroll state intact.
    fn commit_document(&mut self, mut doc: Document, secondary: bool) {
        self.global.invalidate_open();
        self.reading.flush();
        if secondary {
            self.global.secondary_generation += 1;
            self.reading
                .bind(&mut doc, "secondary", self.global.secondary_generation);
            self.global.secondary_large.opening();
            self.global.secondary = Some(doc);
        } else {
            self.document_generation += 1;
            self.reading
                .bind(&mut doc, "main", self.document_generation);
            if let Some(mode) = self.reading.mode(&doc) {
                self.mode = match mode {
                    "Read" => Mode::Read,
                    "Write" => Mode::Edit,
                    "Split" => Mode::Split,
                    _ => self.mode,
                };
            }
            self.large.opening();
            self.doc = doc;
            self.cache = CommonMarkCache::default();
        }
    }
    fn save(&mut self, save_as: bool, ctx: &egui::Context) -> bool {
        if !self.flush_rich_edits(ctx) {
            return false;
        }
        let language = self.language;
        self.restore_edit_focus();
        let path = if !save_as && self.doc.path.is_some() {
            self.doc.path.clone()
        } else {
            let mut dialog = rfd::FileDialog::new()
                .add_filter(
                    language.text("Supported text files"),
                    text_files::EXTENSIONS,
                )
                .add_filter(language.text("Named text files (README, .env…)"), &["*"])
                .set_title(language.text("Save document"))
                .set_file_name(self.doc.name(self.language));
            if let Some(parent) = self.doc.path.as_ref().and_then(|p| p.parent()) {
                dialog = dialog.set_directory(parent);
            }
            dialog.save_file()
        };
        let Some(path) = path else {
            return false;
        };
        let added_path = self.doc.path.as_ref() != Some(&path);
        let old_id = self.doc.id;
        match self.doc.save_to(&path) {
            Ok(()) => {
                if added_path {
                    self.reading.copied_path(&mut self.doc, old_id, "main");
                    self.sync_file_root();
                }
                self.cache = CommonMarkCache::default();
                self.status = "Saved to disk";
                true
            }
            Err(e) => {
                self.failed_open = None;
                self.error = Some(format!(
                    "{}\n{}",
                    "Could not save. Your edits are still here.",
                    localized_document_error(Language::English, &e)
                ));
                false
            }
        }
    }
    fn request(&mut self, action: Action, ctx: &egui::Context) {
        if !self.flush_rich_edits(ctx) {
            return;
        }
        if matches!(action, Action::Quit)
            && self.global.secondary.as_ref().is_some_and(|d| d.dirty())
        {
            self.error = Some(
                self.language
                    .text("Save or close the second document before quitting.")
                    .into(),
            );
            return;
        }
        if self.doc.dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }
    fn perform(&mut self, action: Action, ctx: &egui::Context) {
        let language = self.language;
        match action {
            Action::New => {
                self.failed_open = None;
                self.commit_document(Document::default(), false);
                self.sync_file_root();
                self.cache = CommonMarkCache::default();
                self.editor_focus = true;
                self.status = "A fresh start";
            }
            Action::Welcome => {
                self.failed_open = None;
                self.commit_document(Document::from_text(language.welcome()), false);
                self.sync_file_root();
                self.cache = CommonMarkCache::default();
            }
            Action::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        language.text("Supported text files"),
                        text_files::EXTENSIONS,
                    )
                    .add_filter(language.text("Named text files (README, .env…)"), &["*"])
                    .set_title(language.text("Open document"))
                    .set_directory(&self.files.root)
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Action::OpenPath(path) => self.open_path(path),
            Action::SearchPath(path) => self.dispatch_search_open(path, ctx),
            Action::Reopen(path, encoding) => self.open_path_encoding(path, Some(encoding)),
            Action::Quit => {
                self.reading.flush();
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.global.secondary_focused(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            self.save_secondary();
            return;
        }
        if self.encoding_open {
            return;
        }
        if self.settings_open {
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)
                    || i.consume_key(
                        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                        egui::Key::P,
                    )
            }) {
                self.settings_open = false;
            }
            return;
        }
        if self.pending.is_some() || self.error.is_some() {
            return;
        }
        if self.global.busy {
            return;
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::F,
            )
        }) {
            self.open_global_search(ctx, false);
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)
                || i.consume_key(egui::Modifiers::COMMAND, egui::Key::H)
        }) {
            self.open_global_search(ctx, true);
        }
        if ctx.input_mut(consume_file_search_shortcut) {
            self.open_search(ctx);
        }
        if self.global.open || self.global.quick_open {
            return;
        }
        if self.search_open {
            return;
        }
        let mut action = None;
        ctx.input_mut(|i| {
            let cmd = egui::Modifiers::COMMAND;
            if i.consume_key(cmd, egui::Key::Comma)
                || i.consume_key(cmd | egui::Modifiers::SHIFT, egui::Key::P)
            {
                self.settings_open = !self.settings_open;
            }
            if i.consume_key(cmd, egui::Key::N) {
                action = Some(Action::New);
            }
            if i.consume_key(cmd, egui::Key::O) {
                action = Some(Action::Open);
            }
            if i.consume_key(cmd, egui::Key::Q) {
                action = Some(Action::Quit);
            }
            if i.consume_key(cmd | egui::Modifiers::SHIFT, egui::Key::E) {
                self.show_encoding(self.doc.path.clone(), ctx);
            }
            if i.consume_key(cmd, egui::Key::E) {
                self.mode = match self.mode {
                    Mode::Edit => Mode::Split,
                    Mode::Split => Mode::Read,
                    Mode::Read => Mode::Edit,
                };
            }
            if i.consume_key(cmd, egui::Key::B) {
                self.sidebar = !self.sidebar;
            }
        });
        if let Some(action) = action {
            self.request(action, ctx);
        }
        let save_as = ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            )
        });
        let save = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S));
        if save || save_as {
            self.save(save_as, ctx);
        }
    }
    fn titlebar(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = self.palette;
        egui::TopBottomPanel::top("titlebar")
            .exact_height(40.0)
            .frame(
                egui::Frame::new()
                    .fill(p.chrome)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for (color, hint, command) in [
                        (Color32::from_rgb(255, 95, 87), language.text("Close"), 0),
                        (
                            Color32::from_rgb(255, 189, 46),
                            language.text("Minimize"),
                            1,
                        ),
                        (
                            Color32::from_rgb(40, 200, 64),
                            language.text("Maximize / restore"),
                            2,
                        ),
                    ] {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::click());
                        let hover = ctx.animate_bool_with_time(
                            response.id.with("lamp"),
                            response.hovered(),
                            ui.style().animation_time,
                        );
                        ui.painter()
                            .circle_filled(rect.center(), 6.0 + 0.5 * hover, color);
                        ui.painter().circle_stroke(
                            rect.center(),
                            6.0,
                            Stroke::new(0.6, color.gamma_multiply(0.75)),
                        );
                        if hover > 0.0 {
                            let c = rect.center();
                            let stroke = Stroke::new(
                                1.0,
                                Color32::from_rgba_unmultiplied(48, 40, 25, (hover * 200.0) as u8),
                            );
                            let segments = match command {
                                0 => vec![
                                    (Vec2::new(-2.2, -2.2), Vec2::new(2.2, 2.2)),
                                    (Vec2::new(-2.2, 2.2), Vec2::new(2.2, -2.2)),
                                ],
                                1 => vec![(Vec2::new(-3.0, 0.0), Vec2::new(3.0, 0.0))],
                                _ => vec![
                                    (Vec2::new(-3.0, 1.0), Vec2::new(-3.0, 3.0)),
                                    (Vec2::new(-3.0, 3.0), Vec2::new(-1.0, 3.0)),
                                    (Vec2::new(-3.0, 3.0), Vec2::new(3.0, -3.0)),
                                    (Vec2::new(1.0, -3.0), Vec2::new(3.0, -3.0)),
                                    (Vec2::new(3.0, -3.0), Vec2::new(3.0, -1.0)),
                                ],
                            };
                            for (from, to) in segments {
                                ui.painter().line_segment([c + from, c + to], stroke);
                            }
                        }
                        if response.has_focus() {
                            ui.painter().circle_stroke(
                                rect.center(),
                                9.0,
                                Stroke::new(1.5, p.accent),
                            );
                        }
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hint)
                        });
                        if response.on_hover_text(hint).clicked() {
                            match command {
                                0 => self.request(Action::Quit, ctx),
                                1 => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                                _ => ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                                    !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                                )),
                            }
                        }
                    }
                    ui.add_space(8.0);
                    if theme::icon_button(
                        ui,
                        theme::Icon::Sidebar,
                        language.text("Toggle sidebar · Ctrl+B"),
                    )
                    .clicked()
                    {
                        self.sidebar = !self.sidebar;
                    }
                    let available = ui.available_width();
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new((available - 106.0).max(40.0), 32.0),
                        egui::Sense::click_and_drag(),
                    );
                    let title = self
                        .files
                        .root
                        .file_name()
                        .unwrap_or(self.files.root.as_os_str())
                        .to_string_lossy()
                        .into_owned();
                    ui.painter().with_clip_rect(rect).text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        title,
                        FontId::proportional(13.0),
                        p.ink,
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        // X11 window managers can consume the release during a native move.
                        // Drop egui's local drag state before handing the pointer to the OS.
                        ctx.input_mut(|input| input.pointer = egui::PointerState::default());
                        ctx.stop_dragging();
                    }
                    if response.double_clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                            !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                        ));
                    }
                    branding::brand(ui, p.chrome);
                    ui.label(RichText::new("MDcard").size(12.0).color(p.muted));
                });
            });
    }
    fn toolbar(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = self.palette;
        egui::TopBottomPanel::top("toolbar")
            .exact_height(44.0)
            .frame(
                egui::Frame::new()
                    .fill(p.paper)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .stroke(Stroke::new(0.5, p.line)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for (icon, label, action) in [
                        (theme::Icon::New, language.text("+  New"), 0),
                        (theme::Icon::Open, language.text("Open"), 1),
                        (theme::Icon::Save, language.text("Save"), 2),
                    ] {
                        if action < 2 && ctx.content_rect().width() < 900.0 {
                            continue;
                        }
                        let response = theme::icon_button(ui, icon, label)
                            .on_hover_text(["Ctrl+N", "Ctrl+O", "Ctrl+S"][action]);
                        if response.clicked() {
                            match action {
                                0 => self.request(Action::New, ctx),
                                1 => self.request(Action::Open, ctx),
                                _ => {
                                    self.save(false, ctx);
                                }
                            }
                        }
                    }
                    ui.separator();
                    ui.menu_button(language.text("File"), |ui| {
                        ui.set_max_width(320.0_f32.min(ctx.content_rect().width() - 48.0));
                        for (key, action) in [("+  New", 0), ("Open", 1), ("Save", 2)] {
                            if button(ui, language.text(key)).clicked() {
                                ui.close();
                                match action {
                                    0 => self.request(Action::New, ctx),
                                    1 => self.request(Action::Open, ctx),
                                    _ => {
                                        self.save(false, ctx);
                                    }
                                }
                            }
                        }
                        ui.separator();
                        if ui
                            .add_enabled(
                                !self.global.busy,
                                egui::Button::new(language.text("Open folder…")),
                            )
                            .on_disabled_hover_text(language.text("Working in background…"))
                            .clicked()
                        {
                            ui.close();
                            self.choose_workspace(ctx);
                        }
                        ui.separator();
                        if text_files::markdown(self.doc.path.as_deref())
                            && button(ui, language.text("Export…")).clicked()
                        {
                            ui.close();
                            self.open_export(ctx);
                        }
                        if button(ui, language.text("Save As…     Ctrl+Shift+S")).clicked() {
                            ui.close();
                            self.save(true, ctx);
                        }
                        let rename = ui.add_enabled(
                            self.doc.path.is_some(),
                            egui::Button::new(language.text("Rename document…")),
                        );
                        theme::record_control(ui, &rename, "Rename document…");
                        if rename.clicked() {
                            ui.close();
                            self.rename_name = self.doc.name(language);
                            self.rename_open = true;
                        }
                        if button(ui, language.text("Welcome document")).clicked() {
                            ui.close();
                            self.request(Action::Welcome, ctx);
                        }
                        if button(ui, language.text("Text encoding…")).clicked() {
                            ui.close();
                            self.show_encoding(self.doc.path.clone(), ctx);
                        }
                        if button(ui, language.text("Open with encoding…")).clicked() {
                            ui.close();
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter(
                                    language.text("Supported text files"),
                                    text_files::EXTENSIONS,
                                )
                                .set_directory(&self.files.root)
                                .pick_file()
                            {
                                self.show_encoding(Some(path), ctx);
                            }
                        }
                    });
                    ui.menu_button(language.text("Search"), |ui| {
                        if button(ui, language.text("Find files…  Ctrl+P")).clicked() {
                            ui.close();
                            self.open_search(ctx);
                        }
                        if button(ui, language.text("Find and replace"))
                            .on_hover_text("Ctrl+F / Ctrl+H")
                            .clicked()
                        {
                            ui.close();
                            self.open_global_search(ctx, true);
                        }
                        if button(ui, language.text("Global search · Ctrl+Shift+F")).clicked() {
                            ui.close();
                            self.open_global_search(ctx, false);
                        }
                    });
                    ui.menu_button("···", |ui| {
                        if text_files::markdown(self.doc.path.as_deref())
                            && button(ui, language.text("Structure…     Ctrl+Shift+T")).clicked()
                        {
                            ui.close();
                            self.structure.open = !self.structure.open;
                        }
                        if button(ui, language.text("Preferences…       Ctrl+,")).clicked() {
                            ui.close();
                            self.settings_open = true;
                        }
                    });
                    if ctx.content_rect().width() >= 1100.0 {
                        let search_label = if ctx.content_rect().width() < 900.0 {
                            language.text("Find files")
                        } else {
                            language.text("Find files…  Ctrl+P")
                        };
                        let search = theme::search_button(
                            ui,
                            search_label,
                            if ctx.content_rect().width() < 900.0 {
                                128.0
                            } else {
                                230.0
                            },
                            p,
                        );
                        theme::record_control(ui, &search, language.text("Find files…  Ctrl+P"));
                        if search.on_hover_text("Ctrl+P").clicked() {
                            self.open_search(ctx);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_split = self.can_split_ui(ctx);
                        ctx.data_mut(|d| {
                            d.insert_temp(egui::Id::new("ui-toolbar-can-split"), can_split)
                        });
                        let visible_mode = if self.mode == Mode::Split && !can_split {
                            Mode::Edit
                        } else {
                            self.mode
                        };
                        let segment_width = ["Write", "Split", "Read"]
                            .iter()
                            .map(|key| {
                                ui.painter()
                                    .layout_no_wrap(
                                        language.text(key).into(),
                                        FontId::proportional(13.0),
                                        p.ink,
                                    )
                                    .size()
                                    .x
                                    + 18.0
                            })
                            .fold(63.0_f32, f32::max);
                        let (group, _) = ui.allocate_exact_size(
                            Vec2::new(segment_width * 3.0 + 3.0, 32.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(group, 8, p.control);
                        let index = match visible_mode {
                            Mode::Edit => 0.0,
                            Mode::Split => 1.0,
                            Mode::Read => 2.0,
                        };
                        let x = ctx.animate_value_with_time(
                            egui::Id::new("mode-indicator"),
                            index,
                            ui.style().animation_time,
                        );
                        let selected = egui::Rect::from_min_size(
                            group.min + Vec2::new(2.0 + x * segment_width, 2.0),
                            Vec2::new(segment_width - 1.0, 28.0),
                        );
                        ui.painter().rect(
                            selected,
                            6,
                            p.paper,
                            Stroke::new(0.7, p.line),
                            egui::StrokeKind::Inside,
                        );
                        for (i, (mode, label)) in [
                            (Mode::Edit, language.text("Write")),
                            (Mode::Split, language.text("Split")),
                            (Mode::Read, language.text("Read")),
                        ]
                        .iter()
                        .enumerate()
                        {
                            let r = egui::Rect::from_min_size(
                                group.min + Vec2::new(1.0 + i as f32 * segment_width, 1.0),
                                Vec2::new(segment_width, 30.0),
                            );
                            let enabled = *mode != Mode::Split || can_split;
                            let response = ui
                                .add_enabled_ui(enabled, |ui| {
                                    ui.interact(r, ui.id().with(("mode", i)), egui::Sense::click())
                                })
                                .inner;
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::RadioButton,
                                    enabled,
                                    *mode == visible_mode,
                                    *label,
                                )
                            });
                            theme::record_control(ui, &response, label);
                            let color = if !enabled {
                                p.muted.gamma_multiply(0.55)
                            } else if *mode == visible_mode {
                                p.accent
                            } else {
                                p.muted
                            };
                            let hover = ctx.animate_bool_with_time(
                                response.id.with("hover"),
                                response.hovered() && enabled,
                                ui.style().animation_time,
                            );
                            if hover > 0.0 && *mode != visible_mode {
                                ui.painter().rect_filled(
                                    r.shrink(2.0),
                                    5,
                                    p.selected.gamma_multiply(hover),
                                );
                            }
                            ui.painter().text(
                                r.center(),
                                egui::Align2::CENTER_CENTER,
                                *label,
                                FontId::proportional(13.0),
                                color,
                            );
                            if response.has_focus() {
                                ui.painter().rect_stroke(
                                    r.shrink(1.0),
                                    6,
                                    Stroke::new(1.5, p.accent),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if response
                                .on_hover_text(if enabled {
                                    language.text("Cycle views · Ctrl+E")
                                } else {
                                    language.text("Widen the window for split view")
                                })
                                .clicked()
                                && enabled
                            {
                                self.mode = *mode;
                            }
                        }
                    });
                });
            });
    }
    fn choose_workspace(&mut self, ctx: &egui::Context) {
        if self.global.busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title(self.language.text("Open folder…"))
            .set_directory(&self.files.root)
            .pick_folder()
        {
            self.set_workspace(path);
        }
        // A native picker also removes egui focus, including on Cancel.
        // Restore on the next frame, after the native focus events settle.
        let target = if self.last_secondary && self.global.secondary.is_some() {
            egui::Id::new(("second-editor", self.global.secondary_identity()))
        } else {
            egui::Id::new(("source-editor", self.document_generation))
        };
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("ui-return-focus-pending"), target));
        ctx.request_repaint();
    }
    fn set_workspace(&mut self, path: PathBuf) {
        if self.global.busy {
            return;
        }
        let path = path.canonicalize().unwrap_or(path);
        if path == self.files.root {
            return;
        }
        self.files.ensure(path);
        self.navigation = 0;
        self.global.workspace_changed();
        self.search_query.clear();
        self.search_selected = 0;
    }
    fn sync_file_root(&mut self) {
        // Browsing descendants must not replace the workspace or its expanded tree.
        // Only a successfully opened/saved file outside it establishes a new root.
        let root = match self.doc.path.as_deref() {
            Some(path) if !path.starts_with(&self.files.root) => {
                text_files::base_directory(Some(path), &self.startup_directory)
            }
            _ => self.files.root.clone(),
        };
        if root != self.files.root {
            self.search_query.clear();
            self.search_selected = 0;
        }
        self.files.ensure(root);
    }
    fn open_search(&mut self, ctx: &egui::Context) {
        if !self.flush_rich_edits(ctx) {
            return;
        }
        self.global.open = false;
        self.global.quick_open = true;
        self.global.focus = true;
        self.global.changed = Some(Instant::now());
    }
    fn file_search(&mut self, ctx: &egui::Context) {
        if !self.search_open || self.pending.is_some() || self.error.is_some() {
            return;
        }
        let language = self.language;
        let mut open = true;
        let mut chosen = None;
        let down = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        egui::Window::new(theme::window_title(language.text("Find text files · Ctrl+P")))
            .id(egui::Id::new("file-search"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(660.0_f32.min(ctx.content_rect().width() - 48.0))
            .max_width((ctx.content_rect().width() - 48.0).max(260.0))
            .default_pos(ctx.content_rect().center() - Vec2::new(330.0, 250.0))
            .show(ctx, |ui| {
                ui.label(language.text("Search below the workspace folder"));
                ui.add(egui::Label::new(self.files.root.display().to_string()).truncate())
                    .on_hover_text(self.files.root.display().to_string());
                ui.horizontal(|ui| {
                    let response = ui.add_sized([ui.available_width() - 85.0, 28.0],
                        egui::TextEdit::singleline(&mut self.search_query)
                            .id(egui::Id::new("file-query"))
                            .hint_text(language.text("Relative path or filename…")));
                    if self.search_focus { response.request_focus(); self.search_focus = false; }
                    if response.changed() || self.search_revision != self.files.revision {
                        self.search_matches = text_files::filter(&self.files.scan.entries, &self.search_query);
                        self.search_revision = self.files.revision;
                        self.search_selected = 0;
                    }
                    if button(ui, language.text("Refresh")).clicked() {
                        self.files.restart(self.files.root.clone());
                        self.search_matches.clear();
                        self.search_selected = 0;
                    }
                });
                if self.files.scanning {
                    ui.horizontal(|ui| { ui.spinner(); ui.label(language.text("Indexing text files…")); });
                } else {
                    ui.label(language.format("{matches} matches · {total} supported files", &[
                        ("matches", &self.search_matches.len().to_string()), ("total", &self.files.scan.entries.len().to_string())]));
                }
                if self.files.scan.limited {
                    ui.colored_label(self.palette.accent, language.text("Partial index: scan limit reached. Restart MDcard with a file in a smaller folder to narrow the search."));
                }
                if self.files.scan.errors > 0 {
                    ui.label(language.format("{count} folders or entries could not be read", &[("count", &self.files.scan.errors.to_string())]))
                        .on_hover_text(self.files.scan.first_error.as_deref().unwrap_or_default());
                }
                ui.label(RichText::new(language.text("Skips links and build/dependency folders; UTF-8 text, up to 16 MiB per file.")).small())
                    .on_hover_text(text_files::SKIPPED_DIRECTORIES.join(", "));
                ui.separator();
                if !self.files.scanning && self.search_matches.is_empty() {
                    ui.label(language.text("No supported text files match. Use a descendant relative path; ../ and absolute paths are excluded."));
                }
                if down { self.search_selected = (self.search_selected + 1).min(self.search_matches.len().saturating_sub(1)); }
                if up { self.search_selected = self.search_selected.saturating_sub(1); }
                egui::ScrollArea::vertical().id_salt("file-results").max_height((ctx.content_rect().height() - 280.0).clamp(80.0, 330.0))
                    .auto_shrink([false; 2])
                    .show_rows(ui, 25.0, self.search_matches.len(), |ui, range| {
                        for row in range {
                            let entry = &self.files.scan.entries[self.search_matches[row]];
                            let response = ui.add_sized([ui.available_width(), 25.0],
                                egui::Button::new(RichText::new(&entry.label).monospace()).selected(row == self.search_selected));
                            if row == self.search_selected && (down || up) { response.scroll_to_me(None); }
                            if response.on_hover_text(&entry.label).clicked() { chosen = Some(self.files.root.join(&entry.relative)); }
                        }
                    });
                if enter && let Some(index) = self.search_matches.get(self.search_selected) {
                    chosen = Some(self.files.root.join(&self.files.scan.entries[*index].relative));
                }
            });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            open = false;
        }
        self.search_open = open;
        if let Some(path) = chosen {
            self.search_open = false;
            self.request(Action::OpenPath(path), ctx);
        }
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = self.palette;
        let expanded = self.sidebar && ctx.content_rect().width() >= 850.0;
        let response=egui::SidePanel::left("sidebar")
            .exact_width(theme::SIDEBAR_WIDTH)
            .resizable(false)
            .frame(egui::Frame::new().fill(p.sidebar).inner_margin(egui::Margin::symmetric(12, 12)))
            .show_animated(ctx, expanded, |ui| {
                let sidebar_height = ui.available_height();
                egui::ScrollArea::vertical().id_salt("workspace-sections").show(ui, |ui| {
                ui.horizontal(|ui| {
                    let folder = self.files.root.file_name().unwrap_or(self.files.root.as_os_str()).to_string_lossy();
                    ui.allocate_ui_with_layout(Vec2::new(ui.available_width()-80.0,32.0),egui::Layout::left_to_right(egui::Align::Center),|ui|ui.add(egui::Label::new(RichText::new(folder).size(13.0).strong()).truncate())).inner.on_hover_text(self.files.root.display().to_string());
                    if ui.add_enabled_ui(!self.global.busy,|ui| theme::icon_button(ui,theme::Icon::Open,language.text("Open folder…"))).inner.on_disabled_hover_text(language.text("Working in background…")).clicked() { self.choose_workspace(ctx); }
                    if theme::icon_button(ui,theme::Icon::Refresh,language.text("Refresh")).clicked() { self.files.restart(self.files.root.clone()); }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (i,key) in ["Files","Outline","Recent"].iter().enumerate() {
                        let r=ui.selectable_value(&mut self.navigation,i as u8,RichText::new(language.text(key)).size(12.0));
                        theme::record_control(ui,&r,&format!("navigation:{key}"));
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                if self.navigation == 0 {
                if self.files.scanning {
                    ui.horizontal(|ui| { ui.spinner(); ui.label(RichText::new(language.text("Indexing text files…")).small()); });
                } else {
                    ui.label(RichText::new(language.format("{count} supported files", &[("count", &self.files.scan.entries.len().to_string())])).small().color(p.muted));
                }
                if self.files.scan.limited {
                    ui.colored_label(p.accent, language.text("Partial index · scan limit reached"))
                        .on_hover_text(language.text("Partial index: scan limit reached. Restart MDcard with a file in a smaller folder to narrow the search."));
                }
                if self.files.scan.errors > 0 {
                    ui.label(RichText::new(language.format("{count} folders or entries could not be read", &[("count", &self.files.scan.errors.to_string())])).small())
                        .on_hover_text(self.files.scan.first_error.as_deref().unwrap_or_default());
                }
                self.tree.sync(&self.files);
                let mut chosen = None;
                let mut toggled = None;
                egui::ScrollArea::vertical().id_salt(("file-tree", &self.files.root))
                    .max_height((sidebar_height - 150.0).max(70.0))
                    .auto_shrink([false, true])
                    .show_rows(ui, 28.0, self.tree.rows.len(), |ui, range| {
                        ui.spacing_mut().interact_size.y = 28.0;
                        ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
                        for row in range {
                            let (id, depth) = self.tree.rows[row];
                            let node = &self.files.scan.tree[id];
                            let directory = node.file.is_none();
                            let label = if id == 0 {
                                self.files.root.file_name().unwrap_or(self.files.root.as_os_str()).to_string_lossy().into_owned()
                            } else { node.label.clone() };
                                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), egui::Sense::click());
                                let path = self.files.root.join(&node.relative);
                                let selected = !directory && self.doc.path.as_ref() == Some(&path);
                                if selected || response.hovered() {
                                    ui.painter().rect_filled(rect, 5, if selected { p.selected } else { p.selected.gamma_multiply(0.5) });
                                }
                                let icon = egui::Rect::from_min_size(rect.min + Vec2::new((depth as f32 * 12.0).min(84.0), 0.0), Vec2::new(27.0, 26.0));
                                let center = icon.center();
                                if directory {
                                    let x = icon.left() + 4.0;
                                    let points = if self.tree.expanded(node) {
                                        vec![egui::pos2(x - 3.0, center.y - 2.0), egui::pos2(x + 3.0, center.y - 2.0), egui::pos2(x, center.y + 2.0)]
                                    } else {
                                        vec![egui::pos2(x - 2.0, center.y - 3.0), egui::pos2(x - 2.0, center.y + 3.0), egui::pos2(x + 2.0, center.y)]
                                    };
                                    ui.painter().add(egui::Shape::convex_polygon(points, p.muted, Stroke::NONE));
                                    let folder = egui::Rect::from_min_size(egui::pos2(icon.left() + 12.0, center.y - 4.0), Vec2::new(13.0, 9.0));
                                    ui.painter().rect_filled(folder, 1, p.muted.gamma_multiply(0.65));
                                    ui.painter().rect_filled(egui::Rect::from_min_size(folder.min - Vec2::new(0.0, 2.0), Vec2::new(6.0, 3.0)), 1, p.muted.gamma_multiply(0.65));
                                } else {
                                    let paper = egui::Rect::from_center_size(center + Vec2::new(4.0, 0.0), Vec2::new(9.0, 12.0));
                                    ui.painter().rect_stroke(paper, 1, Stroke::new(1.0, p.muted), egui::StrokeKind::Inside);
                                }
                                let text_x = icon.right() + 4.0;
                                response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, &label));
                                theme::record_control(ui, &response, &format!("tree:{}", node.relative.to_string_lossy().replace('\\', "/")));
                                if response.has_focus() {ui.painter().rect_stroke(rect.shrink(1.),5,Stroke::new(1.5,p.accent),egui::StrokeKind::Inside);}
                                let galley = egui::WidgetText::from(RichText::new(label).size(12.0))
                                    .into_galley(ui, Some(egui::TextWrapMode::Truncate), (rect.right() - text_x - 4.0).max(0.0), FontId::proportional(12.0));
                                let text_y = rect.center().y - galley.size().y / 2.0;
                                ui.painter().galley(egui::pos2(text_x, text_y), galley, if selected { p.accent } else { p.ink });
                                if response.on_hover_text(path.display().to_string()).clicked() {
                                    if directory { toggled = Some(id); }
                                    else if !selected { chosen = Some(path); }
                                }
                        }
                    });
                if let Some(id) = toggled { self.tree.toggle(id, &self.files.scan.tree); }
                if let Some(path) = chosen { self.request(Action::OpenPath(path), ctx); }
                if !self.files.scanning && self.files.scan.entries.is_empty() {
                    ui.label(RichText::new(language.text("No supported text files in this folder.")).small().color(p.muted));
                }
                }
                if self.navigation == 1 {
                ui.label(RichText::new(self.doc.name(language)).size(12.0).color(p.muted));
                if !self.large.active && text_files::markdown(self.doc.path.as_deref()) { self.refresh_outline(ctx); }
                if !text_files::markdown(self.doc.path.as_deref()) || (!self.large.active && self.outline_cache.is_empty()) {
                    ui.add_space(12.0);
                    ui.label(RichText::new(language.text("Headings will appear here.")).size(12.0).color(p.muted));
                }
                if self.large.active && text_files::markdown(self.doc.path.as_deref()) {
                    let mut clicked=None;
                    let mut toc_controls=Vec::new();
                    if let Some(parsed)=&self.large.parsed {
                        let toc=egui::ScrollArea::vertical().id_salt("outline").max_height((sidebar_height-150.0).max(40.0)).show_rows(ui,28.,parsed.headings.len(),|ui,rows|{ui.spacing_mut().interact_size.y=24.;ui.spacing_mut().button_padding.y=4.;ui.spacing_mut().item_spacing.y=4.;
                            for index in rows {let(level,title,byte)=&parsed.headings[index];
                                ui.horizontal(|ui|{ui.add_space((level.saturating_sub(1)as f32*9.).min(36.));let response=ui.add(egui::Button::new(RichText::new(title).size(12.).color(p.ink)).frame(false));theme::record_control(ui,&response,&format!("outline:{byte}"));if std::env::var_os("MDCARD_PERFORMANCE_REPORT").is_some(){toc_controls.push(serde_json::json!({"title":title,"byte":byte,"rect":[response.rect.min.x,response.rect.min.y,response.rect.max.x,response.rect.max.y]}));}
if response.clicked(){clicked=Some(*byte);}});
                            }
                        });
                        self.large.toc_scrollbar=ctx.read_response(toc.id.with(1usize)).map(|r|r.rect);
                    }
                    self.large.toc_controls=toc_controls;
                    if let Some(byte)=clicked{self.large.jump=Some(byte);self.large.preview_jump=Some(byte);self.editor_focus=true;let id=egui::Id::new(("source-editor",self.document_generation));let mut state=egui::text_edit::TextEditState::load(ctx,id).unwrap_or_default();state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(self.large.rope.byte_to_char(byte)))));state.store(ctx,id);}
                } else { egui::ScrollArea::vertical()
                    .id_salt("outline")
                    .max_height((sidebar_height - 150.0).max(40.0))
                    .show(ui, |ui| {
                        ui.spacing_mut().interact_size.y = 24.0;
                        ui.spacing_mut().item_spacing.y = 4.0;
                        for (heading_index, (level, title)) in (if text_files::markdown(self.doc.path.as_deref()) {
                            self.outline_cache.clone()
                        } else {
                            vec![]
                        }).into_iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(((level.saturating_sub(1)) as f32 * 9.0).min(36.0));
                                let response=ui.add(
                                    egui::Button::new(
                                        RichText::new(&title).size(12.0).color(if level == 1 {
                                            p.ink
                                        } else {
                                            p.muted
                                        }),
                                    )
                                    .frame(false).wrap(),
                                );
                                theme::record_control(ui,&response,&format!("outline:{heading_index}"));
                                if response.clicked() {
                                    // Resolve through the Markdown parser, including setext
                                    // and formatted headings, instead of guessing a line prefix.
                                    let mut heading=None;
                                    let mut value=String::new();
                                    let mut index=0;
                                    for (event,range) in pulldown_cmark::Parser::new(&self.doc.text).into_offset_iter() {
                                        match event {
                                            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading{..})=>{heading=Some(range.start);value.clear();},
                                            pulldown_cmark::Event::Text(t)|pulldown_cmark::Event::Code(t) if heading.is_some()=>value.push_str(&t),
                                            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_))=>{
                                                if index==heading_index && value==title {let byte=heading.unwrap_or(0);self.global.jump=Some(search_ui::Jump{path:self.doc.path.clone(),start:byte,end:byte,expected:self.doc.text.clone()});self.mode=Mode::Edit;self.editor_focus=true;break;}
                                                heading=None;
                                                index+=1;
                                            },
                                            _=>{}
                                        }
                                    }
                                }
                            });
                            ui.add_space(5.0);
                        }
                    }); }
                }
                if self.navigation == 2 {
                if self.recent.is_empty() { ui.label(RichText::new(language.text("No recent documents.")).size(12.0).color(p.muted)); }
                if !self.recent.is_empty() {
                    ui.add_space(22.0);
                    ui.label(
                        RichText::new(language.text("RECENT"))
                            .size(11.0)
                            .strong()
                            .color(p.muted),
                    );
                    for path in self.recent.clone().into_iter().take(3) {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        let response=ui
                            .add(egui::Button::new(RichText::new(name).size(11.0)).frame(false))
                            .on_hover_text(path.display().to_string());
                        theme::record_control(ui,&response,&format!("recent:{}",path.file_name().unwrap_or_default().to_string_lossy()));
                        if response.clicked(){
                            self.request(Action::OpenPath(path), ctx);
                        }
                    }
                }
                }
                });
            });
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("ui-sidebar-width"),
                response.map_or(0., |r| r.response.rect.width()),
            )
        });
    }
    fn editor(&mut self, ui: &mut egui::Ui) {
        if self.global.jump.is_some() || self.large.jump.is_some() {
            self.reading.cancel("main/source", "navigation");
            self.reading.cancel("main/preview", "navigation");
        }
        self.large.ensure(
            &self.doc.text,
            self.document_generation,
            self.doc.revision(),
            ui.ctx(),
        );
        let phase_start = Instant::now();
        self.prepare_source_history(ui.ctx());
        let language = self.language;
        let p = self.palette;
        ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(
                    language.text(if text_files::markdown(self.doc.path.as_deref()) {
                        "MARKDOWN"
                    } else {
                        "PLAIN TEXT"
                    }),
                )
                .size(11.0)
                .strong()
                .color(p.muted),
            );
            if text_files::markdown(self.doc.path.as_deref())
                && ui
                    .small_button(language.text("Structure"))
                    .on_hover_text("Ctrl+Shift+T")
                    .clicked()
            {
                self.structure.open = !self.structure.open;
            }
            let automatic = self.doc.text.len() <= 256 * 1024;
            let label = match self.preferences.source_wrap {
                None => {
                    if automatic {
                        "Automatic · wrap"
                    } else {
                        "Automatic · no wrap"
                    }
                }
                Some(true) => "Wrap lines",
                Some(false) => "No wrapping",
            };
            let wrap_combo =
                egui::ComboBox::from_id_salt("source-line-wrap")
                    .width(110.)
                    .selected_text(language.text(label))
                    .show_ui(ui, |ui| {
                        let automatic_response = ui.selectable_value(
                            &mut self.preferences.source_wrap,
                            None,
                            language.text("Automatic wrapping"),
                        );
                        theme::record_control(ui, &automatic_response, "source-wrap-auto");
                        let wrap_response = ui.selectable_value(
                            &mut self.preferences.source_wrap,
                            Some(true),
                            language.text("Wrap lines"),
                        );
                        theme::record_control(ui, &wrap_response, "source-wrap-on");
                        let nowrap_response = ui.selectable_value(
                            &mut self.preferences.source_wrap,
                            Some(false),
                            language.text("No wrapping"),
                        );
                        theme::record_control(ui, &nowrap_response, "source-wrap-off");
                    })
                    .response
                    .on_hover_text(language.text(
                        "Long documents use horizontal scrolling to keep editing responsive.",
                    ));
            theme::record_control(ui, &wrap_combo, "source-wrap");
        });
        ui.add_space(8.0);
        let wrap = self
            .preferences
            .source_wrap
            .unwrap_or(self.doc.text.len() <= 256 * 1024);
        let source_key = "main/source";
        self.reading.layer(source_key, ui.layer_id());
        let source_id = egui::Id::new(("source-editor", self.document_generation));
        if let Some(selection) = self.reading.selection(&self.doc, source_key) {
            let mut state =
                egui::text_edit::TextEditState::load(ui.ctx(), source_id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(crate::reading_state::cursor(selection)));
            state.store(ui.ctx(), source_id);
            self.editor_focus = false;
        }
        if self.large.active {
            if let Some(jump) = self.global.jump.take()
                && jump.path.as_ref() == self.doc.path.as_ref()
                && jump.expected == self.doc.text
            {
                let mut state = egui::text_edit::TextEditState::load(
                    ui.ctx(),
                    egui::Id::new(("source-editor", self.document_generation)),
                )
                .unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(self.large.rope.byte_to_char(jump.start)),
                        egui::text::CCursor::new(self.large.rope.byte_to_char(jump.end)),
                    )));
                state.store(
                    ui.ctx(),
                    egui::Id::new(("source-editor", self.document_generation)),
                );
                self.large.jump = Some(jump.start);
                self.large.preview_jump = Some(jump.start);
                self.editor_focus = true;
            }
            let (response, changed) = self.large.show_source(
                ui,
                &mut self.doc,
                egui::Id::new(("source-editor", self.document_generation)),
                self.font_size,
                wrap,
                self.editor_focus,
                &mut self.reading,
                source_key,
                self.document_generation,
            );
            self.editor_focus = false;
            self.structure.source_rect = self.large.source_rect;
            if response.has_focus() {
                self.structure.cell_focus_owner = false;
            }
            if changed {
                self.global.buffer_changed();
                self.structure.dirty = true;
                self.structure.draft = None;
                self.status = "Unsaved changes";
            }
            self.rich
                .phase("sourceEditor", phase_start.elapsed().as_secs_f64() * 1000.);
            return;
        }
        let source_width = ui.available_width();
        let source_font = FontId::monospace(self.font_size);
        let source_color = ui.visuals().text_color();
        let source_layout = ui
            .ctx()
            .data_mut(|d| {
                d.remove_temp::<std::sync::Arc<std::sync::Mutex<source_layout::SourceLayout>>>(
                    egui::Id::new("source-layout-cache"),
                )
            })
            .unwrap_or_default();
        let source_layout_metrics = self.reading.layout_metrics.clone();
        let mut no_wrap = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _width: f32| {
            let started = std::time::Instant::now();
            let galley = source_layout.lock().unwrap().layout(
                ui,
                text.as_str(),
                source_font.clone(),
                source_color,
                if wrap { _width } else { f32::INFINITY },
            );
            reading_state::layout_sample(&source_layout_metrics, "source-text-layout", started);
            galley
        };
        let layout = format!("source:{:.1}:{}:{}", source_width, self.font_size, wrap);
        let navigation = self.global.jump.is_some();
        let restore = self.reading.plan(
            &self.doc,
            source_key,
            self.document_generation,
            &layout,
            ui.ctx(),
            ui.available_rect_before_wrap(),
            navigation,
        );
        let mut source_scroll = if wrap {
            egui::ScrollArea::vertical()
        } else {
            egui::ScrollArea::both()
        };
        source_scroll = source_scroll
            .id_salt(("source-scroll", self.doc.id, self.document_generation))
            .auto_shrink([false; 2]);
        if let Some(restore) = &restore {
            let target = self
                .reading
                .scroll_target(source_key, restore.position.offset);
            source_scroll = source_scroll
                .horizontal_scroll_offset(target[0])
                .vertical_scroll_offset(target[1]);
        }
        let mut source_galleys = Vec::new();
        let mut source_anchor = (0, 0., 0.);
        let mut restore_target = None;
        let source_output = source_scroll.show_viewport(ui, |ui, viewport| {
            let edit = egui::TextEdit::multiline(&mut self.doc.text)
                .id(egui::Id::new(("source-editor", self.document_generation)))
                .font(FontId::monospace(self.font_size))
                .desired_width(source_width)
                .desired_rows(40)
                .frame(false)
                .hint_text(language.text("# A new idea\n\nStart writing…"))
                .lock_focus(true);
            let output = edit.layouter(&mut no_wrap).show(ui);
            let cursor = output
                .galley
                .cursor_from_pos(egui::vec2(0., viewport.min.y));
            let byte = self.reading.byte_at_char(&self.doc, cursor.index);
            let y = output.galley.pos_from_cursor(cursor).top();
            source_anchor = (byte, 0., y - viewport.min.y);
            if let Some(restore) = &restore {
                let target = if restore.exact {
                    restore.position.offset
                } else {
                    let character = self
                        .reading
                        .char_at_byte(&self.doc, restore.position.anchor.byte);
                    let y = output
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(character))
                        .top()
                        - restore.position.anchor.viewport_y;
                    [restore.position.offset[0], y.max(0.)]
                };
                restore_target = Some(target);
            }
            source_galleys.push(self.reading.geometry_cache.linear(
                output.galley.clone(),
                output.galley_pos,
                0,
            ));
            let response = output.response;
            if let Some(jump) = self.global.jump.take()
                && jump.path.as_ref() == self.doc.path.as_ref()
                && jump.expected == self.doc.text
            {
                let start = self.doc.text[..jump.start].chars().count();
                let end = self.doc.text[..jump.end].chars().count();
                let mut state = output.state.clone();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(start),
                        egui::text::CCursor::new(end),
                    )));
                state.store(ui.ctx(), response.id);
                let cursor_rect = output
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(start))
                    .translate(output.galley_pos.to_vec2());
                ui.scroll_to_rect(cursor_rect, Some(egui::Align::Center));
                response.request_focus();
            }
            self.structure.source_rect = Some(response.rect);
            if self.editor_focus {
                response.request_focus();
                self.editor_focus = false;
            }
            if response.has_focus() {
                self.structure.cell_focus_owner = false;
            }
            if response.changed() {
                self.doc.mark_text_changed();
                self.global.buffer_changed();
                self.structure.dirty = true;
                self.structure.draft = None;
                self.status = "Unsaved changes";
            }
        });
        let mut scroll_state = source_output.state;
        self.reading.viewport(source_key, source_output.inner_rect);
        let precise = self.reading.geometry_frame(
            &self.doc,
            source_key,
            &source_galleys,
            source_output.inner_rect,
            [source_output.state.offset.x, source_output.state.offset.y],
            [
                (source_output.content_size.x - source_output.inner_rect.width()).max(0.),
                (source_output.content_size.y - source_output.inner_rect.height()).max(0.),
            ],
            ui.ctx(),
        );
        if precise.is_some() {
            restore_target = precise;
        }
        if let Some(target) = restore_target {
            let commit_started = std::time::Instant::now();
            scroll_state.offset = egui::vec2(target[0], target[1]);
            scroll_state.store(ui.ctx(), source_output.id);
            self.reading.target(source_key, target, ui.ctx());
            self.reading.commit_sample(commit_started);
        }
        let selection = egui::text_edit::TextEditState::load(ui.ctx(), source_id)
            .and_then(|s| s.cursor.char_range())
            .unwrap_or_default();
        self.reading.store_selection(
            &self.doc,
            source_key,
            [selection.primary.index, selection.secondary.index],
        );
        if restore_target.is_none() || !self.reading.restoring(source_key) {
            self.reading.observe(
                &self.doc,
                source_key,
                layout,
                [scroll_state.offset.x, scroll_state.offset.y],
                [
                    source_output.inner_rect.width(),
                    source_output.inner_rect.height(),
                ],
                source_anchor.0,
                source_anchor.1,
                source_anchor.2,
                source_output
                    .inner_rect
                    .contains(ui.ctx().pointer_hover_pos().unwrap_or_default()),
            );
        }
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("source-layout-cache"), source_layout));
        self.rich
            .phase("sourceEditor", phase_start.elapsed().as_secs_f64() * 1000.);
    }
    fn preview(&mut self, ui: &mut egui::Ui) {
        self.large.ensure(
            &self.doc.text,
            self.document_generation,
            self.doc.revision(),
            ui.ctx(),
        );
        if self.large.active {
            if self.large.installed_revision != Some(self.large.revision)
                && let Some(parsed) = &mut self.large.parsed
                && parsed.revision == self.large.revision
            {
                self.rich.install_incremental(parsed, &self.doc);
                self.large.installed_revision = Some(parsed.revision);
            }
            self.rich.poll_results(ui.ctx());
        } else {
            self.rich
                .ensure_document(&self.doc, self.document_generation, ui.ctx());
        }
        let language = self.language;
        let p = self.palette;
        ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(
                    language.text(if text_files::markdown(self.doc.path.as_deref()) {
                        "LIVE PREVIEW"
                    } else {
                        "PLAIN TEXT"
                    }),
                )
                .size(11.0)
                .strong()
                .color(p.muted),
            );
            ui.allocate_space(Vec2::new(0.0, theme::CONTROL_HEIGHT));
        });
        ui.add_space(8.0);
        if self.large.active && !text_files::markdown(self.doc.path.as_deref()) {
            self.large.show_plain_preview(
                ui,
                self.font_size,
                &self.doc,
                &mut self.reading,
                "main/preview",
                self.document_generation,
            );
            return;
        }
        let reading_key = "main/preview";
        self.reading.layer(reading_key, ui.layer_id());
        let reading_layout = format!("preview:{:.1}:{}", ui.available_width(), self.font_size);
        let reading_restore = self.reading.plan(
            &self.doc,
            reading_key,
            self.document_generation,
            &reading_layout,
            ui.ctx(),
            ui.available_rect_before_wrap(),
            self.large.preview_jump.is_some()
                || self.global.jump.is_some()
                || self.large.sync_from_source,
        );
        let markdown = text_files::markdown(self.doc.path.as_deref());
        if (self.large.active
            && self
                .large
                .parsed
                .as_ref()
                .is_none_or(|p| p.revision != self.large.revision))
            || (markdown && !self.rich.source_ready(&self.doc, self.document_generation))
        {
            self.reading.await_source_layout(reading_key);
            ui.spinner();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }
        if text_files::markdown(self.doc.path.as_deref()) {
            if let Some(heights) =
                self.reading
                    .take_calibration(&self.doc, reading_key, &reading_layout)
            {
                self.rich.restore_reading_calibration(
                    ui.available_width(),
                    self.font_size,
                    heights,
                );
            }
            let _ =
                self.rich
                    .visible_blocks(ui.available_width(), self.font_size, egui::Rect::ZERO);
        }
        let mut scroll = egui::ScrollArea::both()
            .id_salt(("preview-scroll", self.doc.id, self.document_generation))
            .auto_shrink([false; 2]);
        let mut reading_target = reading_restore.as_ref().map(|restore| {
            if restore.exact {
                restore.position.offset
            } else {
                [
                    restore.position.offset[0],
                    self.rich.reading_y(
                        restore.position.anchor.byte,
                        restore.position.anchor.fraction,
                        restore.position.anchor.viewport_y,
                    ),
                ]
            }
        });
        reading_target =
            reading_target.map(|target| self.reading.scroll_target(reading_key, target));
        if let Some(target) = reading_target {
            let text_bootstrap = self
                .reading
                .bootstrap_text_byte(reading_key)
                .filter(|_| reading_restore.as_ref().is_some_and(|r| !r.exact));
            let initial_y = reading_restore
                .as_ref()
                .filter(|restore| {
                    self.reading.bootstrap_async(reading_key)
                        && self
                            .rich
                            .anchor_has_async_content(restore.position.anchor.byte)
                })
                .map_or(target[1], |restore| {
                    self.rich.reading_y(restore.position.anchor.byte, 0., 0.)
                });
            let initial_y =
                text_bootstrap.map_or(initial_y, |byte| self.rich.text_bootstrap_y(byte));
            scroll = scroll
                .horizontal_scroll_offset(target[0])
                .vertical_scroll_offset(initial_y);
            self.large.sync_from_source = false;
            self.large.sync_from_preview = false;
        }
        let driven_by_source =
            self.large.active && (self.large.preview_jump.is_some() || self.large.sync_from_source);
        let mut preview_anchor_delta = 0.;
        let mut preview_galleys = Vec::new();
        let preview_source_pin = self.reading.precise_byte(reading_key);
        let mut image_pending = false;
        let mut image_errors = Vec::new();
        let mut plain_anchor = (0, 0., 0.);
        if self.large.active {
            if let Some(byte) = self.large.preview_jump.take() {
                scroll = scroll.vertical_scroll_offset(self.large.preview_y(byte));
            } else if self.large.sync_from_source {
                scroll =
                    scroll.vertical_scroll_offset(self.large.preview_y(self.large.source_anchor));
                self.large.sync_from_source = false;
            }
        }
        let preview_scroll = scroll.show_viewport(ui, |ui, viewport| {
            // Overflowing tables/code can expand this Ui as blocks are mounted.
            // Keep every block tied to the viewport, not that expanded content width.
            let content_width = viewport.width();
            if !self.large.active && !text_files::markdown(self.doc.path.as_deref()) {
                let layout_started = std::time::Instant::now();
                let galley = ui.fonts_mut(|f| {
                    f.layout(
                        self.doc.text.clone(),
                        FontId::monospace(self.font_size),
                        ui.visuals().text_color(),
                        content_width,
                    )
                });
                reading_state::layout_sample(
                    &self.reading.layout_metrics,
                    "plain-text-font-layout",
                    layout_started,
                );
                let cursor = galley.cursor_from_pos(egui::vec2(0., viewport.min.y));
                plain_anchor = (
                    self.reading.byte_at_char(&self.doc, cursor.index),
                    0.,
                    galley.pos_from_cursor(cursor).top() - viewport.min.y,
                );
                if let Some(restore) = &reading_restore
                    && !restore.exact
                {
                    let ch = self
                        .reading
                        .char_at_byte(&self.doc, restore.position.anchor.byte);
                    reading_target = Some([
                        restore.position.offset[0],
                        (galley.pos_from_cursor(egui::text::CCursor::new(ch)).top()
                            - restore.position.anchor.viewport_y)
                            .max(0.),
                    ]);
                }
                let pos = ui.next_widget_position();
                preview_galleys.push(self.reading.geometry_cache.linear(galley.clone(), pos, 0));
                ui.add(egui::Label::new(galley).selectable(true));
                return;
            }
            ui.style_mut().url_in_tooltip = true;
            let widgets = &mut ui.style_mut().visuals.widgets;
            for widget in [
                &mut widgets.noninteractive,
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                widget.corner_radius = 3.into();
            }
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(self.font_size));
            let base = self
                .doc
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let prefix = text_files::image_base_uri(&base);
            let rich = std::rc::Rc::new(std::cell::RefCell::new(std::mem::take(&mut self.rich)));
            let callback_rich = rich.clone();
            let font = self.font_size;
            let render_node = move |ui: &mut egui::Ui, token: &str, _inline: bool| {
                callback_rich
                    .borrow_mut()
                    .node(ui, token, p.dark, font, language)
            };
            let (blocks, top, total) = rich.borrow_mut().visible_blocks_around(
                content_width,
                font,
                viewport,
                preview_source_pin,
            );
            let anchor_index = rich.borrow().block_at(viewport.min.y);
            let anchor_y = rich.borrow().block_offset(anchor_index);
            if self.large.active && self.large.preview_height_stamp != rich.borrow().height_stamp {
                self.large.heights = rich.borrow().height_index();
                self.large.preview_height_stamp = rich.borrow().height_stamp;
            }
            ui.set_min_height(total.max(viewport.height()));
            if self.large.active {
                self.large.preview_mounted = blocks.len();
                self.large.preview_range = blocks
                    .first()
                    .map_or(0..0, |(first, _)| *first..blocks.last().unwrap().0 + 1);
            }
            let origin = ui.max_rect().min;
            let mut block_y = top;
            for (block_index, block) in blocks {
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(0., block_y),
                    egui::vec2(content_width, block.height),
                );
                {
                    let snippet = rich.borrow().preview[block.range.clone()].to_owned();
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                        |ui| {
                            let before = ui.cursor().min.y;
                            let block_layout_started = std::time::Instant::now();
                            if let Some(page) = &block.code {
                                if let Some((galley, pos)) =
                                    rich.borrow_mut().code_page(ui, page, font, p.dark)
                                {
                                    let mut mapped =
                                        self.reading.geometry_cache.linear(galley, pos, 0);
                                    mapped.segments = block
                                        .mapping
                                        .iter()
                                        .map(|s| crate::reading_geometry::Segment {
                                            rendered: s.preview.clone(),
                                            source: s.source.clone(),
                                            linear: s.linear,
                                        })
                                        .collect();
                                    preview_galleys.push(mapped);
                                }
                            } else {
                                let (_, painted, assets) = ui
                                    .push_id(("preview-block", block.range.start), |ui| {
                                        CommonMarkViewer::new()
                                            .render_math_fn(Some(&render_node))
                                            .syntax_theme_light("InspiredGitHub")
                                            .syntax_theme_dark("base16-ocean.dark")
                                            .max_image_width(Some(ui.available_width() as usize))
                                            .default_implicit_uri_scheme(prefix.clone())
                                            .show_with_geometry_assets(
                                                ui,
                                                &mut self.cache,
                                                snippet.trim_start_matches('\u{feff}'),
                                            )
                                    })
                                    .inner;
                                for asset in assets {
                                    image_pending |= asset.pending;
                                    if let Some(error) = asset.error {
                                        image_errors.push(format!("{}: {}", asset.uri, error));
                                    }
                                }
                                let trimmed = snippet.trim_start_matches('\u{feff}');
                                let bom = snippet.len() - trimmed.len();
                                for actual in painted {
                                    let mapped = self.reading.geometry_cache.markdown(
                                        actual,
                                        trimmed,
                                        &|range| {
                                            rich.borrow().source_range_in(
                                                &block,
                                                bom + range.start..bom + range.end,
                                            )
                                        },
                                    );
                                    preview_galleys.push(mapped);
                                }
                            }
                            reading_state::layout_sample(
                                &self.reading.layout_metrics,
                                if block.code.is_some() {
                                    "code-page-layout-paint"
                                } else {
                                    "markdown-block-layout-paint"
                                },
                                block_layout_started,
                            );
                            rich.borrow_mut()
                                .block_height(block.key, (ui.cursor().min.y - before).max(font));
                        },
                    );
                }
                block_y += rich.borrow().heights_for_block(block.key, block.height);
                if self.large.active && block_index < self.large.heights.len() {
                    self.large.heights.set(
                        block_index,
                        rich.borrow().heights_for_block(block.key, block.height),
                    );
                }
            }
            drop(render_node);
            preview_anchor_delta = rich.borrow().block_offset(anchor_index) - anchor_y;
            self.rich = std::rc::Rc::try_unwrap(rich).ok().unwrap().into_inner();
        });
        let mut scroll_state = preview_scroll.state;
        self.reading
            .viewport(reading_key, preview_scroll.inner_rect);
        if preview_anchor_delta.abs() > 0.01 {
            scroll_state.offset.y = (scroll_state.offset.y + preview_anchor_delta).max(0.);
            scroll_state.store(ui.ctx(), preview_scroll.id);
            ui.ctx().request_repaint();
        }
        if let Some(restore) = &reading_restore
            && !restore.exact
            && text_files::markdown(self.doc.path.as_deref())
        {
            reading_target = Some([
                restore.position.offset[0],
                self.rich.reading_y(
                    restore.position.anchor.byte,
                    restore.position.anchor.fraction,
                    restore.position.anchor.viewport_y,
                ),
            ]);
        }
        self.reading.layout_readiness(
            reading_key,
            image_pending || self.rich.layout_pending(),
            image_errors,
        );
        let precise = self.reading.geometry_frame(
            &self.doc,
            reading_key,
            &preview_galleys,
            preview_scroll.inner_rect,
            [preview_scroll.state.offset.x, preview_scroll.state.offset.y],
            [
                (preview_scroll.content_size.x - preview_scroll.inner_rect.width()).max(0.),
                (preview_scroll.content_size.y - preview_scroll.inner_rect.height()).max(0.),
            ],
            ui.ctx(),
        );
        if precise.is_some() {
            reading_target = precise;
        }
        reading_target =
            reading_target.map(|target| self.reading.reachable_target(reading_key, target));
        if let Some(target) = reading_target {
            let commit_started = std::time::Instant::now();
            scroll_state.offset = egui::vec2(target[0], target[1]);
            scroll_state.store(ui.ctx(), preview_scroll.id);
            self.reading.target(reading_key, target, ui.ctx());
            self.reading.commit_sample(commit_started);
        }
        if reading_target.is_none() || !self.reading.restoring(reading_key) {
            if text_files::markdown(self.doc.path.as_deref())
                && self
                    .reading
                    .needs_calibration(reading_key, self.rich.height_stamp)
            {
                self.reading.calibration(
                    &self.doc,
                    reading_key,
                    &reading_layout,
                    self.rich.height_stamp,
                    self.rich.reading_calibration(),
                );
            }
            let anchor = if text_files::markdown(self.doc.path.as_deref()) {
                self.rich.reading_anchor(scroll_state.offset.y)
            } else {
                plain_anchor
            };
            self.reading.observe(
                &self.doc,
                reading_key,
                reading_layout,
                [scroll_state.offset.x, scroll_state.offset.y],
                [
                    preview_scroll.inner_rect.width(),
                    preview_scroll.inner_rect.height(),
                ],
                anchor.0,
                anchor.1,
                anchor.2,
                preview_scroll
                    .inner_rect
                    .contains(ui.ctx().pointer_hover_pos().unwrap_or_default()),
            );
        }
        if self.large.active {
            self.large.preview_rect = Some(preview_scroll.inner_rect);
            if std::env::var_os("MDCARD_PERFORMANCE_REPORT").is_some() {
                self.large.preview_scrollbar = ui
                    .ctx()
                    .read_response(preview_scroll.id.with(1usize))
                    .map(|r| r.rect);
            }
            let offset = scroll_state.offset.y;
            if (offset - self.large.last_preview_offset).abs() > 0.5 {
                self.large.last_preview_offset = offset;
                self.large.preview_anchor = self.large.preview_byte(offset);
                if !driven_by_source && reading_target.is_none() {
                    self.large.sync_from_preview = true;
                }
            }
        }
    }
    fn show_encoding(&mut self, target: Option<PathBuf>, ctx: &egui::Context) {
        if !self.flush_rich_edits(ctx) {
            return;
        }
        self.encoding_choice = self.doc.encoding;
        self.encoding_bom = self.doc.bom;
        self.encoding_target = target;
        self.encoding_open = true;
    }
    fn encoding_dialog(&mut self, ctx: &egui::Context) {
        if !self.encoding_open || self.error.is_some() {
            return;
        }
        let language = self.language;
        let current_document = self.encoding_target == self.doc.path;
        let mut reopen = false;
        let mut use_for_saving = false;
        let result = theme::modal(ctx, "encoding").show(ctx, |ui| {
            ui.set_width(420.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.label(RichText::new(language.text("Text encoding")).size(20.0).strong());
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 150.0).max(140.0)).show(ui, |ui| {
                ui.label(language.text("Reopen reads the file again. Save encoding converts your current text on the next save."));
                ui.add_space(8.0);
                if let Some(path) = &self.encoding_target { ui.add(egui::Label::new(path.display().to_string()).wrap()); }
                if current_document {
                ui.label(format!("{}: {}{}", language.text("Current encoding"), self.doc.encoding.name(),
                    if self.doc.guessed { format!(" · {}", language.text("Guessed")) } else { String::new() }));
                if self.doc.guessed { ui.label(language.text("Legacy encoding detection is a guess. Check the text and reopen manually if needed.")); }
                } else { ui.label(language.text("Choose the encoding used to read this file.")); }
                let previous = self.encoding_choice;
                egui::Grid::new("text-encoding-choice").num_columns(3).spacing([8.0, 6.0]).show(ui, |ui| {
                    for (index, encoding) in Encoding::ALL.into_iter().enumerate() {
                        ui.selectable_value(&mut self.encoding_choice, encoding, encoding.name());
                        if index % 3 == 2 { ui.end_row(); }
                    }
                });
                if previous != self.encoding_choice {
                    self.encoding_bom = matches!(self.encoding_choice, Encoding::Utf8Bom | Encoding::Utf16Le | Encoding::Utf16Be);
                }
                if current_document && matches!(self.encoding_choice, Encoding::Utf16Le | Encoding::Utf16Be) { ui.checkbox(&mut self.encoding_bom, language.text("Write BOM")); }
                ui.add_space(8.0);
                ui.label(language.text("Reopen discards edits only after the unsaved changes dialog. Conversion never replaces unsupported characters."));
            });
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if button(ui, language.text("Cancel")).clicked() { self.encoding_open = false; }
                    if ui.add_enabled(self.encoding_target.is_some(), egui::Button::new(language.text("Reopen with encoding"))).clicked() { reopen = true; }
                    if current_document && button(ui, language.text("Use for saving")).clicked() { use_for_saving = true; }
                });
        });
        if result.should_close() {
            self.encoding_open = false;
        }
        if reopen {
            self.encoding_open = false;
            if let Some(path) = self.encoding_target.clone() {
                self.request(Action::Reopen(path, self.encoding_choice), ctx);
            }
        }
        if use_for_saving {
            if !self.flush_rich_edits(ctx) {
                return;
            }
            let bom = matches!(self.encoding_choice, Encoding::Utf8Bom)
                || (matches!(self.encoding_choice, Encoding::Utf16Le | Encoding::Utf16Be)
                    && self.encoding_bom);
            match self.doc.set_encoding(self.encoding_choice, bom) {
                Ok(()) => {
                    self.encoding_open = false;
                    self.status = "Save encoding changed · save to apply";
                }
                Err(error) => {
                    self.failed_open = None;
                    self.error = Some(localized_document_error(Language::English, &error))
                }
            }
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        let language = self.language;
        let p = self.palette;
        if self.rename_open {
            let mut apply = false;
            let mut cancel = false;
            theme::modal(ctx, "rename-document").show(ctx, |ui| {
                ui.set_width(360.);
                ui.label(RichText::new(language.text("Rename document…")).size(20.));
                ui.label(language.text("New filename in the same folder"));
                let response = ui.text_edit_singleline(&mut self.rename_name);
                theme::record_control(ui, &response, "rename-filename");
                ui.horizontal(|ui| {
                    apply = button(ui, language.text("Rename")).clicked();
                    cancel = button(ui, language.text("Cancel")).clicked();
                });
            });
            if cancel {
                self.rename_open = false;
            }
            if apply && self.flush_rich_edits(ctx) {
                let old = self.doc.path.clone();
                match self.doc.rename_in_place(&self.rename_name) {
                    Ok(()) => {
                        if let Some(old) = old {
                            self.reading.migrated_path(&self.doc, &old);
                            let old_key = reading_state::path_key(&old);
                            for path in &mut self.recent {
                                if reading_state::path_key(path) == old_key {
                                    *path = self.doc.path.clone().unwrap();
                                }
                            }
                            if let Some(other) = &mut self.global.secondary
                                && other.id == self.doc.id
                            {
                                other.path = self.doc.path.clone();
                            }
                        }
                        self.rename_open = false;
                        self.files.restart(self.files.root.clone());
                        self.status = "Opened";
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        self.rename_open = false;
                    }
                }
            }
        }
        if self.settings_open {
            let result = theme::modal(ctx, "settings").show(ctx, |ui| {
                ui.set_width(390.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.label(
                    RichText::new(language.text("Preferences"))
                        .size(20.0)
                        .strong(),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 160.0).max(160.0))
                    .show(ui, |ui| self.preferences_ui(ui));
                ui.add_space(8.0);
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, language.text("Done")).clicked() {
                        self.settings_open = false;
                    }
                });
            });
            if result.should_close() {
                self.settings_open = false;
            }
        }
        self.encoding_dialog(ctx);
        if self.pending.is_some() {
            let mut decision = None;
            theme::modal(ctx, "unsaved").show(ctx, |ui| {
                let fade = ctx.animate_bool_with_time(
                    egui::Id::new("unsaved-fade"),
                    true,
                    ui.style().animation_time,
                );
                ui.multiply_opacity(fade);
                ui.set_width(380.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.label(
                    RichText::new(language.text("Keep your changes?"))
                        .size(20.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(language.format(
                    "“{name}” has unsaved changes. Save them before continuing.",
                    &[("name", &self.doc.name(self.language))],
                ));
                ui.add_space(18.0);
                ui.horizontal_wrapped(|ui| {
                    if button(ui, language.text("Cancel")).clicked() {
                        decision = Some(0);
                    }
                    if button(ui, language.text("Discard")).clicked() {
                        decision = Some(1);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new(language.text("Save changes")).color(
                                if p.dark {
                                    Color32::from_rgb(20, 22, 28)
                                } else {
                                    Color32::WHITE
                                },
                            ))
                            .fill(p.accent),
                        )
                        .clicked()
                    {
                        decision = Some(2);
                    }
                });
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    decision = Some(0);
                }
            });
            match decision {
                Some(0) => {
                    self.pending = None;
                    self.restore_edit_focus();
                }
                Some(1) => {
                    let action = self.pending.take().unwrap();
                    self.perform(action, ctx);
                }
                Some(2) => {
                    if self.save(false, ctx) {
                        let action = self.pending.take().unwrap();
                        self.perform(action, ctx);
                    }
                }
                _ => {}
            }
        }
        if let Some(message) = self.error.clone() {
            theme::modal(ctx, "error").show(ctx, |ui| {
                ui.set_width(420.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.label(
                    RichText::new(language.text("Something needs attention"))
                        .size(20.0)
                        .strong(),
                );
                ui.add_space(10.0);
                ui.label(localized_message(language, &message));
                if let Some(path) = self.failed_open.clone()
                    && button(ui, language.text("Choose encoding…")).clicked()
                {
                    self.error = None;
                    self.show_encoding(Some(path), ctx);
                }
                ui.add_space(12.0);
                if button(ui, language.text("OK")).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    self.error = None;
                }
            });
        }
    }
}

impl eframe::App for Editor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut _full_update = update_probe::UpdateGuard::start();
        _full_update.tag(
            ctx.cumulative_frame_nr(),
            ctx.cumulative_pass_nr(),
            ctx.input(|i| i.raw_scroll_delta != egui::Vec2::ZERO),
        );
        let primary = egui::Id::new(("source-editor", self.document_generation));
        let secondary = self
            .global
            .secondary
            .as_ref()
            .map(|_| egui::Id::new(("second-editor", self.global.secondary_identity())));
        for key in ["search", "history", "structure", "export", "rich"] {
            theme::begin_window_focus(ctx, key, primary, secondary);
        }
        theme::begin_modal_focus(ctx);
        if std::env::var_os("MDCARD_UI_REPORT").is_some() {
            ctx.data_mut(|d| {
                d.insert_temp(
                    egui::Id::new("ui-report-controls"),
                    Vec::<serde_json::Value>::new(),
                )
            });
        }
        self.large.begin_frame(ctx);
        if self.reading.restoring("main/source") || self.reading.restoring("main/preview") {
            self.large.sync_from_source = false;
            self.large.sync_from_preview = false;
        }
        self.large.ensure(
            &self.doc.text,
            self.document_generation,
            self.doc.revision(),
            ctx,
        );
        self.global.begin_frame();
        self.poll_global_search(ctx);
        if self.global.busy {
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.);
                    ui.spinner();
                    ui.label(self.language.text("Working in background…"));
                });
            });
            self.global_search_windows(ctx);
            ctx.request_repaint_after(std::time::Duration::from_millis(20));
            return;
        }
        let structure_frame_start = Instant::now();
        self.rich.record_input(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.flush_rich_edits(ctx) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let structure_commands_before = self.structure.command_count();
        let measure_previous_structure = self.structure.measure_next;
        self.structure.measure_next = false;
        self.native_icons.update(_frame);
        self.native_corners.update(
            _frame,
            ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
        );
        self.sync_language(ctx);
        if self.global.secondary_focused(ctx) {
            self.last_secondary = true;
        } else if ctx
            .memory(|m| m.has_focus(egui::Id::new(("source-editor", self.document_generation))))
        {
            self.last_secondary = false;
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::E,
            )
        }) {
            self.open_export(ctx);
        }
        let mut protected = self
            .global
            .secondary
            .as_ref()
            .and_then(|d| d.path.clone())
            .into_iter()
            .collect::<Vec<_>>();
        protected.extend(self.doc.path.clone());
        if self.export.open && !self.export.secondary && !self.flush_rich_edits(ctx) {
            self.export.open = false;
        }
        let export_doc = if self.export.secondary {
            self.global.secondary.as_ref().unwrap_or(&self.doc)
        } else {
            &self.doc
        };
        self.export.show(
            ctx,
            self.language,
            &export_doc.text,
            export_doc.path.as_deref(),
            if self.export.secondary {
                if self.global.secondary.is_some() {
                    self.global.secondary_identity()
                } else {
                    u64::MAX
                }
            } else {
                self.document_generation
            },
            &protected,
            self.palette,
        );
        self.files.poll();
        let language = self.language;
        self.animate_theme(ctx);
        let p = self.palette;
        if self.pending.is_none() {
            ctx.animate_bool_with_time(egui::Id::new("unsaved-fade"), false, 0.0);
        }
        for (id, open) in [
            ("settings", self.settings_open),
            ("encoding", self.encoding_open),
            ("unsaved", self.pending.is_some()),
            ("error", self.error.is_some()),
        ] {
            if !open {
                ctx.animate_bool_with_time(egui::Id::new((id, "fade")), false, 0.0);
            }
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && (self.doc.dirty() || self.global.secondary.as_ref().is_some_and(|d| d.dirty()))
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Action::Quit, ctx);
        }
        for path in ctx
            .input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .take(1)
        {
            self.request(Action::OpenPath(path), ctx);
        }
        self.poll_global_search(ctx);
        self.shortcuts(ctx);
        if !self.global.busy {
            self.structure_shortcuts(ctx);
        }
        let title = format!(
            "{}{} — MDcard",
            if self.doc.presentation_dirty() {
                "● "
            } else {
                ""
            },
            self.doc.name(self.language)
        );
        let title_changed = ctx.data_mut(|data| {
            let id = egui::Id::new("native-window-title");
            if data.get_temp::<String>(id).as_ref() == Some(&title) {
                false
            } else {
                data.insert_temp(id, title.clone());
                true
            }
        });
        // send_viewport_cmd itself requests another repaint, so sending an
        // unchanged title every frame creates a permanent native event loop.
        if title_changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        self.titlebar(ctx);
        self.toolbar(ctx);
        egui::TopBottomPanel::bottom("status")
            .exact_height(32.0)
            .frame(
                egui::Frame::new()
                    .fill(p.chrome)
                    .inner_margin(egui::Margin::symmetric(16, 5))
                    .stroke(Stroke::new(0.5, p.line)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(language.text(self.status_key()))
                                .size(11.0)
                                .color(p.muted),
                        )
                        .truncate(),
                    );
                    if self.reading.stats.error.is_some()
                        && button(ui, language.text("Reading position not saved")).clicked()
                    {
                        self.settings_open = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(24.0);
                        let label = format!(
                            "{}{}",
                            self.doc.encoding.name(),
                            if self.doc.guessed {
                                format!(" · {}", language.text("Guessed"))
                            } else {
                                String::new()
                            }
                        );
                        if ui
                            .small_button(label)
                            .on_hover_text(format!(
                                "{} · {}",
                                language.text("Text encoding…"),
                                self.doc.name(language)
                            ))
                            .clicked()
                        {
                            self.show_encoding(self.doc.path.clone(), ctx);
                        }
                        let (lines, characters) = if self.large.active {
                            (self.large.rope.len_lines(), self.large.rope.len_chars())
                        } else {
                            self.doc.text_statistics()
                        };
                        let lines = lines.to_string();
                        let characters = characters.to_string();
                        let renderer =
                            if self.preferences.gpu_acceleration != self.requested_gpu_at_start {
                                language.text("Restart required")
                            } else if self.running_gpu {
                                "OpenGL"
                            } else {
                                "CPU"
                            };
                        let full = language.format(
                            "{renderer}    {lines} lines    {characters} characters",
                            &[
                                ("renderer", renderer),
                                ("lines", &lines),
                                ("characters", &characters),
                            ],
                        );
                        let shown = if ctx.content_rect().width() < 1000. {
                            format!("{lines} · {characters}")
                        } else {
                            language.format(
                                "{lines} lines · {characters} characters",
                                &[("lines", &lines), ("characters", &characters)],
                            )
                        };
                        ui.add(
                            egui::Label::new(RichText::new(shown).size(11.).color(p.muted))
                                .truncate(),
                        )
                        .on_hover_text(full);
                        if self.global.secondary.is_some() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(language.text("Main document"))
                                        .size(11.0)
                                        .color(p.muted),
                                )
                                .truncate(),
                            )
                            .on_hover_text(self.doc.name(language));
                        }
                    });
                });
            });
        self.sidebar(ctx);
        self.secondary_panel(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.paper)
                    .inner_margin(egui::Margin::symmetric(theme::CONTENT_MARGIN as i8, 8)),
            )
            .show(ctx, |ui| {
                let pressed=ctx.input(|i|i.pointer.any_pressed().then(||i.pointer.interact_pos()).flatten());
                if pressed.is_some_and(|pos|ui.max_rect().contains(pos)&&ctx.layer_id_at(pos)==Some(ui.layer_id())) {
                    self.last_secondary = false;
                    if self.global.secondary.is_some() {ctx.memory_mut(|m|m.surrender_focus(egui::Id::new(("second-editor",self.global.secondary_identity()))));}
                }
                let toolbar_can_split=ctx.data(|d|d.get_temp::<bool>(egui::Id::new("ui-toolbar-can-split"))).unwrap_or(false);
                let actual_can_split=ui.available_width()>=theme::SPLIT_CONTENT_MIN;
                ctx.data_mut(|d|d.insert_temp(egui::Id::new("ui-primary-content-width"),ui.available_width()));
                if toolbar_can_split&&!actual_can_split {ctx.request_discard("Secondary pane width changed during layout; redraw mode selector consistently");}
                let mode =
                    if self.mode == Mode::Split && !(toolbar_can_split&&actual_can_split) {
                        Mode::Edit
                    } else {
                        self.mode
                    };
                if mode != self.last_visible_mode {
                    if self.last_visible_mode == Mode::Read && mode != Mode::Read {
                        self.restore_edit_focus();
                    }
                    self.last_visible_mode = mode;
                    self.mode_transition = Some((mode, Instant::now()));
                }
                if let Some((_, started)) = self.mode_transition {
                    let t = if self.preferences.reduce_motion {
                        1.0
                    } else {
                        (started.elapsed().as_secs_f32() / 0.16).min(1.0)
                    };
                    ui.multiply_opacity(0.65 + 0.35 * t);
                    if t < 1.0 {
                        ctx.request_repaint();
                    } else {
                        self.mode_transition = None;
                    }
                }
                let path = self.doc.path.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||language.text("Untitled").into());
                let state = format!("{}{}",language.text(if !self.last_secondary {"Active"} else {"Main document"}),if self.doc.presentation_dirty() {format!(" · {}",language.text("Unsaved changes"))} else {String::new()});
                theme::document_header(ui,&self.doc.name(language),&path,&state,(!self.last_secondary,self.doc.presentation_dirty()),p);
                match mode {
                    Mode::Edit => self.editor(ui),
                    Mode::Read => {
                        let available = ui.available_rect_before_wrap();
                        let width = available.width().min(920.);
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(available.center().x - width * 0.5, available.top()),
                            Vec2::new(width, available.height()),
                        );
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .max_rect(rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                            |ui| self.preview(ui),
                        );
                        ui.advance_cursor_after_rect(available);
                    }
                    Mode::Split => {
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().line_segment(
                            [
                                egui::pos2(rect.center().x, rect.top()),
                                egui::pos2(rect.center().x, rect.bottom()),
                            ],
                            Stroke::new(1.0, p.line),
                        );
                        ui.spacing_mut().item_spacing.x = 32.0;
                        ui.columns(2, |columns| {
                            columns[0].spacing_mut().item_spacing.x = 8.0;
                            columns[1].spacing_mut().item_spacing.x = 8.0;
                            self.editor(&mut columns[0]);
                            self.preview(&mut columns[1]);
                        });
                    }
                }
            });
        self.structure_window(ctx);
        if let Some((expected, node, replacement)) = self.rich.edit_window(ctx, self.language) {
            match rich_nodes::patch(&self.doc.text, expected, &node, &replacement) {
                Ok(next) => {
                    self.apply_rich_source(ctx, next);
                    self.rich.applied(&self.doc.text, None);
                }
                Err(e) => self
                    .rich
                    .applied(&self.doc.text, Some(self.language.text(e))),
            }
        }
        if measure_previous_structure || self.structure.measure_next {
            self.structure
                .frame_samples_ms
                .push(structure_frame_start.elapsed().as_secs_f64() * 1000.0);
            self.structure
                .frame_command_ranges
                .push([structure_commands_before, self.structure.command_count()]);
        }
        if self.pending.is_none()
            && self.error.is_none()
            && !ctx.input(|i| i.viewport().maximized.unwrap_or(false))
        {
            egui::Area::new(egui::Id::new("window-resize-grip"))
                .movable(false)
                .order(egui::Order::Foreground)
                .fixed_pos(ctx.content_rect().right_bottom() - Vec2::splat(32.0))
                .show(ctx, |ui| {
                    let (grip, resize) =
                        ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::drag());
                    let resize = resize.on_hover_cursor(egui::CursorIcon::ResizeNwSe);
                    for offset in [5.0, 9.0, 13.0] {
                        ui.painter().line_segment(
                            [
                                grip.right_bottom() - Vec2::new(offset, 6.0),
                                grip.right_bottom() - Vec2::new(6.0, offset),
                            ],
                            Stroke::new(1.0, p.muted),
                        );
                    }
                    if resize.drag_started() {
                        self.resize_origin = Some(ctx.content_rect().size());
                    }
                    if resize.dragged()
                        && let Some(origin) = self.resize_origin
                    {
                        let size = origin + resize.total_drag_delta().unwrap_or_default();
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(
                            size.x.max(620.0),
                            size.y.max(420.0),
                        )));
                    }
                    if resize.drag_stopped() {
                        self.resize_origin = None;
                    }
                });
        }
        self.file_search(ctx);
        self.global_search_windows(ctx);
        self.dialogs(ctx);
        theme::end_window_focus(ctx, "search", self.global.windows_open());
        if !self.global.open {
            theme::forget_window_focus(ctx, "history");
        }
        theme::end_window_focus(ctx, "history", self.global.history_is_open());
        theme::end_window_focus(ctx, "structure", self.structure.open);
        theme::end_window_focus(ctx, "export", self.export.open);
        theme::end_window_focus(ctx, "rich", self.rich.edit_window_open());
        theme::end_modal_focus(
            ctx,
            self.settings_open
                || self.encoding_open
                || self.pending.is_some()
                || self.error.is_some(),
        );
        self.persist_preferences(ctx);
        let mode = match self.mode {
            Mode::Read => "Read",
            Mode::Edit => "Write",
            Mode::Split => "Split",
        };
        self.reading.set_mode(&self.doc, mode);
        if self.global.secondary_focused(ctx) {
            if let Some(doc) = &self.global.secondary {
                self.reading.activity(doc, "secondary/source", None);
            }
        } else if self.mode != Mode::Read
            && ctx
                .memory(|m| m.has_focus(egui::Id::new(("source-editor", self.document_generation))))
        {
            self.reading.activity(&self.doc, "main/source", None);
        }
        self.reading.tick(ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            self.reading.flush();
        }
        if self.started.elapsed().as_secs_f32() > 3.0
            && self.screenshot_path.is_some()
            && !self.screenshot_requested
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.screenshot_requested = true;
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = &self.screenshot_path
            {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Err(e) = image::save_buffer(
                    path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    self.error = Some(e.to_string());
                }
            }
        }
        if self
            .auto_quit
            .is_some_and(|s| self.started.elapsed().as_secs_f32() > s)
            && !self.doc.dirty()
            && self.global.secondary.as_ref().is_none_or(|d| !d.dirty())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        self.rich
            .report(structure_frame_start.elapsed().as_secs_f64() * 1000.);
        self.large.finish_frame();
        self.rich.release_offscreen(ctx);
        if self.running_gpu {
            rendering::measure_completion(
                ctx,
                self.large.paint.clone(),
                self.large.frame_started,
                self.large.completion_start(),
                self.large.input_frame,
                self.large.revision,
            );
        }
        if std::env::var_os("MDCARD_PERFORMANCE_CONTINUOUS").is_some() {
            ctx.request_repaint();
        }
        self.large.report(
            self.rich.cache_stats(),
            &self.renderer,
            self.document_generation,
        );
        if let Some(path) = std::env::var_os("MDCARD_READING_REPORT")
            && let Ok(bytes) = serde_json::to_vec(
                &self
                    .reading
                    .report(&self.doc, self.global.secondary.as_ref()),
            )
        {
            let _ = self
                .diagnostic_reports
                .write(diagnostic_sink::Kind::Reading, path, &bytes);
        }
        if let Some(path) = std::env::var_os("MDCARD_UI_REPORT") {
            let visible = self.last_visible_mode;
            let mode = match visible {
                Mode::Edit => "Write",
                Mode::Split => "Split",
                Mode::Read => "Read",
            };
            let controls = ctx.data(|d| {
                d.get_temp::<Vec<serde_json::Value>>(egui::Id::new("ui-report-controls"))
                    .unwrap_or_default()
            });
            let r = ctx.content_rect();
            let selection = self.source_selection(ctx);
            let brand = ctx.data(|data| {
                data.get_temp::<serde_json::Value>(egui::Id::new("mdcard-brand-drawn"))
            });
            let report = serde_json::json!({"native_icons":self.native_icons.report(),"branding":brand,"system_theme":format!("{:?}",ctx.system_theme()),"appearance":format!("{:?}",self.preferences.appearance),"active_pane":if self.last_secondary {"secondary"} else {"main"},"navigation":self.navigation,"mode":mode,"content_rect":[r.min.x,r.min.y,r.max.x,r.max.y],"primary_content_width":ctx.data(|d|d.get_temp::<f32>(egui::Id::new("ui-primary-content-width"))),"pixels_per_point":ctx.pixels_per_point(),"native_pixels_per_point":ctx.native_pixels_per_point(),"dirty":self.doc.presentation_dirty(),"status_key":self.status_key(),"document_generation":self.document_generation,"revision":self.doc.revision(),"selection":[selection.primary.index,selection.secondary.index],"focused":format!("{:?}",ctx.memory(|m|m.focused())),"settings_open":self.settings_open,"encoding_open":self.encoding_open,"can_split":self.can_split_ui(ctx),"workspace_root":self.files.root,"workspace_files":self.files.scan.entries.len(),"workspace_scanning":self.files.scanning,"document_path":self.doc.path,"controls":controls});
            if let Ok(bytes) = serde_json::to_vec(&report) {
                let _ = self
                    .diagnostic_reports
                    .write(diagnostic_sink::Kind::Ui, path, &bytes);
            }
        }
    }
}

fn consume_file_search_shortcut(input: &mut egui::InputState) -> bool {
    // egui's logical modifier matching permits extra Shift. Preserve Ctrl+Shift+P.
    !input.modifiers.shift && input.consume_key(egui::Modifiers::COMMAND, egui::Key::P)
}

fn main() {
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("MDcard {APP_VERSION}");
        return;
    }
    if let Err(error) = run() {
        let language = preferences::load()
            .ok()
            .and_then(|p| p.language)
            .unwrap_or_else(localization::system_language);
        let title = language.text("MDcard could not start");
        eprintln!("{title}: {error}");
        let guidance = language.format("The graphics setting was not silently changed. You can explicitly recover with:\nMDcard --gpu on\n\nSettings: {path}", &[("path", &preferences::path().display().to_string())]);
        rfd::MessageDialog::new()
            .set_title(title)
            .set_level(rfd::MessageLevel::Error)
            .set_description(format!("{error}\n\n{guidance}"))
            .show();
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    branding::set_process_identity();
    let language = preferences::load()
        .ok()
        .and_then(|p| p.language)
        .unwrap_or_else(localization::system_language);
    let (mut preferences, settings_error) = match preferences::load() {
        Ok(p) => (p, None),
        Err(e) => (Preferences::default(), Some(e)),
    };
    let mut path = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--gpu" {
            let value = args
                .next()
                .ok_or(language.text("--gpu requires on or off"))?;
            preferences.gpu_acceleration = match value.to_str() {
                Some("on") => true,
                Some("off") => false,
                _ => return Err(language.text("--gpu requires on or off").into()),
            };
            preferences::save(&preferences)?;
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(language
                .text("Only one document may be opened at a time")
                .into());
        }
    }
    let gpu_requested = preferences.gpu_acceleration;
    let automatic_compatibility = rendering::automatic_compatibility();
    let running_gpu = gpu_requested && !automatic_compatibility;
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(branding::window_icon())
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([620.0, 420.0])
            .with_decorations(false)
            .with_title("MDcard"),
        renderer: if running_gpu {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        ..Default::default()
    };
    if !running_gpu {
        options.wgpu_options.wgpu_setup = rendering::software_setup(
            preferences
                .language
                .unwrap_or_else(localization::system_language),
        );
    }
    if let Ok(size) = std::env::var("MDCARD_WINDOW_SIZE") {
        let parts: Vec<f32> = size.split('x').filter_map(|n| n.parse().ok()).collect();
        if parts.len() == 2 {
            options.viewport = options.viewport.with_inner_size([parts[0], parts[1]]);
        }
    }
    let creation_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let creation_observer = creation_started.clone();
    let result = eframe::run_native(
        "MDcard",
        options,
        Box::new(move |cc| {
            creation_observer.store(true, std::sync::atomic::Ordering::Release);
            let app = Editor::new(cc, path, preferences, settings_error);
            if let Some(file) = std::env::var_os("MDCARD_RENDERER_REPORT") {
                std::fs::write(
                    file,
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"version":APP_VERSION,"gpu_requested":app.requested_gpu_at_start,"gpu_running":app.running_gpu,"automatic_compatibility":app.automatic_compatibility,"renderer":app.renderer,"settings_path":preferences::path(),"language":app.language.code(),"language_preference":app.preferences.language}),
                    )?,
                )?;
            }
            Ok(Box::new(app))
        }),
    );
    if let Err(error) = result {
        // Only recover a Windows graphics initialization failure before an
        // editor/document exists. Never restart an editor containing user work.
        let graphics_initialization = matches!(
            error,
            eframe::Error::OpenGL(_)
                | eframe::Error::Glutin(_)
                | eframe::Error::NoGlutinConfigs(..)
        );
        if cfg!(target_os = "windows")
            && gpu_requested
            && !automatic_compatibility
            && !creation_started.load(std::sync::atomic::Ordering::Acquire)
            && graphics_initialization
        {
            eprintln!(
                "OpenGL initialization failed; using Windows CPU compatibility rendering: {error}"
            );
            // winit cannot safely recreate its event loop in this process.
            // The one child inherits arguments, diagnostics and stdio. Its
            // private marker selects WARP without writing the GPU preference.
            let status = std::process::Command::new(std::env::current_exe()?)
                .args(std::env::args_os().skip(1))
                .env(rendering::COMPATIBILITY_ENV, "1")
                .status()?;
            std::process::exit(status.code().unwrap_or(1));
        }
        return Err(error.into());
    }
    if let Err(error) = update_probe::finish() {
        eprintln!("Full-update observer final publication failed: {error}");
        std::process::exit(2);
    }
    Ok(())
}

fn localized_document_error(language: Language, error: &std::io::Error) -> String {
    let detail = error.to_string();
    for key in [
        "The file changed outside MDcard. Use Save As to keep both versions.",
        "Saving through a symbolic link is unsupported. Use Save As.",
        "Only regular text files can be opened.",
        "The document exceeds the 16 MiB opening limit.",
        "The file contains binary control bytes. Only UTF-8 text is supported.",
        "Unsupported file type. Choose a supported text file.",
        encoding::INVALID,
        encoding::BINARY,
        encoding::UNREPRESENTABLE,
        encoding::BOM_CONFLICT,
        encoding::UTF32,
    ] {
        if detail == key {
            return language.text(key).into();
        }
    }
    detail
}
fn localized_settings_error(language: Language, error: &str) -> String {
    for key in [
        "Could not read settings:",
        "Settings are invalid; defaults are active.",
    ] {
        if let Some(detail) = error.strip_prefix(key) {
            return format!("{}{}", language.text(key), detail);
        }
    }
    localized_message(language, error)
}

fn localized_message(language: Language, message: &str) -> String {
    for key in [
        "Could not open the document.",
        "Could not save. Your edits are still here.",
        "Could not save settings; previous settings restored.",
        "The document is not valid UTF-8.",
        "The file changed outside MDcard. Use Save As to keep both versions.",
        "Saving through a symbolic link is unsupported. Use Save As.",
        "Only regular text files can be opened.",
        "The document exceeds the 16 MiB opening limit.",
        "The file contains binary control bytes. Only UTF-8 text is supported.",
        "Unsupported file type. Choose a supported text file.",
        encoding::INVALID,
        encoding::BINARY,
        encoding::UNREPRESENTABLE,
        encoding::BOM_CONFLICT,
        encoding::UTF32,
    ] {
        if let Some(detail) = message.strip_prefix(key) {
            return format!(
                "{}{}",
                language.text(key),
                localized_message(language, detail)
            );
        }
    }
    if let Some(detail) = message.strip_prefix('\n') {
        return format!("\n{}", localized_message(language, detail));
    }
    message.into()
}

#[cfg(test)]
mod workspace_tests {
    use super::*;
    use std::{fs, path::Path, time::Duration};

    fn start(path: &Path) -> (Editor, egui::Context) {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let app = Editor::new(&cc, Some(path.to_owned()), Preferences::default(), None);
        (app, ctx)
    }

    fn indexed(app: &mut Editor) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.files.scanning {
            assert!(Instant::now() < deadline, "workspace index did not finish");
            app.files.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        app.tree.sync(&app.files);
    }

    fn visible(app: &Editor) -> Vec<PathBuf> {
        app.tree
            .rows
            .iter()
            .map(|(id, _)| app.files.scan.tree[*id].relative.clone())
            .collect()
    }

    #[test]
    fn nested_file_navigation_preserves_workspace_tree_and_siblings() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let nested = root.join(".github/workflows/build.yml");
        let sibling = root.join("中文 资源/notes.md");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::create_dir_all(sibling.parent().unwrap()).unwrap();
        fs::write(root.join("main.md"), "# Main").unwrap();
        fs::write(&nested, "name: build\n").unwrap();
        fs::write(&sibling, "# Sibling").unwrap();
        let (mut app, ctx) = start(&root.join("main.md"));
        indexed(&mut app);
        for id in 1..app.files.scan.tree.len() {
            if app.files.scan.tree[id].file.is_none() {
                app.tree.toggle(id, &app.files.scan.tree);
            }
        }
        let rows = visible(&app);
        assert!(rows.contains(&PathBuf::from(".github/workflows/build.yml")));
        for path in [&nested, &sibling, &root.join("main.md")] {
            app.request(Action::OpenPath(path.clone()), &ctx);
            indexed(&mut app);
            assert_eq!(app.doc.path.as_ref(), Some(path));
            assert_eq!(
                app.files.root, root,
                "opening a descendant narrowed the workspace"
            );
            assert_eq!(app.files.scan.entries.len(), 3);
            assert_eq!(visible(&app), rows, "expanded folders were lost");
        }
        app.open_path_encoding(nested, Some(Encoding::Utf8));
        indexed(&mut app);
        assert_eq!(
            app.files.root, root,
            "encoding reopen narrowed the workspace"
        );
        assert_eq!(visible(&app), rows);
        app.perform(Action::New, &ctx);
        assert_eq!(app.files.root, root, "new document replaced the workspace");
        app.perform(Action::Welcome, &ctx);
        assert_eq!(
            app.files.root, root,
            "welcome document replaced the workspace"
        );
    }

    #[test]
    fn external_open_rebases_only_after_success_and_dirty_navigation_can_cancel() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let workspace = root.join("work");
        let outside = root.join("work-other");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&outside).unwrap();
        let original = workspace.join("main.md");
        let target = outside.join("other.md");
        fs::write(&original, "# Main").unwrap();
        fs::write(&target, "# Other").unwrap();
        let (mut app, ctx) = start(&original);
        indexed(&mut app);
        app.doc.text.push_str(" unsaved");
        app.doc.mark_text_changed();
        app.request(Action::OpenPath(target.clone()), &ctx);
        assert!(app.pending.is_some());
        assert_eq!(app.files.root, workspace);
        assert_eq!(app.doc.path.as_ref(), Some(&original));
        let mut frame = eframe::Frame::_new_kittest();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            eframe::App::update(&mut app, ctx, &mut frame)
        });
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut app, ctx, &mut frame),
        );
        assert!(app.pending.is_none());
        assert_eq!(app.doc.text, "# Main unsaved");
        assert_eq!(app.files.root, workspace);
        app.open_path(outside.join("missing.md"));
        assert_eq!(
            app.files.root, workspace,
            "failed open changed the workspace"
        );
        assert_eq!(app.doc.text, "# Main unsaved");
        app.error = None;
        app.perform(Action::OpenPath(target.clone()), &ctx);
        indexed(&mut app);
        assert_eq!(
            app.files.root, outside,
            "external sibling with shared prefix must rebase"
        );
        assert_eq!(app.doc.path, Some(target));
        assert_eq!(app.files.scan.entries.len(), 1);
        assert_eq!(fs::read_to_string(original).unwrap(), "# Main");
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    #[test]
    fn search_does_not_consume_preferences_shortcut() {
        for shift in [false, true] {
            let ctx = egui::Context::default();
            let modifiers = egui::Modifiers {
                ctrl: true,
                command: true,
                shift,
                ..Default::default()
            };
            let input = egui::RawInput {
                modifiers,
                events: vec![egui::Event::Key {
                    key: egui::Key::P,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            };
            ctx.begin_pass(input);
            assert_eq!(ctx.input_mut(consume_file_search_shortcut), !shift);
            if shift {
                assert!(ctx.input_mut(|i| i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::P
                )));
            }
            let _ = ctx.end_pass();
        }
    }
}

#[cfg(test)]
mod ui_polish_tests {
    #[test]
    fn secondary_document_width_remains_stable_for_100_frames_with_long_dirty_names() {
        for language in Language::ALL {
            let ctx = egui::Context::default();
            let mut app = crate::preview_layout_tests::editor(&ctx, "# Main\n\n主稿");
            app.language = language;
            app.preferences.language = Some(language);
            localization::install_fonts(&ctx, language);
            let mut second = Document::from_text("# Other\n\n副稿");
            second.path = Some(PathBuf::from(format!(
                "/notes/{}.md",
                "非常长的文档名称-layout-".repeat(12)
            )));
            second.text.push_str(" dirty");
            second.mark_text_changed();
            app.global.secondary = Some(second);
            let mut frame = eframe::Frame::_new_kittest();
            let mut stable = None;
            for pass in 0..100 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(1280., 800.),
                        )),
                        ..Default::default()
                    },
                    |ctx| app.update(ctx, &mut frame),
                );
                let width = egui::containers::panel::PanelState::load(
                    &ctx,
                    egui::Id::new("second-document"),
                )
                .unwrap()
                .rect
                .width();
                if pass == 5 {
                    stable = Some(width);
                }
                if pass > 5 {
                    assert!(
                        (width - stable.unwrap()).abs() < 0.5,
                        "{language:?} width drift: pass={pass}, initial={stable:?}, now={width}"
                    );
                }
                let primary = ctx
                    .data(|d| d.get_temp::<f32>(egui::Id::new("ui-primary-content-width")))
                    .unwrap_or(0.);
                assert!(
                    primary > 300.,
                    "{language:?} primary document squeezed out at pass={pass}: {primary}"
                );
            }
        }
    }
    #[test]
    fn real_pointer_click_selects_main_read_pane_without_retaining_secondary_focus() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "# Main\n\n正文");
        app.global.secondary = Some(Document::from_text("# Other\n\n副稿"));
        app.mode = Mode::Read;
        let mut frame = eframe::Frame::_new_kittest();
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280., 800.),
            )),
            ..Default::default()
        };
        for _ in 0..3 {
            let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
        }
        let secondary = egui::Id::new(("second-editor", app.global.secondary_identity()));
        let _ = ctx.run(input(), |ctx| {
            ctx.memory_mut(|m| m.request_focus(secondary));
            app.update(ctx, &mut frame);
        });
        assert!(app.last_secondary);
        let mut click = input();
        let pos = egui::pos2(500., 145.);
        click.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        let _ = ctx.run(click, |ctx| app.update(ctx, &mut frame));
        assert!(!app.last_secondary);
        let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
        assert!(!app.last_secondary);
        assert_eq!(app.doc.text, "# Main\n\n正文");
        assert_eq!(
            app.global.secondary.as_ref().unwrap().text,
            "# Other\n\n副稿"
        );
    }
    #[test]
    fn workspace_navigation_preserves_dirty_buffer_history_and_blocks_during_transaction() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "# Original\n\n中文");
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("中文 workspace");
        std::fs::create_dir(&nested).unwrap();
        app.doc.text.push_str(" changed");
        app.doc.mark_text_changed();
        let original = app.doc.text.clone();
        let revision = app.doc.revision();
        let generation = app.document_generation;
        let id = egui::Id::new(("source-editor", generation));
        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(2),
                egui::text::CCursor::new(5),
            )));
        state.store(&ctx, id);
        app.global.busy = true;
        let prior = app.files.root.clone();
        app.set_workspace(nested.clone());
        assert_eq!(app.files.root, prior);
        app.global.busy = false;
        app.set_workspace(nested.clone());
        assert_eq!(app.files.root, nested.canonicalize().unwrap());
        assert_eq!(app.doc.text, original);
        assert!(app.doc.dirty());
        assert_eq!(app.doc.revision(), revision);
        assert_eq!(app.document_generation, generation);
        let cursor = egui::text_edit::TextEditState::load(&ctx, id)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        assert_eq!((cursor.primary.index, cursor.secondary.index), (5, 2));
        assert!(app.global.worker.is_none());
    }
    use super::*;
    use eframe::App;
    #[test]
    fn nonmodal_escape_restores_primary_or_secondary_editor_focus() {
        for secondary in [false, true] {
            for window in ["quick", "find", "structure", "export"] {
                let ctx = egui::Context::default();
                let mut app =
                    crate::preview_layout_tests::editor(&ctx, "# Main\n\n中文 selection-token");
                app.mode = Mode::Edit;
                if secondary {
                    app.global.secondary =
                        Some(Document::from_text("# Other\n\n副文档 selection-token"));
                }
                let target = if secondary {
                    egui::Id::new(("second-editor", app.global.secondary_identity()))
                } else {
                    egui::Id::new(("source-editor", app.document_generation))
                };
                let mut frame = eframe::Frame::_new_kittest();
                let input = || egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1280., 800.),
                    )),
                    ..Default::default()
                };
                for _ in 0..2 {
                    let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
                }
                let _ = ctx.run(input(), |ctx| {
                    ctx.memory_mut(|m| m.request_focus(target));
                    app.update(ctx, &mut frame);
                });
                match window {
                    "quick" => app.open_search(&ctx),
                    "find" => app.open_global_search(&ctx, true),
                    "structure" => app.structure.open = true,
                    _ => app.open_export(&ctx),
                }
                for _ in 0..3 {
                    let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
                }
                let mut escape = input();
                escape.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
                let _ = ctx.run(escape, |ctx| app.update(ctx, &mut frame));
                for _ in 0..4 {
                    let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
                }
                assert!(
                    !app.global.windows_open() && !app.structure.open && !app.export.open,
                    "window did not close: {window}"
                );
                assert_eq!(
                    ctx.memory(|m| m.focused()),
                    Some(target),
                    "secondary={secondary}, window={window}"
                );
                assert_eq!(app.doc.text, "# Main\n\n中文 selection-token");
                if let Some(doc) = &app.global.secondary {
                    assert_eq!(doc.text, "# Other\n\n副文档 selection-token");
                }
            }
        }
    }
    #[test]
    fn returning_to_clean_source_paints_ready_instead_of_stale_unsaved_status() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "baseline");
        let mut frame = eframe::Frame::_new_kittest();
        app.doc.text.push_str(" edit");
        app.doc.mark_text_changed();
        app.status = "Unsaved changes";
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280., 800.),
            )),
            ..Default::default()
        };
        let changed = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
        assert!(app.doc.dirty());
        assert!(changed.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Unsaved changes")
        ));
        app.doc.text = "baseline".into();
        app.doc.mark_text_changed();
        let restored = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
        assert!(!app.doc.dirty());
        assert!(restored.shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Ready when you are")
        ));
        assert!(!restored.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Unsaved changes"||t.galley.text()=="Saved to disk")));
    }
    #[test]
    fn secondary_panel_resizes_keep_primary_mode_and_selector_consistent() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "# Main\n\n中文 source");
        app.mode = Mode::Split;
        app.global.secondary = Some(Document::from_text("# Second\n\nOther source"));
        let mut frame = eframe::Frame::_new_kittest();
        for (width, secondary, expected) in [
            (1280., 440., Mode::Edit),
            (1920., 440., Mode::Split),
            (1920., 800., Mode::Edit),
            (1920., 280., Mode::Split),
            (800., 280., Mode::Edit),
            (1280., 0., Mode::Split),
        ] {
            if secondary == 0. {
                app.global.secondary = None;
            } else {
                ctx.data_mut(|d| {
                    d.insert_persisted(
                        egui::Id::new("second-document"),
                        egui::containers::panel::PanelState {
                            rect: egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                Vec2::new(secondary, 600.),
                            ),
                        },
                    )
                });
            }
            for _ in 0..3 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(width, 800.),
                        )),
                        ..Default::default()
                    },
                    |ctx| app.update(ctx, &mut frame),
                );
            }
            let toolbar = ctx
                .data(|d| d.get_temp::<bool>(egui::Id::new("ui-toolbar-can-split")))
                .unwrap();
            let primary = ctx
                .data(|d| d.get_temp::<f32>(egui::Id::new("ui-primary-content-width")))
                .unwrap();
            assert!(
                app.last_visible_mode == expected,
                "width={width}, secondary={secondary}, primary={primary}"
            );
            assert_eq!(
                toolbar,
                expected == Mode::Split,
                "selector/content mismatch at {width}/{secondary}"
            );
            if expected == Mode::Split {
                assert!(primary >= theme::SPLIT_CONTENT_MIN);
            }
            assert_eq!(app.doc.text, "# Main\n\n中文 source");
            if let Some(doc) = &app.global.secondary {
                assert_eq!(doc.text, "# Second\n\nOther source");
            }
        }
    }
    #[test]
    #[ignore = "Explicit 432-case headless layout matrix, distinct from native Windows DPI"]
    fn headless_scale_language_theme_mode_matrix() {
        let mut cases = Vec::new();
        for language in Language::ALL {
            for dark in [false, true] {
                for scale in [1., 1.25, 1.5, 2.] {
                    let ctx = egui::Context::default();
                    ctx.set_pixels_per_point(scale);
                    let mut app = crate::preview_layout_tests::editor(
                        &ctx,
                        "# 中文排版 / Layout\n\nA clear paragraph. 中文测试正文。\n\n| A | B |\n| --- | --- |\n| table | 内容 |\n",
                    );
                    app.language = language;
                    app.preferences.language = Some(language);
                    app.preferences.appearance = if dark {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    };
                    localization::install_fonts(&ctx, language);
                    let original = app.doc.text.clone();
                    let revision = app.doc.revision();
                    let mut frame = eframe::Frame::_new_kittest();
                    for (width, height) in [(800., 600.), (1280., 800.), (1920., 1080.)] {
                        for requested in [Mode::Edit, Mode::Split, Mode::Read] {
                            app.mode = requested;
                            let mut output = None;
                            for _ in 0..3 {
                                output = Some(ctx.run(
                                    egui::RawInput {
                                        screen_rect: Some(egui::Rect::from_min_size(
                                            egui::Pos2::ZERO,
                                            Vec2::new(width, height),
                                        )),
                                        ..Default::default()
                                    },
                                    |ctx| app.update(ctx, &mut frame),
                                ));
                            }
                            let output = output.unwrap();
                            let bounds = ctx.content_rect();
                            let mut labels = Vec::new();
                            for name in ["Write", "Split", "Read"].map(|key| language.text(key)) {
                                let t=output.shapes.iter().find_map(|s|match &s.shape{egui::Shape::Text(t) if t.galley.text()==name&&t.pos.y>=48.&&t.pos.y<100.=>Some((t,s.clip_rect)),_=>None}).unwrap_or_else(||panic!("mode label missing: {language:?}/{scale}/{width}/{name}"));
                                let rect = egui::Rect::from_min_size(t.0.pos, t.0.galley.size());
                                assert!(
                                    bounds.expand(0.5).contains_rect(rect),
                                    "mode label outside viewport {rect:?}/{bounds:?}"
                                );
                                assert!(
                                    t.1.expand(0.5).contains_rect(rect),
                                    "mode label clipped: {name}"
                                );
                                labels.push(serde_json::json!({"text":name,"text_rect":[rect.min.x,rect.min.y,rect.max.x,rect.max.y],"clip":[t.1.min.x,t.1.min.y,t.1.max.x,t.1.max.y]}));
                            }
                            let visible = if requested == Mode::Split && !app.can_split(width) {
                                Mode::Edit
                            } else {
                                requested
                            };
                            assert!(visible == app.last_visible_mode);
                            assert_eq!(app.doc.text, original);
                            assert_eq!(app.doc.revision(), revision);
                            let controls = ctx.data(|d| {
                                d.get_temp::<Vec<serde_json::Value>>(egui::Id::new(
                                    "ui-report-controls",
                                ))
                                .unwrap_or_default()
                            });
                            let mode_name = |m| match m {
                                Mode::Edit => "Write",
                                Mode::Split => "Split",
                                Mode::Read => "Read",
                            };
                            cases.push(serde_json::json!({"language":language.code(),"theme":if dark{"Dark"}else{"Light"},"logical_size":[width,height],"requested_pixels_per_point":scale,"effective_pixels_per_point":ctx.pixels_per_point(),"virtual_surface_pixels":[width*scale,height*scale],"requested_mode":mode_name(requested),"visible_mode":mode_name(visible),"split_enabled":app.can_split(width),"mode_labels":labels,"controls":controls,"source_unchanged":true}));
                        }
                    }
                }
            }
        }
        assert_eq!(cases.len(), 432);
        let report = serde_json::json!({"environment":"egui headless layout; no OS window, GPU, physical screen or Windows system DPI change","cases":cases,"case_count":432,"passed":true});
        if let Some(path) = std::env::var_os("MDCARD_UI_MATRIX_REPORT") {
            std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        println!(
            "432 headless size/scale/language/theme/mode cases passed; physical Windows DPI is not established by this test"
        );
    }
    #[test]
    fn modal_close_restores_the_source_selection_after_modal_layer_clears() {
        for encoding in [false, true] {
            let ctx = egui::Context::default();
            let mut app =
                crate::preview_layout_tests::editor(&ctx, "# Focus\n\n中文 selection-token");
            app.mode = Mode::Edit;
            let mut frame = eframe::Frame::_new_kittest();
            let input = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280., 800.),
                )),
                ..Default::default()
            };
            for _ in 0..2 {
                let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
            }
            let source_id = egui::Id::new(("source-editor", app.document_generation));
            let mut state = egui::text_edit::TextEditState::load(&ctx, source_id).unwrap();
            let range = egui::text::CCursorRange::two(
                egui::text::CCursor::new(10),
                egui::text::CCursor::new(19),
            );
            state.cursor.set_char_range(Some(range));
            state.store(&ctx, source_id);
            let _ = ctx.run(input(), |ctx| {
                ctx.memory_mut(|m| m.request_focus(source_id));
                app.update(ctx, &mut frame);
            });
            assert_eq!(ctx.memory(|m| m.focused()), Some(source_id));
            if encoding {
                app.show_encoding(None, &ctx);
            } else {
                app.settings_open = true;
            }
            for _ in 0..3 {
                let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
            }
            app.settings_open = false;
            app.encoding_open = false;
            for _ in 0..4 {
                let _ = ctx.run(input(), |ctx| app.update(ctx, &mut frame));
            }
            assert_eq!(
                ctx.memory(|m| m.focused()),
                Some(source_id),
                "encoding={encoding}"
            );
            let actual = app.source_selection(&ctx);
            assert_eq!(
                (actual.primary.index, actual.secondary.index),
                (range.primary.index, range.secondary.index)
            );
            assert_eq!(app.doc.text, "# Focus\n\n中文 selection-token");
        }
    }
    #[test]
    fn six_language_toolbar_labels_remain_inside_compact_and_wide_windows() {
        for language in Language::ALL {
            let ctx = egui::Context::default();
            let mut app = crate::preview_layout_tests::editor(&ctx, "# Layout\n\n正文");
            app.language = language;
            localization::install_fonts(&ctx, language);
            for width in [620., 800., 1280., 1920.] {
                let input = || egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 600.),
                    )),
                    ..Default::default()
                };
                for _ in 0..2 {
                    let _ = ctx.run(input(), |ctx| {
                        app.titlebar(ctx);
                        app.toolbar(ctx);
                    });
                }
                let output = ctx.run(input(), |ctx| {
                    app.titlebar(ctx);
                    app.toolbar(ctx);
                });
                for label in ["Write", "Split", "Read"].map(|key| language.text(key)) {
                    let text = output
                        .shapes
                        .iter()
                        .find_map(|s| match &s.shape {
                            egui::Shape::Text(t) if t.galley.text() == label => Some(t),
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("Missing {language:?} {width} {label}"));
                    let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                    assert!(
                        rect.left() >= 0. && rect.right() <= width,
                        "{language:?} {width} {label} {rect:?}"
                    );
                    assert!(
                        rect.top() >= 40. && rect.bottom() <= 84.,
                        "{language:?} {label} outside toolbar"
                    );
                }
            }
        }
    }
    #[test]
    fn compact_split_falls_back_and_restores_without_changing_source() {
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "# 保留稿件\n\n😀 body");
        let before = app.doc.text.clone();
        let revision = app.doc.revision();
        for sidebar in [false, true] {
            app.sidebar = sidebar;
            assert!(!app.can_split(800.));
            assert!(app.can_split(1280.));
        }
        assert_eq!(app.doc.text, before);
        assert_eq!(app.doc.revision(), revision);
    }
}

#[cfg(test)]
mod preview_layout_tests {
    #[test]
    fn relocated_soft_break_run_is_mounted_inside_one_ordinary_tall_paragraph() {
        let text = format!(
            "# Shared source\n\n{}",
            (0..220)
                .map(|i| format!("SHARED-{i:04} 中文 é 👩🏽‍💻 immutable row {i:04} content.\n"))
                .collect::<String>()
        );
        assert!(text.len() < 64 * 1024);
        let ctx = egui::Context::default();
        let mut app = editor(&ctx, &text);
        app.reading.bind(&mut app.doc, "main", 0);
        let mut frame = 0;
        let mut render = |app: &mut Editor, wheel: bool| {
            let mut input = egui::RawInput {
                time: Some(frame as f64 / 60.),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 650.),
                )),
                ..Default::default()
            };
            frame += 1;
            input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(400., 300.)));
            if wheel {
                input.events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -1200.),
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.preview(ui));
            });
        };
        for i in 0..100 {
            render(&mut app, i == 5);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let old = app.reading.report(&app.doc, None)["views"]["main/preview"].clone();
        let byte = old["geometry"]["source_byte"].as_u64().unwrap();
        let y = old["geometry"]["viewport_y"].as_f64().unwrap();
        assert!(byte > 1000);
        assert!(old["position"]["text_anchor"]["source"]["proof"].is_object());
        app.doc.text = format!("主🙂{text}");
        app.doc.mark_text_changed();
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            render(&mut app, false);
            let view = app.reading.report(&app.doc, None)["views"]["main/preview"].clone();
            assert!(view["failed_restore"].is_null(), "{view}");
            if view["relocation"]["status"] == "completed" {
                assert_eq!(view["relocation"]["exact"], true);
                assert_eq!(
                    view["geometry"]["source_byte"].as_u64(),
                    Some(byte + "主🙂".len() as u64)
                );
                assert!((view["geometry"]["viewport_y"].as_f64().unwrap() - y).abs() <= 2.);
                break;
            }
            assert!(Instant::now() < deadline, "{view}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    #[test]
    fn large_to_ordinary_waits_for_actual_parser_commit_before_mapping_geometry() {
        let ctx = egui::Context::default();
        let original = "## 操作章节\n\nSmall surviving paragraph with **mapped emphasis**.\n";
        let large = format!(
            "{}{}",
            "Earlier obsolete rendered 编 text.\n\n".repeat(3000),
            original
        );
        let mut app = editor(&ctx, &large);
        app.reading.bind(&mut app.doc, "main", 0);
        let render = |app: &mut Editor| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900., 600.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.preview(ui));
                },
            )
        };
        let _ = render(&mut app);
        assert!(app.large.active);
        assert!(app.rich.source_ready(&app.doc, 0));
        app.doc.text = original.into();
        app.doc.mark_text_changed();
        let pending = render(&mut app);
        assert!(!app.large.active);
        assert!(!app.rich.source_ready(&app.doc, 0));
        let report = app.reading.report(&app.doc, None);
        let view = &report["views"]["main/preview"];
        assert_eq!(view["layout_pending"], true);
        assert!(view["geometry"].is_null());
        assert_eq!(view["visible_text"].as_array().unwrap().len(), 0);
        fn obsolete(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().contains("Earlier obsolete"),
                egui::Shape::Vec(shapes) => shapes.iter().any(obsolete),
                _ => false,
            }
        }
        assert!(!pending.shapes.iter().any(|shape| obsolete(&shape.shape)));
        std::thread::sleep(std::time::Duration::from_millis(270));
        let _ = render(&mut app);
        assert!(app.rich.source_ready(&app.doc, 0));
        assert_eq!(app.doc.text, original);
    }
    #[test]
    fn rich_preview_reacquires_a_pin_after_font_remeasurement_unmounts_its_run() {
        for ppp in [1., 1.25, 1.5, 2.] {
            let text=format!("# Layout recovery\n\n{}",(0..60).map(|i|format!("{}Paragraph {i:03}: {}\n\n",if i%10==0{format!("## Section {i}\n\n")}else{String::new()},"中文🙂 é 👩🏽‍💻 **emphasis text** [visible link](https://example.invalid/a-long-hidden-destination) `inline_code` \\*escaped\\* &amp; mixed text crossing wrapped visual rows. ".repeat(3))).collect::<String>());
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(ppp);
            let mut app = editor(&ctx, &text);
            app.reading.bind(&mut app.doc, "main", 0);
            let mut original = None;
            let mut tail = Vec::new();
            for frame in 0..130 {
                if frame == 50 {
                    app.font_size = 23.;
                }
                let mut input = egui::RawInput {
                    time: Some(frame as f64 / 60.),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(944., 670.),
                    )),
                    ..Default::default()
                };
                input
                    .events
                    .push(egui::Event::PointerMoved(egui::pos2(450., 300.)));
                if frame == 5 {
                    input.events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., -1437.),
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.preview(ui));
                });
                let r = app.reading.report(&app.doc, None);
                let v = &r["views"]["main/preview"];
                if frame == 49 {
                    original = Some((
                        v["geometry"]["source_byte"].as_u64().unwrap(),
                        v["geometry"]["viewport_y"].as_f64().unwrap(),
                    ));
                }
                if frame >= 110 {
                    assert!(v["failed_restore"].is_null());
                    assert!(v["pending"].is_null());
                    let (byte, y) = original.unwrap();
                    assert_eq!(v["geometry"]["source_byte"].as_u64(), Some(byte));
                    assert!((v["geometry"]["viewport_y"].as_f64().unwrap() - y).abs() <= 2.);
                    tail.push(v["geometry"]["compensations"].as_u64().unwrap());
                }
            }
            assert_eq!(
                tail.first(),
                tail.last(),
                "finite convergence after real prefix remeasurement"
            );
        }
    }
    #[test]
    fn hidden_destination_does_not_block_either_side_of_a_long_paragraph() {
        for large in [false, true] {
            for after_link in [false, true] {
                let left = "left **emphasized** 中文 word. ".repeat(160);
                let hidden = format!(
                    "[visible link](https://example.invalid/{})",
                    "invisible-destination-".repeat(700)
                );
                let right = "right **emphasized** 中文 word. ".repeat(160);
                let paragraph = format!("{left}{hidden} {right}");
                let body = format!(
                    "# Unique heading\n\n{}\n\n{paragraph}\n\n{}",
                    "Earlier paragraph content.\n\n".repeat(30),
                    "Later paragraph content.\n\n".repeat(30)
                );
                let text = if large {
                    format!("{body}\n\n{}", "Other tail paragraph.\n\n".repeat(5000))
                } else {
                    body
                };
                let goal = if after_link {
                    text.find(&hidden).unwrap() + hidden.len() + 100
                } else {
                    text.find(&hidden).unwrap() - 150
                };
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(1.25);
                let mut app = editor(&ctx, &text);
                app.doc.path = Some(std::path::PathBuf::from("/hidden-variable.md"));
                app.reading.bind(&mut app.doc, "main", 0);
                let mut frame = 0;
                let mut render = |app: &mut Editor, width: f32, offset: Option<f32>| {
                    let input = egui::RawInput {
                        time: Some(frame as f64 / 60.),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width + 24., 655.),
                        )),
                        ..Default::default()
                    };
                    frame += 1;
                    let _ = ctx.run(input, |ctx| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::new().inner_margin(12))
                            .show(ctx, |ui| {
                                if let Some(offset) = offset {
                                    let id = ui.make_persistent_id(egui::Id::new((
                                        "preview-scroll",
                                        app.doc.id,
                                        app.document_generation,
                                    )));
                                    let mut state =
                                        egui::scroll_area::State::load(ctx, id).unwrap_or_default();
                                    state.offset.y = offset;
                                    state.store(ctx, id);
                                }
                                app.preview(ui)
                            });
                    });
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    app.reading.report(&app.doc, None)["views"]["main/preview"].clone()
                };
                for _ in 0..30 {
                    render(&mut app, 920., None);
                }
                let mut old = None;
                for offset in (0..14000).step_by(12) {
                    let v = render(&mut app, 920., Some(offset as f32));
                    if let Some(byte) = v["geometry"]["source_byte"].as_u64()
                        && (goal..goal + 220).contains(&(byte as usize))
                    {
                        old = Some(v);
                        break;
                    }
                }
                let old = old.unwrap_or_else(|| {
                    panic!("actual setup large={large} after={after_link} goal={goal}")
                });
                let byte = old["geometry"]["source_byte"].as_u64().unwrap();
                let y = old["geometry"]["viewport_y"].as_f64().unwrap();
                eprintln!(
                    "VARIABLE large={large} after={after_link} bytes={} byte={byte} y={y}",
                    text.len()
                );
                for width in [580., 920.] {
                    let mut ready = false;
                    for _ in 0..130 {
                        let v = render(&mut app, width, None);
                        assert!(
                            v["failed_restore"].is_null(),
                            "VARIABLE large={large} after={after_link} width={width} {v}"
                        );
                        if v["pending"].is_null()
                            && v["geometry"]["source_byte"].as_u64() == Some(byte)
                        {
                            assert!(
                                (v["geometry"]["viewport_y"].as_f64().unwrap() - y).abs() <= 2.
                                    || v["geometry"]["clamped"] == true,
                                "{v}"
                            );
                            ready = true;
                            break;
                        }
                    }
                    assert!(ready, "unresolved {large} {after_link} {width}");
                }
            }
        }
    }

    use super::*;

    pub(crate) fn editor(ctx: &egui::Context, text: &str) -> Editor {
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut editor = Editor::new(
            &cc,
            None,
            Preferences {
                language: Some(Language::English),
                reduce_motion: true,
                ..Default::default()
            },
            None,
        );
        editor.doc = Document::from_text(text);
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            editor.large.ensure(text, 0, editor.doc.revision(), ctx);
            if !editor.large.active || editor.large.parsed.is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "preview parser did not finish");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        editor
    }

    fn paragraph_rows(ctx: &egui::Context, editor: &mut Editor, width: f32) -> usize {
        let mut rows = 0;
        for _ in 0..4 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 1000.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| editor.preview(ui));
                },
            );
            fn count(shape: &egui::Shape) -> usize {
                match shape {
                    egui::Shape::Text(text) if text.galley.text().contains("Responsive body") => {
                        text.galley.rows.len()
                    }
                    egui::Shape::Vec(shapes) => shapes.iter().map(count).sum(),
                    _ => 0,
                }
            }
            rows = output.shapes.iter().map(|shape| count(&shape.shape)).sum();
        }
        assert!(rows > 0, "responsive paragraph must actually be rendered");
        rows
    }

    #[test]
    fn pending_formula_is_included_in_save_and_navigation_dirty_guard() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formula.md");
        let source = "$x^2$\r\n";
        std::fs::write(&path, source).unwrap();
        let mut app = editor(&ctx, source);
        app.doc = Document::open(&path).unwrap();
        app.rich
            .stage_pending_test_edit(source, app.document_generation, "z^3", &ctx);
        assert!(app.save(false, &ctx));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "$z^3$\r\n");
        assert!(!app.doc.dirty());
        app.rich
            .stage_pending_test_edit(&app.doc.text, app.document_generation, "q^4", &ctx);
        app.request(Action::New, &ctx);
        assert!(
            app.pending.is_some(),
            "navigation must see the pending node as unsaved text"
        );
        assert_eq!(app.doc.text, "$q^4$\r\n");
    }

    #[test]
    fn stale_formula_draft_blocks_save_and_navigation_without_losing_either_buffer() {
        let ctx = egui::Context::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formula.md");
        let source = "$x^2$\n";
        std::fs::write(&path, source).unwrap();
        let mut app = editor(&ctx, source);
        app.doc = Document::open(&path).unwrap();
        app.global.secondary = Some(Document::from_text("secondary kept"));
        app.rich
            .stage_pending_test_edit(source, app.document_generation, "z^3", &ctx);
        app.doc.text = "$outside$\n".into();
        assert!(!app.save(false, &ctx));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert_eq!(app.doc.text, "$outside$\n");
        app.error = None;
        app.request(Action::New, &ctx);
        assert_eq!(app.doc.text, "$outside$\n");
        assert!(app.error.is_some());
        assert_eq!(app.rich.take_pending(true).unwrap().2, "z^3");
        // The visible, confirmed discard action drops only the stale node draft.
        // The independently edited document remains recoverable and saveable.
        app.rich.discard_pending_edits();
        app.error = None;
        assert!(app.save(false, &ctx));
        assert_eq!(app.doc.text, "$outside$\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "$outside$\n");
        assert_eq!(
            app.global.secondary.as_ref().unwrap().text,
            "secondary kept"
        );
        assert!(!app.doc.dirty());
        app.request(Action::New, &ctx);
        assert!(app.doc.text.is_empty());
    }

    #[test]
    fn search_export_and_encoding_observe_the_latest_node_draft() {
        for operation in 0..4 {
            let ctx = egui::Context::default();
            let mut app = editor(&ctx, "$x^2$\n");
            app.rich
                .stage_pending_test_edit(&app.doc.text, app.document_generation, "z^3", &ctx);
            match operation {
                0 => app.open_global_search(&ctx, true),
                1 => app.open_search(&ctx),
                2 => app.open_export(&ctx),
                _ => app.show_encoding(None, &ctx),
            }
            assert_eq!(app.doc.text, "$z^3$\n", "operation {operation}");
        }
    }

    #[test]
    fn preview_reflows_after_resize_in_both_directions() {
        let ctx = egui::Context::default();
        let mut editor = editor(&ctx, &"Responsive body with words to wrap. ".repeat(30));
        let wide = paragraph_rows(&ctx, &mut editor, 900.);
        let narrow = paragraph_rows(&ctx, &mut editor, 360.);
        let wide_again = paragraph_rows(&ctx, &mut editor, 900.);
        assert!(narrow > wide, "narrow={narrow}, wide={wide}");
        assert_eq!(wide_again, wide);
    }

    #[test]
    fn wide_table_does_not_expand_following_paragraphs() {
        let paragraph = "Responsive body with words to wrap. ".repeat(12);
        let ctx = egui::Context::default();
        let mut baseline = editor(&ctx, &paragraph);
        let expected = paragraph_rows(&ctx, &mut baseline, 360.);
        let ctx = egui::Context::default();
        let table = format!(
            "| {} | Other |\n| --- | --- |\n| value | value |\n\n",
            "W".repeat(200)
        );
        let mut with_table = editor(&ctx, &(table + &paragraph));
        let actual = paragraph_rows(&ctx, &mut with_table, 360.);
        assert_eq!(actual, expected, "table must not change body wrapping");
    }

    #[test]
    fn large_preview_reflows_after_wide_table_and_resize() {
        let paragraph = "Responsive body 中文正文随阅读区域宽度自动换行。 ".repeat(12);
        let table = format!(
            "| {} | Other |\n| --- | --- |\n| value | value |\n\n",
            "W".repeat(200)
        );
        let text = format!(
            "{table}{paragraph}\n\n{}",
            "Filler paragraph.\n\n".repeat(5000)
        );
        let ctx = egui::Context::default();
        let mut editor = editor(&ctx, &text);
        assert!(editor.large.active);
        let wide = paragraph_rows(&ctx, &mut editor, 900.);
        let narrow = paragraph_rows(&ctx, &mut editor, 360.);
        let wide_again = paragraph_rows(&ctx, &mut editor, 900.);
        assert!(narrow > wide, "narrow={narrow}, wide={wide}");
        assert_eq!(wide_again, wide);

        let ctx = egui::Context::default();
        let mut baseline = self::editor(&ctx, &paragraph);
        assert_eq!(narrow, paragraph_rows(&ctx, &mut baseline, 360.));
    }

    #[test]
    fn wrapped_table_rows_contain_text_and_do_not_overlap() {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            localization::install_fonts(&ctx, Language::English);
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut cache = CommonMarkCache::default();
            let cell = |row: usize| {
                format!(
                    "R{row} {} `R{row}_code_segment` R{row} {} [R{row}_link](https://example.com) R{row} {}",
                    format!("R{row} 中文正文换行检查 ").repeat(10),
                    format!("R{row} long mixed content ").repeat(8),
                    format!("R{row} 末尾内容 ").repeat(4),
                )
            };
            let table = format!(
                "| Part | Suggestion | Evidence |\n| --- | --- | --- |\n| R1 label | R1 short | {} |\n| R2 label | R2 short | {} |\n| R3 label | R3 short | R3 end |\n\nAFTER_TABLE paragraph",
                cell(1),
                cell(2),
            );
            for width in [1500., 600., 1000., 1500.] {
                for _ in 0..4 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 4000.),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                CommonMarkViewer::new().show(ui, &mut cache, &table);
                            });
                        },
                    );
                    let mut rows = [egui::Rect::NOTHING; 3];
                    let mut stripes = Vec::new();
                    let mut after = egui::Rect::NOTHING;
                    fn collect(
                        shape: &egui::Shape,
                        fill: Color32,
                        rows: &mut [egui::Rect; 3],
                        stripes: &mut Vec<egui::Rect>,
                    ) {
                        match shape {
                            egui::Shape::Text(text) => {
                                for (i, bounds) in rows.iter_mut().enumerate() {
                                    if text.galley.text().contains(&format!("R{}", i + 1)) {
                                        *bounds = bounds.union(egui::Rect::from_min_size(
                                            text.pos,
                                            text.galley.size(),
                                        ));
                                    }
                                }
                            }
                            egui::Shape::Rect(rect) if rect.fill == fill => stripes.push(rect.rect),
                            egui::Shape::Vec(shapes) => {
                                for shape in shapes {
                                    collect(shape, fill, rows, stripes);
                                }
                            }
                            _ => {}
                        }
                    }
                    fn after_rect(shape: &egui::Shape, bounds: &mut egui::Rect) {
                        match shape {
                            egui::Shape::Text(text)
                                if text.galley.text().contains("AFTER_TABLE") =>
                            {
                                *bounds = bounds
                                    .union(egui::Rect::from_min_size(text.pos, text.galley.size()))
                            }
                            egui::Shape::Vec(shapes) => {
                                for shape in shapes {
                                    after_rect(shape, bounds);
                                }
                            }
                            _ => {}
                        }
                    }
                    for shape in &output.shapes {
                        collect(
                            &shape.shape,
                            ctx.style().visuals.faint_bg_color,
                            &mut rows,
                            &mut stripes,
                        );
                        after_rect(&shape.shape, &mut after);
                    }
                    assert!(
                        rows.iter().all(egui::Rect::is_positive),
                        "all table rows must be painted"
                    );
                    assert!(
                        rows[0].bottom() <= rows[1].top(),
                        "row 1 overlaps row 2 at width {width}: {rows:?}"
                    );
                    assert!(
                        rows[1].bottom() <= rows[2].top(),
                        "row 2 overlaps row 3 at width {width}: {rows:?}"
                    );
                    assert!(
                        after.is_positive() && after.top() >= rows[2].bottom(),
                        "table overlaps following content at width {width}: {rows:?}, {after:?}"
                    );
                    assert!(
                        stripes
                            .iter()
                            .any(|rect| rect.expand(0.5).contains_rect(rows[0])),
                        "row background does not contain all wrapped text at width {width}: {:?}, {stripes:?}",
                        rows[0]
                    );
                }
            }
        }
    }

    #[test]
    fn table_cells_reflow_in_full_read_and_split_layouts() {
        for (mode, large) in [
            (Mode::Read, false),
            (Mode::Split, false),
            (Mode::Read, true),
            (Mode::Split, true),
        ] {
            let ctx = egui::Context::default();
            let mut text = format!(
                "| Name | Description | Evidence |\n| --- | --- | --- |\n| item | {} | details |",
                "Responsive body 中文表格内容应该随区域宽度换行。 ".repeat(12)
            );
            if large {
                text.push_str(&"\n\nFiller paragraph.".repeat(5000));
            }
            let mut editor = editor(&ctx, &text);
            assert_eq!(editor.large.active, large);
            editor.mode = mode;
            editor.sidebar = false;
            let mut frame = eframe::Frame::_new_kittest();
            let mut samples = Vec::new();
            // Read caps long lines at 920px; its narrower sample must actually
            // cross that cap. Split retains its readable two-column threshold.
            for width in [1500., if mode == Mode::Read { 800. } else { 1040. }, 1500.] {
                let mut rows = 0;
                for _ in 0..4 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 1200.),
                            )),
                            ..Default::default()
                        },
                        |ctx| eframe::App::update(&mut editor, ctx, &mut frame),
                    );
                    fn inspect(shape: &egui::Shape, min_x: f32, right: f32) -> usize {
                        match shape {
                            egui::Shape::Text(text)
                                if text.pos.x >= min_x
                                    && text.galley.text().contains("Responsive body") =>
                            {
                                assert!(
                                    text.pos.x + text.galley.size().x <= right,
                                    "table content overflows preview: {} > {right}",
                                    text.pos.x + text.galley.size().x
                                );
                                text.galley.rows.len()
                            }
                            egui::Shape::Vec(shapes) => {
                                shapes.iter().map(|s| inspect(s, min_x, right)).sum()
                            }
                            _ => 0,
                        }
                    }
                    rows = output
                        .shapes
                        .iter()
                        .map(|shape| {
                            inspect(
                                &shape.shape,
                                if mode == Mode::Split { width / 2. } else { 0. },
                                width - 20.,
                            )
                        })
                        .sum();
                }
                assert!(rows > 0, "table cell must be mounted in the real preview");
                samples.push(rows);
            }
            assert!(samples[1] > samples[0], "table did not reflow: {samples:?}");
            assert_eq!(samples[0], samples[2]);
        }
    }
}

#[cfg(test)]
mod reading_lifecycle_tests {
    use super::*;
    #[test]
    fn direct_open_invalidates_late_async_search_open_and_failed_open_keeps_identity() {
        let ctx = egui::Context::default();
        let mut app = preview_layout_tests::editor(&ctx, "initial");
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a.md");
        let b = temp.path().join("b.md");
        let c = temp.path().join("c.md");
        for (path, text) in [(&a, "# A"), (&b, "# B"), (&c, "# C")] {
            std::fs::write(path, text).unwrap();
        }
        app.set_workspace(temp.path().to_owned());
        app.open_path(a);
        app.dispatch_search_open(b, &ctx);
        app.open_path(c.clone());
        let id = app.doc.id;
        let generation = app.document_generation;
        for _ in 0..100 {
            app.poll_global_search(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(app.doc.path, Some(c.canonicalize().unwrap()));
        assert_eq!(app.doc.id, id);
        assert_eq!(app.document_generation, generation);
        app.open_path(temp.path().join("missing.md"));
        assert_eq!(app.doc.id, id);
        assert_eq!(app.document_generation, generation);
        assert_eq!(app.doc.text, "# C");
    }
}
