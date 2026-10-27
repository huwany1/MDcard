// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! One type policy shared by the picker, document loader and background file index.
use crate::egui;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};

/// egui_extras::FileLoader consumes a raw file:// filesystem path, not a
/// percent-decoded URL. Canonical Win32 verbatim paths cannot be slash-rewritten:
/// //?/D:/... is a different, unreadable name. Preserve UNC native prefixes.
pub fn image_base_uri(base: &Path) -> String {
    let value = base.to_string_lossy();
    #[cfg(windows)]
    {
        let native = if let Some(tail) = value.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{tail}")
        } else {
            value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
        };
        if native.starts_with(r"\\") {
            return format!("file://{}/", native.trim_end_matches(['\\', '/']));
        }
        format!(
            "file:///{}/",
            native.replace('\\', "/").trim_end_matches('/')
        )
    }
    #[cfg(not(windows))]
    format!("file://{}/", value.trim_end_matches('/'))
}

pub const EXTENSIONS: &[&str] = &[
    "md",
    "markdown",
    "mdown",
    "mkd",
    "mkdn",
    "txt",
    "text",
    "log",
    "rst",
    "adoc",
    "org",
    "csv",
    "tsv",
    "json",
    "jsonl",
    "yaml",
    "yml",
    "toml",
    "xml",
    "ini",
    "cfg",
    "conf",
    "config",
    "properties",
    "env",
    "html",
    "htm",
    "css",
    "scss",
    "less",
    "svg",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "rs",
    "py",
    "pyw",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "java",
    "kt",
    "kts",
    "go",
    "rb",
    "php",
    "swift",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "psm1",
    "psd1",
    "bat",
    "cmd",
    "sql",
    "lua",
    "r",
    "dart",
    "vue",
    "svelte",
    "tex",
    "bib",
    "gitignore",
    "gitattributes",
    "editorconfig",
    "lock",
    "mdx",
];
pub const SKIPPED_DIRECTORIES: &[&str] = &[
    ".mdcard-history",
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
];
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub fn supported(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| EXTENSIONS.contains(&s.to_ascii_lowercase().as_str()))
        || [
            "readme",
            "license",
            "licence",
            "copying",
            "notice",
            "changelog",
            "makefile",
            "dockerfile",
            ".env",
            ".gitignore",
            ".gitattributes",
            ".editorconfig",
            ".dockerignore",
            ".npmrc",
            ".nvmrc",
        ]
        .contains(&name.as_str())
        || name.starts_with(".env.")
}

pub fn markdown(path: Option<&Path>) -> bool {
    path.is_none_or(|p| {
        p.extension().and_then(|s| s.to_str()).is_some_and(|s| {
            ["md", "markdown", "mdown", "mkd", "mkdn"].contains(&s.to_ascii_lowercase().as_str())
        })
    })
}

pub fn base_directory(path: Option<&Path>, startup: &Path) -> PathBuf {
    path.and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| {
            if p.is_absolute() {
                p.to_owned()
            } else {
                startup.join(p)
            }
        })
        .unwrap_or_else(|| startup.to_owned())
}

#[derive(Debug)]
pub struct Entry {
    pub relative: PathBuf,
    pub label: String,
    normalized: String,
}

impl Entry {
    fn new(relative: PathBuf) -> Self {
        let label = relative.to_string_lossy().replace('\\', "/");
        let normalized = label.to_lowercase();
        Self {
            relative,
            label,
            normalized,
        }
    }
}

#[derive(Clone, Copy)]
struct Budget {
    entries: usize,
    files: usize,
    duration: Duration,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            entries: 500_000,
            files: 100_000,
            duration: Duration::from_secs(15),
        }
    }
}

#[derive(Debug, Default)]
pub struct Scan {
    pub entries: Vec<Entry>,
    pub tree: Vec<TreeNode>,
    pub visited: usize,
    pub skipped: usize,
    pub errors: usize,
    pub first_error: Option<String>,
    pub limited: bool,
}

#[derive(Debug)]
pub struct TreeNode {
    pub relative: PathBuf,
    pub label: String,
    pub file: Option<usize>,
    pub children: Vec<usize>,
}

