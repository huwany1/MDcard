// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Local, independent manuscript checkpoints. No manuscript path is opened here.
use crate::{Document, Editor, egui, encoding::Encoding};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const SCHEMA: u32 = 1;
pub const MAX_TEXT: usize = 16 * 1024 * 1024;
pub const MAX_RECORD_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_RECORDS: usize = 128;
const MAX_DRAFTS: usize = 128;
const DEBOUNCE: Duration = Duration::from_secs(2);
const PERIOD: Duration = Duration::from_secs(10);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn random_id() -> io::Result<String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn valid_key(key: &str) -> bool {
    key.len() == 32 && key.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn state_path() -> PathBuf {
    if let Some(p) = std::env::var_os("MDCARD_RECOVERY_DIR") {
        return p.into();
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
    base.unwrap_or_else(std::env::temp_dir)
        .join("MDcard/recovery")
}

/// Kernel-owned lease: a killed process releases it; PID reuse is irrelevant.
/// Lock files are never unlinked while they can have a holder.
fn lease(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let file = options.open(path)?;
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: flock acts on this live descriptor and has no pointer arguments.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(file)
}
fn wait_for_lease(path: &Path) -> io::Result<File> {
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        match lease(path) {
            Ok(f) => return Ok(f),
            Err(e) if e.kind() != io::ErrorKind::WouldBlock && e.raw_os_error() != Some(32) => {
                return Err(e);
            }
            Err(_) if Instant::now() >= deadline => {
                return Err(io::Error::other(
                    "Recovery storage is busy; retrying shortly",
                ));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}
struct Store {
    root: PathBuf,
    session: String,
    _lease: File,
}
#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Reservation {
    sequence: u64,
    token: String,
}
fn reservation(dir: &Path) -> io::Result<Option<Reservation>> {
    let file = match File::open(dir.join("sequence")) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if file.metadata()?.len() > 1024 {
        return Ok(None);
    }
    let mut raw = String::new();
    file.take(1025).read_to_string(&mut raw)?;
    if let Ok(value) = serde_json::from_str::<Reservation>(&raw)
        && valid_key(&value.token)
    {
        return Ok(Some(value));
    }
    Ok(raw.parse::<u64>().ok().map(|sequence| Reservation {
        sequence,
        token: String::new(),
    }))
}
impl Store {
    fn open(root: PathBuf) -> io::Result<Arc<Self>> {
        fs::create_dir_all(root.join("sessions"))?;
        fs::create_dir_all(root.join("records"))?;
        let _global = wait_for_lease(&root.join("store.lock"))?;
        // Session creation and stale lease cleanup share the store lock, so a
        // freshly created but not-yet-locked inode cannot be mistaken for dead.
        for entry in fs::read_dir(root.join("sessions"))? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && valid_key(&entry.file_name().to_string_lossy())
                && let Ok(file) = lease(&entry.path())
            {
                drop(file);
                fs::remove_file(entry.path())?;
            }
        }
        let session = random_id()?;
        let lock = lease(&root.join("sessions").join(&session))?;
        Ok(Arc::new(Self {
            root,
            session,
            _lease: lock,
        }))
    }
    fn lock(&self) -> io::Result<File> {
        wait_for_lease(&self.root.join("store.lock"))
    }
    fn directory(&self, key: &str) -> io::Result<PathBuf> {
        if !valid_key(key) {
            return Err(io::Error::other("Invalid recovery identity"));
        }
        Ok(self.root.join("records").join(key))
    }
    fn active(&self, session: &str) -> bool {
        if !valid_key(session) {
            return true;
        }
        if !self.root.join("sessions").join(session).exists() {
            return false;
        }
        lease(&self.root.join("sessions").join(session)).is_err()
    }
    fn invalidate(&self, key: &str) -> io::Result<()> {
        let _lock = self.lock()?;
        let dir = self.directory(key)?;
        fs::create_dir_all(&dir)?;
        atomic(&dir.join("invalid"), self.session.as_bytes(), 0)?;
        // Retain the tombstone until the session exits: a late worker cannot revive it.
        for name in ["0.json", "1.json"] {
            match fs::remove_file(dir.join(name)) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    fn commit(
        &self,
        key: &str,
        mut snapshot: Snapshot,
        cancelled: &AtomicBool,
        fault: u8,
    ) -> io::Result<Snapshot> {
        if snapshot.schema != SCHEMA
            || !valid_key(&snapshot.owner)
            || snapshot.drafts.len() > MAX_DRAFTS
        {
            return Err(io::Error::other(
                "Unknown recovery format or abnormal dimensions",
            ));
        }
        if snapshot.text.len() > MAX_TEXT
            || snapshot.drafts.iter().map(|d| d.text.len()).sum::<usize>() > MAX_TEXT
        {
            return Err(io::Error::other(
                "Recovery copy exceeds the 16 MiB text or draft limit",
            ));
        }
        let dir = self.directory(key)?;
        let ticket;
        let previous;
        {
            let _lock = self.lock()?;
            if cancelled.load(Ordering::SeqCst) || dir.join("invalid").exists() {
                return Err(io::Error::other("Recovery checkpoint cancelled"));
            }
            // Validate both generations while reserving. A truncated or rolled
            // back counter must never make a successful checkpoint sort older.
            let latest = valid_slots(&dir)
                .into_iter()
                .max_by_key(|(_, s)| s.sequence);
            previous = latest.as_ref().map_or(0, |(index, _)| *index);
            let valid_sequence = latest.as_ref().map_or(0, |(_, s)| s.sequence);
            let prior = reservation(&dir)?
                .map_or(0, |value| value.sequence)
                .max(valid_sequence);
            snapshot.sequence = prior
                .checked_add(1)
                .ok_or_else(|| io::Error::other("Recovery version limit reached"))?;
            // Reserve an order before expensive serialization. Gaps after failure are safe.
            fs::create_dir_all(&dir)?;
            ticket = Reservation {
                sequence: snapshot.sequence,
                token: random_id()?,
            };
            atomic(&dir.join("sequence"), &serde_json::to_vec(&ticket)?, 0)?;
            atomic(&dir.join("owner"), self.session.as_bytes(), 0)?;
        }
        snapshot.owner.clone_from(&self.session);
        snapshot.time_ms = now_ms();
        let payload = serde_json::to_string(&snapshot)?;
        let bytes = serde_json::to_vec(&Envelope {
            checksum: checksum(payload.as_bytes()),
            payload,
        })?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(io::Error::other(
                "Recovery copy exceeds the 128 MiB serialized limit",
            ));
        }
        let _lock = self.lock()?;
        if cancelled.load(Ordering::SeqCst)
            || dir.join("invalid").exists()
            || reservation(&dir)?.as_ref() != Some(&ticket)
        {
            return Err(io::Error::other("Recovery checkpoint cancelled"));
        }
        let mut total = 0u64;
        let mut records = 0;
        for entry in fs::read_dir(self.root.join("records"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if !entry.path().join("invalid").exists()
                && (entry.path().join("0.json").exists() || entry.path().join("1.json").exists())
            {
                records += 1;
            }
            for file in fs::read_dir(entry.path())? {
                total = total.saturating_add(file?.metadata()?.len());
            }
        }
        let existing = dir.join("0.json").exists() || dir.join("1.json").exists();
        if (!existing && records >= MAX_RECORDS)
            || total.saturating_add(bytes.len() as u64) > MAX_TOTAL_BYTES
        {
            return Err(io::Error::other(
                "Recovery storage is full (128 manuscripts / 512 MiB); resolve pending copies and retry",
            ));
        }
        fs::create_dir_all(&dir)?;
        // Rotate only successful commits; failed reservations must not overwrite
        // the sole last valid generation on the next attempt.
        let next = 1 - previous;
        atomic(&dir.join(format!("{next}.json")), &bytes, fault)?;
        atomic(&dir.join("current"), next.to_string().as_bytes(), 0)?;
        Ok(snapshot)
    }
    fn discover(&self) -> io::Result<Vec<Item>> {
        let mut found = Vec::new();
        for entry in fs::read_dir(self.root.join("records"))? {
            let entry = entry?;
            let key = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type()?.is_dir() || !valid_key(&key) {
                continue;
            }
            let dir = entry.path();
            let _lock = self.lock()?;
            if let Ok(owner) = fs::read_to_string(dir.join("owner"))
                && self.active(&owner)
            {
                continue;
            }
            if dir.join("invalid").exists() {
                // Only remove obsolete tombstones after the owning session is gone.
                if let Ok(owner) = fs::read_to_string(dir.join("invalid"))
                    && !self.active(&owner)
                    && lease(&dir.join("claim.lock")).is_ok()
                {
                    fs::remove_dir_all(&dir)?;
                }
                continue;
            }
            let Ok(_claim) = lease(&dir.join("claim.lock")) else {
                continue;
            };
            for file in fs::read_dir(&dir)? {
                let file = file?;
                if file
                    .file_name()
                    .to_string_lossy()
                    .starts_with("checkpoint-")
                {
                    fs::remove_file(file.path())?;
                }
            }
            if !dir.join("0.json").exists() && !dir.join("1.json").exists() {
                drop(_claim);
                fs::remove_dir_all(&dir)?;
                continue;
            }
            drop(_lock);
            let snapshot = read_latest(&dir);
            if snapshot.as_ref().is_ok_and(|s| self.active(&s.owner)) {
                continue;
            }
            let bytes = snapshot.as_ref().map_or(0, |s| s.text.len());
            let snapshot = snapshot.map(|mut s| {
                s.text = s.text.chars().take(180).collect();
                for d in &mut s.drafts {
                    d.text = String::new();
                    d.original = String::new();
                }
                s
            });
            found.push(Item {
                key,
                snapshot: snapshot.as_ref().ok().cloned(),
                error: snapshot.err().map(|e| e.to_string()),
                bytes,
            });
        }
        found
            .sort_by_key(|item| std::cmp::Reverse(item.snapshot.as_ref().map_or(0, |s| s.time_ms)));
        Ok(found)
    }
    fn claim(&self, key: &str) -> io::Result<(Snapshot, File)> {
        let _lock = self.lock()?;
        let dir = self.directory(key)?;
        let claim = lease(&dir.join("claim.lock"))?;
        if dir.join("invalid").exists() {
            return Err(io::Error::other("Recovery copy was already resolved"));
        }
        if let Ok(owner) = fs::read_to_string(dir.join("owner"))
            && self.active(&owner)
        {
            return Err(io::Error::other(
                "Recovery copy is in use by another instance",
            ));
        }
        drop(_lock);
        let snapshot = read_latest(&dir)?;
        if self.active(&snapshot.owner) {
            return Err(io::Error::other(
                "Recovery copy is in use by another instance",
            ));
        }
        Ok((snapshot, claim))
    }
    fn delete(&self, key: &str) -> io::Result<()> {
        let _claim = {
            let _lock = self.lock()?;
            let dir = self.directory(key)?;
            let claim = lease(&dir.join("claim.lock"))?;
            if let Ok(owner) = fs::read_to_string(dir.join("owner"))
                && self.active(&owner)
            {
                return Err(io::Error::other(
                    "Recovery copy is in use by another instance",
                ));
            }
            claim
        };
        self.invalidate(key)
    }
    fn clear_drafts(&self, key: &str) -> io::Result<()> {
        let _lock = self.lock()?;
        let dir = self.directory(key)?;
        let committed = valid_slots(&dir)
            .iter()
            .map(|(_, s)| s.sequence)
            .max()
            .unwrap_or(0);
        let sequence = reservation(&dir)?
            .map_or(0, |value| value.sequence)
            .max(committed);
        atomic(
            &dir.join("drafts-through"),
            sequence.to_string().as_bytes(),
            0,
        )
    }
}
fn atomic(path: &Path, bytes: &[u8], fault: u8) -> io::Result<()> {
    let mut temp = tempfile::Builder::new()
        .prefix("checkpoint-")
        .tempfile_in(path.parent().unwrap())?;
    temp.write_all(bytes)?;
    #[cfg(test)]
    fault_pause(fault, 1);
    if fault == 1 {
        return Err(io::Error::other("Injected failure after temporary write"));
    }
    temp.as_file().sync_all()?;
    #[cfg(test)]
    fault_pause(fault, 2);
    if fault == 2 {
        return Err(io::Error::other("Injected failure after file sync"));
    }
    temp.persist(path).map_err(|e| e.error)?;
    #[cfg(test)]
    fault_pause(fault, 3);
    if fault == 3 {
        return Err(io::Error::other("Injected failure after atomic commit"));
    }
    #[cfg(unix)]
    File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}
#[cfg(test)]
fn fault_pause(fault: u8, stage: u8) {
    if fault == 100 + stage
        && let Some(path) = std::env::var_os("MDCARD_RECOVERY_ACTOR_READY")
    {
        fs::write(path, "ready").unwrap();
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
pub(crate) fn checksum(bytes: &[u8]) -> u64 {
    // Fixed FNV-1a checksum: corruption detection stays stable across Rust upgrades.
    crate::document::content_digest(bytes)
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    checksum: u64,
    payload: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub kind: String,
    pub text: String,
    pub original: String,
    pub range: [usize; 2],
    pub expected: u64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Snapshot {
    schema: u32,
    owner: String,
    sequence: u64,
    time_ms: u64,
    pane: String,
    name: String,
    path: Option<PathBuf>,
    text: String,
    encoding: Encoding,
    bom: bool,
    revision: u64,
    drafts: Vec<Draft>,
}
fn read_slot(path: &Path) -> io::Result<Snapshot> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_RECORD_BYTES {
        return Err(io::Error::other("Recovery copy has an abnormal size"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    let envelope: Envelope = serde_json::from_slice(&bytes)?;
    if checksum(envelope.payload.as_bytes()) != envelope.checksum {
        return Err(io::Error::other("Recovery checksum mismatch"));
    }
    let s: Snapshot = serde_json::from_str(&envelope.payload)?;
    if s.schema != SCHEMA
        || !valid_key(&s.owner)
        || s.text.len() > MAX_TEXT
        || s.drafts.len() > MAX_DRAFTS
        || s.drafts.iter().map(|d| d.text.len()).sum::<usize>() > MAX_TEXT
    {
        return Err(io::Error::other(
            "Unknown recovery format or abnormal dimensions",
        ));
    }
    Ok(s)
}
fn valid_slots(dir: &Path) -> Vec<(u8, Snapshot)> {
    (0..=1)
        .filter_map(|index| {
            read_slot(&dir.join(format!("{index}.json")))
                .ok()
                .map(|s| (index, s))
        })
        .collect()
}
fn read_latest(dir: &Path) -> io::Result<Snapshot> {
    let mut snapshot = valid_slots(dir)
        .into_iter()
        .max_by_key(|(_, s)| s.sequence)
        .map(|(_, s)| s)
        .ok_or_else(|| {
            io::Error::other("No complete supported recovery version; copies were retained")
        })?;
    let discarded = fs::read_to_string(dir.join("drafts-through"))
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    if snapshot.sequence <= discarded {
        snapshot.drafts.clear();
    }
    Ok(snapshot)
}

struct Item {
    key: String,
    snapshot: Option<Snapshot>,
    error: Option<String>,
    bytes: usize,
}
enum Operation {
    Restored(String, Snapshot, File),
    Deleted(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    buffer: u64,
    revision: u64,
    encoding: Encoding,
    bom: bool,
    path: Option<PathBuf>,
    draft_stamp: u64,
    has_drafts: bool,
}
impl Version {
    fn of(doc: &Document, draft_stamp: u64, has_drafts: bool) -> Self {
        Self {
            buffer: doc.buffer_id(),
            revision: doc.revision(),
            encoding: doc.encoding,
            bom: doc.bom,
            path: doc.path.clone(),
            draft_stamp,
            has_drafts,
        }
    }
}
struct Job {
    version: Version,
    result: mpsc::Receiver<Result<(Snapshot, u64, f64), String>>,
}
#[derive(Default)]
struct Slot {
    key: Option<String>,
    buffer: Option<u64>,
    observed: Option<Version>,
    committed: Option<Version>,
    changed: Option<Instant>,
    first: Option<Instant>,
    job: Option<Job>,
    cancelled: Arc<AtomicBool>,
    claim: Option<File>,
    pub error: Option<String>,
    time_ms: u64,
    commit_ms: f64,
    restored_drafts: Vec<Draft>,
    recovered_path: Option<PathBuf>,
    retry_after: Option<Instant>,
    schedule_count: u64,
    scheduled_ms: u64,
    committed_ms: u64,
    observed_ms: u64,
}
#[derive(Default)]
pub struct Recovery {
    store: Option<Arc<Store>>,
    slots: [Slot; 2],
    catalog: Vec<Item>,
    scan: Option<mpsc::Receiver<Result<Vec<Item>, String>>>,
    pub error: Option<String>,
    pub open: bool,
    delete: Option<String>,
    inspect_drafts: bool,
    rich_discard_serial: u64,
    discard_restored: bool,
    operation: Option<mpsc::Receiver<Result<Operation, String>>>,
    trace: std::collections::VecDeque<String>,
}
struct Tick<'a> {
    language: crate::Language,
    now: Instant,
    enabled: bool,
    ctx: &'a egui::Context,
}
impl Recovery {
    fn trace(&mut self, event: impl Into<String>) {
        if std::env::var_os("MDCARD_UI_REPORT").is_none() {
            return;
        }
        if self.trace.len() >= 24 {
            self.trace.pop_front();
        }
        self.trace.push_back(event.into());
    }
    pub fn persistent() -> Self {
        let mut this = Self::default();
        match Store::open(state_path()) {
            Ok(store) => {
                this.store = Some(store);
                this.refresh();
            }
            Err(e) => this.error = Some(e.to_string()),
        }
        this
    }
    fn refresh(&mut self) {
        if self.scan.is_some() {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("recovery-discovery".into())
            .spawn(move || {
                let _ = tx.send(store.discover().map_err(|e| e.to_string()));
            }) {
            Ok(_) => self.scan = Some(rx),
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn operation(&mut self, key: String, delete: bool, ctx: &egui::Context) {
        if self.operation.is_some() {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        self.trace(if delete {
            "dispatch delete"
        } else {
            "dispatch restore"
        });
        let (tx, rx) = mpsc::sync_channel(1);
        let repaint = ctx.clone();
        match std::thread::Builder::new()
            .name("recovery-action".into())
            .spawn(move || {
                let result = if delete {
                    store.delete(&key).map(|()| Operation::Deleted(key))
                } else {
                    store
                        .claim(&key)
                        .map(|(s, claim)| Operation::Restored(key, s, claim))
                };
                let _ = tx.send(result.map_err(|e| e.to_string()));
                repaint.request_repaint();
            }) {
            Ok(_) => self.operation = Some(rx),
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn invalidate(&mut self, secondary: bool) -> bool {
        let slot = &mut self.slots[usize::from(secondary)];
        slot.cancelled.store(true, Ordering::SeqCst);
        if let (Some(store), Some(key)) = (&self.store, &slot.key)
            && let Err(e) = store.invalidate(key)
        {
            slot.error = Some(e.to_string());
            return false;
        }
        // Keep the cancelled task accounted for until completion; never accumulate workers.
        let job = slot.job.take();
        *slot = Slot {
            job,
            ..Slot::default()
        };
        true
    }
    pub fn has_restored_drafts(&self, secondary: bool) -> bool {
        !self.slots[usize::from(secondary)]
            .restored_drafts
            .is_empty()
    }
    fn discard_drafts(&mut self, secondary: bool) -> bool {
        let slot = &mut self.slots[usize::from(secondary)];
        slot.cancelled.store(true, Ordering::SeqCst);
        if let (Some(store), Some(key)) = (&self.store, &slot.key)
            && let Err(e) = store.clear_drafts(key)
        {
            slot.error = Some(e.to_string());
            return false;
        }
        slot.restored_drafts.clear();
        slot.committed = None;
        slot.observed = None;
        true
    }
    fn observe(
        &mut self,
        secondary: bool,
        doc: &Document,
        draft_state: (u64, bool),
        drafts: impl FnOnce() -> Vec<Draft>,
        tick: Tick<'_>,
    ) {
        let Tick {
            language,
            now,
            enabled,
            ctx,
        } = tick;
        let Some(store) = self.store.clone() else {
            return;
        };
        let index = usize::from(secondary);
        let version = Version::of(doc, draft_state.0, draft_state.1);
        let slot = &mut self.slots[index];
        if let Some(job) = &slot.job {
            match job.result.try_recv() {
                Ok(result) => {
                    match result {
                        Ok((s, committed_ms, worker_ms))
                            if !slot.cancelled.load(Ordering::SeqCst)
                                && slot.buffer == Some(job.version.buffer) =>
                        {
                            slot.committed = Some(job.version.clone());
                            slot.time_ms = s.time_ms;
                            slot.commit_ms = worker_ms;
                            slot.committed_ms = committed_ms;
                            slot.observed_ms = now_ms();
                        }
                        Ok(_) => {}
                        Err(e) if !slot.cancelled.load(Ordering::SeqCst) => {
                            if e == "Recovery storage is busy; retrying shortly" {
                                slot.retry_after = Some(now + Duration::from_secs(1));
                            } else {
                                slot.error = Some(e);
                            }
                        }
                        Err(_) => {}
                    }
                    slot.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    slot.error = Some("Recovery worker stopped".into());
                    slot.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50))
                }
            }
        }
        // Poll the retiring task before rebinding; there is at most one worker per pane.
        if slot.buffer != Some(doc.buffer_id()) {
            if slot.job.is_some() {
                return;
            }
            *slot = Slot {
                buffer: Some(doc.buffer_id()),
                ..Slot::default()
            };
        }
        if !doc.presentation_dirty() && !version.has_drafts && slot.restored_drafts.is_empty() {
            if slot.key.is_some() && slot.error.is_none() {
                self.invalidate(secondary);
            }
            return;
        }
        if !enabled {
            slot.cancelled.store(true, Ordering::SeqCst);
            return;
        }
        if slot.cancelled.load(Ordering::SeqCst) && slot.job.is_none() {
            slot.cancelled = Arc::new(AtomicBool::new(false));
        }
        if slot.observed.as_ref() != Some(&version) {
            if slot.job.is_none() && slot.committed == slot.observed {
                slot.first = Some(now);
            }
            slot.observed = Some(version.clone());
            slot.changed = Some(now);
            slot.first.get_or_insert(now);
        }
        if slot.committed.as_ref() == Some(&version) || slot.error.is_some() || slot.job.is_some() {
            return;
        }
        let due = (slot.changed.unwrap_or(now) + DEBOUNCE).min(slot.first.unwrap_or(now) + PERIOD);
        if let Some(retry) = slot.retry_after
            && now < retry
        {
            ctx.request_repaint_after(retry - now);
            return;
        }
        if now < due {
            ctx.request_repaint_after(due - now);
            return;
        }
        let key = match slot.key.clone().map(Ok).unwrap_or_else(random_id) {
            Ok(k) => k,
            Err(e) => {
                slot.error = Some(e.to_string());
                return;
            }
        };
        slot.key = Some(key.clone());
        let mut drafts = drafts();
        drafts.extend(slot.restored_drafts.clone());
        let snapshot = Snapshot {
            schema: SCHEMA,
            owner: store.session.clone(),
            sequence: 0,
            time_ms: 0,
            pane: if secondary { "secondary" } else { "main" }.into(),
            name: doc.name(language),
            path: doc.path.clone().or_else(|| slot.recovered_path.clone()),
            text: doc.text.clone(),
            encoding: doc.encoding,
            bom: doc.bom,
            revision: doc.revision(),
            drafts,
        };
        let cancelled = slot.cancelled.clone();
        let repaint = ctx.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("recovery-checkpoint".into())
            .spawn(move || {
                let started = Instant::now();
                let result = store
                    .commit(&key, snapshot, &cancelled, 0)
                    .map(|s| (s, now_ms(), started.elapsed().as_secs_f64() * 1000.))
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
                repaint.request_repaint();
            }) {
            Ok(_) => {
                slot.first = Some(now);
                slot.schedule_count += 1;
                slot.scheduled_ms = now_ms();
                slot.retry_after = None;
                slot.job = Some(Job {
                    version,
                    result: rx,
                });
            }
            Err(e) => slot.error = Some(e.to_string()),
        }
    }
}

impl Editor {
    fn install_recovered(&mut self, key: String, s: Snapshot, claim: File) -> Result<(), String> {
        self.recovery.trace("install requested");
        if self.doc.dirty()
            || self.rich.pending_edits()
            || self.structure.pending_edits()
            || self.saves.active(false)
            || self.recovery.has_restored_drafts(false)
        {
            return Err(self
                .language
                .text("Save or discard the current manuscript before recovering another.")
                .into());
        }
        if !self.commit_document(Document::recovered(s.text, s.encoding, s.bom), false) {
            return Err(self
                .language
                .text("A save is finishing. Please wait.")
                .into());
        }
        self.recovery.slots[0] = Slot {
            key: Some(key.clone()),
            buffer: Some(self.doc.buffer_id()),
            claim: Some(claim),
            restored_drafts: s.drafts,
            recovered_path: s.path,
            time_ms: s.time_ms,
            ..Slot::default()
        };
        self.recovery.catalog.retain(|item| item.key != key);
        self.recovery.open = false;
        self.recovery.inspect_drafts = !self.recovery.slots[0].restored_drafts.is_empty();
        self.editor_focus = true;
        self.recovery.trace("installed");
        Ok(())
    }
    pub(crate) fn tick_recovery(&mut self, ctx: &egui::Context, now: Instant) {
        if let Some(rx) = &self.recovery.operation {
            match rx.try_recv() {
                Ok(result) => {
                    self.recovery.trace("operation received");
                    self.recovery.operation = None;
                    match result {
                        Ok(Operation::Restored(key, s, claim)) => {
                            if let Err(e) = self.install_recovered(key, s, claim) {
                                self.error = Some(e);
                            }
                        }
                        Ok(Operation::Deleted(key)) => {
                            self.recovery.catalog.retain(|item| item.key != key);
                            self.recovery.delete = None;
                        }
                        Err(e) => {
                            self.error = Some(format!(
                                "{}: {e}",
                                self.language.text("Recovery copy failed")
                            ))
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.recovery.operation = None;
                    self.recovery.error = Some("Recovery action stopped".into());
                }
            }
        }
        if self.recovery.rich_discard_serial != self.rich.recovery_discard_serial
            && self.recovery.slots[0].error.is_none()
            && self.recovery.discard_drafts(false)
        {
            self.recovery.rich_discard_serial = self.rich.recovery_discard_serial;
        }
        if let Some(rx) = &self.recovery.scan {
            match rx.try_recv() {
                Ok(Ok(items)) => {
                    self.recovery.open = !items.is_empty();
                    self.recovery.catalog = items;
                    self.recovery.scan = None;
                }
                Ok(Err(e)) => {
                    self.recovery.error = Some(e);
                    self.recovery.scan = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.recovery.error = Some("Recovery discovery stopped".into());
                    self.recovery.scan = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(50))
                }
            }
        }
        let draft_stamp =
            crate::rich_nodes::hash((self.rich.recovery_stamp(), self.structure.recovery_stamp()));
        let has_drafts = self.rich.pending_edits() || self.structure.pending_edits();
        let rich = &self.rich;
        let structure = &self.structure;
        self.recovery.observe(
            false,
            &self.doc,
            (draft_stamp, has_drafts),
            || {
                let mut drafts = rich.recovery_drafts();
                drafts.extend(structure.recovery_drafts());
                drafts
            },
            Tick {
                language: self.language,
                now,
                enabled: self.preferences.recovery,
                ctx,
            },
        );
        if let Some(doc) = &self.global.secondary {
            self.recovery.observe(
                true,
                doc,
                (0, false),
                Vec::new,
                Tick {
                    language: self.language,
                    now,
                    enabled: self.preferences.recovery,
                    ctx,
                },
            );
        }
    }
    pub(crate) fn recovery_status(&mut self, ui: &mut egui::Ui, secondary: bool) {
        let slot = &self.recovery.slots[usize::from(secondary)];
        let language = self.language;
        let mut retry = false;
        let mut show = false;
        let mut drafts = false;
        let mut discard = false;
        ui.horizontal_wrapped(|ui| {
            let label = if slot.error.is_some() || self.recovery.error.is_some() {
                "Recovery copy failed"
            } else if slot.job.is_some() {
                "Updating recovery copy…"
            } else if !self.preferences.recovery {
                "Crash recovery off"
            } else if slot.time_ms > 0 {
                "Recovery copy updated"
            } else {
                "Crash recovery on"
            };
            let status = ui.small(language.text(label));
            if let Some(error) = slot.error.as_ref().or(self.recovery.error.as_ref()) {
                status.on_hover_text(
                    language
                        .text("Existing recovery copies were kept. Resolve the cause and retry."),
                );
                ui.colored_label(ui.visuals().error_fg_color, recovery_error(language, error));
                let response = ui.button(language.text("Retry recovery"));
                crate::theme::record_control(
                    ui,
                    &response,
                    if secondary {
                        "recovery-retry-secondary"
                    } else {
                        "recovery-retry-main"
                    },
                );
                if response.clicked() {
                    retry = true;
                }
            }
            if !self.recovery.catalog.is_empty() {
                let response = ui.button(language.text("Recover manuscripts…"));
                crate::theme::record_control(ui, &response, "recovery-open");
                if response.clicked() {
                    show = true;
                }
            }
            if !secondary && !slot.restored_drafts.is_empty() {
                let response = ui.button(language.text("Independent drafts"));
                crate::theme::record_control(ui, &response, "recovery-drafts");
                drafts = response.clicked();
                let response = ui.button(language.text("Discard independent drafts…"));
                crate::theme::record_control(ui, &response, "recovery-discard-drafts");
                discard = response.clicked();
            }
        });
        if retry {
            self.recovery.slots[usize::from(secondary)].error = None;
            if self.recovery.store.is_none() {
                self.recovery = Recovery::persistent();
            } else {
                self.recovery.error = None;
                self.recovery.refresh();
            }
        }
        if show {
            self.recovery.open = true;
        }
        if drafts {
            self.recovery.inspect_drafts = true;
        }
        if discard {
            self.recovery.discard_restored = true;
        }
    }
    pub(crate) fn recovery_dialog(&mut self, ctx: &egui::Context) {
        let language = self.language;
        if self.recovery.open {
            let mut open = true;
            let mut restore = None;
            egui::Window::new(language.text("Recover manuscripts…"))
                .open(&mut open)
                .default_width(650.)
                .max_height((ctx.content_rect().height() - 80.).max(200.))
                .show(ctx, |ui| {
                    ui.label(language.text(
                        "Recovery opens a new unsaved copy. The original file will not be written.",
                    ));
                    egui::ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() - 230.).max(100.))
                        .show(ui, |ui| {
                            for (index, item) in self.recovery.catalog.iter().enumerate() {
                                ui.separator();
                                if let Some(s) = &item.snapshot {
                                    ui.strong(&s.name);
                                    ui.label(s.path.as_ref().map_or_else(
                                        || language.text("Untitled").to_owned(),
                                        |p| p.display().to_string(),
                                    ));
                                    ui.small(language.format(
                                        "{time} · {pane} · {bytes} bytes · {encoding} · {bom}",
                                        &[
                                            ("time", &timestamp(s.time_ms)),
                                            (
                                                "pane",
                                                language.text(if s.pane == "secondary" {
                                                    "Second document"
                                                } else {
                                                    "Main document"
                                                }),
                                            ),
                                            ("bytes", &item.bytes.to_string()),
                                            ("encoding", s.encoding.name()),
                                            (
                                                "bom",
                                                language.text(if s.bom {
                                                    "With BOM"
                                                } else {
                                                    "Without BOM"
                                                }),
                                            ),
                                        ],
                                    ));
                                    ui.label(
                                        s.text
                                            .chars()
                                            .take(180)
                                            .collect::<String>()
                                            .replace(['\r', '\n'], " "),
                                    );
                                    if !s.drafts.is_empty() {
                                        ui.label(format!(
                                            "{}: {}",
                                            language.text("Independent drafts"),
                                            s.drafts.len()
                                        ));
                                    }
                                    let response = ui.button(language.text("Recover"));
                                    if std::env::var_os("MDCARD_UI_REPORT").is_some()
                                        && response.hovered()
                                        && ui.input(|i| {
                                            i.pointer.any_pressed() || i.pointer.any_released()
                                        })
                                    {
                                        if self.recovery.trace.len() >= 24 {
                                            self.recovery.trace.pop_front();
                                        }
                                        self.recovery.trace.push_back(format!(
                                            "restore {index}: pressed={} released={} clicked={}",
                                            ui.input(|i| i.pointer.any_pressed()),
                                            ui.input(|i| i.pointer.any_released()),
                                            response.clicked()
                                        ));
                                    }
                                    crate::theme::record_control(
                                        ui,
                                        &response,
                                        &format!("recovery-restore-{index}"),
                                    );
                                    if response.clicked() {
                                        restore = Some(item.key.clone());
                                    }
                                } else {
                                    ui.colored_label(
                                        ui.visuals().error_fg_color,
                                        language.text("Unreadable recovery copy was retained"),
                                    );
                                    ui.label(recovery_error(
                                        language,
                                        item.error.as_deref().unwrap_or(""),
                                    ));
                                }
                                let response = ui.button(language.text("Delete recovery copy…"));
                                crate::theme::record_control(
                                    ui,
                                    &response,
                                    &format!("recovery-delete-{index}"),
                                );
                                if response.clicked() {
                                    self.recovery.delete = Some(item.key.clone());
                                }
                            }
                        });
                    ui.separator();
                    let later = ui.button(language.text("Later"));
                    crate::theme::record_control(ui, &later, "recovery-later");
                    if later.clicked() {
                        self.recovery.open = false;
                    }
                });
            self.recovery.open &= open;
            if let Some(key) = restore {
                // Preserve any current buffer; recovery never replaces unsaved work.
                if self.doc.dirty()
                    || self.rich.pending_edits()
                    || self.structure.pending_edits()
                    || self.saves.active(false)
                {
                    self.error = Some(
                        language
                            .text(
                                "Save or discard the current manuscript before recovering another.",
                            )
                            .into(),
                    );
                } else {
                    self.recovery.operation(key, false, ctx);
                }
            }
        }
        if let Some(key) = self.recovery.delete.clone() {
            egui::Window::new(language.text("Delete recovery copy?"))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(language.text(
                        "This permanently discards this recovery copy and its independent drafts.",
                    ));
                    let cancel = ui.button(language.text("Cancel"));
                    crate::theme::record_control(ui, &cancel, "recovery-delete-cancel");
                    if cancel.clicked() {
                        self.recovery.delete = None;
                    }
                    let delete = ui.button(language.text("Delete"));
                    crate::theme::record_control(ui, &delete, "recovery-delete-confirm");
                    if delete.clicked() {
                        self.recovery.operation(key.clone(), true, ctx);
                    }
                });
        }
        if !self.recovery.slots[0].restored_drafts.is_empty() && self.recovery.inspect_drafts {
            let mut open = true;
            let mut close = false;
            egui::Window::new(language.text("Independent drafts")).open(&mut open).default_width(600.).vscroll(true).show(ctx, |ui| {
                    ui.label(language.text("Drafts are kept separately. Review and copy text; no automatic merge is performed."));
                    for (index,draft) in self.recovery.slots[0].restored_drafts.iter().enumerate() {
                        ui.strong(&draft.kind);
                        ui.small(format!("{}..{} · {}", draft.range[0], draft.range[1], draft.expected));
                        let mut text = draft.text.as_str();
                        ui.add(egui::TextEdit::multiline(&mut text).code_editor().desired_width(f32::INFINITY));
                        let copy=ui.button(language.text("Copy"));
                        crate::theme::record_control(ui,&copy,&format!("recovery-draft-copy-{index}"));
                        if copy.clicked() { ctx.copy_text(draft.text.clone()); }
                    }
                    let response=ui.button(language.text("Close"));
                    crate::theme::record_control(ui,&response,"recovery-drafts-close");
                    close=response.clicked();
                });
            self.recovery.inspect_drafts = open && !close;
        }
        if self.recovery.discard_restored {
            egui::Window::new(language.text("Discard independent drafts?"))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(language.text(
                        "The document source will be kept. Discarded drafts cannot be recovered.",
                    ));
                    if ui.button(language.text("Cancel")).clicked() {
                        self.recovery.discard_restored = false;
                    }
                    if ui.button(language.text("Discard")).clicked()
                        && self.recovery.discard_drafts(false)
                    {
                        self.recovery.discard_restored = false;
                    }
                });
        }
    }
    pub(crate) fn recovery_report(&self) -> serde_json::Value {
        serde_json::json!({"enabled":self.preferences.recovery,"trace":self.recovery.trace,"open":self.recovery.open,"pending":self.recovery.catalog.len(),"operation":self.recovery.operation.is_some(),"error":self.recovery.error,"items":self.recovery.catalog.iter().map(|item|serde_json::json!({"key":item.key,"name":item.snapshot.as_ref().map(|s|&s.name),"path":item.snapshot.as_ref().and_then(|s|s.path.as_ref()),"bytes":item.bytes,"drafts":item.snapshot.as_ref().map_or(0,|s|s.drafts.len())})).collect::<Vec<_>>(),"main":slot_report(&self.recovery.slots[0]),"secondary":slot_report(&self.recovery.slots[1])})
    }
}
fn slot_report(slot: &Slot) -> serde_json::Value {
    serde_json::json!({"key":slot.key,"snapshot_at":slot.time_ms,"scheduled_at":slot.scheduled_ms,"committed_at":slot.committed_ms,"observed_at":slot.observed_ms,"worker_ms":slot.commit_ms,"ui_poll_ms":slot.observed_ms.saturating_sub(slot.committed_ms),"schedule_count":slot.schedule_count,"working":slot.job.is_some(),"current_version":slot.committed.is_some()&&slot.committed==slot.observed,"committed_revision":slot.committed.as_ref().map(|v|v.revision),"observed_revision":slot.observed.as_ref().map(|v|v.revision),"pending_versions":usize::from(slot.job.as_ref().is_some_and(|job|Some(&job.version)!=slot.observed.as_ref())),"error":slot.error,"restored_drafts":slot.restored_drafts.len(),"claimed":slot.claim.is_some()})
}
fn recovery_error(language: crate::Language, error: &str) -> String {
    for key in [
        "Recovery copy exceeds the 16 MiB text or draft limit",
        "Recovery copy exceeds the 128 MiB serialized limit",
        "Recovery storage is full (128 manuscripts / 512 MiB); resolve pending copies and retry",
        "Recovery storage is busy; retrying shortly",
        "No complete supported recovery version; copies were retained",
        "Recovery copy is in use by another instance",
        "Recovery copy was already resolved",
        "Recovery checkpoint cancelled",
        "Recovery worker stopped",
        "Recovery discovery stopped",
        "Recovery action stopped",
        "Recovery copy has an abnormal size",
        "Recovery checksum mismatch",
        "Unknown recovery format or abnormal dimensions",
        "Recovery version limit reached",
        "Invalid recovery identity",
    ] {
        if error == key {
            return language.text(key).into();
        }
    }
    error.into()
}
fn timestamp(ms: u64) -> String {
    // UTC civil date, independent of host timezone. Show an unambiguous backup time.
    let seconds = ms / 1000;
    let days = seconds / 86400;
    let z = days as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(text: &str) -> Snapshot {
        Snapshot {
            schema: SCHEMA,
            owner: "0".repeat(32),
            sequence: 0,
            time_ms: 0,
            pane: "main".into(),
            name: "Untitled".into(),
            path: None,
            text: text.into(),
            encoding: Encoding::Utf8,
            bom: false,
            revision: 1,
            drafts: vec![],
        }
    }
    fn commit(store: &Store, key: &str, text: &str) -> Snapshot {
        store
            .commit(key, snapshot(text), &AtomicBool::new(false), 0)
            .unwrap()
    }
    fn fixture() -> (tempfile::TempDir, Editor, egui::Context) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = crate::preview_layout_tests::editor(&ctx, "");
        app.preferences.recovery = true;
        app.recovery.store = Some(Store::open(dir.path().to_owned()).unwrap());
        (dir, app, ctx)
    }
    fn edit(doc: &mut Document, text: &str) {
        doc.text = text.into();
        doc.mark_text_changed();
    }
    fn wait(app: &mut Editor, ctx: &egui::Context, now: Instant) {
        let began = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(15);
        while app
            .recovery
            .slots
            .iter()
            .any(|s| s.job.is_some() || s.retry_after.is_some())
        {
            app.tick_recovery(ctx, now + began.elapsed());
            assert!(
                Instant::now() < deadline,
                "bounded recovery worker did not finish"
            );
            std::thread::yield_now();
        }
    }
    fn checkpoint(app: &mut Editor, ctx: &egui::Context, t: Instant) {
        app.tick_recovery(ctx, t);
        app.tick_recovery(ctx, t + DEBOUNCE);
        wait(app, ctx, t + DEBOUNCE);
    }
    fn latest(app: &Editor, secondary: bool) -> Snapshot {
        let slot = &app.recovery.slots[usize::from(secondary)];
        read_latest(
            &app.recovery
                .store
                .as_ref()
                .unwrap()
                .directory(slot.key.as_ref().unwrap())
                .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn production_scheduling_idle_continuous_no_events_and_dirty_unchanged() {
        let (_dir, mut app, ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "first");
        app.tick_recovery(&ctx, t);
        app.tick_recovery(&ctx, t + Duration::from_millis(1999));
        assert!(app.recovery.slots[0].job.is_none());
        app.tick_recovery(&ctx, t + DEBOUNCE);
        wait(&mut app, &ctx, t + DEBOUNCE);
        assert_eq!(latest(&app, false).text, "first");
        assert!(app.doc.dirty());
        assert!(app.doc.path.is_none());
        for n in 3..=13 {
            edit(&mut app.doc, &format!("continuous {n}"));
            app.tick_recovery(&ctx, t + Duration::from_secs(n));
        }
        wait(&mut app, &ctx, t + Duration::from_secs(13));
        assert!(latest(&app, false).text.starts_with("continuous"));
        assert!(app.recovery.slots[0].time_ms > 0);
    }
    #[test]
    fn empty_untouched_no_record_but_deleted_body_and_format_are_valid() {
        let (_dir, mut app, ctx) = fixture();
        let t = Instant::now();
        checkpoint(&mut app, &ctx, t);
        assert!(app.recovery.slots[0].key.is_none());
        app.doc = Document::from_text("全部删除 Unicode 🙂 e\u{301}\r\n");
        edit(&mut app.doc, "");
        app.doc.encoding = Encoding::Utf16Be;
        app.doc.bom = true;
        checkpoint(&mut app, &ctx, t + PERIOD);
        let s = latest(&app, false);
        assert_eq!(s.text, "");
        assert_eq!(s.encoding, Encoding::Utf16Be);
        assert!(s.bom);
        let restored = Document::recovered(s.text, s.encoding, s.bom);
        assert!(restored.dirty());
        assert!(restored.path.is_none());
    }
    #[test]
    fn named_main_secondary_unicode_mixed_endings_and_original_bytes_preserved() {
        let (dir, mut app, ctx) = fixture();
        let p = dir.path().join("original.md");
        fs::write(&p, b"original\r\n").unwrap();
        app.doc = Document::open(&p).unwrap();
        app.global.secondary = Some(Document::open(&p).unwrap());
        let text = "中文🙂\r\nрусский\n한국어\r日本語\ne\u{301}";
        edit(&mut app.doc, text);
        edit(
            app.global.secondary.as_mut().unwrap(),
            "secondary different",
        );
        checkpoint(&mut app, &ctx, Instant::now());
        assert_eq!(latest(&app, false).text, text);
        assert_eq!(latest(&app, true).text, "secondary different");
        assert_ne!(app.recovery.slots[0].key, app.recovery.slots[1].key);
        assert_eq!(fs::read(&p).unwrap(), b"original\r\n");
        assert!(app.doc.dirty());
        assert!(app.global.secondary.as_ref().unwrap().dirty());
    }
    #[test]
    fn two_untitled_buffers_never_share_identity_and_replacement_drains_old_job() {
        let (_dir, mut app, ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "A");
        app.tick_recovery(&ctx, t);
        app.tick_recovery(&ctx, t + DEBOUNCE);
        let first = app.recovery.slots[0].key.clone();
        app.doc = Document::default();
        edit(&mut app.doc, "B");
        wait(&mut app, &ctx, t + PERIOD);
        checkpoint(&mut app, &ctx, t + PERIOD * 2);
        assert_ne!(first, app.recovery.slots[0].key);
        assert_eq!(latest(&app, false).text, "B");
    }
    #[test]
    fn all_interruption_points_leave_complete_old_or_new_and_corruption_falls_back() {
        for fault in 1..=3 {
            let dir = tempfile::tempdir().unwrap();
            let store = Store::open(dir.path().into()).unwrap();
            let key = random_id().unwrap();
            commit(&store, &key, "old 中文");
            assert!(
                store
                    .commit(&key, snapshot("new 🙂"), &AtomicBool::new(false), fault)
                    .is_err()
            );
            let recovered = read_latest(&store.directory(&key).unwrap()).unwrap();
            assert_eq!(
                recovered.text,
                if fault == 3 { "new 🙂" } else { "old 中文" }
            );
            commit(&store, &key, "third");
            let d = store.directory(&key).unwrap();
            let current = fs::read_to_string(d.join("current")).unwrap();
            fs::write(d.join(format!("{current}.json")), b"truncated").unwrap();
            assert!(["old 中文", "new 🙂"].contains(&read_latest(&d).unwrap().text.as_str()));
        }
    }
    #[test]
    fn invalidation_wins_late_task_and_cannot_resurrect_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        let key = random_id().unwrap();
        commit(&store, &key, "discarded");
        store.invalidate(&key).unwrap();
        assert!(
            store
                .commit(&key, snapshot("late"), &AtomicBool::new(false), 0)
                .is_err()
        );
        drop(store);
        let reader = Store::open(dir.path().into()).unwrap();
        assert!(reader.discover().unwrap().is_empty());
    }
    #[test]
    fn explicit_discard_immediately_invalidates_even_when_open_fails() {
        let (dir, mut app, ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "discard forever");
        checkpoint(&mut app, &ctx, t);
        let key = app.recovery.slots[0].key.clone().unwrap();
        app.pending = Some(crate::Action::OpenPath(dir.path().join("missing.md")));
        app.discard_pending_action(&ctx);
        assert!(
            app.recovery
                .store
                .as_ref()
                .unwrap()
                .directory(&key)
                .unwrap()
                .join("invalid")
                .exists()
        );
        assert!(!app.doc.dirty());
    }
    #[test]
    fn save_a_preserves_b_then_successful_save_cleans_exact_version() {
        let (dir, mut app, ctx) = fixture();
        let path = dir.path().join("saved.md");
        fs::write(&path, "original").unwrap();
        app.doc = Document::open(&path).unwrap();
        let t = Instant::now();
        edit(&mut app.doc, "A");
        checkpoint(&mut app, &ctx, t);
        app.start_save(false, path.clone(), false, &ctx);
        edit(&mut app.doc, "B");
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(app.doc.dirty());
        assert!(app.recovery.slots[0].key.is_some());
        checkpoint(&mut app, &ctx, t + PERIOD);
        assert_eq!(latest(&app, false).text, "B");
        let key = app.recovery.slots[0].key.clone().unwrap();
        app.start_save(false, path.clone(), false, &ctx);
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
        assert!(
            app.recovery
                .store
                .as_ref()
                .unwrap()
                .directory(&key)
                .unwrap()
                .join("invalid")
                .exists()
        );
    }
    #[test]
    fn failed_save_and_cancel_close_keep_checkpoint() {
        let (dir, mut app, ctx) = fixture();
        edit(&mut app.doc, "protected");
        checkpoint(&mut app, &ctx, Instant::now());
        let key = app.recovery.slots[0].key.clone();
        app.pending = Some(crate::Action::Quit);
        app.pending = None;
        app.start_save(false, dir.path().join("missing/target.md"), false, &ctx);
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(app.doc.dirty());
        assert_eq!(key, app.recovery.slots[0].key);
        assert_eq!(latest(&app, false).text, "protected");
    }
    #[test]
    fn successful_secondary_save_close_cleans_before_buffer_removed() {
        let (dir, mut app, ctx) = fixture();
        let p = dir.path().join("secondary.md");
        fs::write(&p, "saved").unwrap();
        app.global.secondary = Some(Document::open(&p).unwrap());
        edit(app.global.secondary.as_mut().unwrap(), "new");
        checkpoint(&mut app, &ctx, Instant::now());
        let key = app.recovery.slots[1].key.clone().unwrap();
        app.global.secondary_save_close = true;
        app.start_save(true, p, false, &ctx);
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(app.global.secondary.is_none());
        assert!(
            app.recovery
                .store
                .as_ref()
                .unwrap()
                .directory(&key)
                .unwrap()
                .join("invalid")
                .exists()
        );
    }
    #[test]
    fn valid_invalid_conflicting_rich_and_structure_drafts_stay_independent() {
        for value in ["z^3", r"\frac{", "graph TD; broken ???"] {
            let (_dir, mut app, ctx) = fixture();
            app.doc = Document::from_text("$x^2$\n");
            app.rich
                .stage_pending_test_edit(&app.doc.text, app.document_generation, value, &ctx);
            app.structure.draft = Some((egui::Id::new("cell"), "uncommitted cell 中文".into()));
            edit(&mut app.doc, "node has changed\n");
            app.ime_composing = true;
            checkpoint(&mut app, &ctx, Instant::now());
            let s = latest(&app, false);
            assert_eq!(s.text, "node has changed\n");
            assert!(s.drafts.iter().any(|d| d.text == value));
            assert!(s.drafts.iter().any(|d| d.text == "uncommitted cell 中文"));
            assert!(app.ime_composing);
            assert_eq!(app.doc.text, "node has changed\n");
        }
    }
    #[test]
    fn active_session_hidden_then_exited_session_claimed_once_and_deleted_safely() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Store::open(dir.path().into()).unwrap();
        let reader = Store::open(dir.path().into()).unwrap();
        let key = random_id().unwrap();
        commit(&writer, &key, "protected");
        assert!(reader.discover().unwrap().is_empty());
        drop(writer);
        assert_eq!(reader.discover().unwrap().len(), 1);
        let (_, claim) = reader.claim(&key).unwrap();
        assert!(reader.claim(&key).is_err());
        assert!(reader.delete(&key).is_err());
        drop(claim);
        reader.delete(&key).unwrap();
        assert!(reader.discover().unwrap().is_empty());
    }
    #[test]
    fn damaged_unknown_and_oversized_copies_retained_and_deletable() {
        let dir = tempfile::tempdir().unwrap();
        let writer = Store::open(dir.path().into()).unwrap();
        let key = random_id().unwrap();
        commit(&writer, &key, "body");
        let d = writer.directory(&key).unwrap();
        drop(writer);
        fs::write(d.join("1.json"), b"truncated").unwrap();
        let reader = Store::open(dir.path().into()).unwrap();
        assert!(reader.discover().unwrap()[0].error.is_some());
        reader.delete(&key).unwrap();
        let key = random_id().unwrap();
        let mut s = snapshot("future");
        s.schema = 999;
        assert!(
            reader
                .commit(&key, s.clone(), &AtomicBool::new(false), 0)
                .is_err()
        );
        let d = reader.directory(&key).unwrap();
        fs::create_dir_all(&d).unwrap();
        let payload = serde_json::to_string(&s).unwrap();
        fs::write(
            d.join("1.json"),
            serde_json::to_vec(&Envelope {
                checksum: checksum(payload.as_bytes()),
                payload,
            })
            .unwrap(),
        )
        .unwrap();
        assert!(read_latest(&d).is_err());
        File::create(d.join("0.json"))
            .unwrap()
            .set_len(MAX_RECORD_BYTES + 1)
            .unwrap();
        assert!(read_latest(&d).is_err());
    }
    #[test]
    fn count_text_and_total_capacity_boundaries_preserve_existing_versions() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        for n in 0..MAX_RECORDS {
            let key = format!("{n:032x}");
            commit(&store, &key, "tiny");
        }
        assert!(
            store
                .commit(
                    &random_id().unwrap(),
                    snapshot("overflow"),
                    &AtomicBool::new(false),
                    0
                )
                .is_err()
        );
        let key = format!("{:032x}", 0);
        commit(&store, &key, "update existing");
        let mut large = snapshot("");
        large.text = "x".repeat(MAX_TEXT + 1);
        assert!(
            store
                .commit(&key, large, &AtomicBool::new(false), 0)
                .is_err()
        );
        let filler = store
            .root
            .join("records")
            .join(format!("{:032x}", 1))
            .join("quota-fixture");
        File::create(&filler)
            .unwrap()
            .set_len(MAX_TOTAL_BYTES)
            .unwrap();
        assert!(
            store
                .commit(&key, snapshot("full"), &AtomicBool::new(false), 0)
                .is_err()
        );
        assert_eq!(
            read_latest(&store.directory(&key).unwrap()).unwrap().text,
            "update existing"
        );
    }
    #[test]
    fn disabled_stops_new_copies_but_keeps_pending_and_settings_failure_keeps_choice() {
        let (_dir, mut app, ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "A");
        checkpoint(&mut app, &ctx, t);
        let key = app.recovery.slots[0].key.clone();
        app.preferences.recovery = false;
        edit(&mut app.doc, "B");
        checkpoint(&mut app, &ctx, t + PERIOD);
        assert_eq!(latest(&app, false).text, "A");
        assert_eq!(key, app.recovery.slots[0].key);
        app.apply_preferences_result(Err(io::Error::other("injected settings error")));
        assert!(!app.preferences.recovery);
        assert!(app.settings_error.is_some());
        app.preferences.recovery = true;
        checkpoint(&mut app, &ctx, t + PERIOD * 2);
        assert_eq!(latest(&app, false).text, "B");
    }
    #[test]
    fn timestamp_is_explicit_utc() {
        assert_eq!(timestamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(timestamp(1792940793000), "2026-10-25 15:06:33 UTC");
    }
    #[test]
    fn edited_untitled_returning_to_empty_is_protected_until_explicit_save_or_discard() {
        let (_dir, mut app, ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "once typed");
        checkpoint(&mut app, &ctx, t);
        edit(&mut app.doc, "");
        assert!(app.doc.dirty());
        checkpoint(&mut app, &ctx, t + PERIOD);
        assert_eq!(latest(&app, false).text, "");
        app.doc.discard_changes();
        assert!(!app.doc.dirty());
    }
    #[test]
    fn save_as_rename_and_delayed_old_completion_keep_independent_identities() {
        let (dir, mut app, ctx) = fixture();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        let c = dir.path().join("renamed.md");
        fs::write(&a, "baseline").unwrap();
        app.doc = Document::open(&a).unwrap();
        let t = Instant::now();
        edit(&mut app.doc, "A");
        checkpoint(&mut app, &ctx, t);
        let key = app.recovery.slots[0].key.clone().unwrap();
        app.start_save(false, b.clone(), false, &ctx);
        edit(&mut app.doc, "B newer than Save As");
        crate::autosave::wait_for_saves(&mut app, &ctx);
        checkpoint(&mut app, &ctx, t + PERIOD);
        assert_eq!(app.recovery.slots[0].key.as_ref(), Some(&key));
        assert_eq!(latest(&app, false).path, app.doc.path);
        fs::rename(&b, &c).unwrap();
        app.doc.path = Some(c.canonicalize().unwrap());
        app.doc.mark_text_changed();
        checkpoint(&mut app, &ctx, t + PERIOD * 2);
        assert_eq!(app.recovery.slots[0].key.as_ref(), Some(&key));
        assert_eq!(latest(&app, false).path, app.doc.path);
        let old_version = app.recovery.slots[0].committed.clone().unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        app.recovery.slots[0].job = Some(Job {
            version: old_version,
            result: rx,
        });
        app.commit_document(Document::open(&c).unwrap(), false);
        edit(&mut app.doc, "independent reopened C");
        app.tick_recovery(&ctx, t + PERIOD * 3);
        assert_eq!(app.recovery.slots[0].key.as_ref(), Some(&key));
        tx.send(Ok((snapshot("late old result"), now_ms(), 0.)))
            .unwrap();
        app.tick_recovery(&ctx, t + PERIOD * 3);
        checkpoint(&mut app, &ctx, t + PERIOD * 4);
        assert_ne!(app.recovery.slots[0].key.as_ref(), Some(&key));
        assert_eq!(latest(&app, false).text, "independent reopened C");
        assert_eq!(
            read_latest(
                &app.recovery
                    .store
                    .as_ref()
                    .unwrap()
                    .directory(&key)
                    .unwrap()
            )
            .unwrap()
            .text,
            "B newer than Save As"
        );
    }
    #[test]
    fn cleanup_failure_is_visible_and_blocks_save_then_exit_until_user_handles_it() {
        let (dir, mut app, ctx) = fixture();
        let path = dir.path().join("saved.md");
        fs::write(&path, "original").unwrap();
        app.doc = Document::open(&path).unwrap();
        edit(&mut app.doc, "saved but cleanup fails");
        checkpoint(&mut app, &ctx, Instant::now());
        let key = app.recovery.slots[0].key.clone().unwrap();
        let invalid = app
            .recovery
            .store
            .as_ref()
            .unwrap()
            .directory(&key)
            .unwrap()
            .join("invalid");
        fs::create_dir(&invalid).unwrap();
        app.pending = Some(crate::Action::Quit);
        app.save_then_action = true;
        app.start_save(false, path, false, &ctx);
        crate::autosave::wait_for_saves(&mut app, &ctx);
        assert!(!app.doc.dirty());
        assert!(!app.allow_close);
        assert!(!app.save_then_action);
        assert!(app.error.is_some());
        assert!(app.recovery.slots[0].error.is_some());
        assert_eq!(latest(&app, false).text, "saved but cleanup fails");
    }
    #[test]
    fn bounded_store_contention_retries_latest_without_losing_either_pane() {
        let (_dir, mut app, ctx) = fixture();
        edit(&mut app.doc, "main retained across contention");
        app.global.secondary = Some(Document::default());
        edit(
            app.global.secondary.as_mut().unwrap(),
            "secondary retained across contention",
        );
        let store = app.recovery.store.clone().unwrap();
        let lock = store.lock().unwrap();
        let t = Instant::now();
        app.tick_recovery(&ctx, t);
        app.tick_recovery(&ctx, t + DEBOUNCE);
        let began = Instant::now();
        while app.recovery.slots.iter().any(|s| s.job.is_some()) {
            app.tick_recovery(&ctx, t + DEBOUNCE + began.elapsed());
            assert!(began.elapsed() < Duration::from_secs(5));
            std::thread::yield_now();
        }
        assert!(
            app.recovery
                .slots
                .iter()
                .all(|s| s.retry_after.is_some() && s.error.is_none())
        );
        drop(lock);
        wait(&mut app, &ctx, t + DEBOUNCE + began.elapsed());
        assert_eq!(latest(&app, false).text, "main retained across contention");
        assert_eq!(
            latest(&app, true).text,
            "secondary retained across contention"
        );
    }
    #[test]
    fn draft_count_write_and_read_boundaries_are_identical() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        let key = random_id().unwrap();
        let mut s = snapshot("body");
        s.drafts = vec![
            Draft {
                kind: "math".into(),
                text: "x".into(),
                original: "$x$".into(),
                range: [0, 3],
                expected: 1
            };
            MAX_DRAFTS
        ];
        store
            .commit(&key, s.clone(), &AtomicBool::new(false), 0)
            .unwrap();
        assert_eq!(
            read_latest(&store.directory(&key).unwrap())
                .unwrap()
                .drafts
                .len(),
            MAX_DRAFTS
        );
        s.drafts.push(s.drafts[0].clone());
        assert!(store.commit(&key, s, &AtomicBool::new(false), 0).is_err());
        assert_eq!(
            read_latest(&store.directory(&key).unwrap())
                .unwrap()
                .drafts
                .len(),
            MAX_DRAFTS
        );
    }
    #[test]
    fn damaged_reservation_counter_never_reports_success_with_older_recovered_content() {
        for value in ["truncated", "1", ""] {
            let dir = tempfile::tempdir().unwrap();
            let store = Store::open(dir.path().into()).unwrap();
            let key = random_id().unwrap();
            commit(&store, &key, "A");
            commit(&store, &key, "B");
            let record = store.directory(&key).unwrap();
            fs::write(record.join("sequence"), value).unwrap();
            let committed = commit(&store, &key, "C");
            assert_eq!(read_latest(&record).unwrap().text, "C");
            assert!(committed.sequence > 2);
        }
    }
    #[test]
    fn delayed_completion_does_not_restart_continuous_edit_period() {
        let (_dir, mut app, ctx) = fixture();
        let t = Instant::now();
        edit(&mut app.doc, "A");
        checkpoint(&mut app, &ctx, t);
        let (tx, rx) = mpsc::sync_channel(1);
        let version = app.recovery.slots[0].committed.clone().unwrap();
        app.recovery.slots[0].job = Some(Job {
            version,
            result: rx,
        });
        for n in 3..12 {
            edit(&mut app.doc, &format!("B{n}"));
            app.tick_recovery(&ctx, t + Duration::from_secs(n));
        }
        tx.send(Ok((snapshot("A"), now_ms(), 0.))).unwrap();
        edit(&mut app.doc, "latest at periodic deadline");
        app.tick_recovery(&ctx, t + Duration::from_secs(12));
        assert_eq!(app.recovery.slots[0].schedule_count, 2);
        wait(&mut app, &ctx, t + Duration::from_secs(12));
        assert_eq!(latest(&app, false).text, "latest at periodic deadline");
    }
    #[test]
    fn recovery_install_is_unsaved_unbound_preserves_original_and_survives_second_crash() {
        for external in [
            "unchanged",
            "modified",
            "deleted",
            "moved",
            "readonly",
            "inaccessible",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("original.md");
            fs::write(&source, "original bytes").unwrap();
            let writer = Store::open(dir.path().join("state")).unwrap();
            let key = random_id().unwrap();
            let mut s = snapshot("recovered 中文\r\nbody");
            s.path = Some(source.clone());
            s.drafts.push(Draft {
                kind: "math".into(),
                text: r"\frac{".into(),
                original: "$x$".into(),
                range: [0, 3],
                expected: 9,
            });
            writer.commit(&key, s, &AtomicBool::new(false), 0).unwrap();
            drop(writer);
            match external {
                "modified" => fs::write(&source, "external bytes").unwrap(),
                "deleted" => fs::remove_file(&source).unwrap(),
                "moved" => fs::rename(&source, dir.path().join("moved.md")).unwrap(),
                "readonly" => {
                    let mut p = fs::metadata(&source).unwrap().permissions();
                    p.set_readonly(true);
                    fs::set_permissions(&source, p).unwrap();
                }
                "inaccessible" => {
                    fs::remove_file(&source).unwrap();
                    fs::create_dir(&source).unwrap();
                }
                _ => {}
            }
            let before = fs::read(&source).ok();
            let ctx = egui::Context::default();
            let mut app = crate::preview_layout_tests::editor(&ctx, "");
            app.preferences.autosave = true;
            app.recovery.store = Some(Store::open(dir.path().join("state")).unwrap());
            let (s, claim) = app.recovery.store.as_ref().unwrap().claim(&key).unwrap();
            app.install_recovered(key.clone(), s, claim).unwrap();
            assert!(app.doc.path.is_none());
            assert!(app.doc.dirty());
            assert_eq!(app.doc.text, "recovered 中文\r\nbody");
            assert_eq!(app.recovery.slots[0].restored_drafts[0].text, r"\frac{");
            let t = Instant::now();
            app.tick_autosave(&ctx, t);
            app.tick_autosave(&ctx, t + PERIOD);
            assert_eq!(fs::read(&source).ok(), before);
            edit(&mut app.doc, "second crash edits");
            checkpoint(&mut app, &ctx, t);
            let previous_session = app.recovery.store.as_ref().unwrap().session.clone();
            drop(app);
            let reader = Store::open(dir.path().join("state")).unwrap();
            // Receiving a checkpoint is not a process exit: its thread can
            // still hold the session Arc while requesting a final repaint.
            let deadline = Instant::now() + Duration::from_secs(15);
            while reader.active(&previous_session) {
                assert!(
                    Instant::now() < deadline,
                    "completed worker retained its lease"
                );
                std::thread::yield_now();
            }
            let (s, _claim) = reader.claim(&key).unwrap();
            assert_eq!(s.text, "second crash edits");
            assert_eq!(s.path, Some(source.clone()));
            assert_eq!(s.drafts[0].text, r"\frac{");
            assert_eq!(fs::read(&source).ok(), before);
            #[cfg(windows)]
            if external == "readonly" {
                let mut p = fs::metadata(&source).unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)] // Windows-only fixture cleanup.
                p.set_readonly(false);
                fs::set_permissions(&source, p).unwrap();
            }
        }
    }
    #[test]
    fn completion_signal_does_not_release_a_still_live_worker_session() {
        let dir = tempfile::tempdir().unwrap();
        let owner = Store::open(dir.path().to_owned()).unwrap();
        let key = random_id().unwrap();
        commit(&owner, &key, "committed body");
        let worker_store = owner.clone();
        let (sent, received) = mpsc::sync_channel(1);
        let (release, wait_release) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            sent.send(()).unwrap();
            wait_release.recv().unwrap();
            drop(worker_store);
        });
        drop(owner);
        received.recv().unwrap();
        let reader = Store::open(dir.path().to_owned()).unwrap();
        assert!(
            reader
                .claim(&key)
                .err()
                .unwrap()
                .to_string()
                .contains("in use by another instance")
        );
        release.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(reader.claim(&key).unwrap().0.text, "committed body");
    }
    #[test]
    fn explicit_rich_draft_discard_persists_without_discarding_body() {
        let (_dir, mut app, ctx) = fixture();
        app.doc = Document::from_text("$x$\n");
        edit(&mut app.doc, "$x$\nbody edit");
        app.rich.stage_pending_test_edit(
            &app.doc.text,
            app.document_generation,
            "invalid draft",
            &ctx,
        );
        let t = Instant::now();
        checkpoint(&mut app, &ctx, t);
        assert!(!latest(&app, false).drafts.is_empty());
        app.rich.discard_pending_edits();
        app.tick_recovery(&ctx, t + PERIOD);
        let s = latest(&app, false);
        assert!(s.drafts.is_empty());
        assert_eq!(s.text, "$x$\nbody edit");
    }
    #[test]
    fn current_pointer_corruption_cannot_destroy_previous_valid_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        let key = random_id().unwrap();
        commit(&store, &key, "first");
        let d = store.directory(&key).unwrap();
        fs::write(d.join("current"), "corrupt").unwrap();
        commit(&store, &key, "second");
        let current = fs::read_to_string(d.join("current")).unwrap();
        fs::write(d.join(format!("{current}.json")), "truncated").unwrap();
        assert_eq!(read_latest(&d).unwrap().text, "first");
    }
    fn actor(root: &Path, key: &str, mode: &str, ready: &Path) -> std::process::Child {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "recovery::tests::process_actor",
                "--ignored",
                "--nocapture",
            ])
            .env("MDCARD_RECOVERY_ACTOR_ROOT", root)
            .env("MDCARD_RECOVERY_ACTOR_KEY", key)
            .env("MDCARD_RECOVERY_ACTOR_MODE", mode)
            .env("MDCARD_RECOVERY_ACTOR_READY", ready)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }
    fn ready(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "real child did not reach checkpoint"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn real_process_live_owner_kill_and_competing_claimants() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("state");
        let key = random_id().unwrap();
        let marker = dir.path().join("writer-ready");
        let mut writer = actor(&root, &key, "writer", &marker);
        ready(&marker);
        let reader = Store::open(root.clone()).unwrap();
        assert!(reader.discover().unwrap().is_empty());
        writer.kill().unwrap();
        writer.wait().unwrap();
        assert_eq!(reader.discover().unwrap().len(), 1);
        let a = dir.path().join("claim-a");
        let b = dir.path().join("claim-b");
        let mut child_a = actor(&root, &key, "claim", &a);
        let mut child_b = actor(&root, &key, "claim", &b);
        ready(&a);
        ready(&b);
        let outcomes = [
            fs::read_to_string(&a).unwrap(),
            fs::read_to_string(&b).unwrap(),
        ];
        assert_eq!(
            outcomes.iter().filter(|s| s.as_str() == "claimed").count(),
            1
        );
        assert!(reader.discover().unwrap().is_empty());
        child_a.kill().ok();
        child_b.kill().ok();
        child_a.wait().unwrap();
        child_b.wait().unwrap();
        assert_eq!(reader.discover().unwrap().len(), 1);
    }
    #[test]
    fn real_process_kill_at_write_sync_and_commit_reads_only_complete_generations() {
        for stage in 1..=3 {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("state");
            let key = random_id().unwrap();
            let marker = dir.path().join("ready");
            let mut child = actor(&root, &key, &format!("fault-{stage}"), &marker);
            ready(&marker);
            child.kill().unwrap();
            child.wait().unwrap();
            let reader = Store::open(root).unwrap();
            let items = reader.discover().unwrap();
            assert_eq!(items.len(), 1);
            let (s, _claim) = reader.claim(&key).unwrap();
            assert_eq!(
                s.text,
                if stage == 3 {
                    "new generation"
                } else {
                    "old generation"
                }
            );
            let d = reader.directory(&key).unwrap();
            assert!(!fs::read_dir(d).unwrap().any(|f| {
                f.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("checkpoint-")
            }));
        }
    }
    #[test]
    #[ignore = "subprocess helper; only run with isolated actor environment"]
    fn process_actor() {
        let Ok(root) = std::env::var("MDCARD_RECOVERY_ACTOR_ROOT") else {
            return;
        };
        let key = std::env::var("MDCARD_RECOVERY_ACTOR_KEY").unwrap();
        let mode = std::env::var("MDCARD_RECOVERY_ACTOR_MODE").unwrap();
        let marker = PathBuf::from(std::env::var_os("MDCARD_RECOVERY_ACTOR_READY").unwrap());
        let store = Store::open(root.into()).unwrap();
        if mode == "writer" {
            commit(&store, &key, "real process protected text");
            fs::write(&marker, "ready").unwrap();
        } else if mode == "claim" {
            match store.claim(&key) {
                Ok((_snapshot, claim)) => {
                    fs::write(&marker, "claimed").unwrap();
                    let _claim = claim;
                    loop {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                Err(_) => {
                    fs::write(&marker, "busy").unwrap();
                }
            }
        } else if let Some(stage) = mode.strip_prefix("fault-") {
            commit(&store, &key, "old generation");
            store
                .commit(
                    &key,
                    snapshot("new generation"),
                    &AtomicBool::new(false),
                    100 + stage.parse::<u8>().unwrap(),
                )
                .unwrap();
        }
        loop {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    #[test]
    fn stale_session_and_empty_reservation_cleanup_preserves_pending_manuscripts() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        let session = store.session.clone();
        let key = random_id().unwrap();
        commit(&store, &key, "pending");
        let empty = store.directory(&random_id().unwrap()).unwrap();
        fs::create_dir(&empty).unwrap();
        fs::write(empty.join("owner"), &session).unwrap();
        drop(store);
        let reader = Store::open(dir.path().into()).unwrap();
        assert!(!dir.path().join("sessions").join(session).exists());
        assert_eq!(reader.discover().unwrap().len(), 1);
        assert!(!empty.exists());
        assert_eq!(
            read_latest(&reader.directory(&key).unwrap()).unwrap().text,
            "pending"
        );
    }
    #[test]
    #[ignore = "requires an explicitly isolated 1 MiB tmpfs; scripts/verify-recovery-storage-linux.sh"]
    fn real_os_disk_full_preserves_last_valid_copy() {
        let root = PathBuf::from(
            std::env::var_os("MDCARD_RECOVERY_OS_TEST_DIR").expect("isolated tmpfs required"),
        );
        let isolated = tempfile::Builder::new()
            .prefix("mdcard-recovery-")
            .tempdir_in(root)
            .unwrap();
        let store = Store::open(isolated.path().to_owned()).unwrap();
        let key = random_id().unwrap();
        commit(&store, &key, "last valid checkpoint");
        #[cfg(target_os = "linux")]
        let _filler = {
            let path = std::ffi::CString::new(store.root.to_str().unwrap()).unwrap();
            let mut info = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: statvfs writes a complete statvfs to this valid output pointer.
            assert_eq!(
                unsafe { libc::statvfs(path.as_ptr(), info.as_mut_ptr()) },
                0
            );
            let info = unsafe { info.assume_init() };
            let available = info.f_bavail * info.f_frsize;
            // This dedicated test runs inside the dev container's bounded shm.
            assert!(
                available <= 128 * 1024 * 1024,
                "requires a bounded test filesystem"
            );
            let mut filler = tempfile::NamedTempFile::new_in(&store.root).unwrap();
            let chunk = [0u8; 64 * 1024];
            let mut left = available.saturating_sub(1024 * 1024);
            while left > 0 {
                let n = (left as usize).min(chunk.len());
                filler.write_all(&chunk[..n]).unwrap();
                left -= n as u64;
            }
            filler.as_file().sync_all().unwrap();
            filler
        };
        let error = store
            .commit(
                &key,
                snapshot(&"x".repeat(2 * 1024 * 1024)),
                &AtomicBool::new(false),
                0,
            )
            .err()
            .expect("small filesystem must return ENOSPC");
        println!("actual OS error: {error}");
        assert!(
            error.raw_os_error() == Some(28)
                || error.kind() == io::ErrorKind::StorageFull
                || error.to_string().contains("os error 28")
        );
        assert_eq!(
            read_latest(&store.directory(&key).unwrap()).unwrap().text,
            "last valid checkpoint"
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires unprivileged uid; scripts/verify-recovery-storage-linux.sh"]
    fn real_os_permission_denied_preserves_last_valid_copy() {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "run this test as an unprivileged uid"
        );
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().into()).unwrap();
        let key = random_id().unwrap();
        commit(&store, &key, "last valid checkpoint");
        let record = store.directory(&key).unwrap();
        fs::set_permissions(&record, fs::Permissions::from_mode(0o500)).unwrap();
        let error = store
            .commit(&key, snapshot("new edit"), &AtomicBool::new(false), 0)
            .err()
            .unwrap();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        println!("actual OS error: {error}");
        assert_eq!(read_latest(&record).unwrap().text, "last valid checkpoint");
        fs::set_permissions(&record, fs::Permissions::from_mode(0o700)).unwrap();
    }
}
