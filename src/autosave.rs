// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Bounded, per-buffer save scheduling. Workers own snapshots; only baselines return.
use crate::{Document, Editor, egui};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const DELAY: Duration = Duration::from_secs(2);
const PREPARING: u8 = 0;
const COMMITTING: u8 = 1;
const CANCELLED: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    buffer: u64,
    generation: u64,
    path: Option<PathBuf>,
    revision: u64,
    encoding: crate::encoding::Encoding,
    bom: bool,
}
impl Version {
    fn of(doc: &Document, generation: u64) -> Self {
        Self {
            buffer: doc.buffer_id(),
            generation,
            path: doc.path.clone(),
            revision: doc.revision(),
            encoding: doc.encoding,
            bom: doc.bom,
        }
    }
    fn owns(&self, doc: &Document, generation: u64) -> bool {
        self.buffer == doc.buffer_id() && self.generation == generation && self.path == doc.path
    }
}
struct Job {
    version: Version,
    automatic: bool,
    gate: Arc<AtomicU8>,
    result: mpsc::Receiver<Result<Document, String>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum SaveTarget {
    Current,
    Explicit(PathBuf),
}
#[derive(Default)]
struct Slot {
    observed: Option<Version>,
    due: Option<Instant>,
    job: Option<Job>,
    // Keep a cancelled worker accounted for until it acknowledges cancellation.
    // New documents can replace the buffer, but cannot accumulate worker threads.
    retiring: Option<Job>,
    error: Option<String>,
    manual: Option<SaveTarget>,
}
#[derive(Default)]
pub struct SaveCoordinator {
    slots: [Slot; 2],
    pub context: Option<egui::Context>,
}
impl SaveCoordinator {
    fn slot(&self, secondary: bool) -> &Slot {
        &self.slots[usize::from(secondary)]
    }
    fn slot_mut(&mut self, secondary: bool) -> &mut Slot {
        &mut self.slots[usize::from(secondary)]
    }
    pub fn active(&self, secondary: bool) -> bool {
        self.slot(secondary).job.is_some() || self.slot(secondary).manual.is_some()
    }
    /// Cancellation wins before the commit gate. If commit won, retain the job
    /// and its receiver until the UI has acknowledged the actual disk outcome.
    pub fn quiesce(&mut self, secondary: bool) -> bool {
        let slot = self.slot_mut(secondary);
        slot.due = None;
        slot.manual = None;
        if let Some(job) = &slot.job {
            if job
                .gate
                .compare_exchange(PREPARING, CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return false;
            }
            slot.retiring = slot.job.take();
        }
        slot.observed = None;
        true
    }
    pub fn reset(&mut self, secondary: bool) {
        let retiring = self.slot_mut(secondary).retiring.take();
        *self.slot_mut(secondary) = Slot {
            retiring,
            ..Slot::default()
        };
    }
    fn observe(
        &mut self,
        secondary: bool,
        doc: &Document,
        generation: u64,
        now: Instant,
        enabled: bool,
        blocked: bool,
    ) -> bool {
        let slot = self.slot_mut(secondary);
        if !enabled {
            slot.due = None;
            slot.observed = None;
            if slot.job.as_ref().is_some_and(|j| j.automatic) {
                self.quiesce(secondary);
            }
            return false;
        }
        let version = Version::of(doc, generation);
        if slot.observed.as_ref() != Some(&version) {
            slot.observed = Some(version);
            slot.due = Some(now + DELAY);
        }
        if !doc.dirty() || doc.path.is_none() {
            slot.due = None;
            return false;
        }
        if slot.error.is_some() {
            slot.due = None;
            return false;
        }
        if slot.due.is_none() && slot.job.is_none() && slot.error.is_none() {
            slot.due = Some(now + DELAY);
        }
        if blocked {
            slot.due = Some(now + DELAY);
            return false;
        }
        slot.error.is_none()
            && slot.job.is_none()
            && slot.retiring.is_none()
            && slot.due.is_some_and(|due| due <= now)
    }
}
impl Drop for SaveCoordinator {
    fn drop(&mut self) {
        for slot in &self.slots {
            if let Some(job) = &slot.job {
                let _ = job.gate.compare_exchange(
                    PREPARING,
                    CANCELLED,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
            }
        }
    }
}

impl Editor {
    pub(crate) fn autosave_report(&self) -> serde_json::Value {
        serde_json::json!({"enabled": self.preferences.autosave, "main": {"saving": self.saves.active(false), "waiting": self.saves.slot(false).due.is_some(), "error": self.saves.slot(false).error}, "secondary": {"saving": self.saves.active(true), "waiting": self.saves.slot(true).due.is_some(), "error": self.saves.slot(true).error}, "pending_navigation": self.pending.is_some(), "ime": self.ime_composing})
    }
    pub(crate) fn start_save(
        &mut self,
        secondary: bool,
        path: PathBuf,
        automatic: bool,
        ctx: &egui::Context,
    ) {
        self.start_save_target(secondary, SaveTarget::Explicit(path), automatic, ctx);
    }
    pub(crate) fn start_current_save(&mut self, secondary: bool, ctx: &egui::Context) {
        self.start_save_target(secondary, SaveTarget::Current, false, ctx);
    }
    fn start_save_target(
        &mut self,
        secondary: bool,
        target: SaveTarget,
        automatic: bool,
        ctx: &egui::Context,
    ) {
        if self.global.busy {
            return;
        }
        if self.saves.active(secondary) || self.saves.slot(secondary).retiring.is_some() {
            if !automatic {
                self.saves.slot_mut(secondary).manual = Some(target);
            }
            return;
        }
        let (doc, generation) = if secondary {
            let Some(doc) = &self.global.secondary else {
                return;
            };
            (doc, self.global.secondary_identity())
        } else {
            (&self.doc, self.document_generation)
        };
        let path = match target {
            SaveTarget::Current => {
                let Some(path) = doc.path.clone() else { return };
                path
            }
            SaveTarget::Explicit(path) => path,
        };
        let version = Version::of(doc, generation);
        let mut snapshot = doc.clone();
        let gate = Arc::new(AtomicU8::new(PREPARING));
        let worker_gate = gate.clone();
        let (tx, result) = mpsc::sync_channel(1);
        let repaint = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("document-save".into())
            .spawn(move || {
                let saved = snapshot
                    .save_guarded(&path, || {
                        worker_gate
                            .compare_exchange(
                                PREPARING,
                                COMMITTING,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            )
                            .is_ok()
                    })
                    .map(|()| snapshot)
                    .map_err(|e| e.to_string());
                let _ = tx.send(saved);
                repaint.request_repaint();
            });
        let slot = self.saves.slot_mut(secondary);
        slot.due = None;
        slot.error = None;
        match spawned {
            Ok(_) => {
                slot.job = Some(Job {
                    version,
                    automatic,
                    gate,
                    result,
                })
            }
            Err(e) => slot.error = Some(e.to_string()),
        }
    }
    pub(crate) fn poll_saves(&mut self, ctx: &egui::Context) {
        self.saves.context = Some(ctx.clone());
        for secondary in [false, true] {
            let retired = self
                .saves
                .slot(secondary)
                .retiring
                .as_ref()
                .is_some_and(|job| {
                    !matches!(job.result.try_recv(), Err(mpsc::TryRecvError::Empty))
                });
            if retired {
                self.saves.slot_mut(secondary).retiring = None;
                if let Some(target) = self.saves.slot_mut(secondary).manual.take() {
                    self.start_save_target(secondary, target, false, ctx);
                }
            }
            let slot = self.saves.slot_mut(secondary);
            let result = slot
                .job
                .as_ref()
                .and_then(|job| match job.result.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Save worker stopped.".into()))
                    }
                    Err(mpsc::TryRecvError::Empty) => None,
                });
            let Some(result) = result else {
                continue;
            };
            let job = slot.job.take().unwrap();
            let generation = if secondary {
                self.global.secondary_identity()
            } else {
                self.document_generation
            };
            let doc = if secondary {
                self.global.secondary.as_mut()
            } else {
                Some(&mut self.doc)
            };
            let Some(doc) = doc.filter(|doc| job.version.owns(doc, generation)) else {
                slot.manual = None;
                continue;
            };
            match result {
                Ok(saved) => {
                    let copied = saved.path != doc.path;
                    let old_id = doc.id;
                    doc.accept_saved_snapshot(&saved);
                    if !doc.dirty()
                        && (secondary
                            || (!self.rich.pending_edits() && !self.structure.pending_edits()))
                        && !self.recovery.has_restored_drafts(secondary)
                        && !self.recovery.invalidate(secondary)
                    {
                        self.save_then_action = false;
                        self.global.secondary_save_close = false;
                        self.error=Some(self.language.text("The file was saved, but its recovery copy could not be cleared. Retry recovery protection.").into());
                    }
                    if copied {
                        self.reading.copied_path(
                            doc,
                            old_id,
                            if secondary { "secondary" } else { "main" },
                        );
                    }
                    slot.error = None;
                    self.status = "Saved to disk";
                    self.global.buffer_changed();
                    if !secondary && copied {
                        self.sync_file_root();
                    }
                    if secondary
                        && self.global.secondary_save_close
                        && self.global.secondary.as_ref().is_some_and(|d| !d.dirty())
                    {
                        self.global.secondary = None;
                        self.global.secondary_save_close = false;
                        self.global.secondary_close = false;
                    }
                }
                Err(error) => {
                    slot.error = Some(error);
                    slot.manual = None;
                    if !secondary {
                        self.save_then_action = false;
                    }
                    self.global.secondary_save_close = false;
                }
            }
            let manual = self.saves.slot_mut(secondary).manual.take();
            if let Some(target) = manual {
                self.start_save_target(secondary, target, false, ctx);
            }
        }
        if self.save_then_action
            && !self.saves.active(false)
            && !self.doc.dirty()
            && !self.rich.pending_edits()
        {
            self.save_then_action = false;
            if let Some(action) = self.pending.take() {
                self.perform(action, ctx);
            }
        }
    }
    pub(crate) fn tick_autosave(&mut self, ctx: &egui::Context, now: Instant) {
        let blocked = self.global.busy
            || self.pending.is_some()
            || self.rename_open
            || self.error.is_some()
            || self.ime_composing;
        let main_blocked = blocked
            || self.rich.pending_edits()
            || self.rich.synchronization_error().is_some()
            || self.structure.pending_edits();
        if self.saves.observe(
            false,
            &self.doc,
            self.document_generation,
            now,
            self.preferences.autosave,
            main_blocked,
        ) {
            self.start_save(false, self.doc.path.clone().unwrap(), true, ctx);
        }
        if let Some(doc) = &self.global.secondary {
            let path = doc.path.clone();
            if self.saves.observe(
                true,
                doc,
                self.global.secondary_identity(),
                now,
                self.preferences.autosave,
                blocked || self.global.secondary_close,
            ) {
                self.start_save(true, path.unwrap(), true, ctx);
            }
        } else if self.saves.quiesce(true) {
            self.saves.reset(true);
        }
        for slot in &self.saves.slots {
            if let Some(due) = slot.due {
                ctx.request_repaint_after(
                    due.saturating_duration_since(now)
                        .max(Duration::from_millis(20)),
                );
            }
            if slot.job.is_some() || slot.retiring.is_some() {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
    }
    pub(crate) fn autosave_status(&mut self, ui: &mut egui::Ui, secondary: bool) {
        let slot = self.saves.slot(secondary);
        let doc = if secondary {
            self.global.secondary.as_ref()
        } else {
            Some(&self.doc)
        };
        let Some(doc) = doc else {
            return;
        };
        let pending = !secondary
            && (self.rich.pending_edits()
                || self.ime_composing
                || self.rich.synchronization_error().is_some()
                || self.structure.pending_edits());
        let label = if slot.error.is_some() {
            "Save failed · automatic saving paused"
        } else if slot.job.is_some() {
            "Saving…"
        } else if !self.preferences.autosave {
            "Auto save off"
        } else if doc.path.is_none() {
            "Save manually first to enable auto save"
        } else if pending {
            "Waiting for valid edits to synchronize"
        } else if doc.presentation_dirty() {
            "Auto save on · waiting to save"
        } else {
            "Auto save on · saved"
        };
        let error = slot.error.clone();
        ui.horizontal_wrapped(|ui| {
            ui.small(self.language.text(label));
            if let Some(error) = error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    crate::localized_document_error(self.language, &std::io::Error::other(error)),
                );
                let retry = ui.button(self.language.text("Retry save"));
                crate::theme::record_control(
                    ui,
                    &retry,
                    if secondary {
                        "retry-save-secondary"
                    } else {
                        "retry-save-main"
                    },
                );
                if retry.clicked() {
                    if secondary {
                        self.save_secondary();
                    } else {
                        self.save(false, ui.ctx());
                    }
                }
            }
        });
    }
}

