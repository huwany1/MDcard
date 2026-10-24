// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Native background search. Filesystem I/O, regex compilation, matching, diff
//! construction and transaction persistence never run on the egui frame thread.
use crate::{
    document::{self, Document},
    encoding, text_files,
};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MAX_RESULTS: usize = 10_000;
pub const MAX_INDEX_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
pub const QUERY_BUDGET: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub query: String,
    pub replacement: String,
    pub regex: bool,
    pub case: bool,
    pub word: bool,
    pub include: String,
    pub exclude: String,
}

pub struct Matcher {
    regex: Regex,
    include: GlobSet,
    exclude: GlobSet,
    word: bool,
    replacement: String,
    nfa_states: Option<usize>,
}
impl Matcher {
    pub fn new(options: &Options) -> Result<Self, String> {
        if options.query.len() > 1024 {
            return Err("Pattern exceeds the 1024 byte limit".into());
        }
        if options.replacement.len() > 64 * 1024 {
            return Err("Replacement exceeds the 64 KiB limit".into());
        }
        let pattern = if options.regex {
            options.query.clone()
        } else {
            regex::escape(&options.query)
        };
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(!options.case)
            .size_limit(1024 * 1024)
            .dfa_size_limit(1024 * 1024)
            .build()
            .map_err(|e| e.to_string())?;
        // Count the expanded Thompson NFA, including counted repetitions and
        // Unicode classes. Raw pattern length is not a safe estimate of work.
        let nfa_states = if options.regex {
            let nfa = regex_automata::nfa::thompson::NFA::compiler()
                .configure(
                    regex_automata::nfa::thompson::Config::new().nfa_size_limit(Some(1024 * 1024)),
                )
                .syntax(regex_automata::util::syntax::Config::new().case_insensitive(!options.case))
                .build(&pattern)
                .map_err(|e| e.to_string())?;
            Some(nfa.states().len())
        } else {
            None
        };
        Ok(Self {
            regex,
            include: globs(&options.include)?,
            exclude: globs(&options.exclude)?,
            word: options.word,
            replacement: if options.regex {
                angle_captures(&options.replacement)
            } else {
                options.replacement.replace('$', "$$")
            },
            nfa_states,
        })
    }
    fn allowed(&self, relative: &Path) -> bool {
        (self.include.is_empty() || self.include.is_match(relative))
            && !self.exclude.is_match(relative)
    }
    pub fn edits(&self, text: &str, guard: &Guard) -> Result<Vec<Edit>, String> {
        self.scan(text, guard, true)
    }
    fn scan(&self, text: &str, guard: &Guard, replacing: bool) -> Result<Vec<Edit>, String> {
        guard.check()?;
        if text.len() > text_files::MAX_FILE_BYTES as usize {
            return Err("Document exceeds the 16 MiB search limit".into());
        }
        let mut edits = Vec::new();
        let mut output_bytes = text.len();
        // Reject excessive work *before* entering a non-interruptible regex call.
        // This conservative budget also bounds repeated greedy captures (whose
        // worst-case iterator cost can be quadratic despite linear single finds).
        let mut work_remaining = guard
            .deadline
            .saturating_duration_since(Instant::now())
            .as_micros()
            .saturating_mul(64);
        let mut cursor = 0;
        let mut captures = self.regex.captures_iter(text);
        loop {
            guard.check()?;
            if let Some(states) = self.nfa_states {
                let work = (text.len() - cursor).saturating_mul(states) as u128;
                if work > work_remaining {
                    return Err("Search timed out: expanded pattern/input work budget exceeded; narrow the query".into());
                }
                work_remaining -= work;
            }
            let Some(cap) = captures.next() else {
                break;
            };
            guard.check()?;
            let m = cap.get(0).unwrap();
            cursor = m.end();
            if self.word && !whole_word(text, m.start(), m.end()) {
                continue;
            }
            // Keep one lookahead hit for search so the caller can distinguish an
            // exact limit from a truncated single-file result. Never truncate a diff.
            if edits.len() >= MAX_RESULTS + usize::from(!replacing) {
                if replacing {
                    return Err("Match limit reached; narrow the search before replacing".into());
                } else {
                    break;
                }
            }
            let mut replacement = String::new();
            if replacing {
                cap.expand(&self.replacement, &mut replacement);
            }
            output_bytes = output_bytes
                .saturating_sub(m.len())
                .saturating_add(replacement.len());
            if output_bytes > MAX_OUTPUT_BYTES {
                return Err("Replacement exceeds the 32 MiB output limit".into());
            }
            edits.push(Edit {
                start: m.start(),
                end: m.end(),
                replacement,
                selected: true,
            });
        }
        guard.check()?;
        Ok(edits)
    }
}
fn globs(source: &str) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in source.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        builder.add(
            GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|e| format!("Glob: {e}"))?,
        );
    }
    builder.build().map_err(|e| e.to_string())
}
fn angle_captures(input: &str) -> String {
    // Preserve $$ escapes. Rust's native ${name} and $1 forms remain supported.
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'$') {
            out.push_str("$$");
            chars.next();
        } else if c == '$' && chars.peek().is_some_and(|c| c.is_ascii_digit()) {
            out.push_str("${");
            while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                out.push(chars.next().unwrap());
            }
            out.push('}');
        } else if c == '$' && chars.peek() == Some(&'<') {
            let mut probe = chars.clone();
            probe.next();
            let mut name = String::new();
            let mut closed = false;
            for n in probe {
                if n == '>' {
                    closed = true;
                    break;
                }
                name.push(n);
            }
            if closed {
                for _ in 0..name.chars().count() + 2 {
                    chars.next();
                }
                out.push_str("${");
                out.push_str(&name);
                out.push('}');
            } else {
                out.push('$');
            }
        } else {
            out.push(c);
        }
    }
    out
}
fn word_char(c: char) -> bool {
    static WORD: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let mut buf = [0; 4];
    WORD.get_or_init(|| Regex::new(r"^\w$").unwrap())
        .is_match(c.encode_utf8(&mut buf))
}
fn whole_word(text: &str, start: usize, end: usize) -> bool {
    text[..start]
        .chars()
        .next_back()
        .is_none_or(|c| !word_char(c))
        && text[end..].chars().next().is_none_or(|c| !word_char(c))
}

