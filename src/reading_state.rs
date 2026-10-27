// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Local reading positions, including bounded context fragments. This store never writes manuscript files.
use crate::{
    document::Document,
    egui,
    reading_geometry::{GeometryCache, Glyph, MappedGalley},
    reading_precision,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SCHEMA: u32 = 3;
const MAX_RECORDS: usize = 512;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_UTC_TIMESTAMP_MS: u64 = 253_402_300_799_999; // 9999-12-31T23:59:59.999Z
const MAX_FUTURE_SKEW_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_CLEAR_SERIAL: u64 = u32::MAX as u64;
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn hash(value: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub u64);
impl Default for DocumentId {
    fn default() -> Self {
        Self(hash((
            now(),
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        )))
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
fn event_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u64::MAX as u128) as u64
}
pub fn path_key(path: &Path) -> String {
    let absolute = path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    let mut clean = PathBuf::new();
    for c in absolute.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    let value = clean.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.trim_start_matches("//?/").to_lowercase()
    } else {
        value
    }
}
pub fn state_path() -> PathBuf {
    if let Some(path) = std::env::var_os("MDCARD_READING_STATE_PATH") {
        return path.into();
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
    base.unwrap_or_else(std::env::temp_dir)
        .join("MDcard")
        // Older released writers cannot understand schema 3 and would discard
        // unknown metadata. Keep their default path physically independent.
        .join("reading-state-v3.json")
}
pub fn byte_floor(text: &str, byte: usize) -> usize {
    let mut b = byte.min(text.len());
    while !text.is_char_boundary(b) {
        b -= 1;
    }
    b
}
pub fn char_to_byte(text: &str, character: usize) -> usize {
    text.char_indices()
        .nth(character)
        .map_or(text.len(), |(b, _)| b)
}
pub fn byte_to_char(text: &str, byte: usize) -> usize {
    text[..byte_floor(text, byte)].chars().count()
}
pub fn cursor(selection: [usize; 2]) -> egui::text::CCursorRange {
    egui::text::CCursorRange {
        primary: egui::text::CCursor::new(selection[0]),
        secondary: egui::text::CCursor::new(selection[1]),
        h_pos: None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Anchor {
    pub byte: usize,
    pub fraction: f32,
    pub viewport_y: f32,
    pub context: String,
    pub before: String,
    pub heading: Vec<String>,
    pub approximate: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextAnchor {
    pub source: reading_precision::Anchor,
    pub viewport_y: f32,
    pub token_range: std::ops::Range<usize>,
    pub logical_line: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Position {
    pub offset: [f32; 2],
    pub viewport: [f32; 2],
    pub anchor: Anchor,
    pub text_anchor: Option<TextAnchor>,
    pub layout: String,
    pub fingerprint: u64,
    pub updated_ms: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RestoreReason {
    Found,
    MissingOrAmbiguous,
    NoCorrespondence,
    NoValidPosition,
    LayoutUnavailable,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestoreStatus {
    pub tier: reading_precision::Precision,
    reason: RestoreReason,
    updated_ms: u64,
}
impl RestoreStatus {
    pub fn tier_label(&self) -> &'static str {
        match self.tier {
            reading_precision::Precision::Text => "Text correspondence",
            reading_precision::Precision::SourceLine => "Source logical line",
            reading_precision::Precision::Block => "Content block",
            reading_precision::Precision::Section => "Section",
            reading_precision::Precision::Approximate => "Approximate position",
            reading_precision::Precision::Invalid => "No valid position",
        }
    }
    pub fn reason_label(&self) -> &'static str {
        match self.reason {
            RestoreReason::Found => "Original text found.",
            RestoreReason::MissingOrAmbiguous => {
                "The original text was missing or ambiguous; a safer position was used."
            }
            RestoreReason::NoCorrespondence => {
                "No reliable match; an approximate position was used."
            }
            RestoreReason::NoValidPosition => "No valid saved position; started at the top.",
            RestoreReason::LayoutUnavailable => {
                "The saved text could not be shown reliably; the previous position was kept."
            }
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    pub id: DocumentId,
    pub path: Option<String>,
    pub views: BTreeMap<String, Position>,
    pub selections: BTreeMap<String, [usize; 2]>,
    pub last_view: String,
    pub mode: String,
    pub calibration: BTreeMap<String, HeightCalibration>,
    pub furthest: f64,
    pub read: bool,
    pub updated_ms: u64,
    pub activity_ms: u64,
    /// Logical clear barrier, independent of sortable metadata wall time.
    pub clear_serial: u64,
    pub cleared_ns: u64,
    pub activity_ns: u64,
    pub deleted: bool,
    pub restore_status: BTreeMap<String, RestoreStatus>,
}
impl Default for Record {
    fn default() -> Self {
        Self {
            id: DocumentId::default(),
            path: None,
            views: BTreeMap::new(),
            selections: BTreeMap::new(),
            last_view: String::new(),
            mode: String::new(),
            calibration: BTreeMap::new(),
            furthest: 0.,
            read: false,
            updated_ms: 0,
            activity_ms: 0,
            clear_serial: 0,
            cleared_ns: 0,
            activity_ns: 0,
            deleted: false,
            restore_status: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HeightCalibration {
    layout: String,
    fingerprint: u64,
    heights: Vec<(u64, f32)>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Disk {
    version: u32,
    cleared_ms: u64,
    clear_serial: u64,
    cleared_serial: u64,
    cleared_ns: u64,
    latest_clear_ns: u64,
    records: BTreeMap<String, Record>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Restore {
    pub position: Position,
    pub tier: String,
    pub exact: bool,
}
#[derive(Default)]
struct Runtime {
    id: Option<DocumentId>,
    buffer_id: u64,
    revision: u64,
    generation: u64,
    pending: Option<Restore>,
    last: Option<Position>,
    target: Option<[f32; 2]>,
    precise_target: Option<[f32; 2]>,
    stable: u8,
    passes: u16,
    reason: String,
    selection_pending: bool,
    viewport_rect: Option<[f32; 4]>,
    calibrated: bool,
    calibration_stamp: u64,
    suspended: bool,
    layer: Option<egui::LayerId>,
    pinned: Option<TextAnchor>,
    geometry: Option<Glyph>,
    visible_text: Vec<Glyph>,
    visible_runs: Vec<crate::reading_geometry::VisibleText>,
    recapture: bool,
    compensations: u64,
    presented_offset: Option<[f32; 2]>,
    presented_max: [f32; 2],
    layout_pending: bool,
    asset_errors: Vec<String>,
    range_clamp: Option<String>,
    restore_started: Option<Instant>,
    ready_started: Option<Instant>,
    ready_passes: u16,
    failed_restore: Option<serde_json::Value>,
    cancel_events: std::collections::VecDeque<serde_json::Value>,
    precision_epoch: u64,
    proof_attempt: Option<(u64, usize)>,
    proof_job: Option<JobControl>,
    relocation_job: Option<JobControl>,
    relocation: Option<serde_json::Value>,
    block_relocation: bool,
}
impl Runtime {
    fn cancel_precision(&mut self) {
        for job in [&mut self.proof_job, &mut self.relocation_job] {
            if let Some(control) = job.take() {
                control.cancel();
            }
        }
        self.precision_epoch = self.precision_epoch.wrapping_add(1);
        self.proof_attempt = None;
    }
}
struct SourceIndex {
    revision: u64,
    fingerprint: u64,
    headings: Vec<(usize, usize, String)>,
    paths: Vec<Vec<String>>,
    characters: Option<Vec<usize>>,
    line_starts: std::sync::Arc<Vec<usize>>,
    snapshot: Option<std::sync::Arc<String>>,
}
impl SourceIndex {
    fn new(doc: &Document) -> Self {
        Self::from_text_cancelable(doc.revision(), doc.text.clone(), &|| false).unwrap()
    }
    fn from_text_cancelable(
        revision: u64,
        text: String,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<Self> {
        if cancelled() {
            return None;
        }
        // Associate the outline with source boundaries once per content revision.
        let mut mapped = Vec::new();
        let mut heading = None;
        let mut title = String::new();
        for (event, range) in pulldown_cmark::Parser::new(&text).into_offset_iter() {
            if cancelled() {
                return None;
            }
            match event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading { level, .. }) => {
                    heading = Some((range.start, level as usize));
                    title.clear();
                }
                pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t)
                    if heading.is_some() =>
                {
                    title.push_str(&t)
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                    if let Some((at, level)) = heading.take() {
                        mapped.push((at, level, title.clone()));
                    }
                }
                _ => {}
            }
        }
        let mut path: Vec<(usize, String)> = Vec::new();
        let mut paths = Vec::new();
        for (_, level, title) in &mapped {
            if cancelled() {
                return None;
            }
            while path.last().is_some_and(|(l, _)| l >= level) {
                path.pop();
            }
            path.push((*level, title.clone()));
            paths.push(path.iter().map(|(_, t)| t.clone()).collect());
        }
        let fingerprint = hash(&text);
        if cancelled() {
            return None;
        }
        let mut lines = vec![0];
        for (i, byte) in text.bytes().enumerate() {
            if i % 4096 == 0 && cancelled() {
                return None;
            }
            if byte == b'\n' {
                lines.push(i + 1);
            }
        }
        let characters = (text.len() <= 64 * 1024).then(|| {
            text.char_indices()
                .map(|(b, _)| b)
                .chain(std::iter::once(text.len()))
                .collect()
        });
        if cancelled() {
            return None;
        }
        Some(Self {
            revision,
            fingerprint,
            line_starts: std::sync::Arc::new(lines),
            headings: mapped,
            paths,
            characters,
            snapshot: Some(std::sync::Arc::new(text)),
        })
    }
    fn heading(&self, byte: usize) -> Vec<String> {
        let end = self.headings.partition_point(|(at, _, _)| *at <= byte);
        end.checked_sub(1)
            .and_then(|i| self.paths.get(i))
            .cloned()
            .unwrap_or_default()
    }
    fn anchor(&self, text: &str, byte: usize, fraction: f32, viewport_y: f32) -> Anchor {
        let byte = byte_floor(text, byte);
        let start = text[..byte]
            .char_indices()
            .rev()
            .nth(63)
            .map_or(0, |(b, _)| b);
        let end = byte
            + text[byte..]
                .char_indices()
                .nth(96)
                .map_or(text.len() - byte, |(b, _)| b);
        Anchor {
            byte,
            fraction: fraction.clamp(0., 1.),
            viewport_y,
            context: text[byte..end].to_owned(),
            before: text[start..byte].to_owned(),
            heading: self.heading(byte),
            approximate: byte as f64 / text.len().max(1) as f64,
        }
    }
    fn locate(&self, text: &str, old: &Anchor) -> (usize, &'static str) {
        if text.is_empty() {
            return (0, "empty");
        }
        if !old.context.is_empty() {
            let mut matches = Vec::new();
            for (byte, _) in text.match_indices(&old.context).take(258) {
                let before = text[..byte].ends_with(&old.before);
                let heading = self.heading(byte) == old.heading;
                matches.push((byte, usize::from(before) * 2 + usize::from(heading)));
            }
            // The extra candidate is a truncation sentinel. A unique best score
            // within a prefix is not proof of uniqueness in the document.
            if matches.len() == 1 {
                return (matches[0].0, "context");
            }
            if matches.len() <= 257
                && let Some(best) = matches.iter().max_by_key(|(_, score)| score)
                && best.1 > 0
                && matches.iter().filter(|(_, score)| *score == best.1).count() == 1
            {
                return (best.0, "context-and-heading");
            }
        }
        if !old.heading.is_empty() {
            let matches = self
                .headings
                .iter()
                .filter(|(b, _, _)| self.heading(*b) == old.heading)
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                return (matches[0].0, "nearby-heading");
            }
        }
        (
            byte_floor(
                text,
                (old.approximate.clamp(0., 1.) * text.len() as f64) as usize,
            ),
            "approximate",
        )
    }
}

#[derive(Default)]
struct PendingWrite {
    records: BTreeMap<String, Record>,
    cleared_ms: u64,
    protected: Vec<String>,
    clears: Vec<ClearRequest>,
    activities: BTreeMap<String, u64>,
}
#[derive(Clone)]
struct ClearRequest {
    path: Option<String>,
    requested_ns: u64,
    // Internal filesystem rename intent, applied with its old-path tombstone
    // under the same lock. It is never an invented reading activity.
    rename_to: Option<String>,
}
type IndexKey = (DocumentId, u64);
static TASK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn task_ns() -> u64 {
    static ORIGIN: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(Instant::now)
        .elapsed()
        .as_nanos()
        .min(u64::MAX as u128) as u64
        + 1
}
#[derive(Clone)]
struct JobControl {
    id: u64,
    kind: &'static str,
    metadata: serde_json::Value,
    queued_ns: u64,
    started_ns: std::sync::Arc<std::sync::atomic::AtomicU64>,
    finished_ns: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cancelled_ns: std::sync::Arc<std::sync::atomic::AtomicU64>,
    progress: std::sync::Arc<std::sync::atomic::AtomicU64>,
    phase: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cancelled_phase: std::sync::Arc<std::sync::atomic::AtomicU64>,
    cancelled_progress: std::sync::Arc<std::sync::atomic::AtomicU64>,
    repaint: Option<egui::Context>,
}
impl JobControl {
    fn new(kind: &'static str, metadata: serde_json::Value) -> Self {
        use std::sync::{Arc, atomic::AtomicU64};
        Self {
            id: TASK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            kind,
            metadata,
            queued_ns: task_ns(),
            started_ns: Arc::new(AtomicU64::new(0)),
            finished_ns: Arc::new(AtomicU64::new(0)),
            cancelled_ns: Arc::new(AtomicU64::new(0)),
            progress: Arc::new(AtomicU64::new(0)),
            phase: Arc::new(AtomicU64::new(0)),
            cancelled_phase: Arc::new(AtomicU64::new(0)),
            cancelled_progress: Arc::new(AtomicU64::new(0)),
            repaint: None,
        }
    }
    fn with_repaint(mut self, context: Option<&egui::Context>) -> Self {
        self.repaint = context.cloned();
        self
    }
    fn cancel(&self) {
        use std::sync::atomic::Ordering;
        if self.finished_ns.load(Ordering::Acquire) == 0
            && self
                .cancelled_ns
                .compare_exchange(0, task_ns(), Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            self.cancelled_phase
                .store(self.phase.load(Ordering::Acquire), Ordering::Release);
            self.cancelled_progress
                .store(self.progress.load(Ordering::Acquire), Ordering::Release);
        }
    }
    fn cancelled(&self) -> bool {
        self.progress
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.cancelled_ns.load(std::sync::atomic::Ordering::Acquire) != 0
    }
    fn start(&self) {
        self.started_ns
            .store(task_ns(), std::sync::atomic::Ordering::Release);
    }
    fn finish(&self) {
        self.finished_ns
            .store(task_ns(), std::sync::atomic::Ordering::Release);
        // A switch can remove this job from the active polling set. Its
        // completion must still wake the native event loop, including when it
        // was cancelled. The result is queued before this notification.
        if let Some(context) = &self.repaint {
            context.request_repaint();
        }
    }
    fn report(&self) -> serde_json::Value {
        use std::sync::atomic::Ordering;
        let start = self.started_ns.load(Ordering::Acquire);
        let finish = self.finished_ns.load(Ordering::Acquire);
        let cancel = self.cancelled_ns.load(Ordering::Acquire);
        let phase_name = |phase| match phase {
            1 => "source-index",
            2 => "old-anchor-proof",
            3 => "relocation-match",
            4 => "new-anchor-proof",
            _ => "queued",
        };
        serde_json::json!({"task_id":self.id,"kind":self.kind,"metadata":self.metadata,"phase":phase_name(self.phase.load(Ordering::Acquire)),"cancelled_phase":phase_name(self.cancelled_phase.load(Ordering::Acquire)),"progress_checkpoints":self.progress.load(Ordering::Acquire),"progress_checkpoints_at_cancel":self.cancelled_progress.load(Ordering::Acquire),"queued_monotonic_ns":self.queued_ns,"started_monotonic_ns":start,"finished_monotonic_ns":finish,"cancel_requested_monotonic_ns":cancel,"running":start>0&&finish==0,"cancel_requested_while_running":cancel>=start&&start>0&&(finish==0||cancel<=finish),"queue_wait_wall_ms":(start>0).then(||start.saturating_sub(self.queued_ns) as f64/1e6),"computation_wall_ms":(finish>0&&start>0).then(||finish.saturating_sub(start) as f64/1e6),"cancel_to_worker_exit_wall_ms":(cancel>0&&finish>=cancel).then(||(finish-cancel)as f64/1e6)})
    }
}
struct IndexJob {
    key: IndexKey,
    epoch: u64,
    revision: u64,
    text: String,
    control: JobControl,
}
struct IndexResult {
    key: IndexKey,
    epoch: u64,
    index: Option<SourceIndex>,
    ms: f64,
}
struct IndexWorker {
    tx: Option<mpsc::SyncSender<IndexJob>>,
    rx: mpsc::Receiver<IndexResult>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl IndexWorker {
    fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<IndexJob>(1);
        let (results, rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("reading-index".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let started = Instant::now();
                    job.control.start();
                    job.control
                        .phase
                        .store(1, std::sync::atomic::Ordering::Release);
                    let index = SourceIndex::from_text_cancelable(job.revision, job.text, &|| {
                        job.control.cancelled()
                    });
                    let _ = results.send(IndexResult {
                        key: job.key,
                        epoch: job.epoch,
                        index,
                        ms: started.elapsed().as_secs_f64() * 1000.,
                    });
                    job.control.finish();
                }
            })
            .expect("reading index thread");
        Self {
            tx: Some(tx),
            rx,
            join: Some(join),
        }
    }
}
impl Drop for IndexWorker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
#[derive(Clone)]
struct PrecisionIdentity {
    view: String,
    document_id: DocumentId,
    buffer_id: u64,
    revision: u64,
    open_generation: u64,
    restore_epoch: u64,
    clear_epoch: u64,
}
impl PrecisionIdentity {
    fn report(&self) -> serde_json::Value {
        serde_json::json!({"view":self.view,"document_id":self.document_id,"buffer_id":self.buffer_id,"revision":self.revision,"open_generation":self.open_generation,"restore_epoch":self.restore_epoch,"clear_epoch":self.clear_epoch})
    }
}
enum PrecisionWork {
    Enrich(reading_precision::Anchor),
    Relocate(reading_precision::Anchor),
}
struct PrecisionJob {
    identity: PrecisionIdentity,
    text: std::sync::Arc<String>,
    fingerprint: u64,
    work: PrecisionWork,
    control: JobControl,
}
enum PrecisionOutput {
    Enriched(reading_precision::Anchor),
    Relocated {
        decision: reading_precision::Resolved,
        anchor: reading_precision::Anchor,
    },
    Cancelled,
}
struct PrecisionResult {
    identity: PrecisionIdentity,
    control: JobControl,
    output: PrecisionOutput,
    match_ms: f64,
    enrich_ms: f64,
}
struct PrecisionWorker {
    tx: Option<mpsc::SyncSender<PrecisionJob>>,
    rx: mpsc::Receiver<PrecisionResult>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl PrecisionWorker {
    fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<PrecisionJob>(4);
        let (results, rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("reading-precision".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    job.control.start();
                    let mut match_ms = 0.;
                    let mut enrich_ms = 0.;
                    let output = match job.work {
                        PrecisionWork::Enrich(anchor) => {
                            job.control
                                .phase
                                .store(2, std::sync::atomic::Ordering::Release);
                            let started = Instant::now();
                            let result = reading_precision::enrich_cancelable(
                                &job.text,
                                anchor.source_byte,
                                job.fingerprint,
                                job.control.cancelled_ns.clone(),
                                job.control.progress.clone(),
                            );
                            enrich_ms = started.elapsed().as_secs_f64() * 1000.;
                            result.map_or(PrecisionOutput::Cancelled, PrecisionOutput::Enriched)
                        }
                        PrecisionWork::Relocate(old) => {
                            job.control
                                .phase
                                .store(3, std::sync::atomic::Ordering::Release);
                            let started = Instant::now();
                            let decision = reading_precision::resolve_cancelable(
                                &job.text,
                                &old,
                                job.control.cancelled_ns.clone(),
                                job.control.progress.clone(),
                            );
                            match_ms = started.elapsed().as_secs_f64() * 1000.;
                            if let Some(decision) = decision {
                                job.control
                                    .phase
                                    .store(4, std::sync::atomic::Ordering::Release);
                                let started = Instant::now();
                                let anchor = if decision.exact {
                                    reading_precision::enrich_cancelable(
                                        &job.text,
                                        decision.source_byte,
                                        job.fingerprint,
                                        job.control.cancelled_ns.clone(),
                                        job.control.progress.clone(),
                                    )
                                } else {
                                    Some(reading_precision::capture_with_fingerprint(
                                        &job.text,
                                        decision.source_byte,
                                        job.fingerprint,
                                    ))
                                };
                                enrich_ms = started.elapsed().as_secs_f64() * 1000.;
                                anchor.map_or(PrecisionOutput::Cancelled, |anchor| {
                                    PrecisionOutput::Relocated { decision, anchor }
                                })
                            } else {
                                PrecisionOutput::Cancelled
                            }
                        }
                    };
                    drop(job.text);
                    let control = job.control;
                    let _ = results.send(PrecisionResult {
                        identity: job.identity,
                        control: control.clone(),
                        output,
                        match_ms,
                        enrich_ms,
                    });
                    control.finish();
                }
            })
            .expect("reading precision thread");
        Self {
            tx: Some(tx),
            rx,
            join: Some(join),
        }
    }
}
impl Drop for PrecisionWorker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
#[derive(Clone, Default, Serialize)]
pub struct WriteStats {
    pub writes: u64,
    pub last_success_ms: u64,
    pub last_duration_ms: f64,
    pub error: Option<String>,
}
struct Writer {
    tx: mpsc::Sender<Option<PendingWrite>>,
    rx: mpsc::Receiver<Result<WriteStats, (String, Vec<ClearRequest>)>>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl Writer {
    fn new(path: PathBuf) -> Self {
        let (tx, requests) = mpsc::channel::<Option<PendingWrite>>();
        let (results, rx) = mpsc::channel();
        let join = std::thread::spawn(move || {
            let mut stats = WriteStats::default();
            // UI debounce already coalesces scrolling. Preserve transaction
            // order here: an old deletion snapshot is never another clear.
            while let Ok(Some(job)) = requests.recv() {
                let retry_clears = job.clears.clone();
                let started = Instant::now();
                let result = write_merged(&path, job);
                let output = match result {
                    Ok(()) => {
                        stats.writes += 1;
                        stats.last_success_ms = now();
                        stats.last_duration_ms = started.elapsed().as_secs_f64() * 1000.;
                        Ok(stats.clone())
                    }
                    Err(e) => Err((e.to_string(), retry_clears)),
                };
                let _ = results.send(output);
            }
        });
        Self {
            tx,
            rx,
            join: Some(join),
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.tx.send(None);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
fn validate_timestamps(disk: &Disk) -> io::Result<()> {
    let maximum_timestamp = now()
        .saturating_add(MAX_FUTURE_SKEW_MS)
        .min(MAX_UTC_TIMESTAMP_MS);
    if disk.cleared_ms > maximum_timestamp
        || disk.cleared_ns / 1_000_000 > maximum_timestamp
        || disk.latest_clear_ns / 1_000_000 > maximum_timestamp
        || disk.clear_serial > MAX_CLEAR_SERIAL
        || disk.cleared_serial > disk.clear_serial
        || disk.records.values().any(|r| {
            r.updated_ms > maximum_timestamp
                || r.activity_ms > maximum_timestamp
                || r.activity_ns / 1_000_000 > maximum_timestamp
                || r.cleared_ns / 1_000_000 > maximum_timestamp
                || r.clear_serial > disk.clear_serial
                || r.views.values().any(|p| p.updated_ms > maximum_timestamp)
                || r.restore_status
                    .values()
                    .any(|s| s.updated_ms > maximum_timestamp)
        })
    {
        return Err(io::Error::other(
            "Invalid reading-state timestamp; original state was preserved",
        ));
    }
    Ok(())
}
fn load(path: &Path) -> io::Result<Disk> {
    let legacy = (std::env::var_os("MDCARD_READING_STATE_PATH").is_none() && path == state_path())
        .then(|| path.with_file_name("reading-state.json"));
    load_with_legacy(path, legacy.as_deref())
}
fn load_with_legacy(path: &Path, legacy: Option<&Path>) -> io::Result<Disk> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(legacy) = legacy {
                // Read only: the first successful new-path transaction writes
                // the migrated records without touching the older runtime.
                return load_with_legacy(legacy, None);
            }
            return Ok(Disk {
                version: SCHEMA,
                ..Disk::default()
            });
        }
        Err(e) => return Err(e),
    };
    if metadata.len() > MAX_BYTES {
        return Err(io::Error::other("Reading state exceeds the 4 MiB limit"));
    }
    let mut disk: Disk = serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)?;
    if disk.version != SCHEMA && disk.version != 1 && disk.version != 2 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Unsupported reading-state version; positions start at the top",
        ));
    }
    disk.version = SCHEMA;
    validate_timestamps(&disk)?;
    // Older schema-3 snapshots did not retain a clock after a single-file
    // tombstone was evicted. Recover any surviving explicit clear evidence;
    // zero remains an unknown barrier, never permission for an older snapshot.
    disk.latest_clear_ns = disk.latest_clear_ns.max(disk.cleared_ns).max(
        disk.records
            .values()
            .map(|r| r.cleared_ns)
            .max()
            .unwrap_or(0),
    );
    for record in disk.records.values_mut() {
        if record.activity_ms == 0 {
            record.activity_ms = record.updated_ms;
        }
        if record.restore_status.len() > 4
            || record.restore_status.keys().any(|k| {
                !matches!(
                    k.as_str(),
                    "main/source" | "main/preview" | "secondary/source" | "secondary/preview"
                )
            })
        {
            return Err(io::Error::other("Invalid reading restore summary"));
        }
        if record.calibration.values().any(|c| {
            c.heights.len() > 10000
                || c.heights
                    .iter()
                    .any(|(_, h)| !h.is_finite() || *h <= 0. || *h > 10_000_000.)
        }) {
            return Err(io::Error::other("Invalid reading height calibration"));
        }
        for p in record.views.values() {
            if !p
                .offset
                .iter()
                .chain(p.viewport.iter())
                .all(|v| v.is_finite() && *v >= 0. && *v <= 1_000_000_000.)
                || !p.anchor.fraction.is_finite()
                || !p.anchor.approximate.is_finite()
                || p.anchor.context.len() > 4096
                || p.anchor.before.len() > 4096
                || p.text_anchor.as_ref().is_some_and(|t| {
                    !t.viewport_y.is_finite()
                        || t.viewport_y.abs() > 1_000_000_000.
                        || t.source.before.len() > 4096
                        || t.source.after.len() > 4096
                        || t.source.grapheme.len() > 1024
                        || t.source.grapheme_range.start > t.source.grapheme_range.end
                        || t.source.grapheme_range.end > t.source.source_len
                        || t.source.source_byte != t.source.grapheme_range.start
                        || t.source.grapheme.is_empty()
                        || t.source.grapheme_range.len() != t.source.grapheme.len()
                        || !t.source.valid()
                        || t.token_range.start > t.token_range.end
                        || t.token_range.end > t.source.source_len
                })
            {
                return Err(io::Error::other("Invalid reading-state coordinates"));
            }
        }
    }
    while disk.records.len() > MAX_RECORDS {
        let key = disk
            .records
            .iter()
            .min_by_key(|(_, r)| r.updated_ms)
            .map(|(k, _)| k.clone())
            .unwrap();
        disk.records.remove(&key);
    }
    Ok(disk)
}
#[cfg(unix)]
fn try_lock(file: &fs::File) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    if unsafe { flock(file.as_raw_fd(), 2 | 4) } == 0 {
        Ok(true)
    } else {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            Ok(false)
        } else {
            Err(error)
        }
    }
}
#[cfg(windows)]
fn try_lock(file: &fs::File) -> io::Result<bool> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut c_void,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LockFileEx(
            file: *mut c_void,
            flags: u32,
            reserved: u32,
            low: u32,
            high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        event: std::ptr::null_mut(),
    };
    if unsafe { LockFileEx(file.as_raw_handle(), 3, 0, 1, 0, &mut overlapped) } != 0 {
        Ok(true)
    } else {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(33) {
            Ok(false)
        } else {
            Err(error)
        }
    }
}
fn write_merged(path: &Path, mut job: PendingWrite) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let lock_path = path.with_extension("json.lock");
    let started = Instant::now();
    // The inode is stable and never unlinked. OS locks are released by process
    // termination; a slow writer cannot have its live lock stolen by a timeout.
    let _lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    while !try_lock(&_lock)? {
        if started.elapsed() > Duration::from_secs(2) {
            return Err(io::Error::other(
                "Reading-state lock is busy; positions remain in memory",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Existing unreadable/unsupported state must remain byte-for-byte intact.
    // Only a genuinely missing file (handled by load) authorizes an empty one.
    let mut disk = load(path)?;
    // Missing-record fences from earlier transactions remain authoritative.
    // A single-file clear in this same job must not reject an unrelated new
    // record merely because its first persistence is queued alongside it.
    let entering_clear_serial = disk.clear_serial;
    let entering_clear_ns = disk.latest_clear_ns;
    for request in &job.clears {
        disk.clear_serial = disk
            .clear_serial
            .checked_add(1)
            .filter(|serial| *serial <= MAX_CLEAR_SERIAL)
            .ok_or_else(|| io::Error::other("Reading-state clear sequence exhausted"))?;
        let serial = disk.clear_serial;
        if let Some(next) = &request.rename_to
            && let Some(incoming) = job.records.get_mut(next)
        {
            let previous = request.path.as_ref().and_then(|key| disk.records.get(key));
            let barrier = previous.map_or(entering_clear_serial.max(disk.cleared_serial), |r| {
                r.clear_serial.max(disk.cleared_serial)
            });
            let missing_clear_ns = if disk.cleared_serial > entering_clear_serial {
                disk.cleared_ns
            } else {
                entering_clear_ns
            };
            let cleared_ns = previous.map_or(missing_clear_ns, |r| {
                if r.clear_serial >= disk.cleared_serial {
                    r.cleared_ns
                } else {
                    disk.cleared_ns
                }
            });
            let activity = job.activities.get(next).copied().unwrap_or(0);
            let source_survived = previous.is_none_or(|r| r.id == incoming.id && !r.deleted)
                && (incoming.clear_serial >= barrier || activity > cleared_ns);
            if !source_survived {
                // A genuine clear that committed before this rename must not
                // be undone by its stale in-memory history. The renamed file
                // still keeps its stable identity, with no old view position.
                empty_record(incoming);
                incoming.deleted = false;
            }
            incoming.clear_serial = serial;
            incoming.cleared_ns = request.requested_ns;
        }
        disk.latest_clear_ns = disk.latest_clear_ns.max(request.requested_ns);
        let committed_ns = event_ns();
        if request.path.is_none() {
            disk.cleared_serial = serial;
            disk.cleared_ns = request.requested_ns;
            disk.cleared_ms = disk.cleared_ms.max(request.requested_ns / 1_000_000);
        }
        for (key, record) in &mut disk.records {
            if request.path.as_ref().is_some_and(|path| path != key) {
                continue;
            }
            // A real activity already committed after this clear was requested
            // survives even if the clear waited for the OS lock. Future-loaded
            // wall-clock metadata is not evidence of such an intervening input.
            let intervening_activity =
                record.activity_ns > request.requested_ns && record.activity_ns <= committed_ns;
            if !intervening_activity {
                empty_record(record);
                record.updated_ms = record.updated_ms.saturating_add(1).max(now());
                record.activity_ns = 0;
            }
            record.clear_serial = serial;
            record.cleared_ns = request.requested_ns;
        }
        if let Some(key) = &request.path
            && !disk.records.contains_key(key)
            && let Some(source) = job.records.get(key)
        {
            let mut record = source.clone();
            empty_record(&mut record);
            record.clear_serial = serial;
            record.cleared_ns = request.requested_ns;
            record.activity_ns = 0;
            disk.records.insert(key.clone(), record);
        }
    }
    disk.cleared_ms = disk.cleared_ms.max(job.cleared_ms);
    if disk.cleared_serial == 0 {
        disk.records.retain(|_, r| r.updated_ms > disk.cleared_ms);
    }
    let protected = job.protected;
    for (key, mut record) in job.records {
        // Only the explicit transaction above may delete. Replayed tombstones
        // cannot erase an activity that followed their original clear.
        if record.deleted {
            continue;
        }
        let old = disk.records.get(&key);
        // A missing key may be an evicted single-file tombstone. The bounded
        // global fence survives that eviction; existing unrelated records keep
        // their own per-document/all-clear barrier.
        let serial = old.map_or(entering_clear_serial.max(disk.cleared_serial), |r| {
            r.clear_serial.max(disk.cleared_serial)
        });
        let missing_clear_ns = if disk.cleared_serial > entering_clear_serial {
            disk.cleared_ns
        } else {
            entering_clear_ns
        };
        let cleared_ns = old.map_or(missing_clear_ns, |r| {
            if r.clear_serial >= disk.cleared_serial {
                r.cleared_ns
            } else {
                disk.cleared_ns
            }
        });
        let fresh_activity = job.activities.get(&key).copied().unwrap_or(0);
        if fresh_activity > 0 {
            record.activity_ns = fresh_activity;
        }
        if serial > 0 {
            if record.clear_serial < serial && (cleared_ns == 0 || fresh_activity <= cleared_ns) {
                continue;
            }
            let fresh_after_clear = fresh_activity > cleared_ns;
            if old.is_some_and(|old| {
                if fresh_after_clear {
                    fresh_activity < old.activity_ns
                } else {
                    old.deleted || record.updated_ms < old.updated_ms
                }
            }) {
                continue;
            }
            record.clear_serial = serial;
            record.cleared_ns = cleared_ns;
            record.updated_ms = old.map_or(record.updated_ms, |old| {
                record.updated_ms.max(old.updated_ms.saturating_add(1))
            });
            disk.records.insert(key, record);
            continue;
        }
        let activity = if record.activity_ms == 0 {
            record.updated_ms
        } else {
            record.activity_ms
        };
        if record.updated_ms > disk.cleared_ms
            && (record.deleted || activity > disk.cleared_ms)
            && disk.records.get(&key).is_none_or(|old| {
                record.updated_ms >= old.updated_ms
                    && (record.deleted || !old.deleted || activity > old.updated_ms)
            })
        {
            disk.records.insert(key, record);
        }
    }
    validate_timestamps(&disk)?;
    while disk.records.len() > MAX_RECORDS {
        let oldest = disk
            .records
            .iter()
            .filter(|(k, _)| !protected.contains(k))
            .min_by_key(|(_, r)| r.updated_ms)
            .map(|(k, _)| k.clone())
            .or_else(|| {
                disk.records
                    .iter()
                    .min_by_key(|(_, r)| r.updated_ms)
                    .map(|(k, _)| k.clone())
            });
        if let Some(key) = oldest {
            disk.records.remove(&key);
        } else {
            break;
        }
    }
    let mut bytes = serde_json::to_vec(&disk)?;
    while bytes.len() as u64 > MAX_BYTES {
        let key = disk
            .records
            .iter()
            .filter(|(k, _)| !protected.contains(k))
            .min_by_key(|(_, r)| r.updated_ms)
            .map(|(k, _)| k.clone())
            .ok_or_else(|| io::Error::other("Active reading records exceed capacity"))?;
        disk.records.remove(&key);
        bytes = serde_json::to_vec(&disk)?;
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn empty_record(record: &mut Record) {
    record.views.clear();
    record.selections.clear();
    record.calibration.clear();
    record.restore_status.clear();
    record.mode.clear();
    record.furthest = 0.;
    record.read = false;
    record.deleted = true;
}

pub type LayoutMetrics =
    std::sync::Arc<std::sync::Mutex<BTreeMap<String, std::collections::VecDeque<f64>>>>;
pub fn layout_sample(metrics: &LayoutMetrics, name: &str, started: Instant) {
    let ms = started.elapsed().as_secs_f64() * 1000.;
    sample(
        metrics.lock().unwrap().entry(name.to_owned()).or_default(),
        ms,
    );
}
pub struct ReadingState {
    records: HashMap<DocumentId, Record>,
    paths: HashMap<String, DocumentId>,
    runtime: HashMap<String, Runtime>,
    indexes: HashMap<IndexKey, SourceIndex>,
    index_order: std::collections::VecDeque<IndexKey>,
    sent: HashMap<DocumentId, u64>,
    writer: Option<Writer>,
    state_file: Option<PathBuf>,
    dirty: bool,
    changed: Instant,
    submitted: Instant,
    cleared_ms: u64,
    observed_clear_serial: u64,
    observed_clear_ns: u64,
    activity_events: HashMap<DocumentId, u64>,
    pending_clears: Vec<ClearRequest>,
    pub stats: WriteStats,
    capture_cpu_ms: std::collections::VecDeque<f64>,
    submit_cpu_ms: std::collections::VecDeque<f64>,
    index_build_ms: std::collections::VecDeque<f64>,
    index_worker: Option<IndexWorker>,
    index_requested: HashMap<IndexKey, (u64, u64)>,
    index_epoch: u64,
    index_in_flight: usize,
    index_tokens: HashMap<IndexKey, JobControl>,
    task_history: std::collections::VecDeque<JobControl>,
    repaint: Option<egui::Context>,
    precision_worker: Option<PrecisionWorker>,
    precision_results: std::collections::VecDeque<PrecisionResult>,
    clear_epoch: u64,
    enrichment_ms: std::collections::VecDeque<f64>,
    matching_ms: std::collections::VecDeque<f64>,
    pub geometry_cache: GeometryCache,
    pub layout_metrics: LayoutMetrics,
    commit_ms: std::collections::VecDeque<f64>,
    geometry_ms: std::collections::VecDeque<f64>,
    plan_ms: std::collections::VecDeque<f64>,
    hot_frame: u64,
    hot_samples: std::collections::VecDeque<(u64, f64)>,
}
impl Default for ReadingState {
    fn default() -> Self {
        Self::new(None)
    }
}
impl ReadingState {
    fn remember_context(&mut self, context: &egui::Context) {
        if self.repaint.as_ref() != Some(context) {
            self.repaint = Some(context.clone());
        }
    }
    pub fn persistent() -> Self {
        Self::new(Some(state_path()))
    }
    fn new(path: Option<PathBuf>) -> Self {
        let (disk, error) = path
            .as_ref()
            .map(|p| match load(p) {
                Ok(d) => (d, None),
                Err(e) => (Disk::default(), Some(e.to_string())),
            })
            .unwrap_or_default();
        let mut records = HashMap::new();
        let mut paths = HashMap::new();
        for (key, r) in disk.records {
            if !r.deleted {
                paths.insert(key, r.id);
                records.insert(r.id, r);
            }
        }
        Self {
            records,
            paths,
            runtime: HashMap::new(),
            indexes: HashMap::new(),
            index_order: std::collections::VecDeque::new(),
            sent: HashMap::new(),
            state_file: path.clone(),
            writer: path.map(Writer::new),
            dirty: false,
            changed: Instant::now(),
            submitted: Instant::now(),
            cleared_ms: disk.cleared_ms,
            observed_clear_serial: disk.clear_serial,
            observed_clear_ns: disk.latest_clear_ns,
            activity_events: HashMap::new(),
            pending_clears: Vec::new(),
            stats: WriteStats {
                error,
                ..WriteStats::default()
            },
            capture_cpu_ms: std::collections::VecDeque::new(),
            submit_cpu_ms: std::collections::VecDeque::new(),
            index_build_ms: std::collections::VecDeque::new(),
            index_worker: None,
            index_requested: HashMap::new(),
            index_epoch: 0,
            index_in_flight: 0,
            index_tokens: HashMap::new(),
            task_history: std::collections::VecDeque::new(),
            repaint: None,
            precision_worker: None,
            precision_results: std::collections::VecDeque::new(),
            clear_epoch: 0,
            enrichment_ms: std::collections::VecDeque::new(),
            matching_ms: std::collections::VecDeque::new(),
            geometry_cache: GeometryCache::default(),
            layout_metrics: LayoutMetrics::default(),
            commit_ms: std::collections::VecDeque::new(),
            geometry_ms: std::collections::VecDeque::new(),
            plan_ms: std::collections::VecDeque::new(),
            hot_frame: 0,
            hot_samples: std::collections::VecDeque::new(),
        }
    }
    pub fn bind(&mut self, doc: &mut Document, pane: &str, generation: u64) {
        self.bind_inner(doc, pane, generation, true);
    }
    pub fn rebind_after_commit(&mut self, doc: &mut Document, pane: &str, generation: u64) {
        self.bind_inner(doc, pane, generation, false);
    }
    fn bind_inner(&mut self, doc: &mut Document, pane: &str, generation: u64, new_activity: bool) {
        self.sync_external_clears();
        for control in self.index_tokens.values() {
            control.cancel();
        }
        self.index_tokens.clear();
        self.index_epoch = self.index_epoch.wrapping_add(1);
        self.index_requested.clear();
        let key = doc.path.as_deref().map(path_key);
        if let Some(id) = key.as_ref().and_then(|k| self.paths.get(k)) {
            doc.id = *id;
        }
        if let Some(key) = &key {
            self.paths.insert(key.clone(), doc.id);
        }
        self.records.entry(doc.id).or_insert_with(|| Record {
            id: doc.id,
            path: key,
            clear_serial: self.observed_clear_serial,
            cleared_ns: self.observed_clear_ns,
            ..Record::default()
        });
        if new_activity {
            self.records.get_mut(&doc.id).unwrap().activity_ms = now();
            let event = event_ns();
            self.records.get_mut(&doc.id).unwrap().activity_ns = event;
            self.activity_events.insert(doc.id, event);
        }
        self.indexes.remove(&(doc.id, doc.buffer_id()));
        for view in ["source", "preview"] {
            let key = format!("{pane}/{view}");
            if let Some(rt) = self.runtime.get_mut(&key) {
                rt.cancel_precision();
            }
            let history = self.records[&doc.id].views.get(&key).cloned();
            self.runtime.insert(
                key,
                Runtime {
                    id: Some(doc.id),
                    buffer_id: doc.buffer_id(),
                    revision: doc.revision(),
                    generation,
                    pending: history.map(|p| Restore {
                        position: p,
                        tier: "pending-layout".into(),
                        exact: false,
                    }),
                    selection_pending: true,
                    reason: "opened".into(),
                    ..Runtime::default()
                },
            );
        }
        self.enforce_budget();
        self.indexes
            .retain(|(id, _), _| self.runtime.values().any(|rt| rt.id == Some(*id)));
        self.index_order
            .retain(|(id, _)| self.runtime.values().any(|rt| rt.id == Some(*id)));
    }
    /// File switching is an explicit boundary, not a per-frame disk path.
    /// Observe another instance's clear before a new open can become activity.
    fn sync_external_clears(&mut self) {
        let Some(path) = &self.state_file else {
            return;
        };
        let disk = match load(path) {
            Ok(disk) => disk,
            Err(error) => {
                self.stats.error = Some(error.to_string());
                return;
            }
        };
        self.cleared_ms = self.cleared_ms.max(disk.cleared_ms);
        self.observed_clear_serial = disk.clear_serial;
        self.observed_clear_ns = disk.latest_clear_ns;
        let mut affected = Vec::new();
        for (id, record) in &mut self.records {
            let stored = record.path.as_ref().and_then(|p| disk.records.get(p));
            let serial = stored.map_or(disk.clear_serial, |r| {
                r.clear_serial.max(disk.cleared_serial)
            });
            let cleared_ns = stored.map_or(disk.latest_clear_ns, |r| {
                if r.clear_serial >= disk.cleared_serial {
                    r.cleared_ns
                } else {
                    disk.cleared_ns
                }
            });
            let threshold = record
                .path
                .as_ref()
                .and_then(|p| disk.records.get(p))
                .filter(|r| r.deleted)
                .map_or(disk.cleared_ms, |r| r.updated_ms.max(disk.cleared_ms));
            let cleared = if serial > 0 {
                record.clear_serial < serial
                    && (cleared_ns == 0
                        || self.activity_events.get(id).copied().unwrap_or(0) <= cleared_ns)
            } else {
                threshold > 0 && record.activity_ms <= threshold
            };
            if cleared {
                empty_record(record);
                record.clear_serial = serial;
                record.cleared_ns = cleared_ns;
                record.updated_ms = record.updated_ms.max(threshold);
                affected.push(*id);
            }
        }
        if !affected.is_empty() {
            for rt in self
                .runtime
                .values_mut()
                .filter(|r| r.id.is_some_and(|id| affected.contains(&id)))
            {
                rt.cancel_precision();
                rt.pending = None;
                rt.last = None;
                rt.pinned = None;
                rt.geometry = None;
                rt.precise_target = None;
                rt.relocation = None;
                rt.suspended = true;
                rt.reason = "cleared-by-another-instance".into();
            }
            for id in affected {
                self.sent.remove(&id);
            }
        }
    }
    fn enforce_budget(&mut self) {
        let estimate = |r: &Record| -> usize {
            512 + r.path.as_ref().map_or(0, String::len)
                + r.restore_status.len() * 256
                + r.views
                    .values()
                    .map(|p| {
                        256 + p.layout.len()
                            + p.anchor.context.len()
                            + p.anchor.before.len()
                            + p.anchor.heading.iter().map(String::len).sum::<usize>()
                            + p.text_anchor.as_ref().map_or(0, |t| {
                                512 + t.source.before.len()
                                    + t.source.after.len()
                                    + t.source.grapheme.len()
                                    + t.source.estimated_proof_bytes()
                            })
                    })
                    .sum::<usize>()
                + r.calibration
                    .values()
                    .map(|c| 128 + c.layout.len() + c.heights.len() * 16)
                    .sum::<usize>()
        };
        let mut bytes = self.records.values().map(estimate).sum::<usize>();
        while self.records.len() > MAX_RECORDS || bytes > MAX_BYTES as usize {
            let id = self
                .records
                .iter()
                .filter(|(id, _)| !self.runtime.values().any(|rt| rt.id == Some(**id)))
                .min_by_key(|(_, r)| r.updated_ms)
                .map(|(id, _)| *id);
            if let Some(id) = id {
                if let Some(record) = self.records.remove(&id) {
                    bytes = bytes.saturating_sub(estimate(&record));
                }
                self.indexes.retain(|(cached, _), _| *cached != id);
                self.index_order.retain(|(cached, _)| *cached != id);
                self.paths.retain(|_, v| *v != id);
                self.sent.remove(&id);
                self.activity_events.remove(&id);
            } else {
                break;
            }
        }
    }
    fn index(&mut self, doc: &Document) -> &SourceIndex {
        let key = (doc.id, doc.buffer_id());
        self.index_order.retain(|cached| *cached != key);
        self.index_order.push_back(key);
        // There are two editable buffers. Replacements and worker clones get a
        // new cache key without rebinding the persistent document identity.
        while self.index_order.len() > 2 {
            let stale = self.index_order.pop_front().unwrap();
            self.indexes.remove(&stale);
            self.index_requested.remove(&stale);
            if let Some(control) = self.index_tokens.remove(&stale) {
                control.cancel();
            }
        }
        if let Some(worker) = &self.index_worker {
            while let Ok(result) = worker.rx.try_recv() {
                self.index_in_flight = self.index_in_flight.saturating_sub(1);
                let Some(index) = result.index else {
                    continue;
                };
                if result.epoch == self.index_epoch
                    && self.index_requested.get(&result.key)
                        == Some(&(result.epoch, index.revision))
                    && self.index_order.contains(&result.key)
                    && self
                        .runtime
                        .values()
                        .any(|rt| rt.id == Some(result.key.0) && rt.buffer_id == result.key.1)
                {
                    self.index_requested.remove(&result.key);
                    self.index_tokens.remove(&result.key);
                    sample(&mut self.index_build_ms, result.ms);
                    self.indexes.insert(result.key, index);
                }
            }
        }
        if self
            .indexes
            .get(&key)
            .is_none_or(|i| i.revision != doc.revision())
        {
            if doc.text.len() > 64 * 1024 {
                let worker = self.index_worker.get_or_insert_with(IndexWorker::new);
                let token = (self.index_epoch, doc.revision());
                let control = JobControl::new(
                    "source-index",
                    serde_json::json!({"document_id":doc.id,"buffer_id":doc.buffer_id(),"revision":doc.revision(),"epoch":self.index_epoch,"open_generation":self.runtime.values().find(|rt|rt.id==Some(doc.id)&&rt.buffer_id==doc.buffer_id()).map(|rt|rt.generation)}),
                )
                .with_repaint(self.repaint.as_ref());
                if !self.index_requested.contains_key(&key)
                    && self.index_in_flight < 2
                    && worker
                        .tx
                        .as_ref()
                        .unwrap()
                        .try_send(IndexJob {
                            key,
                            epoch: self.index_epoch,
                            revision: doc.revision(),
                            text: doc.text.clone(),
                            control: control.clone(),
                        })
                        .is_ok()
                {
                    self.index_requested.insert(key, token);
                    self.index_tokens.insert(key, control.clone());
                    self.task_history.push_back(control);
                    while self.task_history.len() > 64 {
                        self.task_history.pop_front();
                    }
                    self.index_in_flight += 1;
                }
                self.indexes.entry(key).or_insert_with(|| SourceIndex {
                    revision: u64::MAX,
                    fingerprint: 0,
                    headings: Vec::new(),
                    paths: Vec::new(),
                    characters: None,
                    line_starts: std::sync::Arc::new(vec![0]),
                    snapshot: None,
                });
            } else {
                let started = Instant::now();
                self.indexes.insert(key, SourceIndex::new(doc));
                sample(
                    &mut self.index_build_ms,
                    started.elapsed().as_secs_f64() * 1000.,
                );
            }
        }
        &self.indexes[&key]
    }
    /// Protect only the first history mount after binding a buffer. A visible
    /// editor stays usable while indexing its later edits or matching them.
    pub fn awaiting_initial_source_index(&self, doc: &Document, key: &str) -> bool {
        self.runtime.get(key).is_some_and(|rt| {
            rt.pending.is_some()
                && rt.presented_offset.is_none()
                && self
                    .indexes
                    .get(&(doc.id, doc.buffer_id()))
                    .is_none_or(|index| index.revision != doc.revision())
        })
    }
    pub fn byte_at_char(&mut self, doc: &Document, character: usize) -> usize {
        self.index(doc).characters.as_ref().map_or_else(
            || char_to_byte(&doc.text, character),
            |positions| positions.get(character).copied().unwrap_or(doc.text.len()),
        )
    }
    pub fn char_at_byte(&mut self, doc: &Document, byte: usize) -> usize {
        let byte = byte.min(doc.text.len());
        self.index(doc).characters.as_ref().map_or_else(
            || byte_to_char(&doc.text, byte),
            |positions| positions.partition_point(|b| *b <= byte).saturating_sub(1),
        )
    }
    pub fn selection(&mut self, doc: &Document, key: &str) -> Option<[usize; 2]> {
        let rt = self.runtime.get_mut(key)?;
        if !rt.selection_pending {
            return None;
        }
        rt.selection_pending = false;
        self.records
            .get(&doc.id)?
            .selections
            .get(key)
            .copied()
            .map(|s| {
                let max = doc.text.chars().count();
                [s[0].min(max), s[1].min(max)]
            })
    }
    pub fn store_selection(&mut self, doc: &Document, key: &str, selection: [usize; 2]) {
        if self.runtime.get(key).is_some_and(|r| r.suspended) {
            return;
        }
        if let Some(record) = self.records.get_mut(&doc.id)
            && record.selections.get(key) != Some(&selection)
        {
            record.selections.insert(key.into(), selection);
            record.updated_ms = now()
                .max(record.updated_ms.saturating_add(1))
                .max(self.cleared_ms.saturating_add(1));
            self.mark_dirty();
        }
    }
    pub fn cancel(&mut self, key: &str, reason: &str) {
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.cancel_precision();
            rt.pending = None;
            rt.suspended = false;
            rt.failed_restore = None;
            rt.precise_target = None;
            rt.reason = reason.into();
            rt.recapture = true;
            rt.pinned = None;
            rt.block_relocation = false;
        }
    }
    pub fn mode(&self, doc: &Document) -> Option<&str> {
        self.records
            .get(&doc.id)
            .map(|r| r.mode.as_str())
            .filter(|v| !v.is_empty())
    }
    pub fn activity(&mut self, doc: &Document, key: &str, mode: Option<&str>) {
        if let Some(r) = self.records.get_mut(&doc.id) {
            let changed = r.last_view != key || mode.is_some_and(|m| r.mode != m);
            if changed {
                r.last_view = key.into();
                if let Some(mode) = mode {
                    r.mode = mode.into();
                }
                r.updated_ms = now().max(r.updated_ms.saturating_add(1));
                self.mark_dirty();
            }
        }
    }
    pub fn set_mode(&mut self, doc: &Document, mode: &str) {
        if let Some(r) = self.records.get_mut(&doc.id)
            && r.mode != mode
        {
            r.mode = mode.into();
            if mode == "Read" {
                r.last_view = "main/preview".into();
            } else if mode == "Write" {
                r.last_view = "main/source".into();
            }
            r.updated_ms = now().max(r.updated_ms.saturating_add(1));
            self.mark_dirty();
        }
    }
    pub fn take_calibration(
        &mut self,
        doc: &Document,
        key: &str,
        layout: &str,
    ) -> Option<Vec<(u64, f32)>> {
        if !self.records.contains_key(&doc.id) {
            return None;
        }
        let index = self.index(doc);
        if index.revision != doc.revision() {
            return None;
        }
        let fingerprint = index.fingerprint;
        let rt = self.runtime.get_mut(key)?;
        if rt.calibrated {
            return None;
        }
        rt.calibrated = true;
        self.records
            .get(&doc.id)?
            .calibration
            .get(key)
            .filter(|c| c.layout == layout && c.fingerprint == fingerprint)
            .map(|c| c.heights.clone())
    }
    pub fn needs_calibration(&self, key: &str, stamp: u64) -> bool {
        self.runtime
            .get(key)
            .is_some_and(|rt| rt.calibration_stamp != stamp)
    }
    pub fn calibration(
        &mut self,
        doc: &Document,
        key: &str,
        layout: &str,
        stamp: u64,
        heights: Vec<(u64, f32)>,
    ) {
        if !self.records.contains_key(&doc.id) {
            return;
        }
        let index = self.index(doc);
        if index.revision != doc.revision() {
            return;
        }
        let fingerprint = index.fingerprint;
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.calibration_stamp = stamp;
        }
        if let Some(r) = self.records.get_mut(&doc.id) {
            r.calibration.insert(
                key.into(),
                HeightCalibration {
                    layout: layout.into(),
                    fingerprint,
                    heights,
                },
            );
            r.updated_ms = now().max(r.updated_ms.saturating_add(1));
            self.mark_dirty();
        }
        self.enforce_budget();
    }
    pub fn viewport(&mut self, key: &str, rect: egui::Rect) {
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.viewport_rect = Some([rect.min.x, rect.min.y, rect.max.x, rect.max.y]);
        }
    }
    pub fn layer(&mut self, key: &str, layer: egui::LayerId) {
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.layer = Some(layer);
        }
    }
    pub fn restoring(&self, key: &str) -> bool {
        self.runtime.get(key).is_some_and(|r| r.pending.is_some())
    }
    fn hot_sample(&mut self, ms: f64) {
        if let Some((frame, total)) = self
            .hot_samples
            .back_mut()
            .filter(|(frame, _)| *frame == self.hot_frame)
        {
            *total += ms;
            let _ = frame;
        } else {
            if self.hot_samples.len() >= 2000 {
                self.hot_samples.pop_front();
            }
            self.hot_samples.push_back((self.hot_frame, ms));
        }
    }
    fn precision_identity(&self, doc: &Document, key: &str) -> Option<PrecisionIdentity> {
        let rt = self.runtime.get(key)?;
        Some(PrecisionIdentity {
            view: key.into(),
            document_id: doc.id,
            buffer_id: doc.buffer_id(),
            revision: doc.revision(),
            open_generation: rt.generation,
            restore_epoch: rt.precision_epoch,
            clear_epoch: self.clear_epoch,
        })
    }
    fn queue_precision(
        &mut self,
        doc: &Document,
        key: &str,
        anchor: reading_precision::Anchor,
        relocate: bool,
    ) -> bool {
        let Some(index) = self
            .indexes
            .get(&(doc.id, doc.buffer_id()))
            .filter(|i| i.revision == doc.revision())
        else {
            return false;
        };
        let Some(text) = index.snapshot.clone() else {
            return false;
        };
        let fingerprint = index.fingerprint;
        let Some(identity) = self.precision_identity(doc, key) else {
            return false;
        };
        let control = JobControl::new(
            if relocate {
                "relocation-match"
            } else {
                "anchor-enrichment"
            },
            identity.report(),
        )
        .with_repaint(self.repaint.as_ref());
        let source_byte = anchor.source_byte;
        let worker = self
            .precision_worker
            .get_or_insert_with(PrecisionWorker::new);
        if worker
            .tx
            .as_ref()
            .unwrap()
            .try_send(PrecisionJob {
                identity,
                text,
                fingerprint,
                work: if relocate {
                    PrecisionWork::Relocate(anchor)
                } else {
                    PrecisionWork::Enrich(anchor)
                },
                control: control.clone(),
            })
            .is_err()
        {
            return false;
        }
        let rt = self.runtime.get_mut(key).unwrap();
        if relocate {
            rt.relocation_job = Some(control.clone());
        } else {
            rt.proof_attempt = Some((fingerprint, source_byte));
            rt.proof_job = Some(control.clone());
        }
        self.task_history.push_back(control);
        while self.task_history.len() > 64 {
            self.task_history.pop_front();
        }
        true
    }
    fn poll_precision(&mut self, doc: &Document, key: &str) {
        if let Some(worker) = &self.precision_worker {
            while let Ok(result) = worker.rx.try_recv() {
                self.precision_results.push_back(result);
            }
        }
        let results = std::mem::take(&mut self.precision_results);
        for result in results {
            let identity = &result.identity;
            let valid = self.clear_epoch == identity.clear_epoch
                && self.runtime.get(&identity.view).is_some_and(|rt| {
                    rt.id == Some(identity.document_id)
                        && rt.buffer_id == identity.buffer_id
                        && rt.revision == identity.revision
                        && rt.generation == identity.open_generation
                        && rt.precision_epoch == identity.restore_epoch
                });
            if !valid {
                continue;
            }
            if identity.view != key {
                self.precision_results.push_back(result);
                continue;
            }
            if doc.id != identity.document_id
                || doc.buffer_id() != identity.buffer_id
                || doc.revision() != identity.revision
            {
                continue;
            }
            if result.match_ms > 0. {
                sample(&mut self.matching_ms, result.match_ms);
            }
            if result.enrich_ms > 0. {
                sample(&mut self.enrichment_ms, result.enrich_ms);
            }
            let rt = self.runtime.get_mut(key).unwrap();
            match result.output {
                PrecisionOutput::Cancelled => {
                    if rt
                        .proof_job
                        .as_ref()
                        .is_some_and(|j| j.id == result.control.id)
                    {
                        rt.proof_job = None;
                    }
                    if rt
                        .relocation_job
                        .as_ref()
                        .is_some_and(|j| j.id == result.control.id)
                    {
                        rt.relocation_job = None;
                    }
                }
                PrecisionOutput::Enriched(source) => {
                    if rt
                        .proof_job
                        .as_ref()
                        .is_none_or(|j| j.id != result.control.id)
                    {
                        continue;
                    }
                    rt.proof_job = None;
                    let update = |anchor: &mut TextAnchor| {
                        if anchor.source.source_byte == source.source_byte
                            && anchor.source.source_fingerprint == source.source_fingerprint
                        {
                            anchor.source = source.clone();
                            true
                        } else {
                            false
                        }
                    };
                    if let Some(pin) = &mut rt.pinned {
                        update(pin);
                    }
                    if let Some(anchor) = rt.last.as_mut().and_then(|p| p.text_anchor.as_mut()) {
                        update(anchor);
                    }
                    if let Some(record) = self.records.get_mut(&doc.id)
                        && let Some(anchor) = record
                            .views
                            .get_mut(key)
                            .and_then(|p| p.text_anchor.as_mut())
                        && update(anchor)
                    {
                        self.sent.remove(&doc.id);
                        self.mark_dirty();
                    }
                }
                PrecisionOutput::Relocated { decision, anchor } => {
                    if rt
                        .relocation_job
                        .as_ref()
                        .is_none_or(|j| j.id != result.control.id)
                    {
                        continue;
                    }
                    rt.relocation_job = None;
                    let Some(pending) = rt.pending.as_mut() else {
                        continue;
                    };
                    let old_source = pending
                        .position
                        .text_anchor
                        .as_ref()
                        .map(|p| p.source.clone());
                    let saved_y = pending
                        .position
                        .text_anchor
                        .as_ref()
                        .map_or(pending.position.anchor.viewport_y, |p| p.viewport_y);
                    let index = &self.indexes[&(doc.id, doc.buffer_id())];
                    pending.position.anchor =
                        index.anchor(&doc.text, decision.source_byte, 0., saved_y);
                    pending.position.fingerprint = index.fingerprint;
                    pending.exact = false;
                    pending.tier = if decision.exact {
                        "text-relocation".into()
                    } else {
                        format!("{}-relocation", decision.tier)
                    };
                    pending.position.text_anchor = decision.exact.then(|| TextAnchor {
                        source: anchor,
                        viewport_y: saved_y,
                        token_range: decision.grapheme_range.clone(),
                        logical_line: index
                            .line_starts
                            .partition_point(|b| *b <= decision.source_byte)
                            .saturating_sub(1),
                    });
                    rt.pinned = pending.position.text_anchor.clone();
                    rt.block_relocation = !decision.exact;
                    rt.precise_target = None;
                    rt.target = None;
                    rt.stable = 0;
                    rt.passes = 0;
                    rt.restore_started = None;
                    rt.ready_started = None;
                    rt.ready_passes = 0;
                    rt.relocation = Some(
                        serde_json::json!({"exact":decision.exact,"tier":decision.tier,"reason":decision.evidence.get("method"),"status":"awaiting-layout","old_source":old_source,"new_source_byte":decision.source_byte,"grapheme_range":decision.grapheme_range,"metadata":identity.report(),"evidence":decision.evidence,"match_wall_ms":result.match_ms,"enrichment_wall_ms":result.enrich_ms,"task_id":result.control.id}),
                    );
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn plan(
        &mut self,
        doc: &Document,
        key: &str,
        generation: u64,
        layout: &str,
        ctx: &egui::Context,
        viewport: egui::Rect,
        navigation: bool,
    ) -> Option<Restore> {
        self.remember_context(ctx);
        self.hot_frame = ctx.cumulative_frame_nr();
        let start = Instant::now();
        let result = self.plan_inner(doc, key, generation, layout, ctx, viewport, navigation);
        let ms = start.elapsed().as_secs_f64() * 1000.;
        sample(&mut self.plan_ms, ms);
        self.hot_sample(ms);
        result
    }
    #[allow(clippy::too_many_arguments)]
    fn plan_inner(
        &mut self,
        doc: &Document,
        key: &str,
        generation: u64,
        layout: &str,
        ctx: &egui::Context,
        viewport: egui::Rect,
        navigation: bool,
    ) -> Option<Restore> {
        if self
            .runtime
            .get(key)
            .is_some_and(|rt| rt.id != Some(doc.id) || rt.generation != generation)
            || !self.records.contains_key(&doc.id)
        {
            return None;
        }
        // A worker save/replace can return a cloned buffer while preserving
        // document identity and the open epoch. Trust only the active pane's
        // Document here; old buffer tasks must never bind themselves back.
        if self
            .runtime
            .get(key)
            .is_some_and(|rt| rt.buffer_id != doc.buffer_id())
        {
            let pane = key.split('/').next().unwrap_or("main");
            for (view, rt) in &mut self.runtime {
                if view.starts_with(&format!("{pane}/"))
                    && rt.id == Some(doc.id)
                    && rt.generation == generation
                {
                    if let Some(control) = self.index_tokens.remove(&(doc.id, rt.buffer_id)) {
                        control.cancel();
                    }
                    self.index_requested.remove(&(doc.id, rt.buffer_id));
                    rt.cancel_precision();
                    rt.buffer_id = doc.buffer_id();
                    rt.revision = doc.revision();
                }
            }
        }
        let pane_layer = self
            .runtime
            .get(key)
            .and_then(|rt| rt.layer)
            .unwrap_or_else(egui::LayerId::background);
        let pointer_owned = ctx.pointer_hover_pos().is_some_and(|p| {
            viewport.contains(p) && ctx.layer_id_at(p).is_none_or(|layer| layer == pane_layer)
        });
        let source_id = if key.starts_with("secondary/") {
            egui::Id::new(("second-editor", generation))
        } else {
            egui::Id::new(("source-editor", generation))
        };
        let keyboard_owned = pointer_owned
            || (ctx.memory(|m| m.has_focus(source_id))
                && ctx
                    .layer_id_at(viewport.center())
                    .is_none_or(|layer| layer == pane_layer));
        let user = ctx.input(|i| {
            pointer_owned
                && (i.raw_scroll_delta != egui::Vec2::ZERO
                    || i.smooth_scroll_delta != egui::Vec2::ZERO
                    || i.pointer.primary_down())
                || keyboard_owned
                    && i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::Key {
                                key: egui::Key::PageDown
                                    | egui::Key::PageUp
                                    | egui::Key::Home
                                    | egui::Key::End,
                                pressed: true,
                                ..
                            }
                        )
                    })
        });
        if !self.runtime.contains_key(key) {
            self.runtime.insert(
                key.into(),
                Runtime {
                    id: Some(doc.id),
                    buffer_id: doc.buffer_id(),
                    revision: doc.revision(),
                    generation,
                    ..Runtime::default()
                },
            );
        }
        let rt = self.runtime.get_mut(key).unwrap();
        if rt.id != Some(doc.id) || rt.generation != generation {
            return None;
        }
        if rt.revision != doc.revision() {
            rt.cancel_precision();
            rt.revision = doc.revision();
            if let Some(control) = self.index_tokens.remove(&(doc.id, doc.buffer_id())) {
                control.cancel();
            }
            self.index_requested.remove(&(doc.id, doc.buffer_id()));
        }
        let pane_focus = ctx
            .memory(|m| m.focused())
            .and_then(|id| ctx.read_response(id))
            .is_some_and(|response| {
                response.layer_id == pane_layer && response.rect.intersects(viewport)
            });
        let explicit_keyboard = pane_focus
            && ctx.input(|i| {
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key {
                            key: egui::Key::ArrowLeft
                                | egui::Key::ArrowRight
                                | egui::Key::ArrowUp
                                | egui::Key::ArrowDown
                                | egui::Key::Backspace
                                | egui::Key::Delete
                                | egui::Key::Enter
                                | egui::Key::Tab
                                | egui::Key::Z
                                | egui::Key::Y,
                            pressed: true,
                            ..
                        } | egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Cut
                    )
                })
            });
        if (user || navigation || explicit_keyboard)
            && let Some(record) = self.records.get_mut(&doc.id)
        {
            record.activity_ms = now();
            let event = event_ns();
            record.activity_ns = event;
            self.activity_events.insert(doc.id, event);
        }
        if navigation || user {
            if rt.pending.is_some() {
                if rt.cancel_events.len() >= 16 {
                    rt.cancel_events.pop_front();
                }
                rt.cancel_events.push_back(serde_json::json!({
                    "had_pending":true,"document_id":rt.id,"generation":rt.generation,
                    "frame":ctx.cumulative_frame_nr(),"timestamp_ms":now(),
                    "raw_wheel":ctx.input(|i| i.events.iter().any(|e| matches!(e,egui::Event::MouseWheel{..}))),
                    "navigation":navigation,"tier":rt.pending.as_ref().map(|p|&p.tier),"passes":rt.passes
                }));
            }
            rt.cancel_precision();
            if let Some(decision) = &mut rt.relocation {
                decision["status"] = serde_json::json!("cancelled");
                decision["cancellation_reason"] = serde_json::json!(if navigation {
                    "navigation"
                } else {
                    "user-scroll"
                });
            }
            rt.pending = None;
            rt.restore_started = None;
            rt.failed_restore = None;
            rt.suspended = false;
            rt.recapture = true;
            rt.pinned = None;
            rt.block_relocation = false;
            rt.precise_target = None;
            rt.reason = if navigation {
                "navigation"
            } else {
                "user-scroll"
            }
            .into();
            return None;
        }
        if rt.suspended {
            return None;
        }
        let index = self.index(doc);
        if index.revision != doc.revision() {
            ctx.request_repaint_after(Duration::from_millis(20));
            return None;
        }
        let fingerprint = index.fingerprint;
        self.poll_precision(doc, key);
        let rt = self.runtime.get_mut(key).unwrap();
        if rt.pending.is_none()
            && let Some(last) = &rt.last
            && (last.layout != layout || last.fingerprint != fingerprint)
        {
            rt.precise_target = None;
            rt.restore_started = None;
            rt.ready_started = None;
            rt.ready_passes = 0;
            rt.pending = Some(Restore {
                position: last.clone(),
                tier: "layout-anchor".into(),
                exact: false,
            });
            rt.stable = 0;
            rt.passes = 0;
        }
        let Some(mut restore) = rt.pending.clone() else {
            let enrich = rt
                .last
                .as_ref()
                .filter(|p| p.fingerprint == fingerprint)
                .and_then(|p| p.text_anchor.as_ref())
                .filter(|p| {
                    !p.source.has_relocation_proof()
                        && rt.proof_job.is_none()
                        && rt.proof_attempt != Some((fingerprint, p.source.source_byte))
                })
                .map(|p| p.source.clone());
            if let Some(anchor) = enrich {
                if self.changed.elapsed() >= Duration::from_millis(150) {
                    self.queue_precision(doc, key, anchor, false);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(150));
                }
            }
            return None;
        };
        if restore.position.fingerprint != fingerprint
            && let Some(old) = restore
                .position
                .text_anchor
                .as_ref()
                .map(|p| p.source.clone())
        {
            if self.runtime[key].relocation_job.is_none() {
                self.runtime.get_mut(key).unwrap().relocation = Some(
                    serde_json::json!({"exact":false,"tier":"invalid","reason":"version-relocation-pending","status":"matching","old_source":old,"metadata":self.precision_identity(doc,key).map(|m|m.report()),"evidence":{}}),
                );
                self.queue_precision(doc, key, old, true);
            }
            ctx.request_repaint_after(Duration::from_millis(10));
            return None;
        }
        if restore.tier == "pending-layout"
            || restore.position.layout != layout
            || restore.position.fingerprint != fingerprint
        {
            self.runtime.get_mut(key).unwrap().precise_target = None;
            restore.exact =
                restore.position.layout == layout && restore.position.fingerprint == fingerprint;
            restore.tier = if restore.exact {
                "offset"
            } else if restore.position.fingerprint == fingerprint {
                "layout-anchor"
            } else {
                "content-anchor"
            }
            .into();
            if restore.position.fingerprint != fingerprint {
                let old = restore.position.anchor.clone();
                let (byte, tier) = self.indexes[&(doc.id, doc.buffer_id())].locate(&doc.text, &old);
                restore.position.anchor.byte = byte;
                restore.tier = tier.into();
                restore.position.anchor.fraction = 0.;
                restore.position.text_anchor = None;
                let rt = self.runtime.get_mut(key).unwrap();
                rt.pinned = None;
                rt.block_relocation = true;
                rt.relocation = Some(
                    serde_json::json!({"exact":false,"tier":if tier=="heading"{"section"}else if tier=="context"{"block"}else{"approximate"},"reason":"legacy-record-without-text-relocation-proof","status":"awaiting-layout","new_source_byte":byte,"metadata":{"document_id":doc.id,"buffer_id":doc.buffer_id(),"revision":doc.revision(),"open_generation":generation},"evidence":{}}),
                );
            }
            restore.position.layout = layout.into();
            restore.position.fingerprint = fingerprint;
            self.runtime.get_mut(key).unwrap().pending = Some(restore.clone());
        }
        Some(restore)
    }
    /// Observe only galleys painted in this pass. The return value is an
    /// offset for a following pass; the reported rect remains the actual one.
    pub fn precise_byte(&self, key: &str) -> Option<usize> {
        self.runtime
            .get(key)
            .filter(|r| !r.recapture && r.relocation_job.is_none() && !r.block_relocation)
            .and_then(|r| {
                r.pinned
                    .as_ref()
                    .or_else(|| {
                        r.pending
                            .as_ref()
                            .and_then(|p| p.position.text_anchor.as_ref())
                    })
                    .or_else(|| r.last.as_ref().and_then(|p| p.text_anchor.as_ref()))
            })
            .map(|a| a.source.source_byte)
    }
    /// Live layout can preserve a content block's pixels before its optional
    /// relocation proof is enriched. This never authorizes old source offsets.
    pub fn live_snapshot_without_proof(&self, doc: &Document, key: &str, generation: u64) -> bool {
        self.runtime.get(key).is_some_and(|rt| {
            rt.id == Some(doc.id)
                && rt.buffer_id == doc.buffer_id()
                && rt.generation == generation
                && rt
                    .pending
                    .as_ref()
                    .and_then(|p| p.position.text_anchor.as_ref())
                    .or_else(|| rt.last.as_ref().and_then(|p| p.text_anchor.as_ref()))
                    .is_none_or(|anchor| anchor.source.proof.is_none())
        })
    }
    pub fn live_snapshot_geometry_ready(&mut self, doc: &Document, key: &str) -> bool {
        let index = self.index(doc);
        let current = index.revision == doc.revision();
        let fingerprint = index.fingerprint;
        current
            && self.runtime.get(key).is_some_and(|rt| {
                rt.geometry.is_some()
                    && rt
                        .pinned
                        .as_ref()
                        .is_some_and(|pin| pin.source.source_fingerprint == fingerprint)
            })
    }
    #[cfg(test)]
    pub(crate) fn unverified_snapshot_anchor_for_test(&mut self, doc: &Document, key: &str) {
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.cancel_precision();
            if let Some(pin) = &mut rt.pinned {
                pin.source.proof = None;
            }
            if let Some(anchor) = rt.last.as_mut().and_then(|p| p.text_anchor.as_mut()) {
                anchor.source.proof = None;
            }
        }
        if let Some(anchor) = self
            .records
            .get_mut(&doc.id)
            .and_then(|r| r.views.get_mut(key))
            .and_then(|p| p.text_anchor.as_mut())
        {
            anchor.source.proof = None;
        }
    }
    #[allow(clippy::too_many_arguments)] // Actual viewport, current offset and reachable range are distinct layout observations.
    pub fn geometry_frame(
        &mut self,
        doc: &Document,
        key: &str,
        galleys: &[MappedGalley],
        viewport: egui::Rect,
        offset: [f32; 2],
        max_offset: [f32; 2],
        ctx: &egui::Context,
    ) -> Option<[f32; 2]> {
        let started = Instant::now();
        let frame = ctx.cumulative_frame_nr();
        let cached_index_work = self.geometry_cache.take_frame_ms();
        let index = self.index(doc);
        if index.revision != doc.revision() {
            return None;
        }
        let fingerprint = index.fingerprint;
        let lines = index.line_starts.clone();
        let first = galleys
            .iter()
            .filter_map(|g| g.first_visible(&doc.text, viewport, frame))
            .min_by(|a, b| {
                a.rect[1]
                    .total_cmp(&b.rect[1])
                    .then(a.rect[0].total_cmp(&b.rect[0]))
            });
        let rt = self.runtime.get_mut(key)?;
        rt.presented_offset = Some(offset);
        rt.presented_max = max_offset;
        if rt.relocation_job.is_some() {
            rt.geometry = None;
            return None;
        }
        let scroll_changed = rt.pending.is_none()
            && rt.last.as_ref().is_some_and(|last| {
                (last.offset[0] - offset[0]).abs() > 0.5 || (last.offset[1] - offset[1]).abs() > 0.5
            });
        let replace = rt.recapture
            || scroll_changed
            || rt
                .pinned
                .as_ref()
                .is_some_and(|p| p.source.source_fingerprint != fingerprint);
        if !rt.suspended
            && (replace
                || (rt.pending.is_none()
                    && rt.pinned.is_none()
                    && rt
                        .pending
                        .as_ref()
                        .and_then(|p| p.position.text_anchor.as_ref())
                        .is_none()
                    && rt
                        .last
                        .as_ref()
                        .and_then(|p| p.text_anchor.as_ref())
                        .is_none()))
        {
            rt.pinned = first.as_ref().map(|g| TextAnchor {
                source: reading_precision::capture_with_fingerprint(
                    &doc.text,
                    g.source_byte,
                    fingerprint,
                ),
                viewport_y: g.viewport_y,
                logical_line: lines
                    .partition_point(|b| *b <= g.source_byte)
                    .saturating_sub(1),
                token_range: g.token_range.clone(),
            });
            rt.recapture = false;
        }
        if rt.pinned.is_none() && !rt.recapture && !replace && !rt.block_relocation {
            rt.pinned = rt
                .pending
                .as_ref()
                .and_then(|p| p.position.text_anchor.clone())
                .or_else(|| rt.last.as_ref().and_then(|p| p.text_anchor.clone()));
        }
        let pin = rt.pinned.clone();
        rt.geometry = pin.as_ref().and_then(|p| {
            galleys
                .iter()
                .find_map(|g| g.target(p.source.source_byte, &doc.text, viewport, frame))
        });
        if let Some(g) = &mut rt.geometry {
            g.logical_line = lines
                .partition_point(|b| *b <= g.source_byte)
                .saturating_sub(1);
        }
        rt.visible_text = if std::env::var_os("MDCARD_READING_GEOMETRY_REPORT").is_some() {
            galleys
                .iter()
                .flat_map(|g| g.visible_glyphs(&doc.text, viewport, frame))
                .take(1024)
                .collect()
        } else {
            Vec::new()
        };
        rt.visible_runs = if std::env::var_os("MDCARD_READING_GEOMETRY_REPORT").is_some() {
            galleys
                .iter()
                .flat_map(|g| g.visible_runs(&doc.text, viewport, frame))
                .take(256)
                .collect()
        } else {
            Vec::new()
        };
        let mut target = None;
        if !replace
            && !rt.suspended
            && let (Some(pin), Some(glyph)) = (&pin, &mut rt.geometry)
        {
            let correction = glyph.viewport_y - pin.viewport_y;
            // Raw offsets take precedence with identical content and layout.
            let offset_restore = rt.pending.as_ref().is_some_and(|p| p.exact);
            if !offset_restore {
                let wanted = offset[1] + correction;
                let y = wanted.clamp(0., max_offset[1].max(0.));
                glyph.clamped = (wanted - y).abs() > 0.1;
                glyph.clamp_reason = glyph.clamped.then(|| {
                    if wanted < 0. {
                        "document-start"
                    } else {
                        "document-end"
                    }
                    .into()
                });
                let horizontal = if glyph.rect[0] < viewport.left() {
                    glyph.rect[0] - viewport.left() - 1.
                } else if glyph.rect[2] > viewport.right() {
                    glyph.rect[2] - viewport.right() + 1.
                } else {
                    0.
                };
                let wanted_x = offset[0] + horizontal;
                let x = wanted_x.clamp(0., max_offset[0].max(0.));
                if (wanted_x - x).abs() > 0.1 {
                    glyph.clamped = true;
                    glyph.clamp_reason = Some("horizontal-range-insufficient".into());
                }
                if rt.pending.is_some() {
                    target = Some(offset);
                }
                // Pixel snapping and the two GUI-rounded coordinate origins
                // can alternate by a physical pixel plus GUI rounding steps.
                // Stop inside that bounded quantum, still below the 2px
                // logical precision contract, rather than chasing the phase.
                let quantum = (1. / ctx.pixels_per_point() + 2. * egui::emath::GUI_ROUNDING)
                    .clamp(0.75, 1.25);
                if (y - offset[1]).abs() > quantum || (x - offset[0]).abs() > quantum {
                    target = Some([x, y]);
                    rt.compensations = rt.compensations.saturating_add(1);
                    ctx.request_repaint();
                }
            }
            glyph.compensations = rt.compensations;
        }
        if replace {
            rt.precise_target = None;
        } else if rt.pending.is_some()
            && rt.geometry.is_none()
            && let Some(pin) = &pin
        {
            // Hidden Markdown destinations can make a source-byte proportion
            // place the target run outside the painter's clip. Once actual
            // visible runs establish which side contains it, carry that local
            // movement into the next pass instead of repeating the estimate.
            // This is mounting only; precision still needs the target Galley.
            let byte = pin.source.source_byte;
            let (mut before, mut after) = (false, false);
            for actual in galleys
                .iter()
                .filter_map(|g| g.first_visible(&doc.text, viewport, frame))
            {
                before |= actual.source_byte < byte;
                after |= actual.source_byte > byte;
            }
            let direction = match (before, after) {
                (true, false) => 1.,
                (false, true) => -1.,
                _ => 0.,
            };
            let y =
                (offset[1] + direction * viewport.height() * 0.5).clamp(0., max_offset[1].max(0.));
            if direction != 0. && (y - offset[1]).abs() > 0.75 {
                target = Some([offset[0], y]);
                rt.precise_target = target;
                rt.compensations = rt.compensations.saturating_add(1);
                ctx.request_repaint();
            } else {
                rt.precise_target = None;
            }
        } else if let Some(target) = target {
            rt.precise_target = Some(target);
        }
        let ms = started.elapsed().as_secs_f64() * 1000. + cached_index_work;
        sample(&mut self.geometry_ms, ms);
        self.hot_sample(ms);
        target
    }
    pub fn commit_sample(&mut self, started: Instant) {
        let ms = started.elapsed().as_secs_f64() * 1000.;
        sample(&mut self.commit_ms, ms);
    }
    pub fn bootstrap_text_byte(&self, key: &str) -> Option<usize> {
        self.runtime
            .get(key)
            .filter(|rt| {
                rt.pending.is_some() && rt.precise_target.is_none() && rt.relocation_job.is_none()
            })
            .and_then(|rt| rt.pending.as_ref()?.position.text_anchor.as_ref())
            .map(|pin| pin.source.source_byte)
    }
    /// Reuse the last actual-glyph correction as next pass's input. Reapplying
    /// the coarse block estimate every pass would prevent actual convergence.
    pub fn scroll_target(&self, key: &str, coarse: [f32; 2]) -> [f32; 2] {
        self.runtime
            .get(key)
            .filter(|rt| rt.pending.is_some())
            .and_then(|rt| rt.precise_target)
            .unwrap_or(coarse)
    }
    /// A new process may have calibrated block heights but no decoded image.
    /// Mount that block before applying its saved within-block raw offset.
    pub fn bootstrap_async(&self, key: &str) -> bool {
        self.runtime.get(key).is_some_and(|rt| {
            rt.pending.is_some() && (rt.presented_offset.is_none() || rt.layout_pending)
        })
    }
    /// A parser may still display nodes from the preceding content revision.
    /// Keep its saved anchor/restore task, but expose no geometry from that source.
    pub fn await_source_layout(&mut self, key: &str) {
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.layout_pending = true;
            rt.geometry = None;
            rt.visible_text.clear();
            rt.visible_runs.clear();
            rt.stable = 0;
        }
    }
    pub fn layout_readiness(&mut self, key: &str, pending: bool, errors: Vec<String>) {
        if let Some(rt) = self.runtime.get_mut(key) {
            rt.layout_pending = pending;
            rt.asset_errors = errors;
        }
    }
    pub fn reachable_target(&mut self, key: &str, target: [f32; 2]) -> [f32; 2] {
        let Some(rt) = self.runtime.get_mut(key) else {
            return target;
        };
        if rt.layout_pending {
            return rt.presented_offset.unwrap_or(target);
        }
        let Some(_) = rt.presented_offset else {
            return target;
        };
        let reachable = [
            target[0].clamp(0., rt.presented_max[0].max(0.)),
            target[1].clamp(0., rt.presented_max[1].max(0.)),
        ];
        rt.range_clamp = (reachable
            .iter()
            .zip(target)
            .any(|(a, b)| (*a - b).abs() > 0.1))
        .then(|| {
            if rt.asset_errors.is_empty() {
                "reachable-document-range"
            } else {
                "asset-error-document-range"
            }
            .into()
        });
        reachable
    }
    pub fn restore_status(&self, id: DocumentId, pane: &str) -> Option<&RestoreStatus> {
        self.records
            .get(&id)?
            .restore_status
            .iter()
            .filter(|(key, _)| key.starts_with(&format!("{pane}/")))
            .map(|(_, status)| status)
            .max_by_key(|status| status.updated_ms)
    }
    pub fn target(&mut self, key: &str, target: [f32; 2], ctx: &egui::Context) {
        self.target_inner(key, target, ctx);
        let Some(rt) = self.runtime.get(key) else {
            return;
        };
        let Some(decision) = rt
            .relocation
            .as_ref()
            .filter(|d| matches!(d["status"].as_str(), Some("completed" | "layout-degraded")))
        else {
            return;
        };
        let tier = match decision["tier"].as_str() {
            Some("text") => reading_precision::Precision::Text,
            Some("source-line") => reading_precision::Precision::SourceLine,
            Some("block") => reading_precision::Precision::Block,
            Some("section") => reading_precision::Precision::Section,
            Some("approximate") => reading_precision::Precision::Approximate,
            _ => reading_precision::Precision::Invalid,
        };
        let reason = if decision["status"] == "layout-degraded" {
            RestoreReason::LayoutUnavailable
        } else {
            match tier {
                reading_precision::Precision::Text | reading_precision::Precision::SourceLine => {
                    RestoreReason::Found
                }
                reading_precision::Precision::Block | reading_precision::Precision::Section => {
                    RestoreReason::MissingOrAmbiguous
                }
                reading_precision::Precision::Approximate => RestoreReason::NoCorrespondence,
                reading_precision::Precision::Invalid => RestoreReason::NoValidPosition,
            }
        };
        let Some(record) = rt.id.and_then(|id| self.records.get_mut(&id)) else {
            return;
        };
        if record
            .restore_status
            .get(key)
            .is_none_or(|s| s.tier != tier || s.reason != reason)
        {
            record.restore_status.insert(
                key.into(),
                RestoreStatus {
                    tier,
                    reason,
                    updated_ms: now(),
                },
            );
            record.updated_ms = now().max(record.updated_ms.saturating_add(1));
            self.mark_dirty();
        }
    }
    fn target_inner(&mut self, key: &str, target: [f32; 2], ctx: &egui::Context) {
        if let Some(rt) = self.runtime.get_mut(key) {
            // Ordinary source builds its ScrollArea before the galley can
            // locate a coarse changed-content fallback. Carry that committed
            // value into the following pass instead of reapplying old pixels.
            if rt.block_relocation && rt.pending.is_some() {
                rt.precise_target = Some(target);
            }
            rt.passes = rt.passes.saturating_add(1);
            let elapsed = rt
                .restore_started
                .get_or_insert_with(Instant::now)
                .elapsed();
            let timed_out = if rt.layout_pending {
                rt.ready_started = None;
                rt.ready_passes = 0;
                elapsed >= Duration::from_secs(30)
            } else {
                rt.ready_passes = rt.ready_passes.saturating_add(1);
                rt.ready_passes >= 120
                    || rt.ready_started.get_or_insert_with(Instant::now).elapsed()
                        >= Duration::from_secs(3)
            };
            let asset_failed = !rt.asset_errors.is_empty() && rt.range_clamp.is_some();
            if rt.pending.is_some() && (timed_out || asset_failed) {
                if let Some(decision) = &mut rt.relocation {
                    decision["exact"] = serde_json::json!(false);
                    if decision["tier"] == "text" {
                        decision["tier"] = serde_json::json!("block");
                    }
                    decision["status"] = serde_json::json!("layout-degraded");
                    decision["layout_reason"] =
                        serde_json::json!("target-not-verifiably-presented");
                }
                rt.failed_restore = Some(serde_json::json!({
                    "tier":"block","exact":false,"reason":if asset_failed {"asset-error-history-preserved"} else if rt.layout_pending {"async-layout-timeout"} else {"unreachable-or-unmapped-text"},
                    "saved":rt.pending.as_ref().map(|p|&p.position),"requested_target":target,
                    "presented_offset":rt.presented_offset,"passes":rt.passes,"elapsed_ms":elapsed.as_secs_f64()*1000.
                }));
                rt.pending = None;
                rt.suspended = true;
                rt.precise_target = None;
                rt.target = rt.presented_offset;
                rt.reason = "restore-degraded-history-preserved".into();
                return;
            }
            let presented = rt.presented_offset.is_none_or(|actual| {
                (actual[0] - target[0]).abs() <= 2. && (actual[1] - target[1]).abs() <= 2.
            });
            let text_ready = rt
                .pending
                .as_ref()
                .is_none_or(|p| p.position.text_anchor.is_none() || rt.geometry.is_some());
            if !rt.layout_pending
                && presented
                && text_ready
                && rt.target.is_some_and(|last| {
                    (last[0] - target[0]).abs() < 0.5 && (last[1] - target[1]).abs() < 0.5
                })
            {
                rt.stable = rt.stable.saturating_add(1);
            } else {
                rt.stable = 0;
            }
            rt.target = Some(target);
            if rt.stable >= 3 {
                rt.reason = rt.range_clamp.clone().unwrap_or_else(|| {
                    rt.pending
                        .as_ref()
                        .map_or("ready".into(), |p| p.tier.clone())
                });
                rt.pending = None;
                if let Some(decision) = &mut rt.relocation {
                    decision["status"] = serde_json::json!("completed");
                    decision["actual_geometry"] = serde_json::json!(rt.geometry);
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(20));
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        doc: &Document,
        key: &str,
        layout: String,
        offset: [f32; 2],
        viewport: [f32; 2],
        byte: usize,
        fraction: f32,
        viewport_y: f32,
        active: bool,
    ) {
        let started = Instant::now();
        if !self.records.contains_key(&doc.id) {
            return;
        }
        if self
            .runtime
            .get(key)
            .is_some_and(|rt| rt.pending.is_some() || rt.suspended)
        {
            return;
        }
        let index = self.index(doc);
        if index.revision != doc.revision() {
            return;
        }
        let fingerprint = index.fingerprint;
        let anchor = index.anchor(&doc.text, byte, fraction, viewport_y);
        let text_anchor = self.runtime.get(key).and_then(|r| r.pinned.clone());
        let position = Position {
            text_anchor,
            offset,
            viewport,
            anchor,
            layout,
            fingerprint,
            updated_ms: now(),
        };
        let rt = self.runtime.entry(key.into()).or_default();
        rt.last = Some(position.clone());
        let Some(record) = self.records.get_mut(&doc.id) else {
            return;
        };
        let changed = record.views.get(key).is_none_or(|p| {
            p.offset != position.offset
                || p.anchor.byte != position.anchor.byte
                || p.layout != position.layout
                || p.fingerprint != fingerprint
                || p.text_anchor != position.text_anchor
        });
        if changed {
            record.deleted = false;
            record.furthest = record.furthest.max(position.anchor.approximate);
            record.views.insert(key.into(), position);
            record.updated_ms = now()
                .max(record.updated_ms.saturating_add(1))
                .max(self.cleared_ms.saturating_add(1));
            if active {
                record.last_view = key.into();
            }
            self.mark_dirty();
        }
        let ms = started.elapsed().as_secs_f64() * 1000.;
        sample(&mut self.capture_cpu_ms, ms);
        self.hot_sample(ms);
    }
    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.changed = Instant::now();
    }
    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(writer) = &self.writer else {
            return;
        };
        let started = Instant::now();
        let records = self
            .records
            .values()
            .filter(|r| {
                self.sent
                    .get(&r.id)
                    .is_none_or(|stamp| *stamp < r.updated_ms)
            })
            .filter_map(|r| r.path.as_ref().map(|p| (p.clone(), r.clone())))
            .collect();
        let protected = self
            .runtime
            .values()
            .filter_map(|rt| rt.id)
            .filter_map(|id| self.records.get(&id).and_then(|r| r.path.clone()))
            .collect();
        if writer
            .tx
            .send(Some(PendingWrite {
                records,
                cleared_ms: self.cleared_ms,
                protected,
                clears: std::mem::take(&mut self.pending_clears),
                activities: self
                    .activity_events
                    .iter()
                    .filter_map(|(id, event)| {
                        self.records
                            .get(id)
                            .and_then(|r| r.path.as_ref())
                            .map(|p| (p.clone(), *event))
                    })
                    .collect(),
            }))
            .is_ok()
        {
            for r in self.records.values() {
                self.sent.insert(r.id, r.updated_ms);
            }
            self.dirty = false;
            self.submitted = Instant::now();
        }
        let ms = started.elapsed().as_secs_f64() * 1000.;
        sample(&mut self.submit_cpu_ms, ms);
        self.hot_sample(ms);
    }
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.remember_context(ctx);
        if !self.index_requested.is_empty()
            || self
                .runtime
                .values()
                .any(|r| r.proof_job.is_some() || r.relocation_job.is_some())
        {
            ctx.request_repaint_after(Duration::from_millis(20));
        }
        if let Some(writer) = &self.writer {
            while let Ok(result) = writer.rx.try_recv() {
                match result {
                    Ok(stats) => self.stats = stats,
                    Err((error, mut clears)) => {
                        clears.append(&mut self.pending_clears);
                        self.pending_clears = clears;
                        self.stats.error = Some(error);
                        self.dirty = true;
                        self.sent.clear();
                        self.submitted = Instant::now();
                    }
                }
            }
        }
        if self.dirty
            && (self.changed.elapsed() >= Duration::from_millis(500)
                || self.submitted.elapsed() >= Duration::from_secs(3))
        {
            self.flush();
        }
        if self.dirty {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
    pub fn copied_path(&mut self, doc: &mut Document, old_id: DocumentId, pane: &str) {
        let Some(path) = doc.path.as_deref().map(path_key) else {
            return;
        };
        let mut record = self.records.get(&old_id).cloned().unwrap_or_default();
        let id = DocumentId::default();
        record.id = id;
        record.path = Some(path.clone());
        record.updated_ms = now();
        doc.id = id;
        self.activity_events.insert(id, event_ns());
        self.records.insert(id, record);
        self.paths.insert(path, id);
        for (key, rt) in self.runtime.iter_mut() {
            if key.starts_with(&format!("{pane}/")) && rt.id == Some(old_id) {
                rt.cancel_precision();
                rt.id = Some(id);
                let position = rt
                    .last
                    .clone()
                    .or_else(|| self.records[&id].views.get(key).cloned());
                rt.pending = position.map(|position| Restore {
                    position,
                    tier: "pending-layout".into(),
                    exact: false,
                });
                rt.passes = 0;
                rt.stable = 0;
                rt.target = None;
                rt.calibrated = false;
                rt.calibration_stamp = 0;
            }
        }
        self.mark_dirty();
        self.flush();
    }
    pub fn migrated_path(&mut self, doc: &Document, old: &Path) {
        // The old file no longer exists after rename. Re-canonicalizing an
        // aliased parent (e.g. Windows 8.3 TEMP) can produce a different key.
        // Migrate the key already bound to this document's persisted identity.
        let old = self
            .records
            .get(&doc.id)
            .and_then(|record| record.path.clone())
            .unwrap_or_else(|| path_key(old));
        let Some(next) = doc.path.as_deref().map(path_key) else {
            return;
        };
        self.paths.remove(&old);
        self.paths.insert(next.clone(), doc.id);
        self.pending_clears.push(ClearRequest {
            path: Some(old.clone()),
            requested_ns: event_ns(),
            rename_to: Some(next.clone()),
        });
        if let Some(record) = self.records.get_mut(&doc.id) {
            let mut tombstone = record.clone();
            tombstone.path = Some(old);
            tombstone.deleted = true;
            tombstone.updated_ms = now();
            let tombstone_id = DocumentId::default();
            tombstone.id = tombstone_id;
            record.path = Some(next);
            record.updated_ms = now().max(record.updated_ms.saturating_add(1));
            self.records.insert(tombstone_id, tombstone);
        }
        // Submission stamps are keyed by identity, whereas this transaction
        // changes the persistence key. Even a seed still queued on the writer
        // must be followed by the complete record at its new path.
        self.sent.remove(&doc.id);
        self.mark_dirty();
        self.flush();
    }
    pub fn clear(&mut self, id: Option<DocumentId>) {
        let requested_ns = event_ns();
        if id.is_none()
            || id
                .and_then(|id| self.records.get(&id))
                .and_then(|r| r.path.as_ref())
                .is_some()
        {
            self.pending_clears.push(ClearRequest {
                path: id
                    .and_then(|id| self.records.get(&id))
                    .and_then(|r| r.path.clone()),
                requested_ns,
                rename_to: None,
            });
        }
        self.clear_epoch = self.clear_epoch.wrapping_add(1);
        for rt in self.runtime.values_mut() {
            rt.cancel_precision();
            rt.relocation = None;
            rt.block_relocation = false;
        }
        self.precision_results.clear();
        for control in self.index_tokens.values() {
            control.cancel();
        }
        self.index_tokens.clear();
        self.index_requested.clear();
        self.index_epoch = self.index_epoch.wrapping_add(1);
        if let Some(id) = id {
            if let Some(r) = self.records.get_mut(&id) {
                empty_record(r);
                r.updated_ms = r.updated_ms.saturating_add(1).max(now());
            }
            self.activity_events.remove(&id);
            for rt in self.runtime.values_mut() {
                if rt.id == Some(id) {
                    rt.pending = None;
                    rt.last = None;
                    rt.pinned = None;
                    rt.geometry = None;
                    rt.visible_text.clear();
                    rt.visible_runs.clear();
                    rt.reason = "cleared".into();
                    rt.suspended = true;
                }
            }
        } else {
            self.cleared_ms = now();
            self.activity_events.clear();
            self.records
                .retain(|id, _| self.runtime.values().any(|rt| rt.id == Some(*id)));
            self.paths.retain(|_, id| self.records.contains_key(id));
            for r in self.records.values_mut() {
                empty_record(r);
                r.updated_ms = r
                    .updated_ms
                    .saturating_add(1)
                    .max(self.cleared_ms.saturating_add(1));
            }
            for rt in self.runtime.values_mut() {
                rt.pending = None;
                rt.last = None;
                rt.pinned = None;
                rt.geometry = None;
                rt.visible_text.clear();
                rt.visible_runs.clear();
                rt.reason = "cleared".into();
                rt.suspended = true;
            }
        }
        self.mark_dirty();
        self.flush();
    }
    pub fn report(&self, main: &Document, secondary: Option<&Document>) -> serde_json::Value {
        let views=self.runtime.iter().map(|(key,rt)|(key.clone(),serde_json::json!({"document_id":rt.id,"open_generation":rt.generation,"position":rt.last,"pending":rt.pending,"buffer_id":rt.buffer_id,"revision":rt.revision,"relocation":rt.relocation,"proof_pending":rt.proof_job.as_ref().map(JobControl::report),"match_pending":rt.relocation_job.as_ref().map(JobControl::report),"reason":rt.reason,"target":rt.target,"passes":rt.passes,"layout_pending":rt.layout_pending,"asset_errors":rt.asset_errors,"range_clamp":rt.range_clamp,"presented_offset":rt.presented_offset,"failed_restore":rt.failed_restore,"cancel_events":rt.cancel_events,"viewport_rect":rt.viewport_rect,"geometry":rt.geometry,"precision_tier":if rt.geometry.is_some(){"text"}else{"block"},"precision_reason":if rt.geometry.is_some(){"actual-galley-source-provenance"}else{"no-visible-mapped-text"},"visible_text":rt.visible_text,"visible_runs":rt.visible_runs}))).collect::<serde_json::Map<_,_>>();
        serde_json::json!({"schema":SCHEMA,"main_id":main.id,"secondary_id":secondary.map(|d|d.id),"main_revision":main.revision(),"secondary_revision":secondary.map(|d|d.revision()),"main_buffer_id":main.buffer_id(),"secondary_buffer_id":secondary.map(|d|d.buffer_id()),"path":state_path(),"views":views,"records":self.records.values().collect::<Vec<_>>(),"writes":self.stats,"dirty":self.dirty,"background_tasks":self.task_history.iter().map(JobControl::report).collect::<Vec<_>>(),"metrics":{"position_capture_cpu_ms":self.capture_cpu_ms,"submit_cpu_ms":self.submit_cpu_ms,"index_build_ms":self.index_build_ms,"enrichment_wall_ms":self.enrichment_ms,"match_wall_ms":self.matching_ms,"geometry_wall_ms":self.geometry_ms,"plan_wall_ms":self.plan_ms,"hot_frame_wall_ms":self.hot_samples.iter().map(|(_,ms)|*ms).collect::<Vec<_>>(),"geometry_cache_bytes":self.geometry_cache.bytes(),"galley_index_wall_ms":self.geometry_cache.index_timings(),"layout_wall_ms":&*self.layout_metrics.lock().unwrap(),"main_commit_wall_ms":self.commit_ms,"note":"All Instant.elapsed values are function wall-clock intervals, not thread CPU. hot_frame_wall_ms sums plan, actual-geometry capture/galley indexing, record capture and submit in each observed frame. Actual font layout/rendering is separate. Heavy visible_text/visible_runs diagnostics are opt-in. Submit clones dirty records and enqueues; disk transaction runs on worker. Diagnostic JSON writes are separate."},"max_records":MAX_RECORDS,"max_bytes":MAX_BYTES,"precision":"actual mapped grapheme where geometry is available; explicit block fallback otherwise","current_position_label":"当前位置"})
    }
}
fn sample(samples: &mut std::collections::VecDeque<f64>, value: f64) {
    if samples.len() == 2000 {
        samples.pop_front();
    }
    samples.push_back(value);
}
impl Drop for ReadingState {
    fn drop(&mut self) {
        for control in self.index_tokens.values() {
            control.cancel();
        }
        for rt in self.runtime.values_mut() {
            rt.cancel_precision();
        }
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_rename_immediately_after_a_queued_seed_keeps_the_new_path_record() {
        for clear_neighbor in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("state.json");
            // A parent alias reproduces Windows short TEMP paths on Unix too:
            // the existing file canonicalizes, but its removed old name cannot.
            #[cfg(unix)]
            let source_dir = {
                let actual = temp.path().join("actual");
                fs::create_dir(&actual).unwrap();
                let alias = temp.path().join("alias");
                std::os::unix::fs::symlink(&actual, &alias).unwrap();
                alias
            };
            #[cfg(not(unix))]
            let source_dir = temp.path().to_owned();
            let old = source_dir.join("original.md");
            let source = "Original 中文🙂 source\n".repeat(100);
            fs::write(&old, &source).unwrap();
            let old_key = path_key(&old);
            let mut doc = Document::open(&old).unwrap();
            let id;
            {
                let mut book = ReadingState::new(Some(path.clone()));
                book.bind(&mut doc, "main", 1);
                id = doc.id;
                book.observe(
                    &doc,
                    "main/preview",
                    "layout".into(),
                    [0., 700.],
                    [800., 600.],
                    100,
                    0.,
                    0.,
                    true,
                );
                book.store_selection(&doc, "main/source", [2, 7]);
                let mut neighbor = Document::from_text("neighbor");
                neighbor.path = Some(temp.path().join("other.md"));
                book.bind(&mut neighbor, "secondary", 1);
                if clear_neighbor {
                    book.clear(Some(neighbor.id));
                }
                book.flush();
                doc.rename_in_place("renamed.md").unwrap();
                book.migrated_path(&doc, &old);
                book.flush();
            }
            let next = doc.path.as_deref().unwrap();
            let disk = load(&path).unwrap();
            assert!(
                disk.records.contains_key(&path_key(next)),
                "rename after queued seed clear_neighbor={clear_neighbor}: {}",
                serde_json::to_string(&disk).unwrap()
            );
            let moved = &disk.records[&path_key(next)];
            assert!(
                disk.records
                    .get(&old_key)
                    .is_none_or(|record| record.deleted)
            );
            assert_eq!(moved.id, id);
            assert!(
                !moved.views.is_empty(),
                "clear_neighbor={clear_neighbor}: {}",
                serde_json::to_string(&disk).unwrap()
            );
            assert_eq!(moved.views["main/preview"].offset, [0., 700.]);
            assert_eq!(moved.selections["main/source"], [2, 7]);
            assert_eq!(fs::read_to_string(next).unwrap(), source);
        }
    }
    #[test]
    fn real_rename_preserves_identity_and_views_after_an_unrelated_clear_and_eviction() {
        for capacity in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let state_path = temp.path().join("state.json");
            let old = temp.path().join("a.md");
            let source = "# Rename source\n\n中文🙂 paragraph.\n".repeat(50);
            fs::write(&old, &source).unwrap();
            let mut doc = Document::open(&old).unwrap();
            let id;
            {
                let mut book = ReadingState::new(Some(state_path.clone()));
                book.bind(&mut doc, "main", 1);
                id = doc.id;
                book.observe(
                    &doc,
                    "main/source",
                    "layout".into(),
                    [0., 700.],
                    [800., 600.],
                    100,
                    0.,
                    0.,
                    true,
                );
                book.store_selection(&doc, "main/source", [2, 7]);
            }
            let mut disk = load(&state_path).unwrap();
            let unrelated = Record {
                path: Some("neighbor".into()),
                ..Record::default()
            };
            disk.records.insert("neighbor".into(), unrelated);
            if capacity {
                for i in 0..MAX_RECORDS {
                    disk.records.insert(
                        format!("historical-{i}"),
                        Record {
                            updated_ms: 1,
                            ..Record::default()
                        },
                    );
                }
            }
            fs::write(&state_path, serde_json::to_vec(&disk).unwrap()).unwrap();
            write_merged(
                &state_path,
                PendingWrite {
                    clears: vec![ClearRequest {
                        path: Some("neighbor".into()),
                        requested_ns: event_ns(),
                        rename_to: None,
                    }],
                    protected: vec![path_key(&old)],
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            {
                let mut book = ReadingState::new(Some(state_path.clone()));
                book.bind(&mut doc, "main", 2);
                doc.rename_in_place("renamed.md").unwrap();
                book.migrated_path(&doc, &old);
            }
            let next = temp.path().join("renamed.md");
            let persisted = load(&state_path).unwrap();
            let saved = &persisted.records[&path_key(&next)];
            assert_eq!(saved.id, id);
            assert_eq!(saved.views["main/source"].offset, [0., 700.]);
            assert_eq!(saved.selections["main/source"], [2, 7]);
            assert!(
                persisted
                    .records
                    .get(&path_key(&old))
                    .is_none_or(|r| r.deleted)
            );
            let mut reopened = Document::open(&next).unwrap();
            let mut book = ReadingState::new(Some(state_path));
            book.bind(&mut reopened, "main", 3);
            assert_eq!(reopened.id, id);
            assert_eq!(book.records[&id].views["main/source"].offset, [0., 700.]);
            assert_eq!(fs::read_to_string(&next).unwrap(), source);
            assert!(!old.exists());
        }
    }
    #[test]
    fn same_instance_scroll_after_its_own_future_clear_has_a_new_submission_stamp() {
        for all in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("state.json");
            let mut doc =
                Document::from_text("Original 中文🙂 paragraph.\n\nAnother original paragraph.");
            doc.path = Some(temp.path().join("a.md"));
            fs::write(doc.path.as_ref().unwrap(), &doc.text).unwrap();
            let mut book = ReadingState::new(Some(path.clone()));
            book.bind(&mut doc, "main", 1);
            book.observe(
                &doc,
                "main/preview",
                "layout".into(),
                [0., 800.],
                [800., 600.],
                10,
                0.,
                0.,
                true,
            );
            book.records.get_mut(&doc.id).unwrap().updated_ms = now() + 3_600_000;
            book.flush();
            book.clear(if all { None } else { Some(doc.id) });
            let cleared_stamp = book.records[&doc.id].updated_ms;
            let ctx = egui::Context::default();
            ctx.begin_pass(egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(100., 100.)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0., -80.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..egui::RawInput::default()
            });
            book.plan(
                &doc,
                "main/preview",
                1,
                "layout",
                &ctx,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
                false,
            );
            book.observe(
                &doc,
                "main/preview",
                "layout".into(),
                [0., 450.],
                [800., 600.],
                20,
                0.,
                0.,
                true,
            );
            let _ = ctx.end_pass();
            assert!(book.records[&doc.id].updated_ms > cleared_stamp);
            book.flush();
            drop(book);
            let disk = load(&path).unwrap();
            let record = &disk.records[&path_key(doc.path.as_ref().unwrap())];
            assert_eq!(record.views["main/preview"].offset[1], 450.);
            assert_eq!(
                fs::read_to_string(doc.path.as_ref().unwrap()).unwrap(),
                doc.text
            );
        }
    }
    #[test]
    fn explicit_clear_transactions_cover_future_snapshots_but_preserve_new_activity() {
        for all in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("state.json");
            let future = now() + 3_600_000;
            let mut original = record("a", future);
            original.activity_ms = future;
            original.activity_ns = event_ns() + 3_600_000_000_000;
            original.views.insert(
                "main/preview".into(),
                Position {
                    offset: [0., 900.],
                    ..Position::default()
                },
            );
            write_merged(
                &path,
                PendingWrite {
                    records: BTreeMap::from([
                        ("a".into(), original.clone()),
                        ("inactive".into(), original.clone()),
                    ]),
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            let requested_ns = event_ns();
            write_merged(
                &path,
                PendingWrite {
                    clears: vec![ClearRequest {
                        path: (!all).then(|| "a".into()),
                        requested_ns,
                        rename_to: None,
                    }],
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            let cleared = load(&path).unwrap();
            let tombstone = cleared.records["a"].clone();
            assert!(tombstone.deleted && tombstone.views.is_empty());
            assert_eq!(cleared.records["inactive"].deleted, all);
            let mut late = original.clone();
            late.updated_ms += 1000; // Proof/summary completion is not real input.
            write_merged(
                &path,
                PendingWrite {
                    records: BTreeMap::from([
                        ("a".into(), late.clone()),
                        ("inactive".into(), late),
                    ]),
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            assert!(load(&path).unwrap().records["a"].views.is_empty());
            if all {
                assert!(load(&path).unwrap().records["inactive"].views.is_empty());
            }
            std::thread::sleep(Duration::from_millis(1));
            let new_event = event_ns();
            let mut genuine = original;
            genuine.updated_ms = now(); // Valid old +1h metadata must not block this.
            genuine.activity_ms = now();
            genuine.activity_ns = new_event;
            genuine.views.get_mut("main/preview").unwrap().offset[1] = 450.;
            write_merged(
                &path,
                PendingWrite {
                    records: BTreeMap::from([("a".into(), genuine)]),
                    activities: BTreeMap::from([("a".into(), new_event)]),
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            assert_eq!(
                load(&path).unwrap().records["a"].views["main/preview"].offset[1],
                450.
            );
            let mut late_deleted = tombstone;
            late_deleted.updated_ms += 5000;
            write_merged(
                &path,
                PendingWrite {
                    records: BTreeMap::from([("a".into(), late_deleted)]),
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            assert_eq!(
                load(&path).unwrap().records["a"].views["main/preview"].offset[1],
                450.
            );
        }
    }
    #[test]
    fn clear_waiting_for_lock_does_not_erase_a_later_committed_real_activity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let requested_ns = event_ns();
        std::thread::sleep(Duration::from_millis(1));
        let input = event_ns();
        let mut newer = record("a", now());
        newer.views.insert(
            "main/preview".into(),
            Position {
                offset: [0., 450.],
                ..Position::default()
            },
        );
        write_merged(
            &path,
            PendingWrite {
                records: BTreeMap::from([("a".into(), newer)]),
                activities: BTreeMap::from([("a".into(), input)]),
                ..PendingWrite::default()
            },
        )
        .unwrap();
        write_merged(
            &path,
            PendingWrite {
                clears: vec![ClearRequest {
                    path: None,
                    requested_ns,
                    rename_to: None,
                }],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(restored.records["a"].views["main/preview"].offset[1], 450.);
        assert_eq!(restored.records["a"].clear_serial, restored.cleared_serial);
    }
    #[test]
    fn clear_transaction_fields_default_safely_and_reject_invalid_or_exhausted_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let legacy = serde_json::json!({"version":3,"records":{"a":{"path":"a","updated_ms":10}}});
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(load(&path).unwrap().clear_serial, 0);
        assert_eq!(load(&path).unwrap().records["a"].clear_serial, 0);
        assert_eq!(load(&path).unwrap().latest_clear_ns, 0);
        for (field, value) in [
            ("/clear_serial", serde_json::json!(u64::MAX)),
            ("/clear_serial", serde_json::json!("invalid")),
            ("/cleared_serial", serde_json::json!(1)),
            ("/cleared_ns", serde_json::json!(u64::MAX)),
            ("/latest_clear_ns", serde_json::json!(u64::MAX)),
            ("/latest_clear_ns", serde_json::json!("invalid")),
            ("/latest_clear_ns", serde_json::json!(-1)),
            ("/records/a/clear_serial", serde_json::json!(1)),
            ("/records/a/cleared_ns", serde_json::json!(u64::MAX)),
            ("/records/a/activity_ns", serde_json::json!(u64::MAX)),
        ] {
            let mut malformed = legacy.clone();
            let (parent, name) = field.rsplit_once('/').unwrap();
            malformed
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(name.into(), value);
            let original = serde_json::to_vec(&malformed).unwrap();
            fs::write(&path, &original).unwrap();
            assert!(load(&path).is_err(), "{field}");
            assert!(
                write_merged(&path, PendingWrite::default()).is_err(),
                "{field}"
            );
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        let exhausted =
            serde_json::json!({"version":3,"clear_serial":MAX_CLEAR_SERIAL,"records":{}});
        let original = serde_json::to_vec(&exhausted).unwrap();
        fs::write(&path, &original).unwrap();
        assert!(
            write_merged(
                &path,
                PendingWrite {
                    clears: vec![ClearRequest {
                        path: None,
                        requested_ns: event_ns(),
                        rename_to: None,
                    }],
                    ..PendingWrite::default()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    #[test]
    fn completed_buffer_rebind_does_not_become_new_activity_or_revive_external_clear() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut doc = Document::from_text("Original secondary source remains available.");
        doc.path = Some(temp.path().join("a.md"));
        let mut book = ReadingState::new(Some(path.clone()));
        book.bind(&mut doc, "secondary", 1);
        let r = book.records.get_mut(&doc.id).unwrap();
        r.activity_ms = 10;
        r.updated_ms = 20;
        r.views.insert(
            "secondary/source".into(),
            Position {
                offset: [0., 500.],
                ..Position::default()
            },
        );
        let id = doc.id;
        let mut next = doc.clone();
        book.rebind_after_commit(&mut next, "secondary", 2);
        assert_eq!(book.records[&id].activity_ms, 10);
        write_merged(
            &path,
            PendingWrite {
                records: BTreeMap::new(),
                cleared_ms: 100,
                protected: vec![],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        let mut later = next.clone();
        book.rebind_after_commit(&mut later, "secondary", 3);
        assert_eq!(book.records[&id].activity_ms, 10);
        assert!(book.records[&id].views.is_empty());
        assert!(book.runtime["secondary/source"].pending.is_none());
        assert_eq!(book.runtime["secondary/source"].generation, 3);
        assert_eq!(
            book.runtime["secondary/source"].buffer_id,
            later.buffer_id()
        );
        assert_eq!(later.text, doc.text);
    }
    #[test]
    fn missing_record_fence_survives_legacy_clock_absence_and_allows_a_fresh_bind() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let file = temp.path().join("fresh.md");
        let source = "# A new genuine open\n\nOriginal source remains available.\n\n".repeat(30);
        fs::write(&file, &source).unwrap();
        for clock in [event_ns(), 0] {
            fs::write(
                &path,
                serde_json::to_vec(&Disk {
                    version: SCHEMA,
                    clear_serial: 1,
                    latest_clear_ns: clock,
                    ..Disk::default()
                })
                .unwrap(),
            )
            .unwrap();
            // A metadata-only snapshot from before the fence stays rejected,
            // including an older file that no longer retains a clear clock.
            write_merged(
                &path,
                PendingWrite {
                    records: BTreeMap::from([("stale".into(), record("stale", now()))]),
                    ..PendingWrite::default()
                },
            )
            .unwrap();
            assert!(load(&path).unwrap().records.is_empty());
            {
                let mut doc = Document::open(&file).unwrap();
                let mut book = ReadingState::new(Some(path.clone()));
                book.bind(&mut doc, "main", 1);
                assert_eq!(book.records[&doc.id].clear_serial, 1);
                book.observe(
                    &doc,
                    "main/preview",
                    "layout".into(),
                    [0., 450.],
                    [800., 600.],
                    100,
                    0.,
                    0.,
                    true,
                );
                book.flush();
            }
            assert_eq!(
                load(&path).unwrap().records[&path_key(&file)].views["main/preview"].offset[1],
                450.
            );
            assert_eq!(fs::read_to_string(&file).unwrap(), source);
        }
        let mut older = serde_json::to_value(Disk {
            version: SCHEMA,
            clear_serial: 1,
            cleared_ns: 10,
            records: BTreeMap::from([(
                "old-cleared".into(),
                Record {
                    clear_serial: 1,
                    cleared_ns: 20,
                    ..Record::default()
                },
            )]),
            ..Disk::default()
        })
        .unwrap();
        older.as_object_mut().unwrap().remove("latest_clear_ns");
        fs::write(&path, serde_json::to_vec(&older).unwrap()).unwrap();
        assert_eq!(load(&path).unwrap().latest_clear_ns, 20);
    }
    #[test]
    fn extreme_or_implausibly_future_timestamps_reject_writes_without_losing_bytes_or_memory() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut invalid_new = record("new", u64::MAX);
        invalid_new.activity_ms = u64::MAX;
        assert!(write(&path, vec![("new", invalid_new)], vec![]).is_err());
        assert!(
            !path.exists(),
            "A new file must not publish an invalid timestamp"
        );
        let mut entry = record("a", 20);
        entry
            .views
            .insert("main/preview".into(), Position::default());
        entry.restore_status.insert(
            "main/preview".into(),
            RestoreStatus {
                tier: reading_precision::Precision::Block,
                reason: RestoreReason::MissingOrAmbiguous,
                updated_ms: 20,
            },
        );
        let base = serde_json::to_value(Disk {
            version: SCHEMA,
            records: BTreeMap::from([("a".into(), entry)]),
            ..Disk::default()
        })
        .unwrap();
        for pointer in [
            "/cleared_ms",
            "/records/a/updated_ms",
            "/records/a/activity_ms",
            "/records/a/views/main~1preview/updated_ms",
            "/records/a/restore_status/main~1preview/updated_ms",
        ] {
            for invalid in [u64::MAX, now().saturating_add(MAX_FUTURE_SKEW_MS * 2)] {
                let mut corrupted = base.clone();
                *corrupted.pointer_mut(pointer).unwrap() = invalid.into();
                let bytes = serde_json::to_vec(&corrupted).unwrap();
                fs::write(&path, &bytes).unwrap();
                assert!(load(&path).is_err(), "{pointer}");
                assert!(write(&path, vec![("b", record("b", 30))], vec![]).is_err());
                assert_eq!(fs::read(&path).unwrap(), bytes);
                let mut memory = ReadingState::new(Some(path.clone()));
                assert!(memory.stats.error.as_ref().unwrap().contains("timestamp"));
                let mut doc = Document::from_text("The manuscript stays available in memory.");
                doc.path = Some(temp.path().join("a.md"));
                memory.bind(&mut doc, "main", 1);
                memory.observe(
                    &doc,
                    "main/preview",
                    "layout".into(),
                    [0., 360.],
                    [600., 400.],
                    0,
                    0.,
                    0.,
                    true,
                );
                assert_eq!(
                    memory.records[&doc.id].views["main/preview"].offset[1],
                    360.
                );
                drop(memory);
                assert_eq!(fs::read(&path).unwrap(), bytes);
            }
        }
    }
    #[test]
    fn future_or_damaged_disk_is_not_replaced_by_a_background_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        for original in [b"{\"version\":999,\"future\":true}".as_slice(), b"{"] {
            fs::write(&path, original).unwrap();
            assert!(write(&path, vec![("a", record("a", 20))], vec![]).is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        assert_eq!(load(&path).err().unwrap().kind(), io::ErrorKind::Other);
    }
    #[test]
    fn versioned_default_imports_legacy_read_only_and_never_reimports_after_clear() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join("reading-state.json");
        let path = temp.path().join("reading-state-v3.json");
        let mut old = serde_json::to_value(Disk {
            version: 2,
            records: BTreeMap::from([("a".into(), record("a", 20))]),
            ..Disk::default()
        })
        .unwrap();
        old["records"]["a"]
            .as_object_mut()
            .unwrap()
            .remove("activity_ms");
        fs::write(&legacy, serde_json::to_vec(&old).unwrap()).unwrap();
        let original = fs::read(&legacy).unwrap();
        let migrated = load_with_legacy(&path, Some(&legacy)).unwrap();
        assert_eq!(migrated.version, SCHEMA);
        assert_eq!(migrated.records["a"].activity_ms, 20);
        write_merged(
            &path,
            PendingWrite {
                records: migrated.records,
                cleared_ms: 0,
                protected: vec![],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        assert_eq!(fs::read(&legacy).unwrap(), original);
        write_merged(
            &path,
            PendingWrite {
                records: BTreeMap::new(),
                cleared_ms: 30,
                protected: vec![],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        assert!(
            load_with_legacy(&path, Some(&legacy))
                .unwrap()
                .records
                .is_empty()
        );
        assert_eq!(fs::read(&legacy).unwrap(), original);
    }
    #[test]
    fn another_instances_clear_is_observed_before_reopening_old_memory_history() {
        for clear_all in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("state.json");
            let mut a = Document::from_text("Original A manuscript.\n\nAnother paragraph.");
            a.path = Some(temp.path().join("a.md"));
            let mut b = Document::from_text("Independent B manuscript.");
            b.path = Some(temp.path().join("b.md"));
            let mut old = ReadingState::new(Some(path.clone()));
            old.bind(&mut a, "main", 1);
            let record = old.records.get_mut(&a.id).unwrap();
            record.views.insert(
                "main/preview".into(),
                Position {
                    offset: [0., 640.],
                    ..Position::default()
                },
            );
            record.activity_ms = 1;
            record.updated_ms = 1;
            write(
                &path,
                vec![(path_key(a.path.as_ref().unwrap()).as_str(), record.clone())],
                vec![],
            )
            .unwrap();
            let mut other = ReadingState::new(Some(path.clone()));
            other.clear(if clear_all { None } else { Some(a.id) });
            drop(other); // Waits for the separate writer's actual OS-locked transaction.
            std::thread::sleep(Duration::from_millis(2));
            // Completion of an old background job updates metadata, never activity.
            let record = old.records.get_mut(&a.id).unwrap();
            record.updated_ms = now();
            record.restore_status.insert(
                "main/preview".into(),
                RestoreStatus {
                    tier: reading_precision::Precision::Text,
                    reason: RestoreReason::Found,
                    updated_ms: now(),
                },
            );
            old.mark_dirty();
            old.flush();
            // The next open must read the external tombstone before updating activity.
            old.bind(&mut b, "main", 2);
            old.bind(&mut a, "main", 3);
            assert!(old.records[&a.id].views.is_empty());
            assert!(old.restore_status(a.id, "main").is_none());
            assert!(old.runtime["main/preview"].pending.is_none());
            drop(old);
            let disk = load(&path).unwrap();
            assert!(disk.records.values().all(|r| r.views.is_empty()));
            assert!(disk.records.values().all(|r| r.restore_status.is_empty()));
            assert_eq!(a.text, "Original A manuscript.\n\nAnother paragraph.");
        }
    }
    #[test]
    fn background_completion_after_another_instances_clear_does_not_become_new_activity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut before = record("a", 10);
        before.activity_ms = 10;
        let text = relocation_document();
        before.views.insert(
            "main/preview".into(),
            Position {
                text_anchor: Some(TextAnchor {
                    source: reading_precision::capture(&text, 1000),
                    ..TextAnchor::default()
                }),
                ..Position::default()
            },
        );
        write(&path, vec![("a", before.clone())], vec![]).unwrap();
        write_merged(
            &path,
            PendingWrite {
                records: BTreeMap::new(),
                cleared_ms: 100,
                protected: vec![],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        let mut late = before;
        late.updated_ms = 200;
        late.restore_status.insert(
            "main/preview".into(),
            RestoreStatus {
                tier: reading_precision::Precision::Approximate,
                reason: RestoreReason::NoCorrespondence,
                updated_ms: 200,
            },
        );
        write(&path, vec![("a", late.clone())], vec![]).unwrap();
        assert!(!load(&path).unwrap().records.contains_key("a"));
        late.activity_ms = 101;
        late.updated_ms = 201;
        write(&path, vec![("a", late.clone())], vec![]).unwrap();
        assert!(load(&path).unwrap().records.contains_key("a"));
        let mut cleared = late.clone();
        cleared.deleted = true;
        cleared.views.clear();
        cleared.restore_status.clear();
        cleared.updated_ms = 300;
        write(&path, vec![("a", cleared)], vec![]).unwrap();
        late.updated_ms = 400;
        write(&path, vec![("a", late.clone())], vec![]).unwrap();
        assert!(load(&path).unwrap().records["a"].views.is_empty());
        late.activity_ms = 301;
        late.updated_ms = 401;
        write(&path, vec![("a", late)], vec![]).unwrap();
        assert!(!load(&path).unwrap().records["a"].views.is_empty());
    }
    #[test]
    fn user_visible_restore_summary_survives_restart_stays_per_view_and_clears() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("reading.json");
        let mut doc = Document::from_text("Original manuscript remains unchanged.");
        doc.path = Some(temp.path().join("manuscript.md"));
        let mut book = ReadingState::new(Some(state.clone()));
        book.bind(&mut doc, "main", 1);
        book.runtime.get_mut("main/preview").unwrap().relocation =
            Some(serde_json::json!({"status":"completed","tier":"approximate","exact":false}));
        book.target("main/preview", [0., 0.], &egui::Context::default());
        assert_eq!(
            book.restore_status(doc.id, "main").unwrap().tier,
            reading_precision::Precision::Approximate
        );
        assert!(book.restore_status(doc.id, "secondary").is_none());
        drop(book);
        let mut next = ReadingState::new(Some(state.clone()));
        next.bind(&mut doc, "main", 2);
        assert_eq!(
            next.restore_status(doc.id, "main").unwrap().reason_label(),
            "No reliable match; an approximate position was used."
        );
        next.clear(Some(doc.id));
        assert!(next.restore_status(doc.id, "main").is_none());
        drop(next);
        assert!(
            load(&state)
                .unwrap()
                .records
                .values()
                .all(|r| r.restore_status.is_empty())
        );
        assert_eq!(doc.text, "Original manuscript remains unchanged.");
        assert!(!doc.path.unwrap().exists());
    }
    #[test]
    fn proof_metadata_respects_memory_disk_capacity_and_clear_tombstones() {
        let text = relocation_document();
        let source = reading_precision::capture(&text, text.find("identity 043").unwrap());
        assert!(source.has_relocation_proof());
        let position = Position {
            fingerprint: source.source_fingerprint,
            text_anchor: Some(TextAnchor {
                source,
                ..TextAnchor::default()
            }),
            ..Position::default()
        };
        let mut book = ReadingState::default();
        let mut active = None;
        for i in 0..MAX_RECORDS + 4 {
            let mut r = record(&format!("file-{i}"), i as u64 + 1);
            for view in [
                "main/source",
                "main/preview",
                "secondary/source",
                "secondary/preview",
            ] {
                r.views.insert(view.into(), position.clone());
            }
            if i == 0 {
                active = Some(r.id);
            }
            book.records.insert(r.id, r);
        }
        book.runtime.insert(
            "main/preview".into(),
            Runtime {
                id: active,
                ..Runtime::default()
            },
        );
        book.enforce_budget();
        assert!(book.records.len() <= MAX_RECORDS);
        assert!(book.records.contains_key(&active.unwrap()));
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        write_merged(
            &path,
            PendingWrite {
                records: book
                    .records
                    .values()
                    .map(|r| (r.path.clone().unwrap(), r.clone()))
                    .collect(),
                cleared_ms: 0,
                protected: vec!["file-0".into()],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        assert!(fs::metadata(&path).unwrap().len() <= MAX_BYTES);
        assert!(load(&path).unwrap().records.contains_key("file-0"));
        let snapshot = load(&path).unwrap().records["file-0"].clone();
        assert!(
            snapshot.views["main/preview"]
                .text_anchor
                .as_ref()
                .unwrap()
                .source
                .has_relocation_proof()
        );
        let mut tombstone = snapshot.clone();
        tombstone.views.clear();
        tombstone.deleted = true;
        tombstone.updated_ms = 10_000;
        write(&path, vec![("file-0", tombstone)], vec![]).unwrap();
        write(&path, vec![("file-0", snapshot.clone())], vec![]).unwrap();
        assert!(
            load(&path).unwrap().records["file-0"].views.is_empty(),
            "Another instance's late proof snapshot cannot undo single-file clear"
        );
        write_merged(
            &path,
            PendingWrite {
                records: BTreeMap::new(),
                cleared_ms: 20_000,
                clears: vec![ClearRequest {
                    path: None,
                    requested_ns: 20_000_000_000,
                    rename_to: None,
                }],
                protected: vec![],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        write(&path, vec![("file-0", snapshot)], vec![]).unwrap();
        assert!(
            load(&path)
                .unwrap()
                .records
                .get("file-0")
                .is_none_or(|r| r.deleted && r.views.is_empty())
        );
    }
    fn relocation_document() -> String {
        (0..80).map(|i|format!("## Section {i}\n\nUnique original paragraph identity {i:03} 中文 👩🏽‍💻 has independently distinguishable complete source information.\n\n")).collect()
    }
    #[test]
    fn changed_revision_is_matched_off_thread_then_restores_the_original_glyph_y() {
        let text = relocation_document();
        let byte = text.find("identity 043").unwrap() + 10;
        let mut doc = Document::from_text(&text);
        let mut book = ReadingState::default();
        book.bind(&mut doc, "main", 1);
        let source = reading_precision::capture(&text, byte);
        let old = Position {
            fingerprint: source.source_fingerprint,
            layout: "same".into(),
            text_anchor: Some(TextAnchor {
                source,
                viewport_y: 19.5,
                logical_line: 0,
                token_range: byte..byte + 1,
            }),
            ..Position::default()
        };
        book.runtime.get_mut("main/preview").unwrap().last = Some(old.clone());
        book.records
            .get_mut(&doc.id)
            .unwrap()
            .views
            .insert("main/preview".into(), old);
        let prefix = "Inserted independent front section. ".repeat(500);
        doc.text.insert_str(0, &prefix);
        doc.mark_text_changed();
        let ctx = egui::Context::default();
        assert!(
            book.plan(
                &doc,
                "main/preview",
                1,
                "same",
                &ctx,
                egui::Rect::EVERYTHING,
                false
            )
            .is_none()
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while book.runtime["main/preview"].relocation_job.is_some() {
            assert!(Instant::now() < deadline);
            book.poll_precision(&doc, "main/preview");
            std::thread::yield_now();
        }
        let rt = &book.runtime["main/preview"];
        let anchor = rt
            .pending
            .as_ref()
            .unwrap()
            .position
            .text_anchor
            .as_ref()
            .unwrap();
        assert_eq!(anchor.source.source_byte, byte + prefix.len());
        assert_eq!(anchor.viewport_y, 19.5);
        assert_eq!(rt.relocation.as_ref().unwrap()["exact"], true);
        assert!(
            !rt.pending.as_ref().unwrap().exact,
            "version correspondence does not authorize old raw offsets"
        );
    }
    #[test]
    fn late_enrichment_cannot_resurrect_a_cleared_record() {
        let text = relocation_document();
        let mut doc = Document::from_text(&text);
        let mut book = ReadingState::default();
        book.bind(&mut doc, "main", 1);
        book.index(&doc);
        assert!(book.queue_precision(
            &doc,
            "main/preview",
            reading_precision::capture_with_fingerprint(&text, 300, hash(&text)),
            false
        ));
        let control = book.runtime["main/preview"].proof_job.clone().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while control
            .finished_ns
            .load(std::sync::atomic::Ordering::Acquire)
            == 0
        {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        book.clear(Some(doc.id));
        book.poll_precision(&doc, "main/preview");
        assert!(book.records[&doc.id].views.is_empty());
        assert!(book.runtime["main/preview"].pinned.is_none());
        assert!(book.runtime["main/preview"].suspended);
    }
    #[test]
    fn background_index_completion_publishes_before_waking_its_native_context() {
        use std::sync::atomic::Ordering;
        let context = egui::Context::default();
        let unrelated = egui::Context::default();
        // Consume egui's initial requested frames so a fresh wake reaches the
        // platform callback, just as it does after the application becomes idle.
        for _ in 0..4 {
            let _ = context.run(egui::RawInput::default(), |_| {});
            let _ = unrelated.run(egui::RawInput::default(), |_| {});
        }
        let (notifications, wakes) = mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = notifications.send(());
        });
        let (other_notifications, other_wakes) = mpsc::channel();
        unrelated.set_request_repaint_callback(move |_| {
            let _ = other_notifications.send(());
        });
        let mut book = ReadingState::default();
        book.tick(&context);
        let control = JobControl::new("source-index", serde_json::json!({"epoch": 7}))
            .with_repaint(book.repaint.as_ref());
        book.tick(&unrelated);
        assert_eq!(book.repaint.as_ref(), Some(&unrelated));
        let worker = IndexWorker::new();
        worker
            .tx
            .as_ref()
            .unwrap()
            .send(IndexJob {
                key: (DocumentId::default(), 4),
                epoch: 7,
                revision: 3,
                text: "# Original section\n\n中文 é 👩🏽‍💻 body.\n\n".repeat(2_000),
                control: control.clone(),
            })
            .unwrap();
        wakes.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(control.finished_ns.load(Ordering::Acquire) > 0);
        let result = worker.rx.try_recv().unwrap();
        assert_eq!(result.epoch, 7);
        assert_eq!(result.index.unwrap().revision, 3);
        assert!(other_wakes.try_recv().is_err());
    }
    #[test]
    fn cancelled_precision_completion_wakes_after_its_result_is_available() {
        use std::sync::{Arc, atomic::Ordering};
        let context = egui::Context::default();
        for _ in 0..4 {
            let _ = context.run(egui::RawInput::default(), |_| {});
        }
        let (notifications, wakes) = mpsc::channel();
        context.set_request_repaint_callback(move |_| {
            let _ = notifications.send(());
        });
        let worker = PrecisionWorker::new();
        let identity = PrecisionIdentity {
            view: "secondary/source".into(),
            document_id: DocumentId::default(),
            buffer_id: 19,
            revision: 8,
            open_generation: 4,
            restore_epoch: 3,
            clear_epoch: 2,
        };
        let control =
            JobControl::new("anchor-enrichment", identity.report()).with_repaint(Some(&context));
        let text = Arc::new("A source paragraph with 中文 é 👩🏽‍💻.\n\n".repeat(200_000));
        let anchor = reading_precision::capture_with_fingerprint(&text, 8, 0);
        worker
            .tx
            .as_ref()
            .unwrap()
            .send(PrecisionJob {
                identity,
                text,
                fingerprint: 0,
                work: PrecisionWork::Enrich(anchor),
                control: control.clone(),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while control.progress.load(Ordering::Acquire) < 100 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(control.finished_ns.load(Ordering::Acquire), 0);
        control.cancel();
        wakes.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(control.finished_ns.load(Ordering::Acquire) > 0);
        let result = worker.rx.try_recv().unwrap();
        assert!(matches!(result.output, PrecisionOutput::Cancelled));
        assert_eq!(result.identity.buffer_id, 19);
        assert_eq!(result.identity.revision, 8);
        assert_eq!(result.identity.open_generation, 4);
        assert_eq!(result.identity.restore_epoch, 3);
        assert_eq!(result.identity.clear_epoch, 2);
    }
    #[test]
    fn running_precision_computation_observes_cancellation_and_releases_the_worker() {
        use std::sync::{Arc, atomic::Ordering};
        let mut worker = PrecisionWorker::new();
        let text = Arc::new(
            "A naturally large source paragraph with 中文 é 👩🏽‍💻 and stable source text.\n\n"
                .repeat(140_000),
        );
        let identity = PrecisionIdentity {
            view: "main/preview".into(),
            document_id: DocumentId::default(),
            buffer_id: 1,
            revision: 0,
            open_generation: 1,
            restore_epoch: 0,
            clear_epoch: 0,
        };
        let control = JobControl::new("anchor-enrichment", identity.report());
        let source = reading_precision::capture_with_fingerprint(&text, 400, 0);
        worker
            .tx
            .as_ref()
            .unwrap()
            .send(PrecisionJob {
                identity: identity.clone(),
                text,
                fingerprint: 0,
                work: PrecisionWork::Enrich(source),
                control: control.clone(),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while control.progress.load(Ordering::Acquire) < 100 {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(control.finished_ns.load(Ordering::Acquire), 0);
        let cancelled_at = Instant::now();
        control.cancel();
        let tiny = Arc::new("Small new document with distinct source text.".to_owned());
        let next = JobControl::new("anchor-enrichment", identity.report());
        worker
            .tx
            .as_ref()
            .unwrap()
            .send(PrecisionJob {
                identity,
                text: tiny.clone(),
                fingerprint: 0,
                work: PrecisionWork::Enrich(reading_precision::capture_with_fingerprint(
                    &tiny, 8, 0,
                )),
                control: next.clone(),
            })
            .unwrap();
        let old = worker.rx.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(matches!(old.output, PrecisionOutput::Cancelled));
        // Production deliberately enqueues the result before publishing finish
        // and waking the event loop. Receiving it is not a finish observation.
        while control.finished_ns.load(Ordering::Acquire) == 0 {
            assert!(cancelled_at.elapsed() <= Duration::from_millis(100));
            std::thread::yield_now();
        }
        assert!(
            control.report()["cancel_requested_while_running"]
                .as_bool()
                .unwrap()
        );
        assert!(
            control.report()["cancel_to_worker_exit_wall_ms"]
                .as_f64()
                .unwrap()
                <= 100.
        );
        worker.rx.recv_timeout(Duration::from_millis(100)).unwrap();
        assert!(next.report()["queue_wait_wall_ms"].as_f64().unwrap() <= 100.);
        worker.tx.take();
    }
    #[test]
    fn unmappable_legal_text_history_degrades_with_finite_budget_without_overwriting_history() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text("# hidden syntax then normal text");
        let ctx = egui::Context::default();
        book.bind(&mut doc, "main", 1);
        let old = Position {
            offset: [0., 100.],
            layout: "old-layout".into(),
            fingerprint: reading_precision::fingerprint(&doc.text),
            text_anchor: Some(TextAnchor {
                source: reading_precision::capture(&doc.text, 0),
                ..TextAnchor::default()
            }),
            ..Position::default()
        };
        book.records
            .get_mut(&doc.id)
            .unwrap()
            .views
            .insert("main/preview".into(), old.clone());
        let rt = book.runtime.get_mut("main/preview").unwrap();
        rt.last = Some(old.clone());
        rt.pending = Some(Restore {
            position: old.clone(),
            tier: "layout-anchor".into(),
            exact: false,
        });
        rt.presented_offset = Some([0., 50.]);
        for _ in 0..125 {
            book.target("main/preview", [0., 50.], &ctx);
        }
        let rt = &book.runtime["main/preview"];
        assert!(rt.suspended && rt.pending.is_none());
        assert_eq!(rt.failed_restore.as_ref().unwrap()["exact"], false);
        for _ in 0..10 {
            assert!(
                book.plan(
                    &doc,
                    "main/preview",
                    1,
                    "new-layout",
                    &ctx,
                    egui::Rect::NOTHING,
                    false
                )
                .is_none()
            );
            book.observe(
                &doc,
                "main/preview",
                "new-layout".into(),
                [0., 50.],
                [600., 400.],
                0,
                0.,
                0.,
                false,
            );
        }
        assert_eq!(
            book.records[&doc.id].views["main/preview"], old,
            "valid saved history survives a failed automatic restore"
        );
        book.cancel("main/preview", "navigation");
        assert!(!book.runtime["main/preview"].suspended);
    }
    #[test]
    fn pending_nontext_restore_does_not_capture_temporary_top_text() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text("temporary heading above an unloaded image");
        let ctx = egui::Context::default();
        book.bind(&mut doc, "main", 1);
        let rt = book.runtime.get_mut("main/preview").unwrap();
        rt.pending = Some(Restore {
            position: Position {
                offset: [0., 1600.],
                ..Position::default()
            },
            tier: "offset".into(),
            exact: true,
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let galley = ui.fonts_mut(|f| {
                    f.layout_no_wrap(
                        doc.text.clone(),
                        egui::FontId::proportional(15.),
                        egui::Color32::WHITE,
                    )
                });
                let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600., 400.));
                let mapped = book.geometry_cache.linear(galley, egui::pos2(10., 10.), 0);
                book.geometry_frame(
                    &doc,
                    "main/preview",
                    &[mapped],
                    viewport,
                    [0., 0.],
                    [0., 0.],
                    ctx,
                );
                assert!(book.runtime["main/preview"].pinned.is_none());
                assert!(book.runtime["main/preview"].geometry.is_none());
            });
        });
    }
    #[test]
    fn restore_waits_for_asset_and_actual_presentation_not_requested_offset() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text("# image");
        let ctx = egui::Context::default();
        book.bind(&mut doc, "main", 1);
        let rt = book.runtime.get_mut("main/preview").unwrap();
        rt.pending = Some(Restore {
            position: Position {
                offset: [0., 1600.],
                ..Position::default()
            },
            tier: "offset".into(),
            exact: true,
        });
        rt.presented_offset = Some([0., 0.]);
        rt.presented_max = [0., 0.];
        book.layout_readiness("main/preview", true, Vec::new());
        for _ in 0..80 {
            let target = book.reachable_target("main/preview", [0., 1600.]);
            book.target("main/preview", target, &ctx);
        }
        assert!(book.restoring("main/preview"));
        assert_eq!(
            book.runtime["main/preview"]
                .pending
                .as_ref()
                .unwrap()
                .position
                .offset,
            [0., 1600.]
        );
        book.layout_readiness("main/preview", false, Vec::new());
        book.runtime.get_mut("main/preview").unwrap().presented_max = [0., 3000.];
        for _ in 0..20 {
            book.target("main/preview", [0., 1600.], &ctx);
        }
        assert!(
            book.restoring("main/preview"),
            "requested target is not rendered position"
        );
        book.runtime
            .get_mut("main/preview")
            .unwrap()
            .presented_offset = Some([0., 1600.]);
        for _ in 0..4 {
            book.target("main/preview", [0., 1600.], &ctx);
        }
        assert!(!book.restoring("main/preview"));
    }
    #[test]
    fn user_scroll_without_visible_text_drops_the_old_text_pin() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text("visible text");
        let ctx = egui::Context::default();
        book.bind(&mut doc, "main", 1);
        let text = TextAnchor {
            source: reading_precision::capture(&doc.text, 0),
            viewport_y: 0.,
            ..TextAnchor::default()
        };
        let rt = book.runtime.get_mut("main/preview").unwrap();
        rt.pinned = Some(text.clone());
        rt.last = Some(Position {
            text_anchor: Some(text),
            ..Position::default()
        });
        rt.recapture = true;
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600., 400.));
        for _ in 0..3 {
            assert!(
                book.geometry_frame(
                    &doc,
                    "main/preview",
                    &[],
                    viewport,
                    [0., 100.],
                    [0., 1000.],
                    &ctx
                )
                .is_none()
            );
            book.observe(
                &doc,
                "main/preview",
                "image".into(),
                [0., 100.],
                [600., 400.],
                0,
                0.,
                0.,
                false,
            );
            assert!(
                book.records[&doc.id].views["main/preview"]
                    .text_anchor
                    .is_none()
            );
        }
    }
    #[test]
    fn subpixel_rounding_stops_without_discarding_the_pinned_grapheme() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text(&"word 中文 reading text ".repeat(300));
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.25);
        book.bind(&mut doc, "main", 1);
        let mut offset = [0., 400.4];
        let mut original = None;
        let mut settled_at = 0;
        for frame in 0..20 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 500.),
                )),
                ..Default::default()
            };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let viewport =
                        egui::Rect::from_min_size(egui::pos2(0., 0.), egui::vec2(400., 300.));
                    let width = if frame == 0 { 400. } else { 300. };
                    let layout = format!("test-{width}");
                    let restore = book.plan(&doc, "main/preview", 1, &layout, ctx, viewport, false);
                    let galley = ui.fonts_mut(|f| {
                        f.layout(
                            doc.text.clone(),
                            egui::FontId::proportional(15.),
                            egui::Color32::WHITE,
                            width,
                        )
                    });
                    let pos = egui::pos2(0., (-offset[1] * 1.25_f32).round() / 1.25);
                    ui.painter()
                        .galley(pos, galley.clone(), egui::Color32::WHITE);
                    let max = (galley.rect.height() - 300.).max(0.);
                    let mapped = book.geometry_cache.linear(galley, pos, 0);
                    let precise = book.geometry_frame(
                        &doc,
                        "main/preview",
                        &[mapped],
                        viewport,
                        offset,
                        [0., max],
                        ctx,
                    );
                    if frame == 0 {
                        original = book.precise_byte("main/preview");
                    }
                    if let Some(target) = precise {
                        offset = target;
                        book.target("main/preview", target, ctx);
                    } else if restore.is_some() {
                        panic!("coarse fallback must not overwrite a converged real glyph");
                    }
                    if !book.restoring("main/preview") {
                        book.observe(
                            &doc,
                            "main/preview",
                            layout,
                            offset,
                            [400., 300.],
                            0,
                            0.,
                            0.,
                            false,
                        );
                        if frame > 0 && settled_at == 0 {
                            settled_at = frame;
                        }
                    }
                });
            });
        }
        assert!(
            settled_at > 0 && settled_at < 10,
            "must converge, got {settled_at}"
        );
        assert_eq!(book.precise_byte("main/preview"), original);
        let rt = &book.runtime["main/preview"];
        let glyph = rt.geometry.as_ref().unwrap();
        assert!((glyph.viewport_y - rt.pinned.as_ref().unwrap().viewport_y).abs() <= 0.75);
        assert!(rt.compensations < 8, "no perpetual half-pixel corrections");
    }
    #[test]
    fn same_document_buffers_with_equal_revisions_keep_independent_source_indexes() {
        let mut book = ReadingState::default();
        let mut main = Document::from_text("中🙂\n# Main\n\nbody");
        let mut secondary = Document::from_text("A😀\n# Secondary\n\nbody");
        secondary.id = main.id;
        main.mark_text_changed();
        secondary.mark_text_changed();
        assert_eq!(main.revision(), secondary.revision());
        book.bind(&mut main, "main", 1);
        book.bind(&mut secondary, "secondary", 1);
        for _ in 0..8 {
            for doc in [&main, &secondary] {
                assert_eq!(book.index(doc).fingerprint, hash(&doc.text));
                assert_eq!(book.byte_at_char(doc, 1), char_to_byte(&doc.text, 1));
                assert_eq!(book.char_at_byte(doc, 3), byte_to_char(&doc.text, 3));
            }
        }
        assert_eq!(book.index_build_ms.len(), 2, "each live buffer builds once");
        assert_eq!(
            main.id, secondary.id,
            "the document identity remains stable"
        );
    }
    #[test]
    fn same_document_large_buffers_keep_independent_background_indexes() {
        let mut book = ReadingState::default();
        let mut main =
            Document::from_text(&format!("# Main 中🙂\n{}", "中🙂 text\n\n".repeat(9000)));
        let mut secondary =
            Document::from_text(&format!("# Secondary\n{}", "A😀 text\n\n".repeat(9000)));
        secondary.id = main.id;
        main.mark_text_changed();
        secondary.mark_text_changed();
        book.bind(&mut main, "main", 1);
        book.bind(&mut secondary, "secondary", 1);
        let expected = [hash(&main.text), hash(&secondary.text)];
        let started = Instant::now();
        loop {
            let actual = [
                book.index(&main).fingerprint,
                book.index(&secondary).fingerprint,
            ];
            if actual == expected {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3), "{actual:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
        for _ in 0..8 {
            assert_eq!(book.index(&main).fingerprint, expected[0]);
            assert_eq!(book.index(&secondary).fingerprint, expected[1]);
        }
        assert_eq!(book.index_build_ms.len(), 2);
        assert_eq!(book.indexes.len(), 2);
    }
    #[test]
    fn cloned_worker_replacements_refresh_buffer_index_without_document_rebinding() {
        let mut book = ReadingState::default();
        let mut main = Document::from_text("中🙂\n# Main");
        let mut secondary = Document::from_text("A😀\n# Secondary");
        secondary.id = main.id;
        book.bind(&mut main, "main", 1);
        book.bind(&mut secondary, "secondary", 1);
        for _ in 0..8 {
            let old_main = main.buffer_id();
            let old_secondary = secondary.buffer_id();
            main = main.clone();
            secondary = secondary.clone();
            assert_ne!(old_main, main.buffer_id());
            assert_ne!(old_secondary, secondary.buffer_id());
            for doc in [&main, &secondary] {
                assert_eq!(book.index(doc).fingerprint, hash(&doc.text));
                assert_eq!(book.byte_at_char(doc, 1), char_to_byte(&doc.text, 1));
            }
            assert_eq!(main.id, secondary.id);
            assert_eq!(book.indexes.len(), 2);
            assert_eq!(book.index_order.len(), 2);
        }
    }
    #[test]
    fn truncated_duplicate_context_candidates_never_claim_unique_best_match() {
        let original = Document::from_text("# Root\n\nUNIQUE TOKEN");
        let old = SourceIndex::new(&original).anchor(
            &original.text,
            original.text.find("TOKEN").unwrap(),
            0.,
            0.,
        );
        let changed = format!(
            "{}{}\n\n{}",
            original.text,
            "\n\nTOKEN".repeat(257),
            original.text
        );
        let doc = Document::from_text(&changed);
        let index = SourceIndex::new(&doc);
        assert_eq!(changed.match_indices(&old.context).count(), 259);
        let (byte, tier) = index.locate(&changed, &old);
        assert_eq!(
            tier, "approximate",
            "a second equally strong match beyond the cap must not be hidden"
        );
        assert!(byte <= changed.len());
        assert!(changed.is_char_boundary(byte));
    }
    #[test]
    fn overlay_pointer_input_does_not_resume_capture_after_clear() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text("one\n\ntwo");
        book.bind(&mut doc, "main", 1);
        book.observe(
            &doc,
            "main/preview",
            "layout".into(),
            [0., 100.],
            [800., 600.],
            5,
            0.,
            0.,
            true,
        );
        book.clear(Some(doc.id));
        let ctx = egui::Context::default();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
        for frame in 0..3 {
            let mut raw = egui::RawInput {
                screen_rect: Some(viewport),
                ..egui::RawInput::default()
            };
            if frame > 0 {
                raw.events = vec![
                    egui::Event::PointerMoved(egui::pos2(120., 120.)),
                    egui::Event::PointerButton {
                        pos: egui::pos2(120., 120.),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
            }
            let _ = ctx.run(raw, |ctx| {
                egui::Area::new(egui::Id::new("quick-overlay"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(80., 80.))
                    .show(ctx, |ui| {
                        ui.allocate_exact_size(egui::vec2(300., 300.), egui::Sense::click());
                    });
                assert!(
                    book.plan(&doc, "main/preview", 1, "layout", ctx, viewport, false)
                        .is_none()
                );
                book.observe(
                    &doc,
                    "main/preview",
                    "layout".into(),
                    [0., 100.],
                    [800., 600.],
                    5,
                    0.,
                    0.,
                    true,
                );
            });
        }
        assert!(book.runtime["main/preview"].suspended);
        assert!(book.records[&doc.id].views.is_empty());
    }
    #[test]
    fn large_metadata_runs_off_ui_and_discards_results_from_older_open_epoch() {
        let mut book = ReadingState::default();
        let mut first = Document::from_text(&"# Old\n\nold paragraph 中文🙂\n\n".repeat(4000));
        book.bind(&mut first, "main", 1);
        assert_eq!(
            book.index(&first).revision,
            u64::MAX,
            "large metadata must be queued rather than parsed inline"
        );
        let mut second = Document::from_text(&"# New\n\nnew paragraph 🦀\n\n".repeat(4000));
        second.id = first.id;
        book.bind(&mut second, "main", 2);
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let current = book.index(&second);
            if current.revision == second.revision() {
                assert_eq!(current.fingerprint, hash(&second.text));
                assert_eq!(current.heading(20), vec!["New".to_owned()]);
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn user_scroll_cancels_history_even_before_large_metadata_is_ready() {
        let mut book = ReadingState::default();
        let mut doc = Document::from_text(&"# Large\n\nparagraph 中文🙂\n\n".repeat(4000));
        book.bind(&mut doc, "main", 1);
        book.records.get_mut(&doc.id).unwrap().views.insert(
            "main/preview".into(),
            Position {
                offset: [0., 1000.],
                layout: "old".into(),
                ..Position::default()
            },
        );
        book.bind(&mut doc, "main", 2);
        let ctx = egui::Context::default();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
        assert!(
            book.plan(&doc, "main/preview", 2, "new", &ctx, viewport, false)
                .is_none()
        );
        assert!(book.restoring("main/preview"));
        ctx.begin_pass(egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(egui::pos2(100., 100.)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -80.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..egui::RawInput::default()
        });
        assert!(
            book.plan(&doc, "main/preview", 2, "new", &ctx, viewport, false)
                .is_none()
        );
        assert!(!book.restoring("main/preview"));
        let _ = ctx.end_pass();
    }
    #[test]
    fn cursor_selection_preserves_direction() {
        for selection in [[10, 0], [0, 10], [7, 7]] {
            let range = cursor(selection);
            assert_eq!([range.primary.index, range.secondary.index], selection);
        }
    }
    fn record(path: &str, stamp: u64) -> Record {
        Record {
            path: Some(path.into()),
            updated_ms: stamp,
            ..Record::default()
        }
    }
    fn write(path: &Path, records: Vec<(&str, Record)>, protected: Vec<String>) -> io::Result<()> {
        // This test helper models explicit user clear/activity transactions.
        // A passive deletion snapshot is deliberately not a new clear.
        let clears = records
            .iter()
            .filter(|(_, r)| r.deleted)
            .map(|(key, r)| ClearRequest {
                path: Some((*key).into()),
                requested_ns: r.updated_ms.saturating_mul(1_000_000),
                rename_to: None,
            })
            .collect();
        let activities = records
            .iter()
            .filter(|(_, r)| !r.deleted && r.activity_ms > 0)
            .map(|(key, r)| ((*key).into(), r.activity_ms.saturating_mul(1_000_000)))
            .collect();
        write_merged(
            path,
            PendingWrite {
                records: records.into_iter().map(|(k, r)| (k.into(), r)).collect(),
                cleared_ms: 0,
                protected,
                clears,
                activities,
            },
        )
    }
    #[test]
    fn incremental_merge_keeps_other_documents_and_rejects_older_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        write(&path, vec![("a", record("a", 10))], vec![]).unwrap();
        write(&path, vec![("b", record("b", 20))], vec![]).unwrap();
        write(&path, vec![("a", record("a", 5))], vec![]).unwrap();
        let disk = load(&path).unwrap();
        assert_eq!(disk.records.len(), 2);
        assert_eq!(disk.records["a"].updated_ms, 10);
    }
    #[test]
    fn os_lock_excludes_second_handle_and_releases_on_close() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lock");
        let first = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let second = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(try_lock(&first).unwrap());
        assert!(!try_lock(&second).unwrap());
        drop(first);
        assert!(try_lock(&second).unwrap());
    }
    #[test]
    fn writer_drop_with_queued_updates_and_shutdown_finishes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let (tx, rx) = mpsc::channel();
        let output = path.clone();
        std::thread::spawn(move || {
            let writer = Writer::new(output);
            for stamp in 1..=12 {
                writer
                    .tx
                    .send(Some(PendingWrite {
                        records: BTreeMap::from([("a".into(), record("a", stamp))]),
                        cleared_ms: 0,
                        protected: vec![],
                        ..PendingWrite::default()
                    }))
                    .unwrap();
            }
            drop(writer);
            tx.send(()).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(5))
            .expect("writer shutdown deadlocked");
        assert_eq!(load(&path).unwrap().records["a"].updated_ms, 12);
    }
    #[test]
    fn concurrent_writers_merge_per_document_under_os_lock() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut threads = Vec::new();
        for i in 0..8 {
            let path = path.clone();
            threads.push(std::thread::spawn(move || {
                let key = format!("document-{i}");
                for update in 1..=5 {
                    write(&path, vec![(&key, record(&key, update))], vec![]).unwrap();
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let disk = load(&path).unwrap();
        assert_eq!(disk.records.len(), 8);
        assert!(disk.records.values().all(|r| r.updated_ms == 5));
    }
    #[test]
    fn clear_all_epoch_prevents_old_snapshot_resurrection() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        write(&path, vec![("a", record("a", 10))], vec![]).unwrap();
        write_merged(
            &path,
            PendingWrite {
                records: BTreeMap::new(),
                cleared_ms: 20,
                protected: vec![],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        write(
            &path,
            vec![("a", record("a", 10)), ("b", record("b", 21))],
            vec![],
        )
        .unwrap();
        let disk = load(&path).unwrap();
        assert!(!disk.records.contains_key("a"));
        assert!(disk.records.contains_key("b"));
    }
    #[test]
    fn capacity_eviction_protects_active_older_record() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut rows = BTreeMap::new();
        for i in 0..MAX_RECORDS + 8 {
            let key = format!("file-{i}");
            rows.insert(key.clone(), record(&key, i as u64 + 1));
        }
        write_merged(
            &path,
            PendingWrite {
                records: rows,
                cleared_ms: 0,
                protected: vec!["file-0".into()],
                ..PendingWrite::default()
            },
        )
        .unwrap();
        let disk = load(&path).unwrap();
        assert_eq!(disk.records.len(), MAX_RECORDS);
        assert!(disk.records.contains_key("file-0"));
    }
    #[test]
    fn corrupt_unknown_version_and_out_of_bounds_state_fail_safely() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        for bytes in [b"{".as_slice(), b"{\"version\":999}".as_slice()] {
            fs::write(&path, bytes).unwrap();
            let reading = ReadingState::new(Some(path.clone()));
            assert!(reading.stats.error.is_some());
            assert!(reading.records.is_empty());
        }
        assert_eq!(
            load(&temp.path().join("missing.json"))
                .unwrap()
                .records
                .len(),
            0
        );
    }
    #[test]
    fn save_as_copies_state_and_rename_migrates_identity() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("old.md");
        let new = temp.path().join("copy.md");
        fs::write(&old, "# 一\n\nhello 🦀").unwrap();
        let mut book = ReadingState::default();
        let mut doc = Document::open(&old).unwrap();
        book.bind(&mut doc, "main", 1);
        let id = doc.id;
        book.observe(
            &doc,
            "main/preview",
            "wide".into(),
            [0., 200.],
            [800., 600.],
            8,
            0.,
            0.,
            true,
        );
        doc.save_to(&new).unwrap();
        book.copied_path(&mut doc, id, "main");
        assert!(
            book.restoring("main/preview"),
            "Save As must seed the new scroll control from the prior viewport"
        );
        let restored = book
            .plan(
                &doc,
                "main/preview",
                1,
                "wide",
                &egui::Context::default(),
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
                false,
            )
            .unwrap();
        assert_eq!(restored.position.offset, [0., 200.]);
        assert_ne!(doc.id, id);
        assert_eq!(book.records[&id].views["main/preview"].offset, [0., 200.]);
        assert_eq!(
            book.records[&doc.id].views["main/preview"].offset,
            [0., 200.]
        );
        let copy_id = doc.id;
        doc.rename_in_place("renamed.md").unwrap();
        book.migrated_path(&doc, &new);
        assert_eq!(doc.id, copy_id);
        assert_eq!(book.paths[&path_key(doc.path.as_deref().unwrap())], copy_id);
        assert!(!new.exists());
        assert!(old.exists());
    }
    #[test]
    fn clear_active_position_does_not_repopulate_without_new_interaction() {
        let mut doc = Document::from_text("one\n\ntwo");
        let mut book = ReadingState::default();
        book.bind(&mut doc, "main", 1);
        book.observe(
            &doc,
            "main/preview",
            "layout".into(),
            [0., 100.],
            [800., 600.],
            5,
            0.,
            0.,
            true,
        );
        book.clear(Some(doc.id));
        book.observe(
            &doc,
            "main/preview",
            "layout".into(),
            [0., 100.],
            [800., 600.],
            5,
            0.,
            0.,
            true,
        );
        assert!(book.records[&doc.id].views.is_empty());
        assert!(book.records[&doc.id].deleted);
    }
}
