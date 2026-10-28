// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use crate::{
    Editor, egui,
    structure::{self, Command, Index},
};
use egui::text::{CCursor, CCursorRange};
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy)]
enum EditKey {
    History(bool),
    Table(usize),
    Block(usize),
}

fn cell_destination(
    table: &structure::Table,
    row: usize,
    column: usize,
    backward: bool,
) -> ((usize, usize), bool) {
    let width = table
        .rows
        .iter()
        .map(|r| r.cells.len())
        .max()
        .unwrap_or(table.columns)
        .max(table.columns);
    let slot = if row == 0 {
        column.min(table.columns - 1)
    } else {
        table.columns + (row - 2) * width + column
    };
    let next = if backward {
        slot.saturating_sub(1)
    } else {
        slot + 1
    };
    let count = table.columns + (table.rows.len() - 2) * width;
    if next >= count {
        ((table.rows.len(), 0), true)
    } else if next < table.columns {
        ((0, next), false)
    } else {
        (
            (
                (next - table.columns) / width + 2,
                (next - table.columns) % width,
            ),
            false,
        )
    }
}

fn classify_edit_key(event: &egui::Event, structural: bool) -> Option<EditKey> {
    let egui::Event::Key {
        key,
        modifiers,
        pressed: true,
        ..
    } = event
    else {
        return None;
    };
    if modifiers.matches_logically(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)
        && *key == egui::Key::Z
    {
        Some(EditKey::History(true))
    } else if modifiers.matches_logically(egui::Modifiers::COMMAND)
        && matches!(key, egui::Key::Z | egui::Key::Y)
    {
        Some(EditKey::History(*key == egui::Key::Y))
    } else if structural
        && modifiers.matches_logically(egui::Modifiers::COMMAND | egui::Modifiers::ALT)
    {
        [
            egui::Key::Enter,
            egui::Key::Backspace,
            egui::Key::ArrowRight,
            egui::Key::ArrowLeft,
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
        ]
        .iter()
        .position(|k| k == key)
        .map(EditKey::Table)
    } else if structural && modifiers.matches_logically(egui::Modifiers::ALT) {
        [
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
            egui::Key::ArrowRight,
            egui::Key::ArrowLeft,
        ]
        .iter()
        .position(|k| k == key)
        .map(EditKey::Block)
    } else {
        None
    }
}

struct EditStream {
    commands: Vec<(Vec<egui::Event>, EditKey)>,
    trailing: Vec<egui::Event>,
}
fn take_edit_stream(
    input: &mut egui::InputState,
    structural: bool,
    cell_navigation: bool,
) -> Option<EditStream> {
    if !input
        .events
        .iter()
        .any(|e| classify_edit_key(e, structural).is_some() || (cell_navigation && matches!(e,egui::Event::Key {key:egui::Key::Tab|egui::Key::Enter,pressed:true,modifiers,..} if !modifiers.alt && !modifiers.ctrl && !modifiers.command)))
    {
        return None;
    }
    let mut commands = Vec::new();
    let mut pending = Vec::new();
    let mut other = Vec::new();
    for event in std::mem::take(&mut input.events) {
        if let Some(command) = classify_edit_key(&event, structural) {
            commands.push((std::mem::take(&mut pending), command));
        } else if matches!(
            event,
            egui::Event::Text(_)
                | egui::Event::Paste(_)
                | egui::Event::Cut
                | egui::Event::Copy
                | egui::Event::Key { .. }
                | egui::Event::Ime(_)
        ) {
            pending.push(event);
        } else {
            other.push(event);
        }
    }
    input.events = other;
    Some(EditStream {
        commands,
        trailing: pending,
    })
}

#[derive(Clone, Debug)]
struct ViewBookmark {
    stamp: u64,
    cell: Option<(usize, usize, usize)>,
    cell_cursor: Option<CCursorRange>,
    cell_owner: bool,
    source_owner: bool,
    block: Option<(usize, &'static str)>,
}
fn state_stamp(text: &str, cursor: CCursorRange) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hash);
    cursor.primary.index.hash(&mut hash);
    cursor.secondary.index.hash(&mut hash);
    hash.finish()
}

pub(crate) fn bounded_source_state(bytes: usize) -> egui::text_edit::TextEditState {
    let mut state = egui::text_edit::TextEditState::default();
    state.set_undoer(egui::util::undoer::Undoer::with_settings(
        egui::util::undoer::Settings {
            max_undos: (32 * 1024 * 1024 / bytes.max(1)).clamp(2, 100),
            ..Default::default()
        },
    ));
    state
}

pub(crate) fn commit_change(
    state: &mut egui::text_edit::TextEditState,
    text: &mut String,
    change: structure::Change,
    now: f64,
) {
    let before = state.cursor.char_range().unwrap_or_default();
    let mut undoer = state.undoer();
    undoer.add_undo(&(before, text.clone()));
    *text = change.text;
    let cursor = CCursorRange::two(
        CCursor::new(change.selection[1]),
        CCursor::new(change.selection[0]),
    );
    state.cursor.set_char_range(Some(cursor));
    // add_undo alone does NOT discard an abandoned redo branch in egui 0.33.
    undoer.feed_state(now, &(cursor, text.clone()));
    undoer.add_undo(&(cursor, text.clone()));
    state.set_undoer(undoer);
}

#[derive(Default)]
pub struct StructureUi {
    recovery_revision: u64,
    pub open: bool,
    pub dirty: bool,
    generation: Option<u64>,
    pub index: Index,
    table: usize,
    cell: Option<(usize, usize)>,
    focus_cell: Option<(usize, usize)>,
    block: Option<usize>,
    blocks_tab: bool,
    message: Option<&'static str>,
    pub draft: Option<(egui::Id, String)>,
    pub cell_focus_owner: bool,
    pub measure_next: bool,
    pub frame_samples_ms: Vec<f64>,
    pub frame_command_ranges: Vec<[usize; 2]>,
    command_samples_ms: Vec<f64>,
    last_bytes: usize,
    bookmarks: Vec<ViewBookmark>,
    bookmark_position: Option<usize>,
    pending_cell_before: Option<CCursorRange>,
    navigation_target: Option<(usize, usize)>,
    pub source_rect: Option<egui::Rect>,
}

impl StructureUi {
    pub(crate) fn recovery_stamp(&self) -> u64 {
        crate::rich_nodes::hash((
            self.recovery_revision,
            self.draft.as_ref().map(|(id, value)| (id, value.len())),
        ))
    }
    pub(crate) fn recovery_drafts(&self) -> Vec<crate::recovery::Draft> {
        self.draft
            .as_ref()
            .map(|(_, text)| crate::recovery::Draft {
                kind: "table / structure".into(),
                text: text.clone(),
                original: String::new(),
                range: [0, 0],
                expected: self.generation.unwrap_or_default(),
            })
            .into_iter()
            .collect()
    }
    pub fn pending_edits(&self) -> bool {
        self.draft.is_some()
    }
    pub fn command_count(&self) -> usize {
        self.command_samples_ms.len()
    }
    pub fn ensure(&mut self, text: &str, generation: u64) {
        if self.dirty || self.generation != Some(generation) {
            if self.generation != Some(generation) {
                self.cell = None;
                self.focus_cell = None;
                self.block = None;
                self.table = 0;
                self.draft = None;
                self.cell_focus_owner = false;
                self.bookmarks.clear();
                self.bookmark_position = None;
            }
            self.index = Index::parse(text);
            self.last_bytes = text.len();
            self.generation = Some(generation);
            self.dirty = false;
            if let Some(table) = self
                .index
                .tables
                .get(self.table.min(self.index.tables.len().saturating_sub(1)))
            {
                if let Some((r, c)) = self.cell {
                    let last = table.rows.len().saturating_sub(1);
                    let r = r.min(last);
                    let r = if r == 1 { 0 } else { r };
                    let c = c.min(
                        table.rows[r]
                            .cells
                            .len()
                            .max(table.columns)
                            .saturating_sub(1),
                    );
                    if self.cell != Some((r, c)) {
                        self.focus_cell = Some((r, c));
                    }
                    self.cell = Some((r, c));
                }
            } else {
                self.cell = None;
                self.focus_cell = None;
                self.cell_focus_owner = false;
            }
            self.table = self.table.min(self.index.tables.len().saturating_sub(1));
        }
    }