pub struct Guard {
    generation: Arc<AtomicU64>,
    expected: u64,
    deadline: Instant,
}
impl Guard {
    pub fn new(generation: Arc<AtomicU64>, expected: u64, budget: Duration) -> Self {
        Self {
            generation,
            expected,
            deadline: Instant::now() + budget,
        }
    }
    pub fn check(&self) -> Result<(), String> {
        if self.generation.load(Ordering::Relaxed) != self.expected {
            Err("Search cancelled".into())
        } else if Instant::now() >= self.deadline {
            Err("Search timed out; narrow the query".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub selected: bool,
}
#[derive(Clone)]
pub struct PreviewFile {
    pub doc: Document,
    pub relative: PathBuf,
    pub edits: Vec<Edit>,
}
impl PreviewFile {
    pub fn after(&self) -> String {
        replace_selected(&self.doc.text, &self.edits)
    }
}
pub fn replace_selected(text: &str, edits: &[Edit]) -> String {
    let mut out = text.to_owned();
    for e in edits.iter().rev().filter(|e| e.selected) {
        out.replace_range(e.start..e.end, &e.replacement);
    }
    out
}
#[derive(Clone)]
pub struct Preview {
    pub root: PathBuf,
    pub files: Vec<PreviewFile>,
    pub single: bool,
    pub options: Options,
}
#[derive(Clone, Debug)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub snippet: String,
    pub highlight: [usize; 2],
    pub stamp: u64,
}
#[derive(Clone, Debug)]
pub struct Group {
    pub path: PathBuf,
    pub relative: String,
    pub hits: Vec<Hit>,
}
#[derive(Default)]
pub struct Results {
    pub groups: Vec<Group>,
    pub count: usize,
    pub elapsed_ms: f64,
    pub limited: bool,
}
#[derive(Clone, Debug)]
pub struct QuickItem {
    pub path: PathBuf,
    pub label: String,
    pub title: String,
    pub score: i64,
}
#[derive(Clone, Debug, Default)]
pub struct IndexStatus {
    pub files: usize,
    pub bytes: usize,
    pub changed: usize,
    pub errors: usize,
    pub limited: bool,
    pub ready: bool,
    pub full_verifications: u64,
    pub refresh_ms: f64,
}

struct Indexed {
    doc: Document,
    len: u64,
    modified: Option<SystemTime>,
    title: String,
    aliases: String,
    stamp: u64,
}
#[derive(Clone, Default)]
pub struct Index {
    root: PathBuf,
    entries: BTreeMap<PathBuf, Arc<Indexed>>,
    pub status: IndexStatus,
}
impl Index {
    pub fn new(root: &Path) -> io::Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
            ..Default::default()
        })
    }
    pub fn refresh(&mut self, full: bool, stopped: &AtomicBool) {
        self.refresh_limited(full, stopped, 500_000, 100_000, Duration::from_secs(15));
    }
    fn refresh_limited(
        &mut self,
        full: bool,
        stopped: &AtomicBool,
        max_entries: usize,
        max_files: usize,
        duration: Duration,
    ) {
        let mut paths = Vec::new();
        let mut errors = 0;
        let mut limited = false;
        let mut stack = vec![self.root.clone()];
        let started = Instant::now();
        let mut visited = 0;
        'walk: while let Some(dir) = stack.pop() {
            if stopped.load(Ordering::Relaxed) {
                return;
            }
            if started.elapsed() > duration || paths.len() >= max_files {
                limited = true;
                break;
            }
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            for e in entries {
                if stopped.load(Ordering::Relaxed) {
                    return;
                }
                // Enforce limits per entry: one flat directory can otherwise
                // bypass every outer-loop check and allocate an unbounded list.
                if visited >= max_entries
                    || paths.len() >= max_files
                    || started.elapsed() > duration
                {
                    limited = true;
                    break 'walk;
                }
                visited += 1;
                let Ok(e) = e else {
                    errors += 1;
                    continue;
                };
                let Ok(meta) = e.metadata() else {
                    errors += 1;
                    continue;
                };
                let path = e.path();
                if linked(&path) {
                    continue;
                }
                if meta.is_dir() {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    if !text_files::SKIPPED_DIRECTORIES
                        .iter()
                        .any(|n| name.eq_ignore_ascii_case(n))
                    {
                        stack.push(path);
                    }
                } else if meta.is_file() && text_files::supported(&path) {
                    paths.push((path, meta));
                }
            }
        }
        let mut seen = HashSet::new();
        let mut bytes = 0;
        let mut changed = 0;
        for (path, meta) in paths {
            if stopped.load(Ordering::Relaxed) {
                return;
            }
            if started.elapsed() > duration {
                limited = true;
                break;
            }
            if meta.len() > text_files::MAX_FILE_BYTES {
                limited = true;
                continue;
            }
            if bytes + meta.len() as usize > MAX_INDEX_BYTES {
                limited = true;
                continue;
            }
            bytes += meta.len() as usize;
            seen.insert(path.clone());
            let stale = self
                .entries
                .get(&path)
                .is_none_or(|e| full || e.len != meta.len() || e.modified != meta.modified().ok());
            if stale {
                match Document::open(&path) {
                    Ok(doc) => {
                        // Full verification still reads and compares disk bytes. An
                        // identical immutable snapshot needs no new metadata, hash,
                        // allocation or publication to the query worker.
                        if self.entries.get(&path).is_some_and(|e| {
                            e.doc.disk_snapshot() == doc.disk_snapshot()
                                && e.len == meta.len()
                                && e.modified == meta.modified().ok()
                        }) {
                            continue;
                        }
                        if self
                            .entries
                            .get(&path)
                            .is_none_or(|e| e.doc.disk_snapshot() != doc.disk_snapshot())
                        {
                            changed += 1;
                        }
                        let (title, aliases) = metadata(&doc.text);
                        let stamp = text_stamp(&doc.text);
                        self.entries.insert(
                            path,
                            Arc::new(Indexed {
                                doc,
                                len: meta.len(),
                                modified: meta.modified().ok(),
                                title,
                                aliases,
                                stamp,
                            }),
                        );
                    }
                    Err(_) => {
                        if self.entries.remove(&path).is_some() {
                            changed += 1;
                        }
                        errors += 1;
                    }
                }
            }
        }
        let before = self.entries.len();
        self.entries.retain(|p, _| seen.contains(p));
        changed += before.saturating_sub(self.entries.len());
        self.status = IndexStatus {
            files: self.entries.len(),
            bytes,
            changed,
            errors,
            limited,
            ready: true,
            full_verifications: self.status.full_verifications + u64::from(full),
            refresh_ms: started.elapsed().as_secs_f64() * 1000.,
        };
    }
    fn documents<'a>(
        &'a self,
        overlays: &'a [Document],
        single: bool,
    ) -> Vec<(PathBuf, &'a Document, Option<u64>)> {
        if single {
            return overlays
                .first()
                .map(|d| {
                    vec![(
                        d.path
                            .as_ref()
                            .and_then(|p| p.strip_prefix(&self.root).ok())
                            .unwrap_or(Path::new("Untitled.md"))
                            .to_owned(),
                        d,
                        None,
                    )]
                })
                .unwrap_or_default();
        }
        let mut docs: BTreeMap<_, _> = self
            .entries
            .iter()
            .map(|(p, d)| (p.clone(), (&d.doc, Some(d.stamp))))
            .collect();
        for doc in overlays {
            if let Some(path) = &doc.path
                && path.starts_with(&self.root)
            {
                docs.insert(path.clone(), (doc, None));
            }
        }
        docs.into_iter()
            .map(|(p, (d, stamp))| {
                (
                    p.strip_prefix(&self.root).unwrap_or(&p).to_owned(),
                    d,
                    stamp,
                )
            })
            .collect()
    }
    pub fn search(
        &self,
        options: &Options,
        overlays: &[Document],
        single: bool,
        guard: &Guard,
    ) -> Result<Results, String> {
        let started = Instant::now();
        let mut effective = options.clone();
        if single {
            effective.include.clear();
            effective.exclude.clear();
        }
        let matcher = Matcher::new(&effective)?;
        let mut results = Results::default();
        if options.query.is_empty() {
            return Ok(results);
        }
        for (relative, doc, cached_stamp) in self.documents(overlays, single) {
            guard.check()?;
            if !single && !matcher.allowed(&relative) {
                continue;
            }
            let edits = matcher.scan(&doc.text, guard, false)?;
            if edits.is_empty() {
                continue;
            }
            // Indexed documents are immutable snapshots. Their signature is
            // computed once on refresh; dirty overlays always use their own text.
            let stamp = cached_stamp.unwrap_or_else(|| text_stamp(&doc.text));
            // Matches are ordered, so count only lines traversed to each hit.
            // A match near the start no longer scans/allocates all trailing lines.
            let mut line_cursor = 0;
            let mut line_number = 1;
            let mut hits = Vec::new();
            for e in edits {
                if results.count >= MAX_RESULTS {
                    results.limited = true;
                    break;
                }
                let mut h = hit_context(&doc.text, e.start, e.end);
                line_number += doc.text[line_cursor..e.start]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count();
                line_cursor = e.start;
                h.line = line_number;
                h.stamp = stamp;
                hits.push(h);
                results.count += 1;
            }
            results.groups.push(Group {
                path: doc.path.clone().unwrap_or_default(),
                relative: relative.to_string_lossy().replace('\\', "/"),
                hits,
            });
            if results.limited {
                break;
            }
        }
        results.elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
        Ok(results)
    }
    pub fn preview(
        &self,
        options: &Options,
        overlays: &[Document],
        single: bool,
        guard: &Guard,
    ) -> Result<Preview, String> {
        let mut effective = options.clone();
        if single {
            effective.include.clear();
            effective.exclude.clear();
        }
        let matcher = Matcher::new(&effective)?;
        let mut files = Vec::new();
        let mut total = 0;
        if options.query.is_empty() {
            return Err("Enter a search pattern first".into());
        }
        for (relative, doc, _) in self.documents(overlays, single) {
            guard.check()?;
            if !single && !matcher.allowed(&relative) {
                continue;
            }
            let edits = matcher.edits(&doc.text, guard)?;
            total += edits.len();
            if total > MAX_RESULTS {
                return Err("Match limit reached; narrow the search before replacing".into());
            }
            if !edits.is_empty() {
                files.push(PreviewFile {
                    doc: doc.clone(),
                    relative,
                    edits,
                });
            }
        }
        Ok(Preview {
            root: self.root.clone(),
            files,
            single,
            options: options.clone(),
        })
    }
    pub fn quick(
        &self,
        query: &str,
        mru: &[PathBuf],
        guard: &Guard,
    ) -> Result<Vec<QuickItem>, String> {
        let mut items = Vec::new();
        for (path, entry) in &self.entries {
            guard.check()?;
            let label = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let scores = [
                (filename.as_ref(), 500),
                (label.as_str(), 100),
                (entry.title.as_str(), 250),
                (entry.aliases.as_str(), 300),
            ];
            let Some(mut score) = scores
                .into_iter()
                .filter_map(|(s, w)| fuzzy(query, s).map(|n| n + w))
                .max()
            else {
                continue;
            };
            if let Some(recent) = mru.iter().position(|p| p == path) {
                score += 200_i64.saturating_sub(recent as i64 * 15);
            }
            items.push(QuickItem {
                path: path.clone(),
                label,
                title: entry.title.clone(),
                score,
            });
        }
        items.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
        items.truncate(200);
        Ok(items)
    }
}
fn metadata(text: &str) -> (String, String) {
    let mut aliases = String::new();
    let mut title = String::new();
    let mut front = false;
    let mut alias_list = false;
    for (n, line) in text.lines().take(512).enumerate() {
        if line.trim() == "---" {
            if n == 0 {
                front = true;
                continue;
            } else if front {
                front = false;
                continue;
            }
        }
        if front {
            let trimmed = line.trim();
            if let Some(value) = trimmed
                .strip_prefix("aliases:")
                .or_else(|| trimmed.strip_prefix("alias:"))
            {
                aliases.push_str(value);
                alias_list = true;
            } else if alias_list && let Some(value) = trimmed.strip_prefix('-') {
                aliases.push(' ');
                aliases.push_str(value);
            } else if !trimmed.is_empty() {
                alias_list = false;
            }
        } else if title.is_empty()
            && let Some(heading) = line.strip_prefix("# ")
        {
            title = heading.trim().to_owned();
        }
    }
    (title, aliases)
}
pub fn fuzzy(query: &str, candidate: &str) -> Option<i64> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let candidate = candidate.to_lowercase();
    let mut chars = query.chars().peekable();
    let mut score = 0;
    let mut last = None;
    for (i, c) in candidate.chars().enumerate() {
        if Some(&c) == chars.peek() {
            score += 20;
            if last.is_some_and(|n| n + 1 == i) {
                score += 30;
            }
            if i == 0 {
                score += 100;
            }
            last = Some(i);
            chars.next();
        }
        if chars.peek().is_none() {
            return Some(score - candidate.chars().count() as i64);
        }
    }
    None
}
pub fn hit(text: &str, start: usize, end: usize) -> Hit {
    let line = text[..start].bytes().filter(|b| *b == b'\n').count() + 1;
    let mut h = hit_context(text, start, end);
    h.line = line;
    h.stamp = text_stamp(text);
    h
}
pub fn text_stamp(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}
fn hit_context(text: &str, start: usize, end: usize) -> Hit {
    let line_start = text[..start].rfind('\n').map_or(0, |p| p + 1);
    let left = text[line_start..start]
        .char_indices()
        .rev()
        .nth(60)
        .map_or(line_start, |(n, _)| line_start + n);
    let mut right = text[start..]
        .char_indices()
        .nth(180)
        .map_or(text.len(), |(n, _)| start + n);
    if let Some(n) = text[start..right].find('\n') {
        right = start + n;
    }
    Hit {
        start,
        end,
        line: 0,
        snippet: text[left..right].to_owned(),
        highlight: [start - left, end.min(right).saturating_sub(left)],
        stamp: 0,
    }
}