/// Constructed by the index worker, never by an egui frame.
fn build_tree(entries: &[Entry]) -> Vec<TreeNode> {
    let mut nodes = vec![TreeNode {
        relative: PathBuf::new(),
        label: String::new(),
        file: None,
        children: vec![],
    }];
    let mut directories = HashMap::from([(PathBuf::new(), 0)]);
    for (file, entry) in entries.iter().enumerate() {
        let mut relative = PathBuf::new();
        let mut parent = 0;
        let components: Vec<_> = entry.relative.components().collect();
        for (position, part) in components.iter().enumerate() {
            relative.push(part.as_os_str());
            let is_file = position + 1 == components.len();
            if !is_file && let Some(&existing) = directories.get(&relative) {
                parent = existing;
                continue;
            }
            let index = nodes.len();
            nodes.push(TreeNode {
                relative: relative.clone(),
                label: part.as_os_str().to_string_lossy().into_owned(),
                file: is_file.then_some(file),
                children: vec![],
            });
            nodes[parent].children.push(index);
            if !is_file {
                directories.insert(relative.clone(), index);
            }
            parent = index;
        }
    }
    // Sort once in the worker, directories first, then Unicode case-insensitive name.
    let keys: Vec<_> = nodes
        .iter()
        .map(|n| (n.file.is_some(), n.label.to_lowercase(), n.label.clone()))
        .collect();
    for node in &mut nodes {
        node.children.sort_unstable_by_key(|&child| &keys[child]);
    }
    nodes
}

#[derive(Default)]
pub struct TreeView {
    root: PathBuf,
    revision: Option<u64>,
    expanded: HashSet<PathBuf>,
    pub rows: Vec<(usize, usize)>,
}
impl TreeView {
    pub fn sync(&mut self, index: &FileIndex) {
        if self.revision == Some(index.revision) {
            return;
        }
        if self.root != index.root {
            self.root = index.root.clone();
            self.expanded.clear();
            self.expanded.insert(PathBuf::new());
        }
        self.revision = Some(index.revision);
        self.rebuild(&index.scan.tree);
    }
    fn rebuild(&mut self, nodes: &[TreeNode]) {
        self.rows.clear();
        if nodes.is_empty() {
            return;
        }
        let mut stack = vec![(0, 0)];
        while let Some((id, depth)) = stack.pop() {
            self.rows.push((id, depth));
            if self.expanded.contains(&nodes[id].relative) {
                stack.extend(
                    nodes[id]
                        .children
                        .iter()
                        .rev()
                        .map(|&child| (child, depth + 1)),
                );
            }
        }
    }
    pub fn expanded(&self, node: &TreeNode) -> bool {
        self.expanded.contains(&node.relative)
    }
    pub fn toggle(&mut self, id: usize, nodes: &[TreeNode]) {
        if !self.expanded.remove(&nodes[id].relative) {
            self.expanded.insert(nodes[id].relative.clone());
        }
        self.rebuild(nodes);
    }
}
impl Scan {
    fn error(&mut self, path: &Path, error: impl std::fmt::Display) {
        self.errors += 1;
        self.first_error
            .get_or_insert_with(|| format!("{}: {error}", path.display()));
    }
}

fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions, mount points and other reparse points also must not escape the root.
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

fn scan(root: &Path, cancel: &AtomicBool, budget: Budget) -> Scan {
    let mut result = Scan::default();
    match fs::symlink_metadata(root) {
        Ok(m) if m.is_dir() && !linked(&m) => {}
        Ok(_) => {
            result.error(root, "Search root must be a regular directory");
            return result;
        }
        Err(e) => {
            result.error(root, e);
            return result;
        }
    }
    let started = Instant::now();
    let mut directories = vec![root.to_owned()];
    'walk: while let Some(directory) = directories.pop() {
        let reader = match fs::read_dir(&directory) {
            Ok(reader) => reader,
            Err(e) => {
                result.error(&directory, e);
                continue;
            }
        };
        for entry in reader {
            if cancel.load(Ordering::Relaxed) {
                return result;
            }
            if result.visited >= budget.entries
                || result.entries.len() >= budget.files
                || started.elapsed() >= budget.duration
            {
                result.limited = true;
                break 'walk;
            }
            result.visited += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    result.error(&directory, e);
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    result.error(&path, e);
                    continue;
                }
            };
            if linked(&metadata) {
                result.skipped += 1;
                continue;
            }
            if metadata.is_dir() {
                if SKIPPED_DIRECTORIES
                    .iter()
                    .any(|n| entry.file_name().to_string_lossy().eq_ignore_ascii_case(n))
                {
                    result.skipped += 1;
                } else {
                    directories.push(path);
                }
            } else if metadata.is_file() && supported(&path) {
                result.entries.push(Entry::new(
                    path.strip_prefix(root).expect("descendant").to_owned(),
                ));
            }
        }
    }
    result
        .entries
        .sort_unstable_by(|a, b| a.normalized.cmp(&b.normalized).then(a.label.cmp(&b.label)));
    if !cancel.load(Ordering::Relaxed) {
        result.tree = build_tree(&result.entries);
    }
    result
}