    fn record_bookmark(&mut self, bookmark: ViewBookmark) {
        if let Some(current) = self.bookmark_position {
            if self.bookmarks[current].stamp == bookmark.stamp {
                self.bookmarks[current] = bookmark;
                return;
            }
            self.bookmarks.truncate(current + 1);
        }
        self.bookmarks.push(bookmark);
        if self.bookmarks.len() > 256 {
            self.bookmarks.remove(0);
        }
        self.bookmark_position = self.bookmarks.len().checked_sub(1);
    }

    fn history_bookmark(
        &mut self,
        stamp: u64,
        current_stamp: u64,
        redo: bool,
    ) -> Option<ViewBookmark> {
        let current = self.bookmark_position?;
        let found = if redo {
            (current + 1..self.bookmarks.len()).find(|i| self.bookmarks[*i].stamp == stamp)
        } else {
            (0..current + usize::from(stamp != current_stamp))
                .rev()
                .find(|i| self.bookmarks[*i].stamp == stamp)
        }?;
        self.bookmark_position = Some(found);
        Some(self.bookmarks[found].clone())
    }
}

impl Editor {
    pub fn apply_rich_source(&mut self, ctx: &egui::Context, next: String) {
        if next == self.doc.text {
            return;
        }
        self.prepare_source_history(ctx);
        let mut state = self.source_state(ctx);
        let cursor = state.cursor.char_range().unwrap_or_default();
        commit_change(
            &mut state,
            &mut self.doc.text,
            structure::Change {
                text: next,
                selection: [cursor.secondary.index, cursor.primary.index],
                block_anchor: None,
            },
            ctx.input(|i| i.time),
        );
        state.store(ctx, self.source_id());
        self.doc.mark_text_changed();
        self.structure.dirty = true;
        self.structure.draft = None;
        self.status = "Unsaved changes";
        self.global.buffer_changed();
        ctx.request_repaint();
    }
    pub fn restore_edit_focus(&mut self) {
        if self.structure.open && self.structure.cell_focus_owner {
            self.structure.focus_cell = self.structure.cell;
            self.editor_focus = false;
        } else {
            self.editor_focus = true;
        }
    }
    fn source_id(&self) -> egui::Id {
        egui::Id::new(("source-editor", self.document_generation))
    }

    fn source_state(&self, ctx: &egui::Context) -> egui::text_edit::TextEditState {
        egui::text_edit::TextEditState::load(ctx, self.source_id())
            .unwrap_or_else(|| bounded_source_state(self.doc.text.len()))
    }

    pub fn prepare_source_history(&self, ctx: &egui::Context) {
        if egui::text_edit::TextEditState::load(ctx, self.source_id()).is_none() {
            self.source_state(ctx).store(ctx, self.source_id());
        }
    }

    fn view_bookmark(&self, ctx: &egui::Context) -> ViewBookmark {
        let cell = self
            .structure
            .cell
            .map(|(r, c)| (self.structure.table, r, c));
        let cell_cursor = self
            .structure
            .cell
            .and_then(|(r, c)| egui::text_edit::TextEditState::load(ctx, self.cell_id(r, c)))
            .and_then(|s| s.cursor.char_range());
        ViewBookmark {
            stamp: state_stamp(&self.doc.text, self.source_selection(ctx)),
            cell,
            cell_cursor,
            cell_owner: self.structure.cell_focus_owner,
            source_owner: ctx.memory(|m| m.has_focus(self.source_id())),
            block: self
                .structure
                .block
                .and_then(|i| self.structure.index.blocks.get(i))
                .map(|b| (b.range.start, b.kind)),
        }
    }

