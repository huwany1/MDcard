// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use crate::localization::Language;
use crate::{
    egui,
    rich_nodes::{self, Node},
};
use base64::Engine;
use egui::Vec2;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const RENDER_TIMEOUT: Duration = Duration::from_secs(18);
const COLD_START_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Deserialize)]
struct Output {
    #[serde(skip)]
    image: Arc<[u8]>,
    #[serde(skip)]
    vector: Arc<[u8]>,
    #[serde(default)]
    ok: bool,
    #[serde(default, with = "id_text")]
    id: u64,
    #[serde(default)]
    png: String,
    #[serde(default)]
    svg: Option<String>,
    #[serde(default)]
    width: f32,
    #[serde(default)]
    height: f32,
    #[serde(default)]
    error: String,
}
#[derive(Serialize)]
struct Request {
    #[serde(with = "id_text")]
    id: u64,
    kind: String,
    source: String,
    dark: bool,
    inline: bool,
    font: f32,
}
mod id_text {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
struct Worker {
    tx: Option<mpsc::SyncSender<Request>>,
    rx: mpsc::Receiver<Output>,
    thread: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
    watchdog: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(4);
        let (results, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let pid = Arc::new(AtomicU32::new(0));
        let worker_pid = pid.clone();
        let watchdog_pid = pid.clone();
        let watchdog_stop = stop.clone();
        let active = Arc::new(Mutex::new(None::<Instant>));
        let watchdog_active = active.clone();
        let watchdog = std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(50));
                let expired = watchdog_active
                    .lock()
                    .unwrap()
                    .is_some_and(|deadline| Instant::now() > deadline);
                if watchdog_stop.load(Ordering::Relaxed) || expired {
                    let id = watchdog_pid.swap(0, Ordering::Relaxed);
                    if id != 0 {
                        kill_pid(id);
                    }
                    if watchdog_stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        });
        let thread = std::thread::spawn(move || {
            let root = std::env::var_os("MDCARD_RENDERER_DIR")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|p| p.join("renderer")))
                        .filter(|p| p.join("worker.mjs").exists())
                })
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("renderer"));
            let local_node =
                root.join("runtime")
                    .join(if cfg!(windows) { "node.exe" } else { "node" });
            let mut command = std::process::Command::new(if local_node.exists() {
                local_node
            } else {
                PathBuf::from("node")
            });
            command
                .arg(root.join("worker.mjs"))
                .current_dir(&root)
                .env("PLAYWRIGHT_BROWSERS_PATH", root.join("runtime/browsers"))
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            let mut process = None;
            while !stopping.load(Ordering::Relaxed) {
                let request = match requests.recv_timeout(Duration::from_millis(50)) {
                    Ok(r) => r,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                };
                let result = (|| -> Result<Output, String> {
                    // A fresh Node/browser needs a bounded cold-start allowance.
                    // Once any render succeeds, ordinary requests keep the 18s cap.
                    // The watchdog and receive loop share this absolute deadline.
                    let initialized = process.as_ref().is_some_and(|(_, _, _, ready)| *ready);
                    let deadline = Instant::now()
                        + if initialized {
                            RENDER_TIMEOUT
                        } else {
                            COLD_START_TIMEOUT
                        };
                    *active.lock().unwrap() = Some(deadline);
                    if process.is_none() {
                        let mut child = command
                            .spawn()
                            .map_err(|e| format!("Local renderer unavailable: {e}"))?;
                        worker_pid.store(child.id(), Ordering::Relaxed);
                        let input = child.stdin.take().unwrap();
                        let output = child.stdout.take().unwrap();
                        let (lines, line_rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let mut reader = BufReader::new(output);
                            loop {
                                let mut line = String::new();
                                match reader.read_line(&mut line) {
                                    Ok(0) | Err(_) => break,
                                    Ok(_) => {
                                        if lines.send(line).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        });
                        process = Some((child, input, line_rx, false));
                    }
                    let (_, input, lines, initialized) = process.as_mut().unwrap();
                    serde_json::to_writer(&mut *input, &request).map_err(|e| e.to_string())?;
                    input.write_all(b"\n").map_err(|e| e.to_string())?;
                    input.flush().map_err(|e| e.to_string())?;
                    loop {
                        if stopping.load(Ordering::Relaxed) || Instant::now() > deadline {
                            return Err("Renderer timed out; the worker will restart.".into());
                        }
                        match lines.recv_timeout(Duration::from_millis(50)) {
                            Ok(line) => {
                                let output: Output =
                                    serde_json::from_str(&line).map_err(|e| e.to_string())?;
                                if output.ok {
                                    *initialized = true;
                                }
                                return Ok(output);
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(_) => {
                                return Err("Renderer stopped; the worker will restart.".into());
                            }
                        }
                    }
                })();
                *active.lock().unwrap() = None;
                if result.is_err()
                    && let Some((mut child, _, _, _)) = process.take()
                {
                    reap_worker(&mut child, &worker_pid);
                }
                let _ = results.send(result.unwrap_or_else(|error| Output {
                    image: Arc::from([]),
                    vector: Arc::from([]),
                    ok: false,
                    id: request.id,
                    png: String::new(),
                    svg: None,
                    width: 0.,
                    height: 0.,
                    error,
                }));
                ctx.request_repaint();
            }
            if let Some((mut child, _, _, _)) = process.take() {
                reap_worker(&mut child, &worker_pid);
            }
        });
        Self {
            tx: Some(tx),
            rx,
            thread: Some(thread),
            stop,
            pid,
            watchdog: Some(watchdog),
        }
    }
}
fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill.exe")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .status();
    }
}
fn reap_worker(child: &mut std::process::Child, pid: &AtomicU32) {
    let owned = pid
        .compare_exchange(child.id(), 0, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok();
    if child.try_wait().ok().flatten().is_none() {
        if owned {
            kill_pid(child.id());
        }
        let _ = child.kill();
    }
    let _ = child.wait();
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.tx.take();
        let id = self.pid.swap(0, Ordering::Relaxed);
        if id != 0 {
            kill_pid(id);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.watchdog.take() {
            let _ = t.join();
        }
    }
}
#[derive(Default)]
struct View {
    mode: u8,
    zoom: f32,
    pan: Vec2,
    source: String,
    source_hash: u64,
    source_changed: Option<Instant>,
    expected: u64,
}
#[derive(Serialize, Default)]
pub struct Counters {
    pub undo_calls: u64,
    pub redo_calls: u64,
    pub history_changes: u64,
    pub parses: u64,
    pub submissions: u64,
    pub cache_hits: u64,
    pub visible_nodes: u64,
    pub offscreen_nodes: u64,
    pub completed: u64,
    pub errors: u64,
}
#[derive(Serialize)]
struct Control {
    name: String,
    rect: [f32; 4],
}
fn control(controls: &mut Vec<Control>, name: impl Into<String>, rect: egui::Rect) {
    controls.push(Control {
        name: name.into(),
        rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
    });
}
struct Draft {
    node: Node,
    index: usize,
    text: String,
    expected: u64,
    changed: Option<Instant>,
    error: Option<String>,
    closing: bool,
}
#[derive(Clone)]
pub struct Block {
    pub source: std::ops::Range<usize>,
    pub range: std::ops::Range<usize>,
    pub height: f32,
    pub nodes: u64,
    pub key: u64,
    pub code: Option<crate::large_document::CodePage>,
    pub mapping: Vec<crate::large_document::PreviewSourceSegment>,
}
pub struct RichUi {
    recovery_revision: u64,
    pub(crate) recovery_discard_serial: u64,
    ime_composing: bool,
    syntax: crate::syntax_cache::SyntaxCache,
    pub preview: String,
    pub nodes: Vec<Node>,
    original: String,
    source_map: Vec<crate::large_document::PreviewSourceSegment>,
    generation: u64,
    source_revision: Option<(u64, u64)>,
    parsed_source_revision: Option<(u64, u64)>,
    changed: Instant,
    worker: Option<Worker>,
    results: HashMap<u64, Arc<Output>>,
    result_age: HashMap<u64, u64>,
    result_clock: u64,
    frame_resident: HashSet<u64>,
    frame_layout_pending: bool,
    resident: HashSet<u64>,
    dimensions: HashMap<u64, (f32, f32)>,
    evictions: u64,
    pending: HashSet<u64>,
    views: HashMap<usize, View>,
    draft: Option<Draft>,
    inline_edit: Option<(u64, Node, String)>,
    message: Option<String>,
    discard_confirmation: bool,
    stamp: u64,
    controls: Vec<Control>,
    frame_ms: std::collections::VecDeque<f64>,
    input_events: std::collections::VecDeque<String>,
    input_frame: bool,
    scroll_frame: bool,
    interaction_until: Option<Instant>,
    input_cpu: Vec<f64>,
    phase_cpu: HashMap<String, Vec<f64>>,
    scroll_cpu: Vec<f64>,
    interaction_cpu: Vec<f64>,
    blocks: Vec<Block>,
    heights: HashMap<u64, f32>,
    layout_width: f32,
    layout_font: f32,
    layout_dirty: bool,
    block_index: crate::large_document::Heights,
    block_keys: HashMap<u64, Vec<usize>>,
    pub height_stamp: u64,
    pending_source: Option<String>,
    refresh_now: bool,
    pub counters: Counters,
}
impl Default for RichUi {
    fn default() -> Self {
        Self {
            recovery_revision: 0,
            recovery_discard_serial: 0,
            ime_composing: false,
            syntax: crate::syntax_cache::SyntaxCache::default(),
            preview: String::new(),
            nodes: vec![],
            original: String::new(),
            source_map: Vec::new(),
            generation: u64::MAX,
            source_revision: None,
            parsed_source_revision: None,
            changed: Instant::now() - Duration::from_secs(1),
            worker: None,
            results: HashMap::new(),
            result_age: HashMap::new(),
            result_clock: 0,
            frame_resident: HashSet::new(),
            frame_layout_pending: false,
            resident: HashSet::new(),
            dimensions: HashMap::new(),
            evictions: 0,
            pending: HashSet::new(),
            views: HashMap::new(),
            draft: None,
            inline_edit: None,
            message: None,
            discard_confirmation: false,
            stamp: 0,
            controls: vec![],
            frame_ms: std::collections::VecDeque::new(),
            input_events: std::collections::VecDeque::new(),
            input_frame: false,
            scroll_frame: false,
            interaction_until: None,
            input_cpu: vec![],
            phase_cpu: HashMap::new(),
            scroll_cpu: vec![],
            interaction_cpu: vec![],
            blocks: vec![],
            heights: HashMap::new(),
            layout_width: 0.,
            layout_font: 0.,
            layout_dirty: true,
            block_index: crate::large_document::Heights::default(),
            block_keys: HashMap::new(),
            height_stamp: 0,
            pending_source: None,
            refresh_now: false,
            counters: Counters::default(),
        }
    }
}
impl RichUi {
    pub(crate) fn recovery_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.recovery_discard_serial.hash(&mut hash);
        self.recovery_revision.hash(&mut hash);
        if let Some(d) = &self.draft {
            (d.expected, d.changed, d.error.is_some(), d.text.len()).hash(&mut hash);
        }
        if let Some((expected, node, text)) = &self.inline_edit {
            (expected, &node.range, text.len()).hash(&mut hash);
        }
        for (index, view) in &self.views {
            (index, view.expected, view.source_changed, view.source.len()).hash(&mut hash);
        }
        hash.finish()
    }
    pub(crate) fn recovery_drafts(&self) -> Vec<crate::recovery::Draft> {
        let convert = |expected: u64, node: &Node, text: &str| crate::recovery::Draft {
            kind: node.kind.into(),
            text: text.into(),
            original: node.raw.clone(),
            range: [node.range.start, node.range.end],
            expected,
        };
        let mut drafts = Vec::new();
        if let Some(d) = &self.draft
            && (d.changed.is_some() || d.error.is_some() || d.text != d.node.source)
        {
            drafts.push(convert(d.expected, &d.node, &d.text));
        }
        if let Some((expected, node, text)) = &self.inline_edit {
            drafts.push(convert(*expected, node, text));
        }
        for (index, view) in &self.views {
            if let Some(node) = self.nodes.get(*index)
                && (view.source_changed.is_some() || view.source != node.source)
            {
                drafts.push(convert(view.expected, node, &view.source));
            }
        }
        drafts.sort_by(|a, b| (&a.range, &a.kind, &a.text).cmp(&(&b.range, &b.kind, &b.text)));
        drafts.dedup();
        drafts
    }
    #[cfg(test)]
    pub(crate) fn stage_pending_test_edit(
        &mut self,
        text: &str,
        generation: u64,
        value: &str,
        ctx: &egui::Context,
    ) {
        self.ensure(text, generation, ctx);
        self.open(0);
        let draft = self.draft.as_mut().unwrap();
        draft.text = value.into();
        draft.changed = Some(Instant::now());
    }
    #[cfg(test)]
    pub(crate) fn make_pending_test_edit_due(&mut self) {
        self.draft.as_mut().unwrap().changed = Some(Instant::now() - Duration::from_secs(1));
    }
    pub fn install_incremental(
        &mut self,
        parsed: &mut crate::large_document::Parsed,
        doc: &crate::document::Document,
    ) {
        if self.generation != parsed.generation {
            self.draft = None;
            self.views.clear();
            self.inline_edit = None;
            self.dimensions.clear();
        }
        self.source_map.clear();
        self.preview = std::mem::take(&mut parsed.preview);
        self.nodes = std::mem::take(&mut parsed.nodes);
        self.blocks = parsed
            .blocks
            .iter()
            .map(|b| Block {
                source: b.source.clone(),
                range: b.preview.clone(),
                height: 100.,
                nodes: b.nodes,
                key: b.key,
                code: b.code.clone(),
                mapping: b.mapping.clone(),
            })
            .collect();
        self.original = doc.text.clone();
        self.parsed_source_revision = Some((doc.buffer_id(), doc.revision()));
        self.stamp = parsed.source_hash;
        self.generation = parsed.generation;
        self.layout_dirty = true;
        self.pending_source = None;
        self.changed = Instant::now() - Duration::from_secs(1);
        self.counters.parses += 1;
    }
    pub fn visible_blocks(
        &mut self,
        width: f32,
        font: f32,
        viewport: egui::Rect,
    ) -> (Vec<(usize, Block)>, f32, f32) {
        self.visible_blocks_around(width, font, viewport, None)
    }
    pub fn visible_blocks_around(
        &mut self,
        width: f32,
        font: f32,
        viewport: egui::Rect,
        source_pin: Option<usize>,
    ) -> (Vec<(usize, Block)>, f32, f32) {
        if self.layout_dirty || (self.layout_width - width).abs() > 1. || self.layout_font != font {
            let _ = self.layout_blocks(width, font);
            self.block_index =
                crate::large_document::Heights::new(self.blocks.iter().map(|b| b.height).collect());
            self.block_keys.clear();
            for (i, b) in self.blocks.iter().enumerate() {
                self.block_keys.entry(b.key).or_default().push(i);
            }
            self.layout_dirty = false;
            self.height_stamp = self.height_stamp.wrapping_add(1);
        }
        let mut first = self.block_index.at((viewport.min.y - 420.).max(0.));
        let mut last = (self.block_index.at(viewport.max.y + 420.) + 1).min(self.blocks.len());
        if let Some(byte) = source_pin {
            let block = self
                .blocks
                .partition_point(|b| b.source.end <= byte)
                .min(self.blocks.len().saturating_sub(1));
            if block < first || block >= last {
                first = block;
                last = (self
                    .block_index
                    .at(self.block_index.prefix(block) + viewport.height() + 420.)
                    + 1)
                .min(self.blocks.len());
            }
        }
        let visible_nodes = self.blocks[first..last]
            .iter()
            .map(|block| block.nodes)
            .sum::<u64>();
        self.counters.offscreen_nodes += (self.nodes.len() as u64).saturating_sub(visible_nodes);
        (
            (first..last).map(|i| (i, self.blocks[i].clone())).collect(),
            self.block_index.prefix(first),
            self.block_index.total(),
        )
    }
    pub fn height_index(&self) -> crate::large_document::Heights {
        self.block_index.clone()
    }
    pub fn block_offset(&self, index: usize) -> f32 {
        self.block_index.prefix(index)
    }
    pub fn block_at(&self, offset: f32) -> usize {
        self.block_index.at(offset)
    }
    pub fn source_range_in(
        &self,
        block: &Block,
        range: std::ops::Range<usize>,
    ) -> Option<std::ops::Range<usize>> {
        if !block.mapping.is_empty() {
            crate::large_document::map_preview_range(&block.mapping, range)
        } else {
            crate::large_document::map_preview_range(
                &self.source_map,
                block.range.start + range.start..block.range.start + range.end,
            )
        }
    }
    pub fn reading_anchor(&self, y: f32) -> (usize, f32, f32) {
        let i = self.block_at(y);
        let top = self.block_offset(i);
        let height = self.block_index.value(i).max(1.);
        (
            self.blocks.get(i).map_or(0, |b| b.source.start),
            ((y - top) / height).clamp(0., 1.),
            0.,
        )
    }
    pub fn reading_calibration(&self) -> Vec<(u64, f32)> {
        self.blocks
            .iter()
            .filter_map(|b| self.heights.get(&b.key).map(|h| (b.key, *h)))
            .take(10000)
            .collect()
    }
    pub fn restore_reading_calibration(&mut self, width: f32, font: f32, heights: Vec<(u64, f32)>) {
        let _ = self.layout_blocks(width, font);
        for (key, height) in heights {
            if height.is_finite() && height > 0. && height < 10_000_000. {
                self.heights.insert(key, height);
            }
        }
        self.layout_dirty = true;
    }
    pub fn layout_pending(&self) -> bool {
        self.frame_layout_pending
    }
    pub fn anchor_has_async_content(&self, byte: usize) -> bool {
        let i = self
            .blocks
            .partition_point(|b| b.source.end <= byte)
            .min(self.blocks.len().saturating_sub(1));
        self.blocks
            .get(i)
            .is_some_and(|b| b.nodes > 0 || self.preview[b.range.clone()].contains("!["))
    }
    pub fn reading_y(&self, byte: usize, fraction: f32, viewport_y: f32) -> f32 {
        let i = self
            .blocks
            .partition_point(|b| b.source.end <= byte)
            .min(self.blocks.len().saturating_sub(1));
        (self.block_offset(i) + fraction.clamp(0., 1.) * self.block_index.value(i) - viewport_y)
            .max(0.)
    }
    /// Bootstrap only: painted soft-break runs far inside one tall block may
    /// not exist while its top is on screen. Estimate an area to mount first;
    /// only the subsequent actual Galley target can establish text precision.
    pub fn text_bootstrap_y(&self, byte: usize) -> f32 {
        let index = self
            .blocks
            .partition_point(|b| b.source.end <= byte)
            .min(self.blocks.len().saturating_sub(1));
        let Some(block) = self.blocks.get(index) else {
            return 0.;
        };
        let fraction =
            byte.saturating_sub(block.source.start) as f32 / block.source.len().max(1) as f32;
        self.block_offset(index) + fraction.clamp(0., 1.) * self.block_index.value(index)
    }
    pub fn heights_for_block(&self, key: u64, fallback: f32) -> f32 {
        self.heights.get(&key).copied().unwrap_or(fallback)
    }
    pub fn code_page(
        &mut self,
        ui: &mut egui::Ui,
        page: &crate::large_document::CodePage,
        font: f32,
        dark: bool,
    ) -> Option<(Arc<egui::Galley>, egui::Pos2)> {
        self.syntax.show(ui, page, font, dark)
    }
    pub fn cache_stats(&self) -> serde_json::Value {
        serde_json::json!({"entries":self.results.len(),"bytes":self.results.values().map(|r|r.image.len()+r.vector.len()+r.svg.as_ref().map_or(0,String::len)).sum::<usize>(),"entry_limit":1024,"byte_limit":67108864,"resident_textures":self.resident.len(),"evictions":self.evictions,"submissions":self.counters.submissions,"completed":self.counters.completed,"hits":self.counters.cache_hits,"pending":self.pending.len(),"errors":self.counters.errors,"code":self.syntax.stats()})
    }
    pub fn release_offscreen(&mut self, ctx: &egui::Context) {
        for id in self.resident.difference(&self.frame_resident) {
            ctx.forget_image(&format!("bytes://rich-{id}.png"));
            ctx.forget_image(&format!("bytes://rich-{id}.svg"));
        }
        self.resident = std::mem::take(&mut self.frame_resident);
    }
    pub fn layout_blocks(&mut self, width: f32, font: f32) -> Vec<Block> {
        if (self.layout_width - width).abs() > 1. || self.layout_font != font {
            self.heights.clear();
            self.layout_width = width;
            self.layout_font = font;
        }
        for block in &mut self.blocks {
            block.height = self.heights.get(&block.key).copied().unwrap_or_else(|| {
                let text = &self.preview[block.range.clone()];
                if text.trim_start().starts_with("$$mdcard") {
                    180.
                } else if text.contains("![") {
                    240.
                } else {
                    let lines = text
                        .lines()
                        .map(|line| {
                            ((line.chars().count() as f32 * font * 0.52) / width.max(100.))
                                .ceil()
                                .max(1.)
                        })
                        .sum::<f32>();
                    (lines * (font + 5.) + 12.).max(font + 10.)
                }
            });
        }
        self.blocks.clone()
    }
    pub fn block_height(&mut self, key: u64, height: f32) {
        // Absolute coordinates near the tail of a long document lose subpixel
        // precision. Stabilize measurements before updating the Fenwick tree;
        // otherwise rounding noise continually adjusts the scroll anchor.
        let height = (height * 2.).round() / 2.;
        if self
            .heights
            .get(&key)
            .is_some_and(|old| (*old - height).abs() < 0.5)
        {
            return;
        }
        if self.heights.len() > 10000 {
            self.heights.clear();
        }
        self.heights.insert(key, height);
        self.height_stamp = self.height_stamp.wrapping_add(1);
        if let Some(indices) = self.block_keys.get(&key) {
            for index in indices {
                self.blocks[*index].height = height;
                self.block_index.set(*index, height);
            }
        }
    }
    pub fn record_input(&mut self, ctx: &egui::Context) {
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Ime(ime) = event {
                match ime {
                    egui::ImeEvent::Preedit(_) => self.ime_composing = true,
                    egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled => {
                        self.ime_composing = false
                    }
                    _ => {}
                }
            }
        }
        self.frame_resident.clear();
        if std::env::var_os("MDCARD_RICH_REPORT").is_none() {
            return;
        }
        let focus = ctx.memory(|m| m.focused());
        let focus_name = if focus == Some(egui::Id::new("rich-floating-source")) {
            "floating".to_owned()
        } else if let Some(index) = self
            .views
            .keys()
            .find(|index| focus == Some(egui::Id::new(("rich-node-source", *index))))
        {
            format!("node-{index}")
        } else {
            format!("{focus:?}")
        };
        ctx.input(|i| {
            self.input_frame = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Key {
                            key: egui::Key::Backspace | egui::Key::Delete,
                            pressed: true,
                            ..
                        }
                )
            });
            self.scroll_frame = i
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::MouseWheel { .. }));
            if self.input_frame || self.scroll_frame {
                self.interaction_until = Some(Instant::now() + Duration::from_millis(500));
            }
            for e in &i.events {
                let value = match e {
                    egui::Event::Key {
                        key,
                        pressed,
                        modifiers,
                        ..
                    } if *pressed => Some(format!("key:{key:?}:{modifiers:?}")),
                    egui::Event::Paste(s) => Some(format!("paste:{}", s.len())),
                    _ => None,
                };
                if let Some(value) = value {
                    self.input_events
                        .push_back(format!("{value} focus={focus_name}"));
                    if self.input_events.len() > 32 {
                        self.input_events.pop_front();
                    }
                }
            }
        });
    }
    /// Production source identity includes the editable buffer, not only its file.
    pub fn ensure_document(
        &mut self,
        doc: &crate::document::Document,
        generation: u64,
        ctx: &egui::Context,
    ) {
        let key = (doc.buffer_id(), doc.revision());
        let changed = self.source_revision != Some(key);
        self.source_revision = Some(key);
        self.ensure_source(&doc.text, generation, ctx, changed);
        if self.pending_source.is_none() {
            self.parsed_source_revision = Some(key);
        }
    }
    /// Requested revisions can precede the debounced parser commit. Geometry
    /// may use the current document only after its actual rendered source is installed.
    pub fn source_ready(&self, doc: &crate::document::Document, generation: u64) -> bool {
        self.generation == generation
            && self.pending_source.is_none()
            && self.parsed_source_revision == Some((doc.buffer_id(), doc.revision()))
    }
    #[cfg(test)]
    pub fn ensure(&mut self, text: &str, generation: u64, ctx: &egui::Context) {
        self.source_revision = None;
        self.ensure_source(text, generation, ctx, true);
    }
    fn ensure_source(&mut self, text: &str, generation: u64, ctx: &egui::Context, changed: bool) {
        self.controls.clear();
        let initial = self.generation != generation;
        if initial || changed {
            if initial || self.original != text {
                if self.pending_source.as_deref() != Some(text) {
                    self.pending_source = Some(text.to_owned());
                    self.changed = Instant::now();
                }
            } else {
                // Undo can return to the already parsed version before debounce.
                self.pending_source = None;
            }
        }
        if self.pending_source.is_some() {
            if initial || self.refresh_now || self.changed.elapsed() >= Duration::from_millis(250) {
                self.original = self.pending_source.take().unwrap();
                self.stamp = rich_nodes::hash(&self.original);
                self.generation = generation;
                (self.preview, self.nodes) = rich_nodes::parse(&self.original);
                self.blocks = preview_blocks(&self.preview);
                self.source_map = crate::large_document::preview_source_map(
                    &self.original,
                    &self.preview,
                    &self.nodes,
                );
                for block in &mut self.blocks {
                    block.source = crate::large_document::map_preview_boundary(
                        &self.source_map,
                        block.range.start,
                        self.original.len(),
                    )
                        ..crate::large_document::map_preview_boundary(
                            &self.source_map,
                            block.range.end,
                            self.original.len(),
                        );
                }
                self.layout_dirty = true;
                self.counters.parses += 1;
                self.changed = Instant::now() - Duration::from_secs(1);
                self.refresh_now = false;
                if initial {
                    self.draft = None;
                    self.views.clear();
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(260));
            }
        }
        self.poll_results(ctx);
    }
    pub fn poll_results(&mut self, ctx: &egui::Context) {
        self.frame_layout_pending = false;
        self.controls.clear();
        self.frame_resident.clear();
        if let Some(w) = &self.worker {
            while let Ok(mut result) = w.rx.try_recv() {
                self.pending.remove(&result.id);
                self.counters.completed += 1;
                if !result.ok {
                    self.counters.errors += 1;
                }
                result.image = base64::engine::general_purpose::STANDARD
                    .decode(&result.png)
                    .unwrap_or_default()
                    .into();
                result.png.clear();
                result.vector = result
                    .svg
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes()
                    .to_vec()
                    .into();
                let bytes = |r: &Output| {
                    r.image.len() + r.vector.len() + r.svg.as_ref().map_or(0, String::len)
                };
                let incoming = bytes(&result);
                let mut used = self.results.values().map(|r| bytes(r)).sum::<usize>();
                while !self.results.is_empty()
                    && (self.results.len() >= 1024 || used + incoming > 64 * 1024 * 1024)
                {
                    let id = *self
                        .result_age
                        .iter()
                        .min_by_key(|(_, age)| *age)
                        .unwrap()
                        .0;
                    self.result_age.remove(&id);
                    if let Some(old) = self.results.remove(&id) {
                        used = used.saturating_sub(bytes(&old));
                    }
                    ctx.forget_image(&format!("bytes://rich-{id}.png"));
                    ctx.forget_image(&format!("bytes://rich-{id}.svg"));
                    self.resident.remove(&id);
                    self.evictions += 1;
                }
                if incoming > 64 * 1024 * 1024 {
                    continue;
                }
                if result.ok {
                    if self.dimensions.len() >= 10000 {
                        self.dimensions.clear();
                    }
                    self.dimensions
                        .insert(result.id, (result.width, result.height));
                }
                self.result_clock += 1;
                self.result_age.insert(result.id, self.result_clock);
                self.results.insert(result.id, Arc::new(result));
            }
        }
    }
    pub fn node(
        &mut self,
        ui: &mut egui::Ui,
        token: &str,
        dark: bool,
        font: f32,
        language: Language,
    ) {
        let Some(index) = token
            .strip_prefix("mdcard")
            .and_then(|x| x.parse::<usize>().ok())
        else {
            ui.label(token);
            return;
        };
        let Some(node) = self.nodes.get(index).cloned() else {
            return;
        };
        let id = rich_nodes::hash((node.kind, &node.source, dark, node.inline, font.to_bits()));
        let inline = node.inline;
        let result = self.results.get(&id).cloned();
        if result.is_some() {
            self.result_clock += 1;
            self.result_age.insert(id, self.result_clock);
        }
        let historical = self.dimensions.get(&id).copied();
        let mut visible = false;
        if inline {
            let size = result
                .as_ref()
                .filter(|r| r.ok)
                .map(|r| egui::vec2(r.width, r.height) * font / 18.)
                .or_else(|| historical.map(|(w, h)| egui::vec2(w, h) * font / 18.))
                .unwrap_or(egui::vec2(
                    (node.source.len() as f32 * 7.).clamp(30., 180.),
                    font + 8.,
                ));
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
            control(&mut self.controls, format!("node-{index}"), rect);
            visible = ui.is_rect_visible(rect);
            if visible {
                if let Some(r) = result.as_ref().filter(|r| r.ok) {
                    paint(ui, rect, r, id);
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &node.source,
                        egui::FontId::monospace(font),
                        ui.visuals().text_color(),
                    );
                }
            }
            if response
                .on_hover_text(
                    result
                        .as_ref()
                        .filter(|r| !r.ok)
                        .map_or(language.text("Edit formula"), |r| r.error.as_str()),
                )
                .clicked()
            {
                self.open(index);
            }
        } else {
            let mut controls = Vec::new();
            let card_width = ui.available_width().max(120.);
            ui.group(|ui| {
                ui.set_width((card_width - 12.).max(108.));
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    visible = ui.is_rect_visible(egui::Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(ui.available_width(), 300.),
                    ));
                    let view = self.views.entry(index).or_insert_with(|| View {
                        zoom: 1.,
                        ..Default::default()
                    });
                    let source_hash = rich_nodes::hash(&node.source);
                    if view.source_hash != source_hash && view.source_changed.is_none() {
                        view.source = node.source.clone();
                        view.source_hash = source_hash;
                        view.expected = self.stamp;
                    }
                    let mut edit = false;
                    let mut export = None;
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(node.kind);
                        for (mode, label) in [(0, "Preview"), (1, "Source"), (2, "Split")] {
                            let response =
                                ui.selectable_value(&mut view.mode, mode, language.text(label));
                            control(
                                &mut controls,
                                format!("node-{index}-{label}"),
                                response.rect,
                            );
                        }
                        let response = ui.small_button(language.text("Edit"));
                        control(&mut controls, format!("node-{index}-Edit"), response.rect);
                        if response.clicked() {
                            edit = true;
                        }
                        if ui
                            .small_button("↺")
                            .on_hover_text(language.text("Reset canvas"))
                            .clicked()
                        {
                            view.zoom = 1.;
                            view.pan = Vec2::ZERO;
                        }
                        for ext in ["SVG", "PNG"] {
                            let response = ui.add_enabled(
                                result
                                    .as_ref()
                                    .is_some_and(|r| r.ok && (ext == "PNG" || r.svg.is_some())),
                                egui::Button::new(ext).small(),
                            );
                            control(&mut controls, format!("node-{index}-{ext}"), response.rect);
                            if response.clicked() {
                                export = Some(ext);
                            }
                        }
                    });
                    if let Some(message) = &self.message {
                        ui.colored_label(ui.visuals().error_fg_color, message);
                        let response = ui.button(language.text("Discard pending node edits…"));
                        control(&mut controls, "discard-node-drafts", response.rect);
                        if response.clicked() {
                            self.discard_confirmation = true;
                        }
                    }
                    if view.mode == 2 {
                        ui.columns(2, |columns| {
                            let rect = source_editor(&mut columns[0], view, self.stamp, index);
                            control(&mut controls, format!("node-{index}-editor"), rect);
                            visible = canvas(
                                &mut columns[1],
                                view,
                                result.as_deref(),
                                historical.map(|(_, h)| h),
                                id,
                                language,
                                &mut controls,
                                index,
                            );
                        });
                    } else if view.mode == 1 {
                        let rect = source_editor(ui, view, self.stamp, index);
                        control(&mut controls, format!("node-{index}-editor"), rect);
                        visible = false;
                    } else {
                        visible = canvas(
                            ui,
                            view,
                            result.as_deref(),
                            historical.map(|(_, h)| h),
                            id,
                            language,
                            &mut controls,
                            index,
                        );
                    }
                    if view
                        .source_changed
                        .is_some_and(|t| t.elapsed() >= Duration::from_millis(250))
                    {
                        view.source_changed = None;
                        self.inline_edit = Some((view.expected, node.clone(), view.source.clone()));
                        self.recovery_revision = self.recovery_revision.wrapping_add(1);
                    }
                    if let Some(ext) = export
                        && let Some(r) = &result
                    {
                        let bytes = if ext == "SVG" {
                            r.svg.as_ref().map(|s| s.as_bytes().to_vec())
                        } else {
                            Some(r.image.to_vec())
                        };
                        if let Some(bytes) = bytes
                            && let Some(path) = rfd::FileDialog::new()
                                .set_file_name(format!("{}.{}", node.kind, ext.to_lowercase()))
                                .add_filter(ext, &[&ext.to_lowercase()])
                                .save_file()
                            && let Err(e) = crate::document::atomic_write(&path, &bytes)
                        {
                            ui.label(e.to_string());
                        }
                    }
                    if edit {
                        self.open(index);
                    }
                });
            });
            self.controls.extend(controls);
        }
        if visible {
            self.frame_layout_pending |= result.is_none();
            if result.as_ref().is_some_and(|r| r.ok) {
                self.frame_resident.insert(id);
            }
            self.counters.visible_nodes += 1;
            if result.is_some() {
                self.counters.cache_hits += 1;
            } else if !self.pending.contains(&id)
                && self.changed.elapsed() >= Duration::from_millis(250)
            {
                if self.worker.is_none() {
                    self.worker = Some(Worker::new(ui.ctx().clone()));
                }
                if self
                    .worker
                    .as_ref()
                    .unwrap()
                    .tx
                    .as_ref()
                    .unwrap()
                    .try_send(Request {
                        id,
                        kind: node.kind.into(),
                        source: node.source,
                        dark,
                        inline,
                        font: 18.,
                    })
                    .is_ok()
                {
                    self.pending.insert(id);
                    self.counters.submissions += 1;
                } else {
                    ui.ctx().request_repaint_after(Duration::from_millis(50));
                }
            } else if result.is_none() {
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
        } else {
            self.counters.offscreen_nodes += 1;
        }
    }
    fn open(&mut self, index: usize) {
        let node = self.nodes[index].clone();
        self.draft = Some(Draft {
            text: node.source.clone(),
            node,
            index,
            expected: rich_nodes::hash(&self.original),
            changed: None,
            error: None,
            closing: false,
        });
    }
    pub fn edit_window(
        &mut self,
        ctx: &egui::Context,
        language: Language,
    ) -> Option<(u64, Node, String)> {
        if self.discard_confirmation {
            egui::Window::new(crate::theme::window_title(language.text("Discard pending node edits?")))
                .id(egui::Id::new("discard-node-drafts-confirmation"))
                .collapsible(false)
                .resizable(false)
                .default_width(420.)
                .max_size(crate::theme::window_bounds(ctx).size()-egui::vec2(16.,32.))
                .constrain_to(crate::theme::window_bounds(ctx))
                .vscroll(true)
                .show(ctx, |ui| {
                    ui.label(language.text("The document source will be kept. Copy any node drafts you need before discarding."));
                    ui.horizontal_wrapped(|ui| {
                        let cancel = ui.button(language.text("Cancel"));
                        control(&mut self.controls, "cancel-discard-node-drafts", cancel.rect);
                        if cancel.clicked() {
                            self.discard_confirmation = false;
                        }
                        let response = ui.button(language.text("Discard"));
                        control(&mut self.controls, "confirm-discard-node-drafts", response.rect);
                        if response.clicked() {
                            self.discard_pending_edits();
                        }
                    });
                });
            return None;
        }
        if let Some(edit) = self.take_pending(false) {
            return Some(edit);
        }
        if !self.ime_composing && self.inline_edit.is_some() {
            return self.inline_edit.take();
        }
        let draft = self.draft.as_mut()?;
        let mut open = true;
        let mut apply = false;
        let mut discard = false;
        let response = egui::Window::new(crate::theme::window_title(
            language.text("Formula / diagram source"),
        ))
        .id(egui::Id::new("rich-source"))
        .open(&mut open)
        .default_width(540.)
        .min_width(260.)
        .max_size(crate::theme::window_bounds(ctx).size() - egui::vec2(16., 32.))
        .constrain_to(crate::theme::window_bounds(ctx))
        .vscroll(true)
        .show(ctx, |ui| {
            ui.label(language.text("Changes synchronize after 250 ms. Escape or close to return."));
            let response = ui.add(
                egui::TextEdit::multiline(&mut draft.text)
                    .id(egui::Id::new("rich-floating-source"))
                    .code_editor()
                    .desired_rows(if draft.node.inline { 3 } else { 12 })
                    .desired_width(f32::INFINITY),
            );
            control(&mut self.controls, "floating-editor", response.rect);
            if response.changed() {
                draft.changed = Some(Instant::now());
                ctx.request_repaint_after(Duration::from_millis(260));
            }
            if let Some(e) = &draft.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
                let response = ui.button(language.text("Discard pending node edits…"));
                control(&mut self.controls, "discard-node-drafts", response.rect);
                discard = response.clicked();
            }
            if ui.button(language.text("Apply now")).clicked() {
                apply = true;
            }
        });
        if discard {
            self.discard_confirmation = true;
            return None;
        }
        if let Some(response) = &response {
            control(
                &mut self.controls,
                "floating-window",
                response.response.rect,
            );
        }
        if !open
            || response
                .as_ref()
                .is_some_and(|r| crate::theme::window_escape(ctx, r.response.layer_id))
        {
            let d = self.draft.as_mut().unwrap();
            if d.changed.is_some() || d.error.is_some() {
                d.closing = true;
                apply = true;
            } else {
                self.draft = None;
                return None;
            }
        }
        let d = self.draft.as_mut().unwrap();
        if !self.ime_composing
            && (apply
                || d.changed
                    .is_some_and(|t| t.elapsed() >= Duration::from_millis(250)))
        {
            d.changed = None;
            return Some((d.expected, d.node.clone(), d.text.clone()));
        }
        None
    }
    pub fn applied(&mut self, next: &str, error: Option<&str>) {
        self.message = error.map(String::from);
        if error.is_none() {
            self.refresh_now = true;
        }
        if error.is_none() && self.draft.as_ref().is_some_and(|d| d.closing) {
            self.draft = None;
            return;
        }
        if let Some(d) = &mut self.draft {
            if let Some(e) = error {
                d.error = Some(e.into());
            } else {
                let (_, nodes) = rich_nodes::parse(next);
                if let Some(node) = nodes.get(d.index) {
                    d.node = node.clone();
                    d.expected = rich_nodes::hash(next);
                    d.error = None;
                } else {
                    d.error =
                        Some("The edit changed the node delimiters. Reopen this node.".into());
                }
            }
        }
        // A rejected inline patch must not be replaced from the document on the
        // next preview pass. Keep the user's source until it can be synchronized.
        if error.is_some() {
            for (index, view) in &mut self.views {
                if self
                    .nodes
                    .get(*index)
                    .is_some_and(|n| n.source != view.source)
                {
                    view.source_changed = Some(Instant::now());
                }
            }
        }
    }
    pub fn pending_edits(&self) -> bool {
        self.inline_edit.is_some()
            || self
                .draft
                .as_ref()
                .is_some_and(|d| d.changed.is_some() || d.error.is_some())
            || self.views.values().any(|v| v.source_changed.is_some())
    }
    pub fn synchronization_error(&self) -> Option<&str> {
        self.message.as_deref()
    }
    /// Called only by the explicit discard confirmation; authoritative source is untouched.
    pub(crate) fn discard_pending_edits(&mut self) {
        self.recovery_discard_serial = self.recovery_discard_serial.wrapping_add(1);
        self.draft = None;
        self.inline_edit = None;
        self.views.clear();
        self.message = None;
        self.discard_confirmation = false;
        self.refresh_now = true;
    }
    pub fn take_pending(&mut self, force: bool) -> Option<(u64, Node, String)> {
        if self.ime_composing {
            return None;
        }
        if force
            && let Some(d) = self
                .draft
                .as_mut()
                .filter(|d| d.changed.is_some() || d.error.is_some())
        {
            d.changed = None;
            return Some((d.expected, d.node.clone(), d.text.clone()));
        }
        if let Some(edit) = self.inline_edit.take() {
            return Some(edit);
        }
        for (index, view) in &mut self.views {
            if view
                .source_changed
                .is_some_and(|t| force || t.elapsed() >= Duration::from_millis(250))
            {
                view.source_changed = None;
                if let Some(node) = self.nodes.get(*index) {
                    return Some((view.expected, node.clone(), view.source.clone()));
                }
            }
        }
        None
    }
}
fn preview_blocks(text: &str) -> Vec<Block> {
    // Keep documents relying on global link/footnote definitions in one viewer.
    if text.contains("]:") || text.contains("[^") {
        return vec![Block {
            source: 0..text.len(),
            range: 0..text.len(),
            height: 100.,
            nodes: text.matches("mdcard").count() as u64,
            key: rich_nodes::hash(text),
            code: None,
            mapping: Vec::new(),
        }];
    }
    let mut blocks = Vec::new();
    let mut depth = 0;
    for (event, range) in
        pulldown_cmark::Parser::new_ext(text, pulldown_cmark::Options::all()).into_offset_iter()
    {
        let top = match event {
            pulldown_cmark::Event::Start(_) => {
                let top = depth == 0;
                depth += 1;
                top
            }
            pulldown_cmark::Event::End(_) => {
                depth -= 1;
                false
            }
            _ => depth == 0,
        };
        if top {
            blocks.push(Block {
                source: range.clone(),
                nodes: text[range.clone()].matches("mdcard").count() as u64,
                key: rich_nodes::hash(&text[range.clone()]),
                range,
                height: 100.,
                code: None,
                mapping: Vec::new(),
            });
        }
    }
    blocks
}
fn source_editor(ui: &mut egui::Ui, view: &mut View, stamp: u64, index: usize) -> egui::Rect {
    let response = ui.add(
        egui::TextEdit::multiline(&mut view.source)
            .id(egui::Id::new(("rich-node-source", index)))
            .code_editor()
            .desired_rows(10)
            .desired_width(ui.available_width()),
    );
    if response.changed() {
        view.expected = stamp;
        view.source_changed = Some(Instant::now());
        ui.ctx().request_repaint_after(Duration::from_millis(260));
    }
    response.rect
}
#[allow(clippy::too_many_arguments)] // Result and retained geometry have separate lifetimes.
fn canvas(
    ui: &mut egui::Ui,
    view: &mut View,
    result: Option<&Output>,
    height_hint: Option<f32>,
    id: u64,
    language: Language,
    controls: &mut Vec<Control>,
    index: usize,
) -> bool {
    let width = ui.available_width().max(100.);
    let height = result
        .filter(|r| r.ok)
        .map_or(height_hint.unwrap_or(180.).clamp(100., 400.), |r| {
            r.height.clamp(100., 400.)
        });
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());
    control(controls, format!("node-{index}-canvas"), rect);
    let visible = ui.is_rect_visible(rect);
    if visible {
        if response.hovered() {
            let scroll = ui.input_mut(|i| {
                let s = i.raw_scroll_delta.y;
                i.raw_scroll_delta = Vec2::ZERO;
                i.smooth_scroll_delta = Vec2::ZERO;
                s
            });
            if scroll != 0. {
                view.zoom = (view.zoom * (scroll * 0.002).exp()).clamp(0.15, 8.);
            }
        }
        if response.dragged() {
            view.pan += response.drag_delta();
        }
        if let Some(r) = result.filter(|r| r.ok) {
            let fit = (width / r.width).min(1.);
            let size = egui::vec2(r.width, r.height) * fit * view.zoom;
            let image_rect = egui::Rect::from_center_size(rect.center() + view.pan, size);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            child.set_clip_rect(rect.intersect(ui.clip_rect()));
            paint(&mut child, image_rect, r, id);
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                result.map_or(language.text("Rendering…"), |r| r.error.as_str()),
                egui::FontId::proportional(13.),
                ui.visuals().text_color(),
            );
        }
    }
    visible
}
fn paint(ui: &mut egui::Ui, rect: egui::Rect, result: &Output, id: u64) {
    let png = egui::Image::new(egui::ImageSource::Bytes {
        uri: format!("bytes://rich-{id}.png").into(),
        bytes: egui::load::Bytes::Shared(result.image.clone()),
    })
    .fit_to_exact_size(rect.size())
    .show_loading_spinner(false);
    if result.svg.is_some() {
        let svg = egui::Image::new(egui::ImageSource::Bytes {
            uri: format!("bytes://rich-{id}.svg").into(),
            bytes: egui::load::Bytes::Shared(result.vector.clone()),
        })
        .fit_to_exact_size(rect.size())
        .show_loading_spinner(false);
        if matches!(
            svg.load_for_size(ui.ctx(), rect.size()),
            Ok(egui::load::TexturePoll::Ready { .. })
        ) {
            svg.paint_at(ui, rect);
        } else {
            png.paint_at(ui, rect);
        }
    } else {
        png.paint_at(ui, rect);
    }
}