fn linked(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return true;
    };
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
pub fn confined(root: &Path, relative: &Path, existing: bool) -> io::Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(io::Error::other("Use a descendant relative path"));
    }
    let root = fs::canonicalize(root)?;
    let mut path = root.clone();
    for component in relative.components() {
        path.push(component);
        if path.exists() && linked(&path) {
            return Err(io::Error::other(
                "Symbolic links and junctions are excluded",
            ));
        }
    }
    let checked = if existing {
        fs::canonicalize(&path)?
    } else {
        fs::canonicalize(
            path.parent()
                .ok_or_else(|| io::Error::other("Invalid parent"))?,
        )?
        .join(path.file_name().unwrap_or_default())
    };
    if !checked.starts_with(root) {
        return Err(io::Error::other("Path escapes the workspace"));
    }
    Ok(checked)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HistoryFile {
    pub relative: PathBuf,
    pub before: String,
    pub after: String,
    pub source: String,
    pub encoding: String,
    pub bom: bool,
    pub disk_encoding: String,
    pub disk_bom: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct History {
    pub id: String,
    pub state: String,
    pub files: Vec<HistoryFile>,
}
#[derive(Clone, Debug)]
pub struct HistoryItem {
    pub id: String,
    pub state: String,
    pub files: usize,
}
pub fn histories(root: &Path) -> io::Result<Vec<HistoryItem>> {
    let dir = root.join(".mdcard-history");
    let mut items = Vec::new();
    if !dir.exists() {
        return Ok(items);
    }
    if linked(&dir) {
        return Err(io::Error::other("History directory must not be a link"));
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if linked(&path) {
            continue;
        }
        if let Ok(data) = read_bounded(&path.join("manifest.json"), MAX_INDEX_BYTES)
            && let Ok(h) = serde_json::from_slice::<History>(&data)
        {
            items.push(HistoryItem {
                id: h.id,
                state: h.state,
                files: h.files.len(),
            });
        }
    }
    items.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(items)
}
fn manifest(dir: &Path, h: &History) -> io::Result<()> {
    document::atomic_write(&dir.join("manifest.json"), &serde_json::to_vec_pretty(h)?)
}
#[cfg(test)]
thread_local! {static WRITE_FAULT:std::cell::Cell<Option<(usize,bool)>>=const{std::cell::Cell::new(None)};}
fn transaction_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    #[cfg(test)]
    {
        let fail = WRITE_FAULT.with(|fault| match fault.get() {
            Some((n, repeat)) if n <= 1 => {
                fault.set(if repeat { Some((0, true)) } else { None });
                true
            }
            Some((n, repeat)) => {
                fault.set(Some((n - 1, repeat)));
                false
            }
            None => false,
        });
        if fail {
            return Err(io::Error::other("Injected transaction write failure"));
        }
    }
    document::atomic_write(path, bytes)
}
fn read_bounded(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    use std::io::Read;
    if linked(path) {
        return Err(io::Error::other("Snapshot links are excluded"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(io::Error::other("Snapshot exceeds the size limit"));
    }
    Ok(bytes)
}

pub fn commit(preview: &Preview) -> io::Result<(Vec<Document>, String)> {
    if preview.single {
        return Err(io::Error::other(
            "Single-document replacements belong to the source undo transaction",
        ));
    }
    let mut plans = Vec::new();
    for file in &preview.files {
        if !file.edits.iter().any(|e| e.selected) {
            continue;
        }
        if file.relative.components().any(|c| {
            c.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(".mdcard-history")
        }) || !text_files::supported(&file.relative)
        {
            return Err(io::Error::other(
                "History files and unsupported types cannot be replaced",
            ));
        }
        let path = confined(&preview.root, &file.relative, true)?;
        let original = fs::read(&path)?;
        if Some(original.as_slice()) != file.doc.disk_snapshot() {
            return Err(io::Error::other(
                "Stale diff: a file changed outside MDcard; refresh the preview",
            ));
        }
        if fs::metadata(&path)?.permissions().readonly() {
            return Err(io::Error::other("A selected file is read-only"));
        }
        let after = file.after();
        let bytes = encoding::encode(&after, file.doc.encoding, file.doc.bom)?;
        if bytes.len() > text_files::MAX_FILE_BYTES as usize {
            return Err(io::Error::other(
                "Encoded replacement exceeds the 16 MiB file limit",
            ));
        }
        plans.push((file, path, original, after, bytes));
    }
    if plans.is_empty() {
        return Err(io::Error::other("No replacements selected"));
    }
    let base = preview.root.join(".mdcard-history");
    if base.exists() && linked(&base) {
        return Err(io::Error::other("History directory must not be a link"));
    }
    fs::create_dir_all(&base)?;
    let id = format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    );
    let dir = base.join(&id);
    fs::create_dir(&dir)?;
    let mut history = History {
        id: id.clone(),
        state: "prepared".into(),
        files: Vec::new(),
    };
    for (n, (file, _, original, _, bytes)) in plans.iter().enumerate() {
        document::atomic_write(&dir.join(format!("{n}.before")), original)?;
        document::atomic_write(&dir.join(format!("{n}.after")), bytes)?;
        let (disk_encoding, disk_bom) = file.doc.saved_format();
        history.files.push(HistoryFile {
            relative: file.relative.clone(),
            before: format!("{n}.before"),
            after: format!("{n}.after"),
            source: file.doc.text.clone(),
            encoding: file.doc.encoding.name().into(),
            bom: file.doc.bom,
            disk_encoding: disk_encoding.name().into(),
            disk_bom,
        });
    }
    manifest(&dir, &history)?;
    let mut written = 0;
    let result: io::Result<()> = (|| {
        for (file, path, original, _, bytes) in &plans {
            confined(&preview.root, &file.relative, true)?;
            if fs::read(path)? != *original {
                return Err(io::Error::other(
                    "Stale diff: a file changed during replacement",
                ));
            }
            transaction_write(path, bytes)?;
            written += 1;
        }
        history.state = "committed".into();
        manifest(&dir, &history)?;
        Ok(())
    })();
    if let Err(error) = result {
        let mut compensated = true;
        for (_, path, original, _, bytes) in plans[..written].iter().rev() {
            if fs::read(path).is_ok_and(|v| v == *bytes) {
                if transaction_write(path, original).is_err() {
                    compensated = false;
                }
            } else {
                compensated = false;
            }
        }
        history.state = if compensated {
            "aborted"
        } else {
            "recovery_required"
        }
        .into();
        let _ = manifest(&dir, &history);
        return Err(io::Error::other(format!(
            "{error}; history {id}: {}",
            history.state
        )));
    }
    let updates = plans
        .into_iter()
        .map(|(file, _, _, text, bytes)| {
            let mut doc = file.doc.clone();
            doc.accept_committed(text, bytes);
            doc
        })
        .collect();
    Ok((updates, id))
}

pub fn rollback(root: &Path, id: &str, overlays: &[Document]) -> io::Result<Vec<Document>> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err(io::Error::other("Invalid history identifier"));
    }
    let dir = confined(root, &Path::new(".mdcard-history").join(id), true)?;
    let mut history: History =
        serde_json::from_slice(&read_bounded(&dir.join("manifest.json"), MAX_INDEX_BYTES)?)?;
    if history.id != id || history.files.len() > MAX_RESULTS {
        return Err(io::Error::other("Invalid history manifest"));
    }
    if !["committed", "prepared", "recovery_required"].contains(&history.state.as_str()) {
        return Err(io::Error::other(
            "This snapshot has already been rolled back or aborted",
        ));
    }
    let mut plans = Vec::new();
    let mut unique = HashSet::new();
    for file in &history.files {
        let path = confined(root, &file.relative, true)?;
        if !unique.insert(path.clone()) {
            return Err(io::Error::other("Duplicate history path"));
        }
        let before_path = confined(&dir, Path::new(&file.before), true)?;
        let after_path = confined(&dir, Path::new(&file.after), true)?;
        let before = read_bounded(&before_path, 64 * 1024 * 1024)?;
        let after = read_bounded(&after_path, 64 * 1024 * 1024)?;
        let current = fs::read(&path)?;
        if current != after && current != before {
            return Err(io::Error::other(
                "Rollback conflict: a file changed after replacement",
            ));
        }
        for doc in overlays.iter().filter(|d| d.path.as_ref() == Some(&path)) {
            if doc.dirty() {
                return Err(io::Error::other(
                    "Rollback conflict: save or discard edits in open documents first",
                ));
            }
        }
        let format = |name: &str| {
            encoding::Encoding::ALL
                .into_iter()
                .find(|e| e.name() == name)
                .ok_or_else(|| io::Error::other("Unknown snapshot encoding"))
        };
        if file.source.len() > text_files::MAX_FILE_BYTES as usize {
            return Err(io::Error::other("Snapshot source exceeds the size limit"));
        }
        let doc = Document::restore_snapshot(
            path.clone(),
            before.clone(),
            format(&file.disk_encoding)?,
            file.disk_bom,
            file.source.clone(),
            format(&file.encoding)?,
            file.bom,
        )?;
        plans.push((doc, path, before, after, current));
    }
    let mut written = 0;
    let result: io::Result<()> = (|| {
        for (_, path, before, _, current) in &plans {
            if fs::read(path)? != *current {
                return Err(io::Error::other(
                    "Rollback conflict: file changed during rollback",
                ));
            }
            transaction_write(path, before)?;
            written += 1;
        }
        history.state = "rolled_back".into();
        manifest(&dir, &history)?;
        Ok(())
    })();
    if let Err(error) = result {
        for (_, path, before, _, current) in plans[..written].iter().rev() {
            if fs::read(path).is_ok_and(|v| v == *before) {
                let _ = transaction_write(path, current);
            }
        }
        return Err(error);
    }
    Ok(plans.into_iter().map(|(doc, _, _, _, _)| doc).collect())
}