    fn restore_view_bookmark(&mut self, ctx: &egui::Context, bookmark: ViewBookmark) {
        self.structure
            .ensure(&self.doc.text, self.document_generation);
        self.structure.block = bookmark.block.and_then(|(anchor, kind)| {
            self.structure
                .index
                .blocks
                .iter()
                .position(|b| b.range.start == anchor && b.kind == kind)
        });
        self.structure.cell_focus_owner = bookmark.cell_owner;
        self.structure.focus_cell = None;
        self.structure.cell = None;
        if bookmark.source_owner {
            self.editor_focus = true;
            ctx.memory_mut(|m| m.request_focus(self.source_id()));
        } else if !bookmark.cell_owner {
            if let Some(block) = self.structure.block {
                ctx.memory_mut(|m| {
                    m.request_focus(egui::Id::new((
                        "structure-block",
                        self.document_generation,
                        block,
                    )))
                });
            } else if self.mode != crate::Mode::Read {
                self.editor_focus = true;
            }
        }
        if let Some((t, r, c)) = bookmark.cell {
            self.structure.table = t.min(self.structure.index.tables.len().saturating_sub(1));
            self.structure.cell = Some((r, c));
            if bookmark.cell_owner {
                self.structure.focus_cell = Some((r, c));
                let id = self.cell_id(r, c);
                let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
                state.cursor.set_char_range(bookmark.cell_cursor);
                state.store(ctx, id);
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
    }

    pub(crate) fn source_selection(&self, ctx: &egui::Context) -> CCursorRange {
        self.source_state(ctx)
            .cursor
            .char_range()
            .unwrap_or_default()
    }

    fn execute_structure(&mut self, ctx: &egui::Context, command: Command, focus_source: bool) {
        let measured = std::env::var_os("MDCARD_STRUCTURE_REPORT").is_some();
        let started = std::time::Instant::now();
        self.structure
            .ensure(&self.doc.text, self.document_generation);
        let mut state = self.source_state(ctx);
        let cursor = state.cursor.char_range().unwrap_or_default();
        let mut before_bookmark = self.view_bookmark(ctx);
        if let Some(cursor) = self.structure.pending_cell_before.take() {
            before_bookmark.cell_cursor = Some(cursor);
        }
        let active = self.structure.cell.unwrap_or((0, 0));
        let applied_draft = match &command {
            Command::Cell {
                table,
                row,
                column,
                value,
            } => {
                let id = egui::Id::new((
                    "structure-cell",
                    self.document_generation,
                    *table,
                    *row,
                    *column,
                ));
                self.structure
                    .draft
                    .as_ref()
                    .is_some_and(|(draft_id, text)| *draft_id == id && text == value)
                    .then_some(id)
            }
            _ => None,
        };
        let cell_target = if focus_source {
            None
        } else {
            match &command {
                Command::AddRow { after, .. } => Some((after.max(&1) + 1, active.1)),
                Command::DeleteRow { row, .. } => Some((*row, active.1)),
                Command::AddColumn { column, .. }
                | Command::DeleteColumn { column, .. }
                | Command::Align { column, .. } => Some((active.0, *column)),
                _ => None,
            }
        };
        #[cfg(feature = "acceptance-faults")]
        let inject_failure = matches!(&command, Command::Cell { .. })
            && std::env::var_os("MDCARD_ACCEPTANCE_TABLE_PATCH_FAILURE").is_some();
        #[cfg(not(feature = "acceptance-faults"))]
        let inject_failure = false;
        let applied = if inject_failure {
            Err("The source changed. Reopen this node.")
        } else {
            self.structure.index.apply(
                &self.doc.text,
                [cursor.primary.index, cursor.secondary.index],
                command,
            )
        };
        match applied {
            Ok(change) if change.text != self.doc.text => {
                if applied_draft.is_some() {
                    self.structure.draft = None;
                }
                self.structure.record_bookmark(before_bookmark);
                let block_anchor = change.block_anchor;
                // Explicit checkpoints bound the entire operation, even during continuous typing.
                commit_change(
                    &mut state,
                    &mut self.doc.text,
                    change,
                    ctx.input(|i| i.time),
                );
                state.store(ctx, self.source_id());
                self.doc.mark_text_changed();
                self.structure.dirty = true;
                if let Some(cell) = cell_target {
                    let cell = self.structure.navigation_target.take().unwrap_or(cell);
                    self.structure.cell = Some(cell);
                    self.structure.focus_cell = Some(cell);
                    self.structure.draft = None;
                    self.structure.cell_focus_owner = true;
                }
                self.structure
                    .ensure(&self.doc.text, self.document_generation);
                if let Some((anchor, kind)) = block_anchor {
                    self.structure.block = self
                        .structure
                        .index
                        .blocks
                        .iter()
                        .position(|b| b.range.start == anchor && b.kind == kind);
                }
                let after_bookmark = self.view_bookmark(ctx);
                self.structure.record_bookmark(after_bookmark);
                self.structure.message = None;
                self.status = "Unsaved changes";
                if focus_source {
                    self.editor_focus = true;
                }
                if measured {
                    self.structure
                        .command_samples_ms
                        .push(started.elapsed().as_secs_f64() * 1000.0);
                    self.structure.measure_next = true;
                }
                ctx.request_repaint();
            }
            Ok(_) => {
                self.structure.draft = None;
            }
            Err(message) => {
                self.structure.message = Some(message);
                self.status = message;
            }
        }
    }

    pub fn structure_shortcuts(&mut self, ctx: &egui::Context) {
        // The second pane owns a separate native TextEdit history. Never steal
        // its history keys for the authoritative main-document undo stream.
        if self.global.secondary_focused(ctx) {
            return;
        }
        if self.settings_open
            || self.encoding_open
            || self.search_open
            || self.pending.is_some()
            || self.error.is_some()
        {
            return;
        }
        let markdown = crate::text_files::markdown(self.doc.path.as_deref());
        if markdown
            && ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::T,
                )
            })
        {
            self.structure.open = !self.structure.open;
        }
        // Preserve each key event, including repeats coalesced into one frame.
        // consume_key returns a bool and would discard the count of repeated commands.
        let source_focus = ctx.memory(|m| m.has_focus(self.source_id()));
        let cell_focus = self.structure.open
            && self
                .structure
                .cell
                .is_some_and(|(r, c)| ctx.memory(|m| m.has_focus(self.cell_id(r, c))));
        if self.export.open && !source_focus && !cell_focus {
            return;
        }
        let structural_enabled = markdown && (source_focus || cell_focus || self.structure.open);
        let Some(stream) =
            ctx.input_mut(|input| take_edit_stream(input, structural_enabled, cell_focus))
        else {
            return;
        };
        for (input, action) in stream.commands {
            self.replay_edit_input(ctx, input);
            let source_focus = ctx.memory(|m| m.has_focus(self.source_id()));
            let cell_focus = self.structure.open
                && self
                    .structure
                    .cell
                    .is_some_and(|(r, c)| ctx.memory(|m| m.has_focus(self.cell_id(r, c))));
            if let EditKey::History(redo) = action {
                self.restore_history(ctx, redo);
                continue;
            }
            self.structure
                .ensure(&self.doc.text, self.document_generation);
            let cursor = self.source_selection(ctx);
            let byte = structure::byte_at(&self.doc.text, cursor.primary.index);
            let command = match action {
                EditKey::Table(op) => {
                    let target = if cell_focus
                        || (self.structure.open && !self.structure.blocks_tab && !source_focus)
                    {
                        self.structure
                            .cell
                            .map(|(r, c)| (self.structure.table, r, c))
                    } else {
                        self.structure.index.table_at(byte)
                    };
                    if let Some((table, row, column)) = target {
                        Some(match op {
                            0 => Command::AddRow { table, after: row },
                            1 => Command::DeleteRow { table, row },
                            2 => Command::AddColumn {
                                table,
                                column: column + 1,
                            },
                            3 => Command::DeleteColumn { table, column },
                            _ => Command::Align {
                                table,
                                column,
                                alignment: op - 4,
                            },
                        })
                    } else {
                        self.structure.message = Some("Choose a table first.");
                        self.status = "Choose a table first.";
                        None
                    }
                }
                EditKey::Block(op) => {
                    let block = if source_focus {
                        self.structure.index.block_at(byte)
                    } else {
                        self.structure.block
                    };
                    if let Some(block) = block {
                        if op < 2 {
                            self.structure
                                .index
                                .neighbor(block, op == 1)
                                .map(|target| Command::Move { block, target })
                        } else {
                            Some(Command::Indent {
                                block,
                                outdent: op == 3,
                            })
                        }
                    } else {
                        self.structure.message = Some("Choose a block first.");
                        self.status = "Choose a block first.";
                        None
                    }
                }
                EditKey::History(_) => unreachable!(),
            };
            if let Some(command) = command {
                self.execute_structure(ctx, command, source_focus);
            }
        }
        self.replay_edit_input(ctx, stream.trailing);
    }

    fn restore_history(&mut self, ctx: &egui::Context, redo: bool) {
        if redo {
            self.rich.counters.redo_calls += 1;
        } else {
            self.rich.counters.undo_calls += 1;
        }
        let mut state = self.source_state(ctx);
        let current = (
            state.cursor.char_range().unwrap_or_default(),
            self.doc.text.clone(),
        );
        let mut undoer = state.undoer();
        // Focus changes can checkpoint a different selection with identical source.
        // One user undo/redo must reach the adjacent source transaction.
        let mut candidate = current.clone();
        let result = loop {
            let next = if redo {
                undoer.redo(&candidate)
            } else {
                undoer.undo(&candidate)
            }
            .cloned();
            match next {
                Some(next) if next.1 == current.1 => candidate = next,
                other => break other,
            }
        };
        if let Some((cursor, text)) = result {
            self.rich.counters.history_changes += 1;
            let current_stamp = state_stamp(&current.1, current.0);
            self.doc.text = text;
            self.doc.mark_text_changed();
            state.cursor.set_char_range(Some(cursor));
            self.structure.dirty = true;
            self.structure.draft = None;
            if let Some(bookmark) = self.structure.history_bookmark(
                state_stamp(&self.doc.text, cursor),
                current_stamp,
                redo,
            ) {
                self.restore_view_bookmark(ctx, bookmark);
            }
            self.status = "Unsaved changes";
        }
        state.set_undoer(undoer);
        state.store(ctx, self.source_id());
    }

    /// On frames mixing typing with commands, let the real egui TextEdit process
    /// each preceding input segment first. Reusing its state preserves editing,
    /// navigation, selection, clipboard and undo semantics without a second editor.
    fn replay_edit_input(&mut self, ctx: &egui::Context, events: Vec<egui::Event>) {
        if events.is_empty() {
            return;
        }
        let source = ctx.memory(|m| m.has_focus(self.source_id()));
        let cell = !source
            && self.structure.open
            && !self.structure.blocks_tab
            && (self.structure.cell_focus_owner
                || self
                    .structure
                    .cell
                    .is_some_and(|(r, c)| ctx.memory(|m| m.has_focus(self.cell_id(r, c)))))
            && self.structure.cell.is_some();
        if !source && !cell {
            ctx.input_mut(|i| i.events.extend(events));
            return;
        }
        if cell {
            self.structure.cell_focus_owner = true;
            let mut pending = Vec::new();
            for event in events {
                let navigation = match &event {
                    egui::Event::Key {
                        key: egui::Key::Tab | egui::Key::Enter,
                        pressed: true,
                        modifiers,
                        ..
                    } if !modifiers.alt && !modifiers.command && !modifiers.ctrl => {
                        Some(modifiers.shift)
                    }
                    _ => None,
                };
                if let Some(backward) = navigation {
                    self.replay_text_widget(ctx, std::mem::take(&mut pending), false);
                    self.navigate_cell(ctx, backward);
                } else {
                    pending.push(event);
                }
            }
            self.replay_text_widget(ctx, pending, false);
        } else {
            self.replay_text_widget(ctx, events, true);
        }
    }

    fn replay_text_widget(&mut self, ctx: &egui::Context, events: Vec<egui::Event>, source: bool) {
        if events.is_empty() {
            return;
        }
        let saved_events = ctx.input_mut(|i| std::mem::replace(&mut i.events, events));
        let replay_id = egui::Id::new(("structure-input-replay", self.document_generation));
        let original_id = if source {
            self.source_id()
        } else {
            let (r, c) = self.structure.cell.unwrap();
            self.cell_id(r, c)
        };
        let state = if source {
            self.source_state(ctx)
        } else {
            egui::text_edit::TextEditState::load(ctx, original_id).unwrap_or_default()
        };
        let before = state.cursor.char_range();
        state.store(ctx, replay_id);
        ctx.memory_mut(|m| {
            m.move_focus(egui::FocusDirection::None);
            m.request_focus(replay_id);
        });
        let width = if source {
            self.structure.source_rect.map_or(600.0, |r| r.width())
        } else {
            150.0
        };
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("structural-input-context"),
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 100000.0),
            )),
        );
        ui.set_opacity(0.0);
        if source {
            let mut cache = crate::source_layout::SourceLayout::default();
            let wrap = self
                .preferences
                .source_wrap
                .unwrap_or(self.doc.text.len() <= 256 * 1024);
            let font = egui::FontId::monospace(self.font_size);
            let color = ui.visuals().text_color();
            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                cache.layout(
                    ui,
                    text.as_str(),
                    font.clone(),
                    color,
                    if wrap { width } else { f32::INFINITY },
                )
            };
            let output = egui::TextEdit::multiline(&mut self.doc.text)
                .id(replay_id)
                .layouter(&mut layouter)
                .font(egui::FontId::monospace(self.font_size))
                .desired_width(width)
                .frame(false)
                .lock_focus(true)
                .show(&mut ui);
            output.state.store(ctx, original_id);
            if output.response.changed() {
                self.doc.mark_text_changed();
                self.structure.dirty = true;
                self.structure.draft = None;
                self.status = "Unsaved changes";
            }
            self.structure.cell_focus_owner = false;
        } else {
            self.structure
                .ensure(&self.doc.text, self.document_generation);
            let (r, c) = self.structure.cell.unwrap();
            let t = self.structure.table;
            let mut value = self.structure.index.tables[t].rows[r]
                .cells
                .get(c)
                .map_or("", |span| self.doc.text[span.clone()].trim())
                .to_owned();
            if let Some((id, draft)) = &self.structure.draft
                && *id == original_id
            {
                value.clone_from(draft);
            }
            let output = egui::TextEdit::singleline(&mut value)
                .lock_focus(true)
                .id(replay_id)
                .desired_width(width)
                .show(&mut ui);
            output.state.store(ctx, original_id);
            if output.response.changed() {
                self.structure.pending_cell_before = before;
                self.structure.draft = Some((original_id, value.clone()));
                self.structure.recovery_revision = self.structure.recovery_revision.wrapping_add(1);
                ctx.memory_mut(|m| m.request_focus(original_id));
                self.execute_structure(
                    ctx,
                    Command::Cell {
                        table: t,
                        row: r,
                        column: c,
                        value,
                    },
                    false,
                );
            }
        }
        ctx.data_mut(|d| d.remove::<egui::text_edit::TextEditState>(replay_id));
        ctx.memory_mut(|m| m.request_focus(original_id));
        ctx.input_mut(|i| i.events = saved_events);
    }

    fn navigate_cell(&mut self, ctx: &egui::Context, backward: bool) {
        self.structure
            .ensure(&self.doc.text, self.document_generation);
        let Some((r, c)) = self.structure.cell else {
            return;
        };
        let t = self.structure.table;
        let table = &self.structure.index.tables[t];
        let (destination, append) = cell_destination(table, r, c, backward);
        if append {
            self.structure.navigation_target = Some(destination);
            self.execute_structure(ctx, Command::AddRow { table: t, after: r }, false);
        } else {
            self.structure.cell = Some(destination);
        }
        self.structure.focus_cell = Some(destination);
        ctx.memory_mut(|m| {
            m.move_focus(egui::FocusDirection::None);
            m.request_focus(self.cell_id(destination.0, destination.1));
        });
    }

    fn cell_id(&self, row: usize, column: usize) -> egui::Id {
        egui::Id::new((
            "structure-cell",
            self.document_generation,
            self.structure.table,
            row,
            column,
        ))
    }

    pub fn structure_window(&mut self, ctx: &egui::Context) {
        if !self.structure.open || !crate::text_files::markdown(self.doc.path.as_deref()) {
            return;
        }
        if self.pending.is_some()
            || self.settings_open
            || self.encoding_open
            || self.search_open
            || self.error.is_some()
        {
            return;
        }
        self.structure
            .ensure(&self.doc.text, self.document_generation);
        let language = self.language;
        let mut open = true;
        let mut command = None;
        let available = ctx.content_rect().size() - egui::vec2(24.0, 36.0);
        let response = egui::Window::new(crate::theme::window_title(language.text("Structure")))
            .id(egui::Id::new("structure-window"))
            .open(&mut open)
            .default_size(egui::vec2(710.0, 430.0).min(available))
            .min_size(egui::vec2(280.0, 200.0))
            .max_size(available)
            .constrain_to(ctx.content_rect().shrink(8.0))
            .vscroll(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.structure.blocks_tab,
                        false,
                        language.text("Tables"),
                    );
                    ui.selectable_value(
                        &mut self.structure.blocks_tab,
                        true,
                        language.text("Blocks"),
                    );
                    if crate::theme::tool_button(ui, language.text("Insert table")).clicked() {
                        command = Some(Command::InsertTable {
                            at: structure::byte_at(
                                &self.doc.text,
                                self.source_selection(ctx).primary.index,
                            ),
                        });
                    }
                    if crate::theme::tool_button(ui, language.text("Back to source")).clicked() {
                        self.editor_focus = true;
                        if self.mode == crate::Mode::Read {
                            self.mode = crate::Mode::Edit;
                        }
                        self.structure.open = false;
                    }
                });
                ui.label(
                    egui::RichText::new(language.text("Structure shortcuts"))
                        .small()
                        .color(self.palette.muted),
                );
                if let Some(message) = self.structure.message {
                    ui.colored_label(self.palette.accent, language.text(message));
                }
                ui.separator();
                if self.structure.blocks_tab {
                    self.blocks_ui(ui, &mut command);
                } else {
                    self.tables_ui(ui, &mut command);
                }
            });
        if response
            .as_ref()
            .is_some_and(|r| crate::theme::window_escape(ctx, r.response.layer_id))
        {
            open = false;
        }
        self.structure.open &= open;
        if let Some(command) = command {
            self.execute_structure(ctx, command, false);
        }
    }

    fn tables_ui(&mut self, ui: &mut egui::Ui, command: &mut Option<Command>) {
        let language = self.language;
        if self.structure.index.tables.is_empty() {
            ui.label(
                language.text(
                    "No editable table. Place the source cursor between blocks and insert one.",
                ),
            );
            return;
        }
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("structure-tables")
                .selected_text(format!(
                    "{} {}",
                    language.text("Table"),
                    self.structure.table + 1
                ))
                .show_ui(ui, |ui| {
                    for t in 0..self.structure.index.tables.len() {
                        if ui
                            .selectable_value(
                                &mut self.structure.table,
                                t,
                                format!("{} {}", language.text("Table"), t + 1),
                            )
                            .changed()
                        {
                            self.structure.cell = None;
                        }
                    }
                });
            if crate::theme::tool_button(ui, language.text("Delete table")).clicked() {
                *command = Some(Command::DeleteTable {
                    table: self.structure.table,
                });
            }
        });
        let t = self.structure.table;
        let table = &self.structure.index.tables[t];
        let width = table
            .rows
            .iter()
            .map(|r| r.cells.len())
            .max()
            .unwrap_or(table.columns)
            .max(table.columns);
        ui.horizontal_wrapped(|ui| {
            let (row, column) = self.structure.cell.unwrap_or((0, 0));
            for (label, c) in table_commands(t, row, column) {
                if crate::theme::tool_button(ui, language.text(label)).clicked() {
                    *command = Some(c);
                }
            }
        });
        ui.label(egui::RichText::new(language.text("Cell navigation hint")).small());
        let row_height = ui.spacing().interact_size.y;
        ui.spacing_mut().item_spacing.y = 6.0;
        let mut scroll = egui::ScrollArea::both()
            .id_salt("table-grid-scroll")
            .auto_shrink([false; 2]);
        if let Some((r, _)) = self.structure.focus_cell {
            scroll = scroll.vertical_scroll_offset(
                r.saturating_sub(1) as f32 * (row_height + ui.spacing().item_spacing.y),
            );
        }
        scroll.show_rows(ui, row_height, table.rows.len(), |ui, visible| {
            egui::Grid::new(("table-grid", t, self.document_generation))
                .striped(true)
                .min_col_width(32.0)
                .spacing(egui::vec2(6.0, 6.0))
                .show(ui, |ui| {
                    if visible.contains(&0) {
                        ui.label("");
                        for c in 0..width {
                            let response = ui
                                .button(format!(": {}", c + 1))
                                .on_hover_text(language.text("Column actions"));
                            if response.clicked() || response.secondary_clicked() {
                                self.structure.cell =
                                    Some((self.structure.cell.map_or(0, |v| v.0), c));
                                if response.clicked() {
                                    self.structure.focus_cell = self.structure.cell;
                                }
                            }
                            response.context_menu(|ui| {
                                for (label, action) in [
                                    (
                                        "Insert column before",
                                        Command::AddColumn {
                                            table: t,
                                            column: c.min(table.columns),
                                        },
                                    ),
                                    (
                                        "Add column",
                                        Command::AddColumn {
                                            table: t,
                                            column: (c + 1).min(table.columns),
                                        },
                                    ),
                                    (
                                        "Delete column",
                                        Command::DeleteColumn {
                                            table: t,
                                            column: c,
                                        },
                                    ),
                                    (
                                        "Align left",
                                        Command::Align {
                                            table: t,
                                            column: c,
                                            alignment: 0,
                                        },
                                    ),
                                    (
                                        "Align center",
                                        Command::Align {
                                            table: t,
                                            column: c,
                                            alignment: 1,
                                        },
                                    ),
                                    (
                                        "Align right",
                                        Command::Align {
                                            table: t,
                                            column: c,
                                            alignment: 2,
                                        },
                                    ),
                                ] {
                                    if ui.button(language.text(label)).clicked() {
                                        *command = Some(action);
                                        ui.close();
                                    }
                                }
                            });
                        }
                        ui.end_row();
                    }
                    for (r, row) in
                        table.rows.iter().enumerate().filter(|(r, _)| {
                            *r != 1 && visible.contains(&if *r == 0 { 1 } else { *r })
                        })
                    {
                        let handle = ui
                            .button(if r == 0 {
                                ": H".into()
                            } else {
                                format!(": {}", r - 1)
                            })
                            .on_hover_text(language.text("Row actions"));
                        if handle.clicked() || handle.secondary_clicked() {
                            self.structure.cell = Some((r, self.structure.cell.map_or(0, |v| v.1)));
                            if handle.clicked() {
                                self.structure.focus_cell = self.structure.cell;
                            }
                        }
                        handle.context_menu(|ui| {
                            for (label, action) in [
                                (
                                    "Insert row before",
                                    Command::AddRow {
                                        table: t,
                                        after: r.saturating_sub(1).max(1),
                                    },
                                ),
                                ("Add row", Command::AddRow { table: t, after: r }),
                                ("Delete row", Command::DeleteRow { table: t, row: r }),
                            ] {
                                if ui.button(language.text(label)).clicked() {
                                    *command = Some(action);
                                    ui.close();
                                }
                            }
                        });
                        for c in 0..width {
                            let mut value = row
                                .cells
                                .get(c)
                                .map_or("", |span| self.doc.text[span.clone()].trim())
                                .to_owned();
                            let id = egui::Id::new((
                                "structure-cell",
                                self.document_generation,
                                t,
                                r,
                                c,
                            ));
                            if let Some((draft_id, draft)) = &self.structure.draft
                                && *draft_id == id
                            {
                                value.clone_from(draft);
                            }
                            let mut nav = None;
                            if ui.memory(|m| m.has_focus(id)) {
                                let backward = ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
                                });
                                let forward = ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                                });
                                let enter = ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                                });
                                if backward || forward || enter {
                                    let (destination, append) =
                                        cell_destination(table, r, c, backward);
                                    nav = Some(destination);
                                    if append {
                                        *command = Some(Command::AddRow { table: t, after: r });
                                    }
                                }
                            }
                            let cell_before = egui::text_edit::TextEditState::load(ui.ctx(), id)
                                .and_then(|s| s.cursor.char_range());
                            let response = ui.add_sized(
                                [150.0, ui.spacing().interact_size.y],
                                egui::TextEdit::singleline(&mut value)
                                    .lock_focus(true)
                                    .id(id)
                                    .desired_width(150.0)
                                    .interactive(r != 0 || c < table.columns)
                                    .hint_text(if r == 0 && c >= table.columns {
                                        language.text("Add column")
                                    } else {
                                        ""
                                    })
                                    .clip_text(true),
                            );
                            crate::theme::record_control(
                                ui,
                                &response,
                                &format!("structure-cell-{t}-{r}-{c}"),
                            );
                            if self.structure.focus_cell == Some((r, c)) {
                                response.request_focus();
                                response.scroll_to_me(Some(egui::Align::Center));
                                self.structure.focus_cell = None;
                            }
                            if response.has_focus()
                                || response.clicked()
                                || response.secondary_clicked()
                            {
                                self.structure.cell = Some((r, c));
                                self.structure.cell_focus_owner = true;
                            }
                            if response.changed() {
                                self.structure.pending_cell_before = cell_before;
                                self.structure.draft = Some((id, value.clone()));
                                self.structure.recovery_revision =
                                    self.structure.recovery_revision.wrapping_add(1);
                                *command = Some(Command::Cell {
                                    table: t,
                                    row: r,
                                    column: c,
                                    value,
                                });
                            }
                            if let Some(destination) = nav {
                                self.structure.focus_cell = Some(destination);
                                if matches!(&*command, Some(Command::AddRow { .. })) {
                                    self.structure.navigation_target = Some(destination);
                                }
                                ui.ctx().request_repaint();
                            }
                            response.context_menu(|ui| {
                                for (label, c) in table_commands(t, r, c) {
                                    if ui.button(language.text(label)).clicked() {
                                        *command = Some(c);
                                        ui.close();
                                    }
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn blocks_ui(&mut self, ui: &mut egui::Ui, command: &mut Option<Command>) {
        self.structure.cell_focus_owner = false;
        let language = self.language;
        ui.horizontal_wrapped(|ui| {
            for (label, op) in [
                ("Move up", 0),
                ("Move down", 1),
                ("Indent", 2),
                ("Outdent", 3),
            ] {
                if crate::theme::tool_button(ui, language.text(label)).clicked()
                    && let Some(block) = self.structure.block
                {
                    if op < 2 {
                        if let Some(target) = self.structure.index.neighbor(block, op == 1) {
                            *command = Some(Command::Move { block, target });
                        }
                    } else {
                        *command = Some(Command::Indent {
                            block,
                            outdent: op == 3,
                        });
                    }
                }
            }
        });
        ui.label(
            egui::RichText::new(
                language.text("Drag sibling blocks to reorder. Children move with their parent."),
            )
            .small(),
        );
        egui::ScrollArea::vertical()
            .id_salt("structure-blocks-scroll")
            .auto_shrink([false; 2])
            .show_rows(
                ui,
                ui.spacing().interact_size.y + 10.0,
                self.structure.index.blocks.len(),
                |ui, visible| {
                    for i in visible {
                        let block = &self.structure.index.blocks[i];
                        let id = egui::Id::new(("structure-block", self.document_generation, i));
                        let preview = self.doc.text[block.range.clone()]
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim();
                        let preview: String = preview.chars().take(70).collect();
                        let frame = egui::Frame::new().inner_margin(5).corner_radius(4);
                        let (zone, dropped) = ui.dnd_drop_zone::<usize, _>(frame, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space((block.indent as f32 * 5.0).min(90.0));
                                ui.dnd_drag_source(id, i, |ui| {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(16.0, ui.spacing().interact_size.y),
                                        egui::Sense::hover(),
                                    );
                                    for x in [-3.0, 3.0] {
                                        for y in [-5.0, 0.0, 5.0] {
                                            ui.painter().circle_filled(
                                                rect.center() + egui::vec2(x, y),
                                                1.3,
                                                self.palette.muted,
                                            );
                                        }
                                    }
                                    response.on_hover_text(language.text("Drag to move block"));
                                });
                                if ui
                                    .selectable_label(
                                        self.structure.block == Some(i),
                                        format!("{} · {preview}", language.text(block.kind)),
                                    )
                                    .clicked()
                                {
                                    self.structure.block = Some(i);
                                }
                            });
                        });
                        if zone.response.contains_pointer()
                            && egui::DragAndDrop::has_payload_of_type::<usize>(ui.ctx())
                        {
                            ui.painter().hline(
                                zone.response.rect.x_range(),
                                zone.response.rect.bottom(),
                                egui::Stroke::new(2.0, self.palette.accent),
                            );
                        }
                        if let Some(from) = dropped {
                            *command = Some(Command::Move {
                                block: *from,
                                target: i,
                            });
                            self.structure.block = Some(*from);
                        }
                    }
                },
            );
    }
}