impl RichUi {
    pub fn phase(&mut self, name: &str, ms: f64) {
        if self.input_frame && std::env::var_os("MDCARD_RICH_REPORT").is_some() {
            let samples = self.phase_cpu.entry(name.to_owned()).or_default();
            samples.push(ms);
            if samples.len() > 2000 {
                samples.remove(0);
            }
        }
    }
    pub fn edit_window_open(&self) -> bool {
        self.draft.is_some() || self.discard_confirmation
    }
    pub fn add_control(&mut self, name: impl Into<String>, rect: egui::Rect) {
        control(&mut self.controls, name, rect);
    }
    pub fn report(&mut self, ms: f64) {
        if let Some(path) = std::env::var_os("MDCARD_RICH_REPORT") {
            self.frame_ms.push_back(ms);
            if self.input_frame {
                self.input_cpu.push(ms);
            }
            if self.scroll_frame {
                self.scroll_cpu.push(ms);
            }
            if self.interaction_until.is_some_and(|t| Instant::now() < t) {
                self.interaction_cpu.push(ms);
            }
            if self.input_cpu.len() > 2000 {
                self.input_cpu.remove(0);
            }
            if self.scroll_cpu.len() > 2000 {
                self.scroll_cpu.remove(0);
            }
            if self.interaction_cpu.len() > 4000 {
                self.interaction_cpu.remove(0);
            }
            if self.frame_ms.len() > 512 {
                self.frame_ms.pop_front();
            }
            let views=self.views.iter().map(|(id,v)|serde_json::json!({"index":id,"mode":v.mode,"zoom":v.zoom,"pan":[v.pan.x,v.pan.y]})).collect::<Vec<_>>();
            let report = serde_json::json!({"counters":self.counters,"controls":self.controls,"sourceStamp":self.stamp,"nodes":self.nodes.len(),"cachedResults":self.results.len(),"updateCpuMs":self.frame_ms,"inputEvents":self.input_events,"viewStates":views,"inputCpuMs":self.input_cpu,"phaseCpuMs":self.phase_cpu,"scrollCpuMs":self.scroll_cpu,"interactionCpuMs":self.interaction_cpu});
            let _ = write_diagnostic_report(
                &PathBuf::from(path),
                &serde_json::to_vec(&report).unwrap(),
            );
        }
    }
}
// A live diagnostic snapshot must publish complete JSON, but is not durable
// application state. Keep the synchronous same-directory atomic replacement
// without an fsync; manuscript/export and reading-state writers stay separate.
fn write_diagnostic_report(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(std::io::Error::other(
            "Diagnostic report symlink is unsupported",
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.write_all(bytes)?;
    temporary.persist(path).map_err(|e| e.error)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_report_preserves_complete_unicode_json() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let value = serde_json::json!({"text":"中文 e\u{301} 👩🏽‍💻", "running":true,
            "rect":[12.25, 17.5, 32.0, 33.75], "cancel":{"requested":false,"frame":19}});
        let bytes = serde_json::to_vec(&value).unwrap();
        write_diagnostic_report(&path, &bytes).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap(),
            value
        );
    }
    #[test]
    fn diagnostic_report_atomic_readers_never_observe_partial_json() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let first =
            serde_json::to_vec(&serde_json::json!({"frame":1,"text":"中文".repeat(1024)})).unwrap();
        let second =
            serde_json::to_vec(&serde_json::json!({"frame":2,"text":"e\u{301} 👩🏽‍💻".repeat(512)}))
                .unwrap();
        write_diagnostic_report(&path, &first).unwrap();
        let complete = Arc::new(AtomicBool::new(false));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let reader = {
            let path = path.clone();
            let first = first.clone();
            let second = second.clone();
            let complete = complete.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let mut reads = 0;
                while !complete.load(Ordering::Acquire) || reads < 100 {
                    let bytes = std::fs::read(&path).unwrap();
                    assert!(
                        bytes == first || bytes == second,
                        "Reader observed an incomplete snapshot"
                    );
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
                    reads += 1;
                }
                reads
            })
        };
        barrier.wait();
        let mut published = 0;
        let mut publish_errors = 0;
        for index in 0..100 {
            if write_diagnostic_report(&path, if index % 2 == 0 { &second } else { &first }).is_ok()
            {
                published += 1;
            } else {
                // Windows may deny replacement while a concurrent reader has
                // the old file open. A failed best-effort report must leave a
                // complete prior snapshot, and a later update can retry.
                publish_errors += 1;
                let bytes = std::fs::read(&path).unwrap();
                assert!(bytes == first || bytes == second);
            }
        }
        complete.store(true, Ordering::Release);
        assert!(reader.join().unwrap() >= 100);
        assert!(published > 0);
        write_diagnostic_report(&path, &second).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), second);
        println!(
            "atomic snapshots published={published}, best-effort replace errors={publish_errors}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn diagnostic_report_preserves_permissions_and_refuses_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        std::fs::write(&path, b"{\"old\":true}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        write_diagnostic_report(&path, b"{\"new\":true}").unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let link = directory.path().join("report-link.json");
        symlink(&path, &link).unwrap();
        assert!(write_diagnostic_report(&link, b"{\"bad\":true}").is_err());
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(path).unwrap(), b"{\"new\":true}");
    }
    #[cfg(windows)]
    #[test]
    fn diagnostic_report_failed_replace_keeps_old_snapshot_and_retries() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let old = b"{\"old\":true}";
        let new = b"{\"new\":true}";
        write_diagnostic_report(&path, old).unwrap();
        let guard = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(write_diagnostic_report(&path, new).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        drop(guard);
        write_diagnostic_report(&path, new).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), new);
    }
    #[test]
    fn ordinary_preview_source_anchors_survive_transformed_math_and_unicode() {
        let source = "# 中文 🦀\n\nBefore $\\frac{x}{y}$ after.\n\n```mermaid\ngraph LR\n A --> B\n```\n\nTARGET-段落🙂 with **bold**.\n\nLast.\n";
        let mut rich = RichUi::default();
        let ctx = egui::Context::default();
        rich.ensure(source, 1, &ctx);
        let expected = source.find("TARGET-段落").unwrap();
        let block = rich
            .blocks
            .iter()
            .find(|b| rich.preview[b.range.clone()].starts_with("TARGET-"))
            .unwrap();
        assert_eq!(block.source.start, expected);
        assert_ne!(
            block.range.start, block.source.start,
            "fixture must exercise transformed offsets"
        );
        for block in &rich.blocks {
            assert!(source.is_char_boundary(block.source.start));
            assert!(source.is_char_boundary(block.source.end));
        }
        let _ = rich.visible_blocks(
            420.,
            15.,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420., 400.)),
        );
        let y = rich.reading_y(expected, 0.25, 0.);
        let (byte, fraction, _) = rich.reading_anchor(y);
        assert_eq!(byte, expected);
        assert!((fraction - 0.25).abs() < 0.01);
    }
    #[test]
    fn stale_node_discard_requires_confirmation_and_cancel_retains_its_text() {
        let ctx = egui::Context::default();
        let mut rich = RichUi::default();
        rich.stage_pending_test_edit("$x^2$\n", 0, "z^3", &ctx);
        rich.applied("$outside$\n", Some("The source changed. Reopen this node."));
        let frame = |rich: &mut RichUi, events| {
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    rich.edit_window(ctx, Language::English);
                },
            );
        };
        rich.discard_confirmation = true;
        frame(&mut rich, vec![]);
        frame(&mut rich, vec![]);
        let button = |rich: &RichUi, name: &str| {
            let r = rich
                .controls
                .iter()
                .rev()
                .find(|c| c.name == name)
                .unwrap()
                .rect;
            egui::pos2((r[0] + r[2]) / 2., (r[1] + r[3]) / 2.)
        };
        let click = |pos| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        let cancel = button(&rich, "cancel-discard-node-drafts");
        frame(&mut rich, click(cancel));
        assert!(!rich.discard_confirmation);
        assert_eq!(rich.draft.as_ref().unwrap().text, "z^3");
        assert!(rich.synchronization_error().is_some());
        assert_eq!(rich.original, "$x^2$\n");
        rich.discard_confirmation = true;
        frame(&mut rich, vec![]);
        let confirm = button(&rich, "confirm-discard-node-drafts");
        frame(&mut rich, click(confirm));
        assert!(rich.draft.is_none());
        assert!(rich.synchronization_error().is_none());
        assert_eq!(rich.original, "$x^2$\n");
    }
    #[test]
    fn ipc_identity_preserves_all_64_bits() {
        let request = Request {
            id: u64::MAX,
            kind: "math".into(),
            source: "x".into(),
            dark: false,
            inline: true,
            font: 18.,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"18446744073709551615\""));
        let output: Output =
            serde_json::from_str("{\"ok\":true,\"id\":\"18446744073709551615\"}").unwrap();
        assert_eq!(output.id, u64::MAX);
    }
}