pub enum Command {
    Query {
        options: Options,
        overlays: Vec<Document>,
        single: bool,
    },
    Preview {
        options: Options,
        overlays: Vec<Document>,
        single: bool,
    },
    Quick {
        query: String,
        recent: Vec<PathBuf>,
    },
    Apply(Preview),
    Rollback {
        id: String,
        overlays: Vec<Document>,
    },
    Create(PathBuf),
    Open(PathBuf),
    Save(Document),
    Refresh,
    History,
}
pub enum Output {
    Status(IndexStatus),
    Results(Results),
    Preview(Preview),
    Quick(Vec<QuickItem>),
    Committed(Vec<Document>, String),
    RolledBack(Vec<Document>),
    Created(PathBuf),
    Opened(Document),
    Saved(Document),
    History(Vec<HistoryItem>),
    Error(String),
}
pub struct Response {
    pub generation: u64,
    pub output: Output,
    pub roundtrip_ms: f64,
    pub queue_ms: f64,
    pub processing_ms: f64,
    pub completed_at: Instant,
}
pub struct Worker {
    pub root: PathBuf,
    tx: Sender<(u64, Instant, Command)>,
    rx: Receiver<Response>,
    generation: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}
impl Worker {
    pub fn start(root: PathBuf, repaint: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, commands) = mpsc::channel::<(u64, Instant, Command)>();
        let (responses, rx) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let gen_thread = generation.clone();
        let stop_thread = stop.clone();
        let worker_root = root.clone();
        thread::Builder::new()
            .name("mdcard-search".into())
            .spawn(move || {
                let mut index = match Index::new(&worker_root) {
                    Ok(i) => i,
                    Err(e) => {
                        let _ = responses.send(Response {
                            generation: 0,
                            output: Output::Error(e.to_string()),
                            roundtrip_ms: 0.,
                            queue_ms: 0.,
                            processing_ms: 0.,
                            completed_at: Instant::now(),
                        });
                        repaint();
                        return;
                    }
                };
                // The scanner owns a mutable cache. Queries read published immutable snapshots,
                // so even the periodic full-content verification cannot queue filesystem I/O
                // ahead of an interactive query. Cached documents are shared, never copied.
                let (published, snapshots) = mpsc::sync_channel::<Index>(2);
                let scanner_stop = stop_thread.clone();
                let scanner_repaint = repaint.clone();
                let mut scanner = index.clone();
                let refresh = Arc::new(AtomicBool::new(false));
                let scanner_refresh = refresh.clone();
                thread::Builder::new()
                    .name("mdcard-index".into())
                    .spawn(move || {
                        let mut full = Instant::now();
                        let mut first = true;
                        while !scanner_stop.load(Ordering::Relaxed) {
                            let verify_all = scanner_refresh.swap(false, Ordering::Relaxed)
                                || full.elapsed() >= Duration::from_secs(10);
                            scanner.refresh(verify_all, &scanner_stop);
                            if verify_all {
                                full = Instant::now();
                            }
                            if first || scanner.status.ready {
                                let _ = published.try_send(scanner.clone());
                                scanner_repaint();
                                first = false;
                            }
                            for _ in 0..30 {
                                if scanner_stop.load(Ordering::Relaxed)
                                    || scanner_refresh.load(Ordering::Relaxed)
                                {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(25));
                            }
                        }
                    })
                    .expect("index worker thread");
                while !stop_thread.load(Ordering::Relaxed) {
                    while let Ok(snapshot) = snapshots.try_recv() {
                        index = snapshot;
                        let _ = responses.send(Response {
                            generation: 0,
                            output: Output::Status(index.status.clone()),
                            roundtrip_ms: 0.,
                            queue_ms: 0.,
                            processing_ms: 0.,
                            completed_at: Instant::now(),
                        });
                        repaint();
                    }
                    let message = commands.recv_timeout(Duration::from_millis(20));
                    if let Ok((id, sent, command)) = message {
                        let processing_started = Instant::now();
                        let guard = Guard::new(gen_thread.clone(), id, QUERY_BUDGET);
                        // Mutations are never cancelled after dispatch; their completion must be delivered.
                        let output = match command {
                            Command::Query {
                                options,
                                overlays,
                                single,
                            } => index
                                .search(&options, &overlays, single, &guard)
                                .map(Output::Results),
                            Command::Preview {
                                options,
                                overlays,
                                single,
                            } => index
                                .preview(&options, &overlays, single, &guard)
                                .map(Output::Preview),
                            Command::Quick { query, recent } => {
                                index.quick(&query, &recent, &guard).map(Output::Quick)
                            }
                            Command::Apply(preview) => {
                                let result = commit(&preview)
                                    .map(|(d, id)| Output::Committed(d, id))
                                    .map_err(|e| e.to_string());
                                refresh.store(true, Ordering::Relaxed);
                                result
                            }
                            Command::Rollback { id, overlays } => {
                                let result = rollback(&index.root, &id, &overlays)
                                    .map(Output::RolledBack)
                                    .map_err(|e| e.to_string());
                                refresh.store(true, Ordering::Relaxed);
                                result
                            }
                            Command::Create(relative) => (|| {
                                let path = confined(&index.root, &relative, false)?;
                                if !text_files::markdown(Some(&path)) {
                                    return Err(io::Error::other("Create a Markdown filename"));
                                }
                                let file = fs::OpenOptions::new()
                                    .write(true)
                                    .create_new(true)
                                    .open(&path)?;
                                file.sync_all()?;
                                Ok(Output::Created(path))
                            })()
                            .map_err(|e| e.to_string()),
                            Command::Open(path) => Document::open(&path)
                                .map(Output::Opened)
                                .map_err(|e| e.to_string()),
                            Command::Save(mut doc) => (|| -> io::Result<Output> {
                                let path = doc
                                    .path
                                    .clone()
                                    .ok_or_else(|| io::Error::other("No document path"))?;
                                doc.save_to(&path)?;
                                Ok(Output::Saved(doc))
                            })()
                            .map_err(|e| e.to_string()),
                            Command::Refresh => {
                                refresh.store(true, Ordering::Relaxed);
                                Ok(Output::Status(index.status.clone()))
                            }
                            Command::History => histories(&index.root)
                                .map(Output::History)
                                .map_err(|e| e.to_string()),
                        }
                        .unwrap_or_else(Output::Error);
                        let completed_at = Instant::now();
                        let _ = responses.send(Response {
                            generation: id,
                            output,
                            roundtrip_ms: sent.elapsed().as_secs_f64() * 1000.,
                            queue_ms: processing_started.duration_since(sent).as_secs_f64() * 1000.,
                            processing_ms: completed_at
                                .duration_since(processing_started)
                                .as_secs_f64()
                                * 1000.,
                            completed_at,
                        });
                        repaint();
                    }
                }
            })
            .expect("search worker thread");
        Self {
            root,
            tx,
            rx,
            generation,
            stop,
        }
    }
    pub fn send(&self, command: Command) -> u64 {
        let id = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send((id, Instant::now(), command));
        id
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
    pub fn poll(&self) -> Option<Response> {
        self.rx.try_recv().ok()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit follow-up release timing across full byte verification"]
    fn followup_search_timing_across_full_verification_and_cpu_contention() {
        let dir = tempfile::tempdir().unwrap();
        let padding = "payload a reasonable markdown paragraph with ordinary words.\n".repeat(190);
        for n in 0..1600 {
            fs::write(
                dir.path().join(format!("note-{n:04}.md")),
                format!("# Document {n}\nentry-{n} 中文🦀\nneedle\n{padding}"),
            )
            .unwrap();
        }
        let corpus_bytes: u64 = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum();
        let stop_load = Arc::new(AtomicBool::new(false));
        let load_threads: usize = std::env::var("MDCARD_FOLLOWUP_LOAD_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let loads: Vec<_> = (0..load_threads)
            .map(|_| {
                let stop = stop_load.clone();
                thread::spawn(move || {
                    let mut n = 1u64;
                    while !stop.load(Ordering::Relaxed) {
                        for _ in 0..1000 {
                            n = std::hint::black_box(
                                n.wrapping_mul(6364136223846793005).wrapping_add(1),
                            );
                        }
                    }
                })
            })
            .collect();
        let thread_runtime = || -> Option<u64> {
            #[cfg(target_os = "linux")]
            {
                for entry in fs::read_dir("/proc/self/task").ok()?.flatten() {
                    if fs::read_to_string(entry.path().join("comm")).ok()?.trim() == "mdcard-search"
                    {
                        return fs::read_to_string(entry.path().join("schedstat"))
                            .ok()?
                            .split_whitespace()
                            .next()?
                            .parse()
                            .ok();
                    }
                }
            }
            None
        };
        let started = Instant::now();
        let worker = Worker::start(dir.path().to_owned(), Arc::new(|| {}));
        let receive = |worker: &Worker, id: u64| {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(response) = worker.poll()
                    && response.generation == id
                {
                    break response;
                }
                assert!(Instant::now() < until, "worker failed to respond");
                thread::sleep(Duration::from_micros(100));
            }
        };
        while !matches!(receive(&worker, 0).output, Output::Status(ref s) if s.ready) {}
        let index_ms = started.elapsed().as_secs_f64() * 1000.;
        let changed = dir.path().join("note-0000.md");
        let metadata = fs::metadata(&changed).unwrap();
        let original = fs::read_to_string(&changed).unwrap();
        fs::write(&changed, original.replace("needle", "needlX")).unwrap();
        fs::File::options()
            .write(true)
            .open(&changed)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        let queries = Instant::now();
        let mut rounds = Vec::new();
        let mut saw_full_verification = false;
        while queries.elapsed() < Duration::from_secs(14) {
            let n = rounds.len();
            let query = match n % 4 {
                0 => "needle",
                1 => "中文🦀",
                2 => "unfindable-rare-token-987",
                _ => r"(?m)^entry-(\d+) 中文🦀$",
            };
            let cpu_before = thread_runtime();
            let sent = Instant::now();
            let id = worker.send(Command::Query {
                options: Options {
                    query: query.into(),
                    regex: n % 4 == 3,
                    ..Default::default()
                },
                overlays: vec![],
                single: false,
            });
            let response = receive(&worker, id);
            let wall = sent.elapsed().as_secs_f64() * 1000.;
            let cpu = cpu_before
                .zip(thread_runtime())
                .map(|(a, b)| b.saturating_sub(a) as f64 / 1e6);
            if let Output::Results(results) = response.output {
                let expected = match n % 4 {
                    2 => 0,
                    _ => 1600,
                };
                if n % 4 == 0 && results.count == 1599 {
                    saw_full_verification = true;
                }
                assert!(results.count == expected || (n % 4 == 0 && results.count == 1599));
                rounds.push(serde_json::json!({"round":n,"elapsed_ms":queries.elapsed().as_secs_f64()*1000.,"wall_ms":wall,"worker_roundtrip_ms":response.roundtrip_ms,"queue_ms":response.queue_ms,"processing_ms":response.processing_ms,"poll_ms":response.completed_at.elapsed().as_secs_f64()*1000.,"engine_wall_ms":results.elapsed_ms,"worker_cpu_ms":cpu,"matches":results.count}));
            } else {
                panic!("unexpected query output");
            }
            thread::sleep(Duration::from_millis(20));
        }
        stop_load.store(true, Ordering::Relaxed);
        for load in loads {
            load.join().unwrap();
        }
        assert!(
            saw_full_verification,
            "same-size/mtime content change was not published across the 10s byte-verification cycle"
        );
        let mut wall: Vec<_> = rounds
            .iter()
            .map(|r| r["wall_ms"].as_f64().unwrap())
            .collect();
        wall.sort_by(f64::total_cmp);
        let report = serde_json::json!({"documents":1600,"bytes":corpus_bytes,"initial_index_ms":index_ms,"duration_ms":queries.elapsed().as_secs_f64()*1000.,"load_threads":load_threads,"samples":wall.len(),"p50_ms":wall[wall.len()/2],"p95_ms":wall[wall.len()*95/100],"p99_ms":wall[wall.len()*99/100],"max_ms":wall[wall.len()-1],"full_verification_observed":saw_full_verification,"rounds":rounds});
        if let Ok(path) = std::env::var("MDCARD_FOLLOWUP_SEARCH_REPORT") {
            fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        println!("FOLLOWUP_SEARCH={report}");
    }
    fn guard() -> Guard {
        Guard::new(Arc::new(AtomicU64::new(1)), 1, Duration::from_secs(10))
    }
    #[test]
    fn full_verification_reuses_identical_snapshot_but_invalidates_same_stat_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.md");
        fs::write(&path, "needle 中文\n").unwrap();
        let stopped = AtomicBool::new(false);
        let mut index = Index::new(dir.path()).unwrap();
        index.refresh(true, &stopped);
        let path = fs::canonicalize(path).unwrap();
        let first = index.entries[&path].clone();
        index.refresh(true, &stopped);
        assert!(
            Arc::ptr_eq(&first, &index.entries[&path]),
            "unchanged full verification must preserve its immutable snapshot"
        );
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        fs::write(&path, "needlX 中文\n").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        index.refresh(true, &stopped);
        assert!(!Arc::ptr_eq(&first, &index.entries[&path]));
        assert_eq!(index.entries[&path].doc.text, "needlX 中文\n");
        assert_eq!(index.status.changed, 1);
    }
    #[test]
    fn skipped_directories_and_history_are_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("visible.md"), "needle").unwrap();
        for name in ["TARGET", ".GIT", "Node_Modules", ".MDCARD-HISTORY"] {
            fs::create_dir(dir.path().join(name)).unwrap();
            fs::write(dir.path().join(name).join("hidden.md"), "needle").unwrap();
        }
        let mut index = Index::new(dir.path()).unwrap();
        index.refresh(true, &AtomicBool::new(false));
        assert_eq!(index.status.files, 1);
        let mut preview = index
            .preview(
                &Options {
                    query: "needle".into(),
                    replacement: "changed".into(),
                    ..Default::default()
                },
                &[],
                false,
                &guard(),
            )
            .unwrap();
        preview.files[0].relative = PathBuf::from(".MDCARD-HISTORY/hidden.md");
        preview.files[0].doc =
            Document::open(&dir.path().join(".MDCARD-HISTORY/hidden.md")).unwrap();
        assert!(
            commit(&preview).is_err(),
            "history protection must apply at the mutation boundary too"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(".MDCARD-HISTORY/hidden.md")).unwrap(),
            "needle"
        );
    }
    #[test]
    fn single_directory_search_scan_obeys_file_and_entry_budgets() {
        let dir = tempfile::tempdir().unwrap();
        for n in 0..5 {
            fs::write(dir.path().join(format!("{n}.md")), "needle").unwrap();
        }
        for (entries, files) in [(100, 2), (2, 100)] {
            let mut index = Index::new(dir.path()).unwrap();
            index.refresh_limited(
                true,
                &AtomicBool::new(false),
                entries,
                files,
                Duration::from_secs(5),
            );
            assert!(index.status.files <= entries.min(files));
            assert!(index.status.limited);
        }
    }
    #[test]
    fn one_file_result_overflow_is_marked_incomplete_but_exact_limit_is_complete() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::new(dir.path()).unwrap();
        let options = Options {
            query: "x".into(),
            ..Default::default()
        };
        for count in [MAX_RESULTS, MAX_RESULTS + 1] {
            let doc = Document::from_text(&"x ".repeat(count));
            let results = index.search(&options, &[doc], true, &guard()).unwrap();
            assert_eq!(results.count, MAX_RESULTS);
            assert_eq!(results.limited, count > MAX_RESULTS);
        }
    }
    #[test]
    fn captures_multiline_unicode_words_and_zero_width() {
        let options = Options {
            query: "(?m)^(?<name>猫) (\\d+)$".into(),
            replacement: "$<name>=$2/$$/${name}".into(),
            regex: true,
            ..Default::default()
        };
        let matcher = Matcher::new(&options).unwrap();
        let edits = matcher.edits("猫 12\n猫 34", &guard()).unwrap();
        assert_eq!(
            replace_selected("猫 12\n猫 34", &edits),
            "猫=12/$/猫\n猫=34/$/猫"
        );
        let options = Options {
            query: "(cat)".into(),
            replacement: "$1!".into(),
            regex: true,
            word: true,
            ..Default::default()
        };
        assert_eq!(
            replace_selected(
                "cat cats _cat cat猫 CAT",
                &Matcher::new(&options)
                    .unwrap()
                    .edits("cat cats _cat cat猫 CAT", &guard())
                    .unwrap()
            ),
            "cat! cats _cat cat猫 CAT!"
        );
        let options = Options {
            query: "(?s)a.*b".into(),
            replacement: "x".into(),
            regex: true,
            ..Default::default()
        };
        assert_eq!(
            Matcher::new(&options)
                .unwrap()
                .edits("a\n\nb", &guard())
                .unwrap()
                .len(),
            1
        );
        let options = Options {
            query: "".into(),
            regex: true,
            ..Default::default()
        };
        assert_eq!(
            Matcher::new(&options)
                .unwrap()
                .edits("猫🦀", &guard())
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn syntax_deadline_cancellation_and_redos_are_bounded() {
        assert!(
            Matcher::new(&Options {
                query: "(".into(),
                regex: true,
                ..Default::default()
            })
            .is_err()
        );
        let matcher = Matcher::new(&Options {
            query: "(a+)+$".into(),
            regex: true,
            ..Default::default()
        })
        .unwrap();
        let start = Instant::now();
        assert!(
            matcher
                .edits(&("a".repeat(100_000) + "!"), &guard())
                .unwrap()
                .is_empty()
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        let generation = Arc::new(AtomicU64::new(2));
        assert!(
            matcher
                .edits("a", &Guard::new(generation, 1, Duration::from_secs(1)))
                .unwrap_err()
                .contains("cancelled")
        );
        assert!(
            matcher
                .edits(
                    "a",
                    &Guard::new(Arc::new(AtomicU64::new(1)), 1, Duration::ZERO)
                )
                .unwrap_err()
                .contains("timed out")
        );
    }
    #[test]
    fn path_filters_fuzzy_metadata_incremental_and_same_metadata_refresh() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("notes")).unwrap();
        fs::write(
            dir.path().join("notes/a.md"),
            "---\naliases: [kitty, 猫咪]\n---\n# Document\nhello",
        )
        .unwrap();
        fs::write(dir.path().join("b.md"), "hello").unwrap();
        let mut index = Index::new(dir.path()).unwrap();
        index.refresh(false, &AtomicBool::new(false));
        let options = Options {
            query: "hello".into(),
            include: "notes/**/*.md".into(),
            ..Default::default()
        };
        assert_eq!(
            index.search(&options, &[], false, &guard()).unwrap().count,
            1
        );
        assert_eq!(index.quick("kitty", &[], &guard()).unwrap().len(), 1);
        assert_eq!(index.quick("doc", &[], &guard()).unwrap().len(), 1);
        assert!(fuzzy("n/a", "notes/a.md").is_some());
        fs::write(dir.path().join("b.md"), "world").unwrap();
        index.refresh(true, &AtomicBool::new(false));
        assert_eq!(
            index
                .search(
                    &Options {
                        query: "world".into(),
                        ..Default::default()
                    },
                    &[],
                    false,
                    &guard()
                )
                .unwrap()
                .count,
            1
        );
        fs::remove_file(dir.path().join("b.md")).unwrap();
        index.refresh(false, &AtomicBool::new(false));
        assert_eq!(index.status.files, 1);
    }
    #[test]
    fn transaction_dirty_buffer_encoding_stale_conflicts_and_restart_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        let original = encoding::encode("猫 one\r\n", encoding::Encoding::Utf16Le, true).unwrap();
        fs::write(&a, &original).unwrap();
        fs::write(&b, "one two").unwrap();
        let mut index = Index::new(dir.path()).unwrap();
        index.refresh(false, &AtomicBool::new(false));
        let mut overlay = Document::open(&a).unwrap();
        overlay.text.push_str("dirty one");
        let options = Options {
            query: "one".into(),
            replacement: "three".into(),
            ..Default::default()
        };
        let preview = index
            .preview(&options, &[overlay], false, &guard())
            .unwrap();
        let (updates, id) = commit(&preview).unwrap();
        assert_eq!(updates.len(), 2);
        assert!(updates.iter().all(|d| !d.dirty()));
        assert!(Document::open(&a).unwrap().text.ends_with("dirty three"));
        assert_eq!(&fs::read(&a).unwrap()[..2], &[0xff, 0xfe]);
        assert!(commit(&preview).unwrap_err().to_string().contains("Stale"));
        assert_eq!(histories(dir.path()).unwrap()[0].id, id);
        fs::write(&b, "external").unwrap();
        assert!(rollback(dir.path(), &id, &[]).is_err());
        assert!(Document::open(&a).unwrap().text.contains("three"));
        fs::write(&b, "three two").unwrap();
        let restored = rollback(dir.path(), &id, &[]).unwrap();
        assert_eq!(fs::read(&a).unwrap(), original);
        assert!(
            restored
                .iter()
                .find(|d| d.path.as_ref() == Some(&fs::canonicalize(&a).unwrap()))
                .unwrap()
                .dirty()
        );
        assert_eq!(fs::read_to_string(&b).unwrap(), "one two");
        assert!(rollback(dir.path(), &id, &[]).is_err());
    }
    #[test]
    fn traversal_and_replacement_failures_do_not_write_any_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "one").unwrap();
        assert!(confined(dir.path(), Path::new("../escape.md"), false).is_err());
        let mut index = Index::new(dir.path()).unwrap();
        index.refresh(false, &AtomicBool::new(false));
        let mut preview = index
            .preview(
                &Options {
                    query: "one".into(),
                    replacement: "two".into(),
                    ..Default::default()
                },
                &[],
                false,
                &guard(),
            )
            .unwrap();
        preview.files[0].relative = PathBuf::from("../a.md");
        assert!(commit(&preview).is_err());
        assert_eq!(fs::read_to_string(dir.path().join("a.md")).unwrap(), "one");
    }
    #[test]
    fn second_write_failure_compensates_and_failed_compensation_is_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.md", "b.md"] {
            fs::write(dir.path().join(name), "one").unwrap();
        }
        let mut index = Index::new(dir.path()).unwrap();
        index.refresh(true, &AtomicBool::new(false));
        let preview = index
            .preview(
                &Options {
                    query: "one".into(),
                    replacement: "two".into(),
                    ..Default::default()
                },
                &[],
                false,
                &guard(),
            )
            .unwrap();
        WRITE_FAULT.with(|f| f.set(Some((2, false))));
        let error = commit(&preview).unwrap_err().to_string();
        assert!(error.contains("aborted"));
        for name in ["a.md", "b.md"] {
            assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), "one");
        }
        WRITE_FAULT.with(|f| f.set(Some((2, true))));
        assert!(
            commit(&preview)
                .unwrap_err()
                .to_string()
                .contains("recovery_required")
        );
        WRITE_FAULT.with(|f| f.set(None));
        assert_eq!(fs::read_to_string(dir.path().join("a.md")).unwrap(), "two");
        let id = histories(dir.path())
            .unwrap()
            .into_iter()
            .find(|h| h.state == "recovery_required")
            .unwrap()
            .id;
        rollback(dir.path(), &id, &[]).unwrap();
        for name in ["a.md", "b.md"] {
            assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), "one");
        }
        let (_, id) = commit(&preview).unwrap();
        WRITE_FAULT.with(|f| f.set(Some((2, false))));
        assert!(rollback(dir.path(), &id, &[]).is_err());
        for name in ["a.md", "b.md"] {
            assert_eq!(fs::read_to_string(dir.path().join(name)).unwrap(), "two");
        }
        rollback(dir.path(), &id, &[]).unwrap();
    }
}