impl Drop for StructureUi {
    fn drop(&mut self) {
        if let Some(path) = std::env::var_os("MDCARD_STRUCTURE_REPORT") {
            let report = serde_json::json!({"source_bytes":self.last_bytes,"blocks":self.index.blocks.len(),"tables":self.index.tables.len(),"command_ms":self.command_samples_ms,"command_and_following_update_cpu_ms":self.frame_samples_ms,"update_command_ranges":self.frame_command_ranges,"measurement":"eframe update CPU time; excludes renderer/GPU presentation and OS input delivery"});
            if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
                let _ = std::fs::write(path, bytes);
            }
        }
    }
}

fn table_commands(t: usize, r: usize, c: usize) -> [(&'static str, Command); 7] {
    [
        ("Add row", Command::AddRow { table: t, after: r }),
        ("Delete row", Command::DeleteRow { table: t, row: r }),
        (
            "Add column",
            Command::AddColumn {
                table: t,
                column: c + 1,
            },
        ),
        (
            "Delete column",
            Command::DeleteColumn {
                table: t,
                column: c,
            },
        ),
        (
            "Align left",
            Command::Align {
                table: t,
                column: c,
                alignment: 0,
            },
        ),
        (
            "Align center",
            Command::Align {
                table: t,
                column: c,
                alignment: 1,
            },
        ),
        (
            "Align right",
            Command::Align {
                table: t,
                column: c,
                alignment: 2,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ragged_header_is_readonly_and_navigation_skips_virtual_header() {
        let source = "|a|b|\n|---|---|\n|x|y|extra|";
        let index = Index::parse(source);
        let table = &index.tables[0];
        assert!(
            index
                .apply(
                    source,
                    [0, 0],
                    Command::Cell {
                        table: 0,
                        row: 0,
                        column: 2,
                        value: "bad".into()
                    }
                )
                .is_err()
        );
        assert_eq!(cell_destination(table, 0, 1, false), ((2, 0), false));
        assert_eq!(cell_destination(table, 2, 0, true), ((0, 1), false));
        assert_eq!(cell_destination(table, 2, 2, false), ((3, 0), true));
        let change = index
            .apply(
                source,
                [0, 0],
                Command::Cell {
                    table: 0,
                    row: 2,
                    column: 2,
                    value: "kept".into(),
                },
            )
            .unwrap();
        assert_eq!(Index::parse(&change.text).tables[0].columns, 2);
        assert!(change.text.contains("kept"));
    }
    fn editor(text: &str) -> Editor {
        let preferences = crate::preferences::Preferences::default();
        let palette = crate::theme::Palette::new(false, 0);
        let now = std::time::Instant::now();
        Editor {
            virtual_input: None,
            virtual_input_gl: false,
            recovery: crate::recovery::Recovery::default(),
            saves: crate::autosave::SaveCoordinator::default(),
            save_then_action: false,
            ime_composing: false,
            diagnostic_reports: crate::diagnostic_sink::DiagnosticSinks::default(),
            reading: crate::reading_state::ReadingState::default(),
            rename_open: false,
            rename_name: String::new(),
            large: crate::large_document::LargeDocument::default(),
            last_secondary: false,
            export: crate::export_ui::ExportUi::default(),
            global: crate::search_ui::SearchUi::default(),
            outline_cache: vec![],
            outline_source: String::new(),
            outline_changed: None,
            rich: crate::rich_ui::RichUi::default(),
            structure: StructureUi::default(),
            native_corners: Default::default(),
            native_icons: Default::default(),
            language: crate::localization::Language::English,
            preferences: preferences.clone(),
            failed_preferences: None,
            saved_preferences: preferences,
            running_gpu: true,
            requested_gpu_at_start: true,
            automatic_compatibility: false,
            renderer: String::new(),
            palette,
            previous_palette: palette,
            target_palette: palette,
            theme_changed: now,
            theme_key: (false, 0),
            settings_error: None,
            settings_open: false,
            encoding_open: false,
            encoding_choice: crate::encoding::Encoding::Utf8,
            encoding_bom: false,
            encoding_target: None,
            failed_open: None,
            mode_transition: None,
            last_visible_mode: crate::Mode::Split,
            doc: crate::Document::from_text(text),
            cache: Default::default(),
            mode: crate::Mode::Split,
            sidebar: false,
            navigation: 0,
            pending: None,
            error: None,
            status: "Ready when you are",
            recent: Vec::new(),
            editor_focus: false,
            allow_close: false,
            font_size: 15.0,
            document_generation: 0,
            resize_origin: None,
            started: now,
            screenshot_requested: false,
            screenshot_path: None,
            auto_quit: None,
            startup_directory: std::path::PathBuf::from("/tmp"),
            files: crate::text_files::FileIndex::new(std::path::PathBuf::from("/tmp")),
            tree: Default::default(),
            search_open: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_revision: 0,
            search_focus: false,
            search_selected: 0,
        }
    }
    #[test]
    fn successful_cell_patch_clears_only_its_applied_draft_and_allows_save() {
        let ctx = egui::Context::default();
        let mut app = editor("| Name | Value |\n| --- | --- |\n| First | 10 |\n");
        app.structure.ensure(&app.doc.text, app.document_generation);
        let id = app.cell_id(2, 0);
        app.structure.cell = Some((2, 0));
        app.structure.draft = Some((id, "edited cell".into()));
        app.execute_structure(
            &ctx,
            Command::Cell {
                table: 0,
                row: 2,
                column: 0,
                value: "edited cell".into(),
            },
            false,
        );
        assert!(app.doc.text.contains("edited cell"));
        assert!(app.structure.draft.is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saved.md");
        app.start_save(false, path, false, &ctx);
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
        assert!(!app.structure.pending_edits());
    }
    #[test]
    fn explicit_cell_after_copying_cross_block_source_selection_edits_and_saves() {
        let ctx = egui::Context::default();
        let mut app = editor(
            "# Heading\n\n| Name | Value |\n| --- | --- |\n| First | 10 |\n\nOther paragraph.\n",
        );
        app.structure.ensure(&app.doc.text, app.document_generation);
        let mut state = app.source_state(&ctx);
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(app.doc.text.chars().count()),
            )));
        state.store(&ctx, app.source_id());
        let id = app.cell_id(2, 0);
        app.structure.draft = Some((id, "after source copy".into()));
        app.execute_structure(
            &ctx,
            Command::Cell {
                table: 0,
                row: 2,
                column: 0,
                value: "after source copy".into(),
            },
            false,
        );
        assert!(app.doc.text.contains("after source copy"));
        assert!(app.doc.text.starts_with("# Heading"));
        assert!(app.doc.text.ends_with("Other paragraph.\n"));
        assert!(!app.structure.pending_edits());
        let dir = tempfile::tempdir().unwrap();
        app.start_save(false, dir.path().join("saved.md"), false, &ctx);
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
    }
    #[test]
    fn rich_unicode_patch_shares_editor_history_and_rejects_abandoned_redo() {
        let original = "中文😀 $x$ and $x$\r\n";
        let mut editor = editor(original);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
        });
        let (_, nodes) = crate::rich_nodes::parse(original);
        let next = crate::rich_nodes::patch(
            original,
            crate::rich_nodes::hash(original),
            &nodes[1],
            "\\alpha",
        )
        .unwrap();
        editor.apply_rich_source(&ctx, next.clone());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
        });
        assert_eq!(editor.doc.text, next);
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, original);
        assert!(!editor.doc.dirty());
        undo_key(&mut editor, &ctx, true);
        assert_eq!(editor.doc.text, next);
        undo_key(&mut editor, &ctx, false);
        let divergent =
            crate::rich_nodes::patch(original, crate::rich_nodes::hash(original), &nodes[0], "z")
                .unwrap();
        editor.apply_rich_source(&ctx, divergent.clone());
        undo_key(&mut editor, &ctx, true);
        assert_eq!(editor.doc.text, divergent);
    }
    fn undo_key(editor: &mut Editor, ctx: &egui::Context, redo: bool) {
        let modifiers = if redo {
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::COMMAND
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ctx| editor.structure_shortcuts(ctx),
        );
    }
    #[test]
    fn secondary_large_source_has_bounded_layout_and_independent_global_history() {
        let original = "Secondary 中文 line\n".repeat(12000);
        let mut app = editor("# Primary stays intact\n");
        app.global.secondary = Some(crate::Document::from_text(&original));
        let ctx = egui::Context::default();
        let id = egui::Id::new(("second-editor", app.global.secondary_identity()));
        let frame = |app: &mut Editor, time, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100., 700.),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.structure_shortcuts(ctx);
                    app.secondary_panel(ctx);
                },
            );
        };
        let key = |key, modifiers| egui::Event::Key {
            key,
            modifiers,
            physical_key: None,
            pressed: true,
            repeat: false,
        };
        frame(&mut app, 0., vec![]);
        ctx.memory_mut(|m| m.request_focus(id));
        frame(
            &mut app,
            0.1,
            vec![
                key(egui::Key::End, egui::Modifiers::COMMAND),
                egui::Event::Paste("TAIL新增".into()),
            ],
        );
        assert_eq!(
            app.global.secondary.as_ref().unwrap().text,
            format!("{original}TAIL新增")
        );
        assert!(app.global.secondary_large.active);
        assert!(app.global.secondary_large.laid_out_lines < 200);
        assert!(app.global.secondary_large.visible_source.end >= original.chars().count());
        frame(
            &mut app,
            2.,
            vec![key(egui::Key::Z, egui::Modifiers::COMMAND)],
        );
        assert_eq!(app.global.secondary.as_ref().unwrap().text, original);
        frame(
            &mut app,
            3.,
            vec![key(
                egui::Key::Z,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            )],
        );
        assert_eq!(
            app.global.secondary.as_ref().unwrap().text,
            format!("{original}TAIL新增")
        );
        assert_eq!(app.doc.text, "# Primary stays intact\n");
        assert!(!app.doc.dirty());
        assert!(app.global.secondary.as_ref().unwrap().dirty());
    }

    #[test]
    fn secondary_native_undo_redo_keeps_main_history_and_export_focus_isolated() {
        let mut app = editor("# Primary stays intact\n");
        app.global.secondary = Some(crate::Document::from_text("# Secondary\n"));
        let ctx = egui::Context::default();
        let id = egui::Id::new(("second-editor", app.global.secondary_identity()));
        let frame = |app: &mut Editor, time: f64, events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.structure_shortcuts(ctx);
                    app.secondary_panel(ctx);
                },
            );
        };
        frame(&mut app, 0., vec![]);
        ctx.memory_mut(|m| m.request_focus(id));
        frame(
            &mut app,
            0.1,
            vec![
                egui::Event::Key {
                    key: egui::Key::End,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Paste("SECONDARY_DIRTY_INCLUDED".into()),
            ],
        );
        assert!(
            app.global
                .secondary
                .as_ref()
                .unwrap()
                .text
                .contains("SECONDARY_DIRTY_INCLUDED")
        );
        app.export
            .open_for(true, None, app.global.secondary_identity());
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("export-title-input")));
        frame(&mut app, 2.5, vec![]);
        ctx.memory_mut(|m| m.request_focus(id));
        frame(&mut app, 3., vec![]);
        let key = |redo| egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if redo {
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::COMMAND
            },
        };
        frame(&mut app, 4., vec![key(false)]);
        assert_eq!(app.global.secondary.as_ref().unwrap().text, "# Secondary\n");
        assert_eq!(app.doc.text, "# Primary stays intact\n");
        frame(&mut app, 5., vec![key(true)]);
        assert!(
            app.global
                .secondary
                .as_ref()
                .unwrap()
                .text
                .contains("SECONDARY_DIRTY_INCLUDED")
        );
        assert_eq!(app.doc.text, "# Primary stays intact\n");
    }
    #[test]
    fn real_editor_undo_redo_restores_block_identity_cell_coordinates_and_reverse_selection() {
        let ctx = egui::Context::default();
        let mut editor = editor("dup\n\nX\n\ndup\n\nY\n\n| a | b |\n| --- | --- |\n| x | y |\n");
        editor.structure.open = true;
        editor.structure.ensure(&editor.doc.text, 0);
        editor.prepare_source_history(&ctx);
        editor.structure.block = Some(0);
        editor.structure.blocks_tab = true;
        editor.execute_structure(
            &ctx,
            Command::Move {
                block: 0,
                target: 2,
            },
            false,
        );
        assert_eq!(
            editor.structure.index.blocks[editor.structure.block.unwrap()]
                .range
                .start,
            8
        );
        undo_key(&mut editor, &ctx, false);
        assert_eq!(
            editor.structure.index.blocks[editor.structure.block.unwrap()]
                .range
                .start,
            0
        );
        undo_key(&mut editor, &ctx, true);
        assert_eq!(
            editor.structure.index.blocks[editor.structure.block.unwrap()]
                .range
                .start,
            8
        );
        editor.structure.blocks_tab = false;
        editor.structure.block = None;
        editor.structure.cell = Some((2, 1));
        editor.structure.cell_focus_owner = true;
        let id = editor.cell_id(2, 1);
        let reverse = CCursorRange::two(CCursor::new(1), CCursor::new(0));
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(reverse));
        state.store(&ctx, id);
        editor.execute_structure(
            &ctx,
            Command::DeleteColumn {
                table: 0,
                column: 1,
            },
            false,
        );
        assert_eq!(editor.structure.cell, Some((2, 0)));
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.structure.cell, Some((2, 1)));
        assert_eq!(
            editor
                .source_state(&ctx)
                .cursor
                .char_range()
                .unwrap()
                .primary
                .index,
            editor.source_selection(&ctx).primary.index
        );
        assert_eq!(
            egui::text_edit::TextEditState::load(&ctx, id)
                .unwrap()
                .cursor
                .char_range(),
            Some(reverse)
        );
        assert_eq!(editor.structure.focus_cell, Some((2, 1)));
        undo_key(&mut editor, &ctx, true);
        assert_eq!(editor.structure.cell, Some((2, 0)));
        undo_key(&mut editor, &ctx, false);
        editor.execute_structure(&ctx, Command::AddRow { table: 0, after: 2 }, false);
        let branch = editor.doc.text.clone();
        undo_key(&mut editor, &ctx, true);
        assert_eq!(editor.doc.text, branch);
        // A remembered block selection must not steal focus from source shortcuts.
        editor.structure.cell_focus_owner = false;
        editor.structure.block = Some(0);
        editor.structure.blocks_tab = true;
        ctx.memory_mut(|m| m.request_focus(editor.source_id()));
        editor.execute_structure(
            &ctx,
            Command::Move {
                block: 0,
                target: 1,
            },
            true,
        );
        undo_key(&mut editor, &ctx, false);
        assert!(editor.editor_focus);
    }
    fn history(
        state: &mut egui::text_edit::TextEditState,
        text: &mut String,
        redo: bool,
    ) -> CCursorRange {
        let mut undoer = state.undoer();
        let current = (state.cursor.char_range().unwrap_or_default(), text.clone());
        let result = if redo {
            undoer.redo(&current)
        } else {
            undoer.undo(&current)
        }
        .cloned()
        .unwrap();
        *text = result.1;
        state.cursor.set_char_range(Some(result.0));
        state.set_undoer(undoer);
        result.0
    }
    #[test]
    fn source_typing_structure_typing_share_atomic_history_and_reject_old_redo() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("history-integration");
        let original = "| a | b |\n| --- | --- |\n| é🦀 | z |\n";
        let mut text = original.to_owned();
        let frame = |text: &mut String, events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.memory_mut(|m| m.request_focus(id));
                        egui::TextEdit::multiline(text).id(id).show(ui);
                    });
                },
            );
        };
        frame(&mut text, vec![]);
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        let at = structure::char_at(&text, text.find("é🦀").unwrap()) + 2;
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(at))));
        state.store(&ctx, id);
        frame(&mut text, vec![egui::Event::Text(" typed".into())]);
        let typed = text.clone();
        assert!(typed.contains("é🦀 typed"));
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        let reverse = CCursorRange::two(CCursor::new(at), CCursor::new(at - 2));
        state.cursor.set_char_range(Some(reverse));
        let change = Index::parse(&text)
            .apply(
                &text,
                [at - 2, at],
                Command::AddColumn {
                    table: 0,
                    column: 1,
                },
            )
            .unwrap();
        commit_change(&mut state, &mut text, change, 0.1);
        let structured = text.clone();
        let after = state.cursor.char_range().unwrap();
        assert_eq!(history(&mut state, &mut text, false), reverse);
        assert_eq!(text, typed);
        assert_eq!(history(&mut state, &mut text, true), after);
        assert_eq!(text, structured);
        state.store(&ctx, id);
        frame(&mut text, vec![egui::Event::Text("Z".into())]);
        assert_ne!(text, structured);
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        history(&mut state, &mut text, false);
        assert_eq!(text, structured);
        history(&mut state, &mut text, false);
        assert_eq!(text, typed);
        let change = Index::parse(&text)
            .apply(&text, [at - 2, at], Command::AddRow { table: 0, after: 2 })
            .unwrap();
        commit_change(&mut state, &mut text, change, 0.2);
        let new_branch = text.clone();
        let mut undoer = state.undoer();
        assert!(
            undoer
                .redo(&(state.cursor.char_range().unwrap(), text.clone()))
                .is_none()
        );
        assert_eq!(text, new_branch);
        // A new document ID has no access to the previous document history.
        assert!(
            egui::text_edit::TextEditState::load(&ctx, egui::Id::new("new-document")).is_none()
        );
    }
    #[test]
    fn cache_reuses_ast_and_benchmark_records_large_real_source_edits() {
        let mut text = String::from("| a | b |\r\n| --- | --- |\r\n| x | y |\r\n\r\n");
        text.push_str(
            &"Unicode 🦀 paragraph with **markdown** and [link](https://example.com).\r\n\r\n"
                .repeat(14000),
        );
        let bytes = text.len();
        let mut ui = StructureUi::default();
        ui.ensure(&text, 0);
        let blocks = ui.index.blocks.as_ptr();
        for _ in 0..100 {
            ui.ensure(&text, 0);
            assert_eq!(blocks, ui.index.blocks.as_ptr());
        }
        let started = std::time::Instant::now();
        let mut state = bounded_source_state(bytes);
        let mut timings = Vec::new();
        for n in 0..100 {
            let tick = std::time::Instant::now();
            let change = ui
                .index
                .apply(
                    &text,
                    [0, 0],
                    Command::Cell {
                        table: 0,
                        row: 2,
                        column: 0,
                        value: format!("edit {n} 🦀"),
                    },
                )
                .unwrap();
            commit_change(&mut state, &mut text, change, n as f64 * 0.1);
            ui.dirty = true;
            ui.ensure(&text, 0);
            timings.push(tick.elapsed().as_secs_f64() * 1000.0);
        }
        timings.sort_by(f64::total_cmp);
        println!(
            "structure_benchmark bytes={bytes} blocks={} commands=100 total_ms={:.3} p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            ui.index.blocks.len(),
            started.elapsed().as_secs_f64() * 1000.0,
            timings[50],
            timings[95],
            timings[99]
        );
        assert_eq!(ui.index.blocks.len(), 14001);
        assert!(text.contains("edit 99 🦀"));
    }
    #[test]
    fn coalesced_structural_and_history_events_execute_every_command_in_order() {
        let ctx = egui::Context::default();
        let source = "| a | b |\n| --- | --- |\n| x | y |\n";
        let mut editor = editor(source);
        editor.structure.open = true;
        editor.structure.ensure(source, 0);
        editor.structure.cell = Some((2, 0));
        editor.structure.cell_focus_owner = true;
        let event = |key, modifiers| egui::Event::Key {
            key,
            modifiers,
            physical_key: None,
            pressed: true,
            repeat: false,
        };
        let add = event(
            egui::Key::Enter,
            egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        );
        let undo = event(egui::Key::Z, egui::Modifiers::COMMAND);
        let redo = event(
            egui::Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        );
        let _ = ctx.run(
            egui::RawInput {
                events: vec![add.clone(), add.clone(), add.clone()],
                ..Default::default()
            },
            |ctx| editor.structure_shortcuts(ctx),
        );
        assert_eq!(editor.structure.index.tables[0].rows.len(), 6);
        assert_eq!(editor.structure.cell, Some((5, 0)));
        let _ = ctx.run(
            egui::RawInput {
                events: vec![undo.clone(), undo.clone(), undo.clone()],
                ..Default::default()
            },
            |ctx| editor.structure_shortcuts(ctx),
        );
        assert_eq!(editor.doc.text, source);
        // Mixed edit/history ordering is preserved within the same input frame.
        let _ = ctx.run(
            egui::RawInput {
                events: vec![redo, add, undo],
                ..Default::default()
            },
            |ctx| editor.structure_shortcuts(ctx),
        );
        assert_eq!(editor.structure.index.tables[0].rows.len(), 4);
    }
    #[test]
    fn source_text_command_text_in_one_frame_keeps_input_and_undo_order() {
        let ctx = egui::Context::default();
        let source = "| a | b |\n| --- | --- |\n| x | y |\n";
        let mut editor = editor(source);
        editor.editor_focus = true;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
        });
        let mut state = editor.source_state(&ctx);
        let at = structure::char_at(source, source.find("x").unwrap());
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(at))));
        state.store(&ctx, editor.source_id());
        ctx.memory_mut(|m| m.request_focus(editor.source_id()));
        let key = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::Text("pre".into()),
                    key,
                    egui::Event::Text("post".into()),
                ],
                ..Default::default()
            },
            |ctx| {
                editor.structure_shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
            },
        );
        assert!(editor.doc.text.contains("prepostx"));
        assert_eq!(Index::parse(&editor.doc.text).tables[0].rows.len(), 4);
        undo_key(&mut editor, &ctx, false);
        assert!(editor.doc.text.contains("prex"));
        assert_eq!(Index::parse(&editor.doc.text).tables[0].rows.len(), 4);
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, source.replace("x", "prex"));
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, source);
    }
    #[test]
    fn cell_text_command_text_in_one_frame_uses_new_cell_and_restores_selection() {
        let ctx = egui::Context::default();
        let source = "| a | b |\n| --- | --- |\n| x | y |\n";
        let mut editor = editor(source);
        editor.structure.open = true;
        editor.structure.ensure(source, 0);
        editor.structure.cell = Some((2, 0));
        editor.structure.focus_cell = Some((2, 0));
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
            editor.structure_window(ctx);
        });
        let id = editor.cell_id(2, 0);
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(0))));
        state.store(&ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
        let key = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::ALT,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::Text("pre".into()),
                    key,
                    egui::Event::Text("post".into()),
                ],
                ..Default::default()
            },
            |ctx| {
                editor.structure_shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
                editor.structure_window(ctx);
            },
        );
        assert!(editor.doc.text.contains("prex"));
        assert!(editor.doc.text.contains("post"));
        assert!(!editor.doc.text.contains("prepost"));
        assert_eq!(editor.structure.cell, Some((3, 0)));
        undo_key(&mut editor, &ctx, false);
        assert!(!editor.doc.text.contains("post"));
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, source.replace("x", "prex"));
        assert_eq!(editor.structure.cell, Some((2, 0)));
        assert_eq!(
            egui::text_edit::TextEditState::load(&ctx, id)
                .unwrap()
                .cursor
                .char_range()
                .unwrap()
                .primary
                .index,
            3
        );
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, source);
        assert_eq!(
            egui::text_edit::TextEditState::load(&ctx, id)
                .unwrap()
                .cursor
                .char_range()
                .unwrap()
                .primary
                .index,
            0
        );
    }
    #[test]
    fn paste_final_tab_text_without_other_shortcuts_keeps_both_cells_and_one_new_row() {
        let ctx = egui::Context::default();
        let source = "| a | b |\n| --- | --- |\n| x | y |\n";
        let mut editor = editor(source);
        editor.structure.open = true;
        editor.structure.ensure(source, 0);
        editor.structure.cell = Some((2, 1));
        editor.structure.focus_cell = Some((2, 1));
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
            editor.structure_window(ctx);
        });
        let id = editor.cell_id(2, 1);
        let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(1))));
        state.store(&ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
        let tab = egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::Paste("old".into()),
                    tab,
                    egui::Event::Text("new".into()),
                ],
                ..Default::default()
            },
            |ctx| {
                editor.structure_shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| editor.editor(ui));
                editor.structure_window(ctx);
            },
        );
        assert_eq!(editor.structure.index.tables[0].rows.len(), 4);
        assert!(
            editor.doc.text.contains("| x | old |"),
            "{}",
            editor.doc.text
        );
        let row = &editor.structure.index.tables[0].rows[3];
        assert_eq!(editor.doc.text[row.cells[0].clone()].trim(), "new");
        assert_eq!(editor.structure.cell, Some((3, 0)));
        undo_key(&mut editor, &ctx, false);
        assert!(!editor.doc.text.contains("new"));
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, source.replace("y", "old"));
        undo_key(&mut editor, &ctx, false);
        assert_eq!(editor.doc.text, source);
    }
}