#[cfg(test)]
mod revision_gate_tests {
    use super::*;
    #[test]
    fn buffer_revision_gate_keeps_debounce_and_frame_polling_without_repeated_source_checks() {
        let ctx = egui::Context::default();
        let first = crate::document::Document::from_text("# First\n\nbody");
        let mut rich = RichUi::default();
        rich.ensure_document(&first, 7, &ctx);
        assert_eq!(rich.counters.parses, 1);
        let mut other = crate::document::Document::from_text("# Other\n\nbody");
        other.id = first.id;
        assert_eq!(first.revision(), other.revision());
        assert_ne!(first.buffer_id(), other.buffer_id());
        rich.ensure_document(&other, 7, &ctx);
        assert_eq!(rich.pending_source.as_deref(), Some(other.text.as_str()));
        assert!(
            !rich.source_ready(&other, 7),
            "requested identity is not a parser commit"
        );
        assert!(
            !rich.source_ready(&first, 7),
            "pending content invalidates geometry even for the old buffer"
        );
        let pending_started = rich.changed;
        rich.frame_resident.insert(123);
        rich.ensure_document(&other, 7, &ctx);
        assert_eq!(rich.changed, pending_started);
        assert!(
            rich.frame_resident.is_empty(),
            "same revision still polls async results"
        );
        rich.changed = Instant::now() - Duration::from_secs(1);
        rich.ensure_document(&other, 7, &ctx);
        assert_eq!(rich.original, "# Other\n\nbody");
        assert!(rich.source_ready(&other, 7));
        assert_eq!(rich.counters.parses, 2);
        other.text = "# Third\n\nbody".into();
        other.mark_text_changed();
        rich.ensure_document(&other, 7, &ctx);
        assert!(rich.pending_source.is_some());
        other.text = "# Other\n\nbody".into();
        other.mark_text_changed();
        rich.ensure_document(&other, 7, &ctx);
        assert!(
            rich.pending_source.is_none(),
            "undo to parsed text cancels pending obsolete source"
        );
        assert!(rich.source_ready(&other, 7));
        assert_eq!(rich.counters.parses, 2);
    }
}