#[cfg(test)]
pub(crate) fn wait_for_saves(app: &mut Editor, ctx: &egui::Context) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.saves.active(false) || app.saves.active(true) {
        app.poll_saves(ctx);
        assert!(Instant::now() < deadline, "save worker did not finish");
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture() -> (tempfile::TempDir, Editor, egui::Context) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.md");
        fs::write(&path, "original\r\n").unwrap();
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "");
        app.doc = Document::open(&path).unwrap();
        (dir, app, ctx)
    }
    #[test]
    fn default_off_requires_explicit_original_file_autosave_and_restored_false_stays_disabled() {
        let (_dir, mut app, ctx) = fixture();
        app.preferences = crate::preferences::Preferences::default();
        assert!(!app.preferences.autosave);
        app.preferences.autosave = true;
        edit(&mut app.doc, "default on writes");
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY);
        wait_for_saves(&mut app, &ctx);
        let path = app.doc.path.clone().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "default on writes");
        app.preferences = serde_json::from_str(r#"{"autosave":false}"#).unwrap();
        edit(&mut app.doc, "disabled edit stays in memory");
        app.tick_autosave(&ctx, t + DELAY * 2);
        app.tick_autosave(&ctx, t + DELAY * 20);
        assert!(!app.saves.active(false));
        assert!(app.doc.dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "default on writes");
    }
    #[test]
    fn valid_rich_draft_must_sync_before_autosave_and_save_keeps_floating_focus() {
        let (dir, mut app, ctx) = fixture();
        let path = dir.path().join("formula.md");
        fs::write(&path, "$x^2$\n").unwrap();
        app.doc = Document::open(&path).unwrap();
        edit(&mut app.doc, "$x^2$\n\nchanged body\n");
        app.preferences.autosave = true;
        app.rich
            .stage_pending_test_edit(&app.doc.text, app.document_generation, "z^3", &ctx);
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY * 10);
        assert!(!app.saves.active(false));
        assert!(app.doc.dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "$x^2$\n");
        let mut frame = eframe::Frame::_new_kittest();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 800.),
                )),
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut app, ctx, &mut frame),
        );
        app.rich.make_pending_test_edit_due();
        let floating = egui::Id::new("rich-floating-source");
        ctx.memory_mut(|m| m.request_focus(floating));
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 800.),
                )),
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut app, ctx, &mut frame),
        );
        assert!(app.doc.text.starts_with("$z^3$\n"));
        assert!(!app.rich.pending_edits());
        let focus = ctx.memory(|m| m.focused());
        assert_eq!(focus, Some(floating));
        app.tick_autosave(&ctx, Instant::now() + DELAY * 20);
        wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("$z^3$\n\nchanged body")
        );
        assert_eq!(ctx.memory(|m| m.focused()), focus);
    }
    #[test]
    fn cancelled_worker_remains_bounded_across_reopens_and_retains_latest_manual_request() {
        let (_dir, mut app, ctx) = fixture();
        let path = app.doc.path.clone().unwrap();
        let write_lock = crate::document::disk_transaction().unwrap();
        edit(&mut app.doc, "cancel this snapshot");
        app.start_save(false, path.clone(), true, &ctx);
        let gate = app.saves.slot(false).job.as_ref().unwrap().gate.clone();
        assert!(app.saves.quiesce(false));
        for n in 0..32 {
            app.commit_document(Document::open(&path).unwrap(), false);
            edit(&mut app.doc, &format!("latest manual {n}"));
            app.start_save(false, path.clone(), false, &ctx);
            assert!(app.saves.slot(false).job.is_none());
            assert!(Arc::ptr_eq(
                &gate,
                &app.saves.slot(false).retiring.as_ref().unwrap().gate
            ));
            assert_eq!(
                app.saves.slot(false).manual.as_ref(),
                Some(&SaveTarget::Explicit(path.clone()))
            );
            app.poll_saves(&ctx);
        }
        assert_eq!(gate.load(Ordering::SeqCst), CANCELLED);
        assert_eq!(fs::read_to_string(&path).unwrap(), "original\r\n");
        drop(write_lock);
        wait_for_saves(&mut app, &ctx);
        assert_eq!(fs::read_to_string(&path).unwrap(), "latest manual 31");
        assert!(!app.doc.dirty());
    }
    #[test]
    fn failed_preference_write_keeps_explicit_disable_and_does_not_retry_each_frame() {
        let (_dir, mut app, ctx) = fixture();
        app.saved_preferences.autosave = true;
        app.preferences.autosave = false;
        app.apply_preferences_result(Err(std::io::Error::other("settings disk unavailable")));
        assert!(!app.preferences.autosave);
        assert!(app.saved_preferences.autosave);
        assert_eq!(app.failed_preferences.as_ref(), Some(&app.preferences));
        assert!(
            app.settings_error
                .as_ref()
                .unwrap()
                .contains("only to this session")
        );
        edit(&mut app.doc, "must remain in memory");
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY * 3);
        assert!(!app.saves.active(false));
        assert!(app.doc.dirty());
    }
    #[test]
    fn secondary_close_keeps_unconfirmed_save_and_cancel_revokes_save_close_intent() {
        let (_dir, mut app, ctx) = fixture();
        app.global.secondary = Some(Document::open(app.doc.path.as_ref().unwrap()).unwrap());
        edit(app.global.secondary.as_mut().unwrap(), "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, true, false);
        gate.store(COMMITTING, Ordering::SeqCst);
        edit(app.global.secondary.as_mut().unwrap(), "original\r\n");
        app.request_secondary_close();
        assert!(app.global.secondary_close);
        assert!(app.global.secondary.is_some());
        app.global.secondary_save_close = true;
        app.cancel_secondary_close();
        snapshot.save_to(&snapshot.path.clone().unwrap()).unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        assert!(app.global.secondary.is_some());
        assert!(!app.global.secondary_close);
        assert!(!app.global.secondary_save_close);
        assert!(app.global.secondary.as_ref().unwrap().dirty());
        assert_eq!(app.global.secondary.as_ref().unwrap().text, "original\r\n");
    }
    #[test]
    fn confirmed_discard_is_final_even_when_opening_the_replacement_fails() {
        let (dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        edit(&mut app.doc, "discard forever");
        let (mut snapshot, _tx, gate) = controlled(&mut app, false, true);
        let path = app.doc.path.clone().unwrap();
        app.request(crate::Action::OpenPath(dir.path().join("missing.md")), &ctx);
        assert!(app.pending.is_some());
        app.discard_pending_action(&ctx);
        assert_eq!(app.doc.text, "original\r\n");
        assert!(!app.doc.dirty());
        assert!(app.error.is_some());
        assert!(
            snapshot
                .save_guarded(&path, || gate
                    .compare_exchange(PREPARING, COMMITTING, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok())
                .is_err()
        );
        app.error = None;
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY * 10);
        assert!(!app.saves.active(false));
        assert_eq!(fs::read_to_string(path).unwrap(), "original\r\n");
    }
    #[test]
    fn save_as_completion_keeps_newer_b_and_moves_only_future_saves_to_new_path() {
        let (dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        let old = app.doc.path.clone().unwrap();
        let target = dir.path().join("copy.md");
        edit(&mut app.doc, "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, false, false);
        edit(&mut app.doc, "B");
        gate.store(COMMITTING, Ordering::SeqCst);
        snapshot.save_to(&target).unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        assert_eq!(app.doc.text, "B");
        assert!(app.doc.dirty());
        assert_eq!(app.doc.path, Some(target.canonicalize().unwrap()));
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY);
        wait_for_saves(&mut app, &ctx);
        assert_eq!(fs::read_to_string(target).unwrap(), "B");
        assert_eq!(fs::read_to_string(old).unwrap(), "original\r\n");
    }
    #[test]
    fn normal_save_queued_during_save_as_uses_new_path() {
        let (dir, mut app, ctx) = fixture();
        let old = app.doc.path.clone().unwrap();
        let target = dir.path().join("copy.md");
        edit(&mut app.doc, "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, false, false);
        edit(&mut app.doc, "B");
        app.save(false, &ctx);
        assert_eq!(app.saves.slot(false).manual, Some(SaveTarget::Current));
        gate.store(COMMITTING, Ordering::SeqCst);
        snapshot.save_to(&target).unwrap();
        fs::write(&old, "external change to original").unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        wait_for_saves(&mut app, &ctx);
        assert_eq!(app.doc.path, Some(target.canonicalize().unwrap()));
        assert!(!app.doc.dirty());
        assert_eq!(fs::read_to_string(target).unwrap(), "B");
        assert_eq!(
            fs::read_to_string(old).unwrap(),
            "external change to original"
        );
    }
    #[test]
    fn normal_save_queued_during_save_as_detects_new_target_conflict() {
        let (dir, mut app, ctx) = fixture();
        let old = app.doc.path.clone().unwrap();
        let target = dir.path().join("copy.md");
        edit(&mut app.doc, "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, false, false);
        edit(&mut app.doc, "B");
        app.save(false, &ctx);
        gate.store(COMMITTING, Ordering::SeqCst);
        snapshot.save_to(&target).unwrap();
        fs::write(&target, "external change to copy").unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        wait_for_saves(&mut app, &ctx);
        assert_eq!(app.doc.path, Some(target.canonicalize().unwrap()));
        assert!(app.doc.dirty());
        assert!(app.saves.slot(false).error.is_some());
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "external change to copy"
        );
        assert_eq!(fs::read_to_string(old).unwrap(), "original\r\n");
    }
    #[test]
    fn failed_save_as_discards_queued_normal_save() {
        let (_dir, mut app, ctx) = fixture();
        let old = app.doc.path.clone().unwrap();
        edit(&mut app.doc, "unsaved");
        let (_snapshot, tx, _gate) = controlled(&mut app, false, false);
        app.save(false, &ctx);
        tx.send(Err("Save As failed".into())).unwrap();
        app.poll_saves(&ctx);
        assert!(!app.saves.active(false));
        assert!(app.doc.dirty());
        assert_eq!(app.doc.path, Some(old.clone()));
        assert_eq!(fs::read_to_string(old).unwrap(), "original\r\n");
    }
    #[test]
    fn secondary_normal_save_resolves_current_path_after_prior_save() {
        let (dir, mut app, ctx) = fixture();
        let old = app.doc.path.clone().unwrap();
        let target = dir.path().join("secondary-copy.md");
        app.global.secondary = Some(Document::open(&old).unwrap());
        edit(app.global.secondary.as_mut().unwrap(), "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, true, false);
        edit(app.global.secondary.as_mut().unwrap(), "B");
        app.save_secondary();
        assert_eq!(app.saves.slot(true).manual, Some(SaveTarget::Current));
        gate.store(COMMITTING, Ordering::SeqCst);
        snapshot.save_to(&target).unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        wait_for_saves(&mut app, &ctx);
        let secondary = app.global.secondary.as_ref().unwrap();
        assert_eq!(secondary.path, Some(target.canonicalize().unwrap()));
        assert!(!secondary.dirty());
        assert_eq!(fs::read_to_string(target).unwrap(), "B");
        assert_eq!(fs::read_to_string(old).unwrap(), "original\r\n");
    }
    fn edit(doc: &mut Document, text: &str) {
        doc.text = text.into();
        doc.mark_text_changed();
    }
    // Inject an uncompleted worker. Tests decide exactly when the immutable
    // snapshot commits and when its result reaches the UI, without sleeps.
    fn controlled(
        app: &mut Editor,
        secondary: bool,
        automatic: bool,
    ) -> (
        Document,
        mpsc::SyncSender<Result<Document, String>>,
        Arc<AtomicU8>,
    ) {
        let (doc, generation) = if secondary {
            (
                app.global.secondary.as_ref().unwrap(),
                app.global.secondary_identity(),
            )
        } else {
            (&app.doc, app.document_generation)
        };
        let snapshot = doc.clone();
        let version = Version::of(doc, generation);
        let (tx, result) = mpsc::sync_channel(1);
        let gate = Arc::new(AtomicU8::new(PREPARING));
        app.saves.slot_mut(secondary).job = Some(Job {
            version,
            automatic,
            gate: gate.clone(),
            result,
        });
        (snapshot, tx, gate)
    }
    #[test]
    fn disabled_untitled_clean_and_non_edit_activity_never_schedule() {
        let (_dir, mut app, _ctx) = fixture();
        let now = Instant::now();
        edit(&mut app.doc, "changed");
        for n in 0..5 {
            assert!(!app.saves.observe(
                false,
                &app.doc,
                0,
                now + Duration::from_secs(n),
                false,
                false
            ));
        }
        let untitled = Document::from_text("draft");
        assert!(!app.saves.observe(true, &untitled, 0, now, true, false));
        assert!(app.saves.slot(true).due.is_none());
        app.doc = Document::open(app.doc.path.as_ref().unwrap()).unwrap();
        for n in 0..5 {
            assert!(!app.saves.observe(
                false,
                &app.doc,
                0,
                now + Duration::from_secs(n),
                true,
                false
            ));
        }
        assert!(app.saves.slot(false).due.is_none());
    }
    #[test]
    fn two_second_debounce_coalesces_and_format_alone_is_an_edit() {
        let (_dir, mut app, _ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "A");
        assert!(!app.saves.observe(false, &app.doc, 0, t, true, false));
        assert!(!app.saves.observe(
            false,
            &app.doc,
            0,
            t + Duration::from_millis(1999),
            true,
            false
        ));
        edit(&mut app.doc, "B");
        assert!(!app.saves.observe(
            false,
            &app.doc,
            0,
            t + Duration::from_millis(1999),
            true,
            false
        ));
        assert!(!app.saves.observe(
            false,
            &app.doc,
            0,
            t + Duration::from_millis(3998),
            true,
            false
        ));
        assert!(app.saves.observe(
            false,
            &app.doc,
            0,
            t + Duration::from_millis(3999),
            true,
            false
        ));
        let revision = app.doc.revision();
        app.doc
            .set_encoding(crate::encoding::Encoding::Utf16Le, true)
            .unwrap();
        assert_eq!(revision, app.doc.revision());
        assert!(
            !app.saves
                .observe(false, &app.doc, 0, t + Duration::from_secs(4), true, false)
        );
        assert!(
            app.saves
                .observe(false, &app.doc, 0, t + Duration::from_secs(6), true, false)
        );
    }
    #[test]
    fn main_and_secondary_save_independently_and_keep_bom_crlf() {
        let (dir, mut app, ctx) = fixture();
        let path = dir.path().join("second.md");
        fs::write(&path, b"\xef\xbb\xbfsecond\r\n").unwrap();
        app.global.secondary = Some(Document::open(&path).unwrap());
        app.preferences.autosave = true;
        edit(&mut app.doc, "main edited\r\n");
        edit(app.global.secondary.as_mut().unwrap(), "second edited\r\n");
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY);
        assert!(app.saves.active(false) && app.saves.active(true));
        wait_for_saves(&mut app, &ctx);
        assert_eq!(
            fs::read(app.doc.path.as_ref().unwrap()).unwrap(),
            b"main edited\r\n"
        );
        assert_eq!(fs::read(path).unwrap(), b"\xef\xbb\xbfsecond edited\r\n");
        assert!(!app.doc.dirty() && !app.global.secondary.as_ref().unwrap().dirty());
    }
    #[test]
    fn completion_of_a_preserves_b_and_its_pending_format_then_saves_b() {
        let (_dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        edit(&mut app.doc, "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, false, true);
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        edit(&mut app.doc, "B🙂\r\n");
        app.doc
            .set_encoding(crate::encoding::Encoding::Utf16Be, true)
            .unwrap();
        app.tick_autosave(&ctx, t + Duration::from_secs(1));
        gate.store(COMMITTING, Ordering::SeqCst);
        snapshot.save_to(&snapshot.path.clone().unwrap()).unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        assert_eq!(app.doc.text, "B🙂\r\n");
        assert!(app.doc.dirty());
        assert_eq!(app.doc.encoding, crate::encoding::Encoding::Utf16Be);
        app.tick_autosave(&ctx, t + Duration::from_secs(3));
        wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
        assert_eq!(
            &fs::read(app.doc.path.as_ref().unwrap()).unwrap()[..2],
            &[254, 255]
        );
    }
    #[test]
    fn undo_to_old_baseline_while_a_saves_is_rescheduled_after_a() {
        let (_dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        let t = Instant::now();
        edit(&mut app.doc, "A");
        let (mut snapshot, tx, gate) = controlled(&mut app, false, true);
        app.tick_autosave(&ctx, t);
        edit(&mut app.doc, "original\r\n");
        app.tick_autosave(&ctx, t + DELAY);
        assert!(app.saves.slot(false).due.is_none());
        gate.store(COMMITTING, Ordering::SeqCst);
        snapshot.save_to(&snapshot.path.clone().unwrap()).unwrap();
        tx.send(Ok(snapshot)).unwrap();
        app.poll_saves(&ctx);
        assert!(app.doc.dirty());
        app.tick_autosave(&ctx, t + DELAY);
        app.tick_autosave(&ctx, t + DELAY * 2);
        wait_for_saves(&mut app, &ctx);
        assert_eq!(
            fs::read_to_string(app.doc.path.as_ref().unwrap()).unwrap(),
            "original\r\n"
        );
        assert!(!app.doc.dirty());
    }
    #[test]
    fn stale_open_generation_buffer_or_path_results_are_ignored() {
        for scenario in 0..3 {
            let (dir, mut app, ctx) = fixture();
            edit(&mut app.doc, "A");
            let (snapshot, tx, _gate) = controlled(&mut app, false, false);
            match scenario {
                0 => app.document_generation += 1,
                1 => app.doc = Document::from_text("NEW"),
                _ => app.doc.path = Some(dir.path().join("renamed.md")),
            }
            let text = app.doc.text.clone();
            let path = app.doc.path.clone();
            tx.send(Ok(snapshot)).unwrap();
            app.poll_saves(&ctx);
            assert_eq!(app.doc.text, text);
            assert_eq!(app.doc.path, path);
        }
    }
    #[test]
    fn disable_discard_switch_and_rename_cancel_before_commit_without_late_write() {
        let (dir, mut app, ctx) = fixture();
        edit(&mut app.doc, "discarded");
        let (mut snapshot, _tx, gate) = controlled(&mut app, false, true);
        app.preferences.autosave = false;
        app.tick_autosave(&ctx, Instant::now());
        assert_eq!(gate.load(Ordering::SeqCst), CANCELLED);
        let old = snapshot.path.clone().unwrap();
        app.doc.rename_in_place("renamed.md").unwrap();
        assert!(
            snapshot
                .save_guarded(&old, || gate
                    .compare_exchange(PREPARING, COMMITTING, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok())
                .is_err()
        );
        assert!(!old.exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("renamed.md")).unwrap(),
            "original\r\n"
        );
        app.request(crate::Action::New, &ctx);
        assert!(app.pending.is_some());
    }
    #[test]
    fn commit_gate_retains_close_protection_even_when_current_text_looks_clean() {
        let (_dir, mut app, ctx) = fixture();
        edit(&mut app.doc, "A");
        let (_snapshot, _tx, gate) = controlled(&mut app, false, true);
        gate.store(COMMITTING, Ordering::SeqCst);
        edit(&mut app.doc, "original\r\n");
        assert!(!app.doc.dirty());
        app.request(crate::Action::New, &ctx);
        assert!(app.pending.is_some());
        assert!(app.saves.active(false));
        assert!(!app.allow_close);
        app.pending = None;
        let mut frame = eframe::Frame::_new_kittest();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| eframe::App::update(&mut app, ctx, &mut frame));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::CancelClose)
        );
        assert!(app.pending.is_some());
    }
    #[test]
    fn external_change_failure_is_paused_and_manual_retry_recovers() {
        let (_dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        edit(&mut app.doc, "memory");
        fs::write(app.doc.path.as_ref().unwrap(), "external").unwrap();
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY);
        wait_for_saves(&mut app, &ctx);
        assert!(app.doc.dirty());
        assert!(
            app.saves
                .slot(false)
                .error
                .as_ref()
                .unwrap()
                .contains("outside")
        );
        app.tick_autosave(&ctx, t + Duration::from_secs(100));
        assert!(!app.saves.active(false));
        assert_eq!(
            fs::read_to_string(app.doc.path.as_ref().unwrap()).unwrap(),
            "external"
        );
        fs::write(app.doc.path.as_ref().unwrap(), "original\r\n").unwrap();
        app.save(false, &ctx);
        wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
        assert!(app.saves.slot(false).error.is_none());
    }
    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // This branch runs only on Windows; Unix uses mode 0600 below.
    fn deleted_readonly_unencodable_paths_keep_memory_and_do_not_block_other_pane() {
        let (dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        let path = dir.path().join("second.md");
        fs::write(&path, "second").unwrap();
        app.global.secondary = Some(Document::open(&path).unwrap());
        edit(app.global.secondary.as_mut().unwrap(), "second saved");
        edit(&mut app.doc, "memory");
        let original = app.doc.path.clone().unwrap();
        fs::remove_file(&original).unwrap();
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY);
        wait_for_saves(&mut app, &ctx);
        assert!(!original.exists());
        assert!(app.doc.dirty());
        assert!(app.saves.slot(false).error.is_some());
        assert!(!app.global.secondary.as_ref().unwrap().dirty());
        fs::write(&original, "original\r\n").unwrap();
        let mut permissions = fs::metadata(&original).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&original, permissions).unwrap();
        app.save(false, &ctx);
        wait_for_saves(&mut app, &ctx);
        assert!(app.doc.dirty());
        assert!(app.saves.slot(false).error.is_some());
        #[cfg(windows)]
        {
            let mut permissions = fs::metadata(&original).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&original, permissions).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&original, fs::Permissions::from_mode(0o600)).unwrap();
        }
        app.doc.encoding = crate::encoding::Encoding::Windows1252;
        edit(&mut app.doc, "🙂");
        app.save(false, &ctx);
        wait_for_saves(&mut app, &ctx);
        assert!(app.doc.dirty());
        assert_eq!(fs::read_to_string(original).unwrap(), "original\r\n");
    }
    #[test]
    fn untitled_first_manual_save_enables_later_edits_and_ime_drafts_block() {
        let (dir, mut app, ctx) = fixture();
        app.preferences.autosave = true;
        app.doc = Document::default();
        edit(&mut app.doc, "first");
        let t = Instant::now();
        app.tick_autosave(&ctx, t);
        app.tick_autosave(&ctx, t + DELAY);
        assert!(!app.saves.active(false));
        assert!(app.doc.path.is_none());
        let path = dir.path().join("first.md");
        app.start_save(false, path.clone(), false, &ctx);
        wait_for_saves(&mut app, &ctx);
        edit(&mut app.doc, "later");
        app.ime_composing = true;
        app.tick_autosave(&ctx, t + DELAY * 2);
        app.tick_autosave(&ctx, t + DELAY * 4);
        assert!(!app.saves.active(false));
        app.ime_composing = false;
        app.tick_autosave(&ctx, t + DELAY * 5);
        wait_for_saves(&mut app, &ctx);
        assert_eq!(fs::read_to_string(path).unwrap(), "later");
        let source = "$x^2$\n";
        app.doc = Document::from_text(source);
        app.doc.path = Some(dir.path().join("formula.md"));
        app.rich
            .stage_pending_test_edit(source, app.document_generation, "z^3", &ctx);
        app.tick_autosave(&ctx, t + DELAY * 6);
        assert!(!app.saves.active(false));
        assert!(app.rich.pending_edits());
    }
    #[test]
    fn cross_process_conflict_worker() {
        let Some(root) = std::env::var_os("MDCARD_CONFLICT_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let id = std::env::var("MDCARD_CONFLICT_TEST_ID").unwrap();
        let mut doc = Document::open(&root.join("shared.md")).unwrap();
        edit(&mut doc, &format!("writer {id}"));
        fs::write(root.join(format!("ready-{id}")), b"").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !root.join("go").exists() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let result = doc.save_to(&root.join("shared.md"));
        fs::write(
            root.join(format!("result-{id}")),
            if result.is_ok() { "ok" } else { "conflict" },
        )
        .unwrap();
    }
    #[test]
    fn separate_processes_cannot_silently_overwrite_the_same_baseline() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("shared.md"), "baseline").unwrap();
        let mut children = Vec::new();
        for id in ["1", "2"] {
            children.push(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "autosave::tests::cross_process_conflict_worker",
                        "--nocapture",
                    ])
                    .env("MDCARD_CONFLICT_TEST_ROOT", dir.path())
                    .env("MDCARD_CONFLICT_TEST_ID", id)
                    .spawn()
                    .unwrap(),
            );
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while !(dir.path().join("ready-1").exists() && dir.path().join("ready-2").exists()) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        fs::write(dir.path().join("go"), b"").unwrap();
        for child in &mut children {
            assert!(child.wait().unwrap().success());
        }
        let mut results = vec![
            fs::read_to_string(dir.path().join("result-1")).unwrap(),
            fs::read_to_string(dir.path().join("result-2")).unwrap(),
        ];
        results.sort();
        assert_eq!(results, vec!["conflict", "ok"]);
    }
}
