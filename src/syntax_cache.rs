// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Lazy syntax highlighting. Only requested code pages are coloured. Checkpointed
//! Syntect state keeps multi-line strings/comments correct across page boundaries.
use crate::{egui, large_document::CodePage, rich_nodes};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};
use syntect::{
    easy::HighlightLines,
    highlighting::{HighlightState, ThemeSet},
    parsing::{ParseState, SyntaxSet},
};

struct Request {
    page: CodePage,
    dark: bool,
    key: u64,
}
struct Output {
    key: u64,
    sections: Vec<(std::ops::Range<usize>, egui::Color32)>,
}
struct Context {
    source: Arc<str>,
    checkpoints: BTreeMap<usize, (HighlightState, ParseState)>,
    checkpoint_sizes: BTreeMap<usize, usize>,
}
#[derive(Default)]
struct WorkerStats {
    source_bytes: AtomicUsize,
    checkpoints: AtomicUsize,
    checkpoint_estimated_bytes: AtomicUsize,
}
struct Worker {
    tx: mpsc::SyncSender<Request>,
    rx: mpsc::Receiver<Output>,
    stats: Arc<WorkerStats>,
}
impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Request>(8);
        let (results, rx) = mpsc::channel();
        let stats = Arc::new(WorkerStats::default());
        let counters = stats.clone();
        std::thread::Builder::new()
            .name("mdcard-code-highlighter".into())
            .spawn(move || {
                let syntaxes = SyntaxSet::load_defaults_newlines();
                let themes = ThemeSet::load_defaults();
                let mut contexts = HashMap::<u64, Context>::new();
                while let Ok(job) = jobs.recv() {
                    let theme = &themes.themes[if job.dark {
                        "base16-ocean.dark"
                    } else {
                        "InspiredGitHub"
                    }];
                    let context_key = rich_nodes::hash((job.page.document_key, job.dark));
                    if !contexts.contains_key(&context_key) {
                        let used = contexts.values().map(|c| c.source.len()).sum::<usize>();
                        if contexts.len() >= 8 || used + job.page.source.len() > 16 * 1024 * 1024 {
                            contexts.clear();
                        }
                        let syntax = syntaxes
                            .find_syntax_by_token(&job.page.language)
                            .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
                        let state = HighlightLines::new(syntax, theme).state();
                        contexts.insert(
                            context_key,
                            Context {
                                source: job.page.source.clone(),
                                checkpoints: BTreeMap::from([(0, state)]),
                                checkpoint_sizes: BTreeMap::new(),
                            },
                        );
                    }
                    let context = contexts.get_mut(&context_key).unwrap();
                    let (start, (highlight, parse)) = context
                        .checkpoints
                        .range(..=job.page.bytes.start)
                        .next_back()
                        .unwrap();
                    let mut at = *start;
                    let mut highlighter =
                        HighlightLines::from_state(theme, highlight.clone(), parse.clone());
                    let mut sections = Vec::new();
                    let mut lines = 0;
                    for line in context.source[at..].split_inclusive('\n') {
                        let styles = highlighter
                            .highlight_line(line, &syntaxes)
                            .unwrap_or_default();
                        let mut offset = at;
                        for (style, text) in styles {
                            let end = offset + text.len();
                            let a = offset.max(job.page.bytes.start);
                            let b = end.min(job.page.bytes.end);
                            if a < b {
                                sections.push((
                                    a - job.page.bytes.start..b - job.page.bytes.start,
                                    egui::Color32::from_rgb(
                                        style.foreground.r,
                                        style.foreground.g,
                                        style.foreground.b,
                                    ),
                                ));
                            }
                            offset = end;
                        }
                        at += line.len();
                        lines += 1;
                        if lines % 64 == 0 || at >= job.page.bytes.end {
                            let state = highlighter.state();
                            let estimate = format!("{:?}{:?}", state.0, state.1)
                                .len()
                                .saturating_mul(4)
                                + 256;
                            if estimate <= 1024 * 1024 {
                                context
                                    .checkpoints
                                    .insert(at, (state.0.clone(), state.1.clone()));
                                context.checkpoint_sizes.insert(at, estimate);
                            }
                            while context.checkpoints.len() > 128
                                || context.checkpoint_sizes.values().sum::<usize>() > 1024 * 1024
                            {
                                let key = context
                                    .checkpoints
                                    .keys()
                                    .copied()
                                    .find(|key| *key != 0)
                                    .unwrap();
                                context.checkpoints.remove(&key);
                                context.checkpoint_sizes.remove(&key);
                            }
                            highlighter = HighlightLines::from_state(theme, state.0, state.1);
                        }
                        if at >= job.page.bytes.end {
                            break;
                        }
                    }
                    counters.source_bytes.store(
                        contexts.values().map(|c| c.source.len()).sum(),
                        Ordering::Relaxed,
                    );
                    counters.checkpoints.store(
                        contexts.values().map(|c| c.checkpoints.len()).sum(),
                        Ordering::Relaxed,
                    );
                    counters.checkpoint_estimated_bytes.store(
                        contexts
                            .values()
                            .map(|c| c.checkpoint_sizes.values().sum::<usize>())
                            .sum(),
                        Ordering::Relaxed,
                    );
                    if results
                        .send(Output {
                            key: job.key,
                            sections,
                        })
                        .is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("code highlighting worker");
        Self { tx, rx, stats }
    }
}
#[derive(Default)]
pub struct SyntaxCache {
    worker: Option<Worker>,
    results: HashMap<u64, (Arc<Output>, u64)>,
    pending: HashSet<u64>,
    clock: u64,
    pub submissions: u64,
    pub hits: u64,
    bytes: usize,
}
impl SyntaxCache {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        page: &CodePage,
        font: f32,
        dark: bool,
    ) -> Option<(Arc<egui::Galley>, egui::Pos2)> {
        if let Some(worker) = &self.worker {
            while let Ok(output) = worker.rx.try_recv() {
                self.pending.remove(&output.key);
                let size = output.sections.len()
                    * std::mem::size_of::<(std::ops::Range<usize>, egui::Color32)>();
                while !self.results.is_empty()
                    && (self.results.len() >= 512 || self.bytes + size > 16 * 1024 * 1024)
                {
                    let id = *self
                        .results
                        .iter()
                        .min_by_key(|(_, (_, age))| *age)
                        .unwrap()
                        .0;
                    let (old, _) = self.results.remove(&id).unwrap();
                    self.bytes = self.bytes.saturating_sub(
                        old.sections.len()
                            * std::mem::size_of::<(std::ops::Range<usize>, egui::Color32)>(),
                    );
                }
                self.bytes += size;
                self.results
                    .insert(output.key, (Arc::new(output), self.clock));
            }
        }
        let key = rich_nodes::hash((page.key, dark));
        self.clock += 1;
        let text = &page.source[page.bytes.clone()];
        let mut job = egui::text::LayoutJob::simple(
            text.to_owned(),
            egui::FontId::monospace(font),
            ui.visuals().text_color(),
            f32::INFINITY,
        );
        if let Some((output, age)) = self.results.get_mut(&key) {
            *age = self.clock;
            self.hits += 1;
            job.sections = output
                .sections
                .iter()
                .map(|(range, color)| egui::text::LayoutSection {
                    leading_space: 0.,
                    byte_range: range.clone(),
                    format: egui::TextFormat {
                        font_id: egui::FontId::monospace(font),
                        color: *color,
                        ..Default::default()
                    },
                })
                .collect();
        } else if !self.pending.contains(&key) {
            if self.worker.is_none() {
                self.worker = Some(Worker::new(ui.ctx().clone()));
            }
            if self
                .worker
                .as_ref()
                .unwrap()
                .tx
                .try_send(Request {
                    page: page.clone(),
                    dark,
                    key,
                })
                .is_ok()
            {
                self.pending.insert(key);
                self.submissions += 1;
            }
        }
        let layer = ui.layer_id();
        let first = ui
            .ctx()
            .graphics(|g| g.get(layer).map_or(0, |p| p.next_idx().0));
        ui.add(egui::Label::new(job).extend());
        ui.ctx().graphics(|g| {
            g.get(layer).and_then(|p| {
                p.all_entries().skip(first).find_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        Some((t.galley.clone(), t.pos))
                    } else {
                        None
                    }
                })
            })
        })
    }
    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({"entries":self.results.len(),"bytes":self.bytes,"entry_limit":512,"byte_limit":16777216,"submissions":self.submissions,"hits":self.hits,"pending":self.pending.len(),"worker_source_bytes":self.worker.as_ref().map_or(0,|w|w.stats.source_bytes.load(Ordering::Relaxed)),"worker_source_limit":16777216,"worker_checkpoints":self.worker.as_ref().map_or(0,|w|w.stats.checkpoints.load(Ordering::Relaxed)),"worker_checkpoint_estimated_bytes":self.worker.as_ref().map_or(0,|w|w.stats.checkpoint_estimated_bytes.load(Ordering::Relaxed)),"worker_checkpoint_estimate_limit":8388608,"checkpoint_note":"Conservative debug-state size estimate; at most 8 contexts and 128 checkpoints per context."})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lazy_second_page_matches_whole_file_highlight_across_multiline_comment() {
        let text = format!(
            "fn main() {{\n/* start multiline\n{}end */ let value = 42;\n}}\n",
            "still a comment with \"strings\" and $literal$\n".repeat(90)
        );
        let source: Arc<str> = Arc::from(text);
        let start = source
            .split_inclusive('\n')
            .take(64)
            .map(str::len)
            .sum::<usize>();
        let page = CodePage {
            source: source.clone(),
            language: "rust".into(),
            bytes: start..source.len(),
            document_key: 1,
            key: 2,
        };
        let worker = Worker::new(egui::Context::default());
        worker
            .tx
            .send(Request {
                page: page.clone(),
                dark: true,
                key: 7,
            })
            .unwrap();
        let actual = worker
            .rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("background highlight result");
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        let mut full = HighlightLines::new(
            syntaxes.find_syntax_by_token("rust").unwrap(),
            &themes.themes["base16-ocean.dark"],
        );
        let mut offset = 0;
        let mut expected = Vec::new();
        for line in source.split_inclusive('\n') {
            for (style, text) in full.highlight_line(line, &syntaxes).unwrap() {
                let end = offset + text.len();
                if end > start {
                    expected.push((
                        offset.max(start) - start..end - start,
                        egui::Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        ),
                    ));
                }
                offset = end;
            }
        }
        assert_eq!(actual.key, 7);
        assert_eq!(
            actual.sections, expected,
            "page colouring must inherit syntax state from preceding lines even when requested first"
        );
    }
}