/// Relative descendant path substring, normalized once per query; no disk access.
/// Reject absolute paths and parent components rather than suggesting escape results.
pub fn filter(entries: &[Entry], query: &str) -> Vec<usize> {
    let query = query.trim().replace('\\', "/");
    if query.starts_with('/')
        || query.contains(':')
        || query.split('/').any(|p| p == "..")
        || Path::new(&query).components().any(|p| {
            matches!(
                p,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return vec![];
    }
    let query = query.trim_start_matches("./").to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.normalized.contains(&query).then_some(i))
        .collect()
}

pub struct FileIndex {
    pub root: PathBuf,
    pub scan: Scan,
    pub scanning: bool,
    pub revision: u64,
    receiver: Option<Receiver<Scan>>,
    cancel: Arc<AtomicBool>,
    last_started: Option<Instant>,
    pending_refresh: bool,
}
impl FileIndex {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            scan: Scan::default(),
            scanning: false,
            revision: 0,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            last_started: None,
            pending_refresh: false,
        }
    }
    pub fn restart(&mut self, root: PathBuf) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        if self.root != root {
            self.root = root.clone();
            self.scan = Scan::default();
            self.revision += 1;
        }
        self.scanning = true;
        self.pending_refresh = false;
        self.last_started = Some(Instant::now());
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            let result = scan(&root, &cancel, Budget::default());
            let _ = sender.send(result);
        });
    }
    /// Coalesce rapid same-folder navigation; retain the previous tree during refresh.
    pub fn ensure(&mut self, root: PathBuf) {
        if root != self.root
            || (!self.scanning
                && self
                    .last_started
                    .is_none_or(|at| at.elapsed() >= Duration::from_secs(2)))
        {
            self.restart(root);
        } else {
            self.pending_refresh = true;
        }
    }
    /// Keep the UI asleep while idle but wake it for coalesced refreshes.
    pub fn request_repaint(&self, ctx: &egui::Context) {
        if self.scanning {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.pending_refresh
            && let Some(started) = self.last_started
        {
            ctx.request_repaint_after(Duration::from_secs(2).saturating_sub(started.elapsed()));
        }
    }
    pub fn poll(&mut self) {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(scan) => {
                    self.scan = scan;
                    self.scanning = false;
                    self.revision += 1;
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scanning = false;
                    self.scan.error(&self.root, "Search worker stopped");
                    self.revision += 1;
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.pending_refresh
            && !self.scanning
            && self
                .last_started
                .is_some_and(|at| at.elapsed() >= Duration::from_secs(2))
        {
            self.restart(self.root.clone());
        }
    }
}
impl Drop for FileIndex {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_relative_image_prefix_is_readable_by_the_native_file_loader() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("image 目录");
        std::fs::create_dir(&directory).unwrap();
        let name = "test 图片.png";
        std::fs::write(directory.join(name), b"image-test-bytes").unwrap();
        let uri = format!(
            "{}{name}",
            image_base_uri(&directory.canonicalize().unwrap())
        );
        let path = uri.strip_prefix("file://").unwrap();
        #[cfg(windows)]
        let path = path.trim_start_matches('/');
        assert_eq!(std::fs::read(path).unwrap(), b"image-test-bytes");
        assert!(!uri.contains("//?/"));
    }
    #[test]
    fn directory_tree_groups_sorts_and_flattens_only_expanded_folders() {
        let mut index = FileIndex::new(PathBuf::from("workspace"));
        index.scan.entries = [
            "z.txt",
            "中文/a.txt",
            "folder/deep/note.md",
            "folder/B.TXT",
            "A.md",
        ]
        .into_iter()
        .map(|p| Entry::new(PathBuf::from(p)))
        .collect();
        index.scan.tree = build_tree(&index.scan.entries);
        let mut view = TreeView::default();
        view.sync(&index);
        let labels: Vec<_> = view
            .rows
            .iter()
            .map(|&(id, _)| index.scan.tree[id].label.as_str())
            .collect();
        assert_eq!(labels, ["", "folder", "中文", "A.md", "z.txt"]);
        let folder = view.rows[1].0;
        view.toggle(folder, &index.scan.tree);
        assert_eq!(view.rows.len(), 7);
        assert!(
            view.rows
                .iter()
                .any(|&(id, depth)| index.scan.tree[id].label == "B.TXT" && depth == 2)
        );
        assert!(
            !view
                .rows
                .iter()
                .any(|&(id, _)| index.scan.tree[id].label == "note.md")
        );
        view.toggle(0, &index.scan.tree);
        assert_eq!(view.rows, [(0, 0)]);
        view.toggle(0, &index.scan.tree);
        assert_eq!(view.rows.len(), 7);
        let original = view.rows.as_ptr();
        view.sync(&index);
        assert_eq!(
            view.rows.as_ptr(),
            original,
            "unchanged frame retains cached rows"
        );
        index.root = PathBuf::from("other");
        index.revision += 1;
        view.sync(&index);
        assert_eq!(view.rows.len(), 5, "new root clears folder expansion state");
    }
    #[test]
    fn automatic_index_coalesces_navigation_and_retains_tree_during_refresh() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("root.md"), "root").unwrap();
        let mut index = FileIndex::new(directory.path().to_owned());
        index.ensure(directory.path().to_owned());
        let started = index.last_started;
        index.ensure(directory.path().to_owned());
        assert_eq!(
            index.last_started, started,
            "in-flight automatic scan is reused"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while index.scanning && Instant::now() < deadline {
            index.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(index.scan.entries.len(), 1);
        assert_eq!(index.scan.tree.len(), 2);
        index.ensure(directory.path().to_owned());
        assert_eq!(
            index.last_started, started,
            "rapid same-folder navigation is coalesced"
        );
        fs::write(directory.path().join("added.txt"), "added").unwrap();
        assert!(
            index.pending_refresh,
            "coalesced request is retained for eventual refresh"
        );
        // With the scan finished, the idle UI must still schedule the queued
        // refresh deadline; otherwise no event may call poll() again.
        index.last_started = Some(Instant::now() - Duration::from_secs(1));
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |_| {});
        }
        let frame = ctx.run(egui::RawInput::default(), |ctx| index.request_repaint(ctx));
        let delay = frame.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(delay > Duration::ZERO && delay <= Duration::from_secs(1));
        // Advance only the scheduler clock; no two-second delay is needed for this test.
        index.last_started = Some(Instant::now() - Duration::from_secs(3));
        index.poll();
        assert!(
            index.scanning,
            "queued automatic refresh starts after the deadline"
        );
        assert_eq!(
            index.scan.entries.len(),
            1,
            "refresh retains old rows until replacement arrives"
        );
        while index.scanning && Instant::now() < deadline {
            index.poll();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(index.scan.entries.len(), 2);
    }
    #[test]
    fn hundred_thousand_file_tree_has_bounded_visible_rows() {
        let started = Instant::now();
        let entries: Vec<_> = (0..100_000)
            .map(|n| {
                Entry::new(PathBuf::from(format!(
                    "folder-{}/deep/note-{n}.txt",
                    n % 100
                )))
            })
            .collect();
        let mut index = FileIndex::new(PathBuf::from("large"));
        index.scan.tree = build_tree(&entries);
        index.scan.entries = entries;
        let built = started.elapsed();
        let mut view = TreeView::default();
        view.sync(&index);
        assert_eq!(index.scan.tree.len(), 100_201);
        assert_eq!(
            view.rows.len(),
            101,
            "collapsed descendants are absent from frame rows"
        );
        let flat = Instant::now();
        for _ in 0..10_000 {
            view.sync(&index);
        }
        eprintln!(
            "Tree performance: 100000 files, {} nodes, build {} ms, visible {}, 10000 unchanged frames {} us",
            index.scan.tree.len(),
            built.as_millis(),
            view.rows.len(),
            flat.elapsed().as_micros()
        );
        assert_eq!(view.rows.len(), 101);
    }
    #[test]
    fn large_index_filtering_uses_only_cached_paths() {
        let entries: Vec<_> = (0..100_000)
            .map(|n| {
                Entry::new(PathBuf::from(format!(
                    "中文 空格/section-{}/note-{n}.TXT",
                    n % 20
                )))
            })
            .collect();
        let started = Instant::now();
        assert_eq!(filter(&entries, "SECTION-3/").len(), 5_000);
        assert_eq!(filter(&entries, "note-99999.txt").len(), 1);
        assert_eq!(filter(&entries, "missing").len(), 0);
        eprintln!(
            "Cached path filter: 100000 entries, 3 queries, {} ms",
            started.elapsed().as_millis()
        );
    }
    #[test]
    fn supported_types_and_markdown_classification() {
        for name in [
            "a.MD",
            "a.TXT",
            "a.JSON",
            "a.yaml",
            "a.rs",
            "README",
            ".env.local",
            "Dockerfile",
        ] {
            assert!(supported(Path::new(name)), "{name}");
        }
        for name in ["a.pdf", "a.exe", "a.png", "a.zip", "unknown"] {
            assert!(!supported(Path::new(name)));
        }
        assert!(markdown(Some(Path::new("test.MD"))));
        assert!(!markdown(Some(Path::new("test.txt"))));
        assert!(!markdown(Some(Path::new("test.html"))));
    }
    #[test]
    fn recursive_relative_unicode_case_filter_and_budgets() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("中文 空格/deep/more")).unwrap();
        fs::create_dir_all(dir.path().join("node_modules/deep")).unwrap();
        for name in [
            "root.md",
            "中文 空格/deep/more/NOTE.TXT",
            "中文 空格/settings.json",
            "node_modules/deep/hidden.md",
            "bad.exe",
        ] {
            fs::write(dir.path().join(name), "# <literal>").unwrap();
        }
        let result = scan(dir.path(), &AtomicBool::new(false), Budget::default());
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.errors, 0);
        let hits = filter(&result.entries, ".\\中文 空格\\deep\\MORE\\note.txt");
        assert_eq!(hits.len(), 1);
        assert_eq!(
            result.entries[hits[0]].label,
            "中文 空格/deep/more/NOTE.TXT"
        );
        for invalid in ["../", "deep/../", "/root", "C:\\root"] {
            assert!(filter(&result.entries, invalid).is_empty());
        }
        assert!(filter(&result.entries, "absent").is_empty());
        assert!(
            scan(
                dir.path(),
                &AtomicBool::new(false),
                Budget {
                    entries: 1,
                    ..Budget::default()
                }
            )
            .limited
        );
        assert!(
            scan(
                &dir.path().join("missing"),
                &AtomicBool::new(false),
                Budget::default()
            )
            .errors
                > 0
        );
    }
    #[cfg(unix)]
    #[test]
    fn links_and_cycles_are_never_followed() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        std::os::unix::fs::symlink(root.path(), root.path().join("cycle")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            root.path().join("link.txt"),
        )
        .unwrap();
        let result = scan(root.path(), &AtomicBool::new(false), Budget::default());
        assert_eq!(result.skipped, 3);
        assert!(result.entries.is_empty());
    }
    #[test]
    fn base_is_current_file_parent_and_refresh_discards_old_root() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        fs::create_dir(&child).unwrap();
        fs::write(root.path().join("outer.md"), "outer").unwrap();
        fs::write(child.join("inner.txt"), "inner").unwrap();
        assert_eq!(
            base_directory(Some(&child.join("inner.txt")), root.path()),
            child
        );
        assert_eq!(base_directory(None, root.path()), root.path());
        let mut index = FileIndex::new(root.path().to_owned());
        index.restart(root.path().to_owned());
        index.restart(child);
        let deadline = Instant::now() + Duration::from_secs(5);
        while index.scanning && Instant::now() < deadline {
            index.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!index.scanning);
        assert_eq!(index.scan.entries.len(), 1);
        assert_eq!(index.scan.entries[0].label, "inner.txt");
        fs::write(index.root.join("new.yaml"), "x: 1").unwrap();
        index.restart(index.root.clone());
        while index.scanning && Instant::now() < deadline {
            index.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(index.scan.entries.len(), 2);
    }
}
