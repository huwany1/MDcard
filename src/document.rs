// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use crate::encoding::{self, Encoding};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
pub fn content_digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

#[derive(Clone, Copy, Debug)]
struct DirtyPresentation {
    revision: u64,
    bytes: usize,
    format: (Encoding, bool, Encoding, bool),
    dirty: bool,
}

#[derive(Clone, Copy, Debug)]
struct TextStatistics {
    revision: u64,
    bytes: usize,
    lines: usize,
    characters: usize,
}

/// Cloned documents receive independent presentation caches with their buffer.
#[derive(Debug, Default)]
struct PresentationCache(Mutex<(Option<DirtyPresentation>, Option<TextStatistics>)>);
impl Clone for PresentationCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}
#[derive(Debug, Default)]
struct RecoveryDigest(Mutex<Option<(u64, usize, u64)>>);
impl Clone for RecoveryDigest {
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[derive(Debug)]
struct BufferId(u64);
impl Default for BufferId {
    fn default() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}
impl Clone for BufferId {
    fn clone(&self) -> Self {
        // A cloned source is a separate editable buffer, even for the same file.
        Self::default()
    }
}

/// The source buffer is authoritative. Only a successful atomic save updates `saved`.
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub id: crate::reading_state::DocumentId,
    pub text: String,
    pub path: Option<PathBuf>,
    saved: String,
    disk_bytes: Option<Vec<u8>>,
    pub encoding: Encoding,
    pub bom: bool,
    pub guessed: bool,
    saved_encoding: Encoding,
    saved_bom: bool,
    revision: u64,
    buffer_id: BufferId,
    presentation: PresentationCache,
    /// A recovered empty manuscript still requires an explicit save or discard.
    recovered_unsaved: bool,
    recovery_origin: bool,
    recovery_digest: RecoveryDigest,
}

impl Document {
    pub fn buffer_id(&self) -> u64 {
        self.buffer_id.0
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn mark_text_changed(&mut self) {
        // Returning an edited untitled manuscript to empty is a valid unsaved
        // edit, distinct from a new untouched empty buffer.
        if self.path.is_none() && self.text.is_empty() && self.revision > 0 {
            self.recovered_unsaved = true;
        }
        self.revision = self.revision.wrapping_add(1);
        *self
            .presentation
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = (None, None);
    }
    pub fn continue_revision(&mut self, prior: u64) {
        self.revision = prior.wrapping_add(1);
        *self
            .presentation
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = (None, None);
    }
    pub fn saved_format(&self) -> (Encoding, bool) {
        (self.saved_encoding, self.saved_bom)
    }
    pub fn restore_snapshot(
        path: PathBuf,
        bytes: Vec<u8>,
        saved_format: Encoding,
        saved_bom: bool,
        text: String,
        pending_format: Encoding,
        pending_bom: bool,
    ) -> io::Result<Self> {
        let decoded = encoding::decode(&bytes, Some(saved_format))?;
        Ok(Self {
            recovered_unsaved: false,
            recovery_origin: false,
            recovery_digest: Default::default(),
            id: crate::reading_state::DocumentId::default(),
            text,
            path: Some(path),
            saved: decoded.text,
            disk_bytes: Some(bytes),
            encoding: pending_format,
            bom: pending_bom,
            guessed: false,
            saved_encoding: saved_format,
            saved_bom,
            revision: 0,
            buffer_id: BufferId::default(),
            presentation: PresentationCache::default(),
        })
    }
    pub fn disk_snapshot(&self) -> Option<&[u8]> {
        self.disk_bytes.as_deref()
    }
    /// Accept a worker-committed snapshot without losing the source editor's history.
    pub fn accept_committed(&mut self, text: String, bytes: Vec<u8>) {
        self.text = text;
        self.mark_text_changed();
        self.saved.clone_from(&self.text);
        self.disk_bytes = Some(bytes);
        self.saved_encoding = self.encoding;
        self.saved_bom = self.bom;
        self.recovered_unsaved = false;
    }
    pub fn from_text(text: &str) -> Self {
        Self {
            text: text.into(),
            saved: text.into(),
            ..Self::default()
        }
    }
    /// Accept a committed baseline without replacing newer edits or save format.
    pub fn accept_saved_snapshot(&mut self, saved: &Self) {
        self.recovered_unsaved = false;
        self.path.clone_from(&saved.path);
        self.saved.clone_from(&saved.saved);
        self.disk_bytes.clone_from(&saved.disk_bytes);
        self.saved_encoding = saved.saved_encoding;
        self.saved_bom = saved.saved_bom;
        self.presentation
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0 = None;
    }
    /// An explicit discard applies immediately, even if the following open
    /// dialog is cancelled or the requested replacement cannot be loaded.
    pub fn discard_changes(&mut self) {
        self.recovered_unsaved = false;
        self.text.clone_from(&self.saved);
        self.encoding = self.saved_encoding;
        self.bom = self.saved_bom;
        self.mark_text_changed();
        self.recovered_unsaved = false;
    }
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_with_encoding(path, None)
    }
    pub fn open_with_encoding(path: &Path, selected: Option<Encoding>) -> io::Result<Self> {
        validate_type(path)?;
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        let path = if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            path
        } else {
            fs::canonicalize(path)?
        };
        let file = fs::File::open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Only regular text files can be opened.",
            ));
        }
        let mut bytes = Vec::new();
        file.take(crate::text_files::MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > crate::text_files::MAX_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "The document exceeds the 16 MiB opening limit.",
            ));
        }
        let decoded = encoding::decode(&bytes, selected)?;
        let text = decoded.text;
        Ok(Self {
            recovered_unsaved: false,
            recovery_origin: false,
            recovery_digest: Default::default(),
            id: crate::reading_state::DocumentId::default(),
            saved: text.clone(),
            text,
            path: Some(path),
            disk_bytes: Some(bytes),
            encoding: decoded.encoding,
            saved_encoding: decoded.encoding,
            bom: decoded.bom,
            saved_bom: decoded.bom,
            guessed: decoded.guessed,
            revision: 0,
            buffer_id: BufferId::default(),
            presentation: PresentationCache::default(),
        })
    }
    pub fn dirty(&self) -> bool {
        self.recovered_unsaved
            || self.text != self.saved
            || self.encoding != self.saved_encoding
            || self.bom != self.saved_bom
    }
    pub fn recovered(text: String, encoding: Encoding, bom: bool) -> Self {
        Self {
            text,
            encoding,
            bom,
            recovered_unsaved: true,
            recovery_origin: true,
            ..Self::default()
        }
    }
    pub fn is_recovery_copy(&self) -> bool {
        self.recovery_origin
    }
    pub fn diagnostic_digest(&self) -> u64 {
        let mut cache = self
            .recovery_digest
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((revision, bytes, value)) = *cache
            && revision == self.revision
            && bytes == self.text.len()
        {
            return value;
        }
        let value = content_digest(self.text.as_bytes());
        *cache = Some((self.revision, self.text.len(), value));
        value
    }
    /// Cached chrome only. Save, close and switch guards must use `dirty`.
    /// Production edits explicitly invalidate this cache through the revision.
    pub fn presentation_dirty(&self) -> bool {
        let format = (self.encoding, self.bom, self.saved_encoding, self.saved_bom);
        let mut cache = self
            .presentation
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = cache.0
            && cached.revision == self.revision
            && cached.bytes == self.text.len()
            && cached.format == format
        {
            return cached.dirty;
        }
        let dirty = self.dirty();
        cache.0 = Some(DirtyPresentation {
            revision: self.revision,
            bytes: self.text.len(),
            format,
            dirty,
        });
        dirty
    }
    /// Ordinary-document status counts preserve `str::lines` and scalar semantics.
    /// The large-document UI already uses its rope's counters instead.
    pub fn text_statistics(&self) -> (usize, usize) {
        let mut cache = self
            .presentation
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = cache.1
            && cached.revision == self.revision
            && cached.bytes == self.text.len()
        {
            return (cached.lines, cached.characters);
        }
        let lines = self.text.lines().count();
        let characters = self.text.chars().count();
        cache.1 = Some(TextStatistics {
            revision: self.revision,
            bytes: self.text.len(),
            lines,
            characters,
        });
        (lines, characters)
    }
    /// Same-directory rename refuses overwrites and checks the saved baseline.
    /// Pending source edits stay in memory and retain their dirty state.
    pub fn rename_in_place(&mut self, name: &str) -> io::Result<()> {
        let old = self
            .path
            .as_ref()
            .ok_or_else(|| io::Error::other("Save this document before renaming it."))?;
        if name.trim().is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
            return Err(io::Error::other("Enter a filename without folders."));
        }
        if fs::symlink_metadata(old)?.file_type().is_symlink() {
            return Err(io::Error::other("Renaming symbolic links is unsupported."));
        }
        if self.disk_bytes.as_deref() != Some(fs::read(old)?.as_slice()) {
            return Err(io::Error::other(
                "The file changed outside MDcard. Use Save As to keep both versions.",
            ));
        }
        let next = old.parent().unwrap_or(Path::new(".")).join(name);
        validate_type(&next)?;
        if next == *old {
            return Ok(());
        }
        fs::hard_link(old, &next)?;
        if let Err(error) = fs::remove_file(old) {
            let _ = fs::remove_file(&next);
            return Err(error);
        }
        self.path = Some(fs::canonicalize(next)?);
        Ok(())
    }
    /// Change the pending save format only after checking the complete current buffer.
    pub fn set_encoding(&mut self, encoding: Encoding, bom: bool) -> io::Result<()> {
        encoding::encode(&self.text, encoding, bom)?;
        self.encoding = encoding;
        self.bom = bom;
        self.guessed = false;
        Ok(())
    }
    pub fn name(&self, language: crate::localization::Language) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| language.text("Untitled.md").into())
    }
    #[cfg(test)]
    pub fn save_to(&mut self, path: &Path) -> io::Result<()> {
        self.save_guarded(path, || true)
    }
    /// Shared core for manual and cancellable background saves.
    pub fn save_guarded(&mut self, path: &Path, commit: impl FnOnce() -> bool) -> io::Result<()> {
        let _transaction = disk_transaction()?;
        validate_type(path)?;
        let canonical_target = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        if self.path.as_ref() == Some(&canonical_target)
            && let Some(expected) = &self.disk_bytes
        {
            let current = fs::read(path)?;
            if current != *expected {
                return Err(io::Error::other(
                    "The file changed outside MDcard. Use Save As to keep both versions.",
                ));
            }
        }
        // Preserve original bytes for legacy non-bijective mappings on an unchanged save.
        let bytes = if !self.dirty() {
            match &self.disk_bytes {
                Some(bytes) => bytes.clone(),
                None => encoding::encode(&self.text, self.encoding, self.bom)?,
            }
        } else {
            encoding::encode(&self.text, self.encoding, self.bom)?
        };
        let expected = if self.path.as_ref() == Some(&canonical_target) {
            self.disk_bytes.as_deref()
        } else {
            None
        };
        atomic_write_guarded(path, &bytes, expected, commit)?;
        self.path = Some(fs::canonicalize(path)?);
        self.saved.clone_from(&self.text);
        self.disk_bytes = Some(bytes);
        self.saved_encoding = self.encoding;
        self.saved_bom = self.bom;
        self.recovered_unsaved = false;
        self.presentation
            .0
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0 = None;
        Ok(())
    }
}

fn validate_type(path: &Path) -> io::Result<()> {
    if crate::text_files::supported(path) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsupported file type. Choose a supported text file.",
        ))
    }
}

/// Serialize saves with workspace replacement/rollback within this process.
pub struct DiskTransaction {
    _local: std::sync::MutexGuard<'static, ()>,
    _process: ProcessWriteLock,
}
pub fn disk_transaction() -> io::Result<DiskTransaction> {
    static TRANSACTION: Mutex<()> = Mutex::new(());
    let local = TRANSACTION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(DiskTransaction {
        _local: local,
        _process: ProcessWriteLock::acquire()?,
    })
}
// A stable OS lock also covers different MDcard windows/processes, so their
// baseline check + replacement cannot both pass on the same old disk bytes.
#[cfg(windows)]
struct ProcessWriteLock(*mut std::ffi::c_void);
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(
        attributes: *const std::ffi::c_void,
        owner: i32,
        name: *const u16,
    ) -> *mut std::ffi::c_void;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
    fn ReleaseMutex(handle: *mut std::ffi::c_void) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
}
#[cfg(windows)]
impl ProcessWriteLock {
    fn acquire() -> io::Result<Self> {
        let name: Vec<u16> = "Local\\MDcard.DocumentWrites.v1\0".encode_utf16().collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        // Failing closed avoids silently dropping cross-window protection.
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let result = unsafe { WaitForSingleObject(handle, u32::MAX) };
        if result != 0 && result != 0x80 {
            unsafe {
                CloseHandle(handle);
            }
            return Err(io::Error::last_os_error());
        }
        Ok(Self(handle))
    }
}
#[cfg(windows)]
impl Drop for ProcessWriteLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
#[cfg(unix)]
struct ProcessWriteLock(fs::File);
#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> u32;
    fn flock(fd: i32, operation: i32) -> i32;
}
#[cfg(unix)]
impl ProcessWriteLock {
    fn acquire() -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        let path = std::env::temp_dir().join(format!("mdcard-document-writes-{}.lock", unsafe {
            geteuid()
        }));
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        if unsafe { flock(file.as_raw_fd(), 2) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(file))
    }
}
#[cfg(unix)]
impl Drop for ProcessWriteLock {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe {
            flock(self.0.as_raw_fd(), 8);
        }
    }
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_guarded(path, bytes, None, || true)
}
fn atomic_write_guarded(
    path: &Path,
    bytes: &[u8],
    expected: Option<&[u8]>,
    commit: impl FnOnce() -> bool,
) -> io::Result<()> {
    // Replacing symlinks silently would change their semantics. Keep that choice explicit.
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(io::Error::other(
            "Saving through a symbolic link is unsupported. Use Save As.",
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.permissions().readonly() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The file is read-only.",
            ));
        }
        tmp.as_file().set_permissions(metadata.permissions())?;
    }
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    if !commit() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Save cancelled.",
        ));
    }
    // Recheck after staging, immediately before replacement.
    if let Some(expected) = expected
        && fs::read(path)?.as_slice() != expected
    {
        return Err(io::Error::other(
            "The file changed outside MDcard. Use Save As to keep both versions.",
        ));
    }
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(io::Error::other(
            "Saving through a symbolic link is unsupported. Use Save As.",
        ));
    }
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn outline(text: &str) -> Vec<(usize, String)> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut rows = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for event in Parser::new(text.trim_start_matches('\u{feff}')) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level as usize, String::new()))
            }
            Event::Text(s) | Event::Code(s) => {
                if let Some((_, title)) = &mut current {
                    title.push_str(&s);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(row) = current.take() {
                    rows.push(row);
                }
            }
            _ => {}
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_counts_unicode_lines_and_preserves_authoritative_guard() {
        let mut doc = Document::from_text("中文👩🏽‍💻é\r\nline\n");
        assert_eq!(doc.text_statistics(), (2, 15));
        assert!(!doc.presentation_dirty());
        // A caller bypassing the production revision protocol must still be
        // unable to bypass save/close protection with a cached clean display.
        doc.text.replace_range(0..3, "字");
        assert!(doc.dirty());
        doc.mark_text_changed();
        assert!(doc.presentation_dirty());
        assert_eq!(doc.text_statistics(), (2, 15));
        doc.text.clear();
        doc.mark_text_changed();
        assert_eq!(doc.text_statistics(), (0, 0));
    }

    #[test]
    fn presentation_dirty_follows_undo_save_and_pending_format_without_revision_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.txt");
        fs::write(&path, "saved").unwrap();
        let mut doc = Document::open(&path).unwrap();
        assert!(!doc.presentation_dirty());
        doc.text = "edits".into();
        doc.mark_text_changed();
        assert!(doc.presentation_dirty());
        doc.text = "saved".into();
        doc.mark_text_changed();
        assert!(!doc.presentation_dirty());
        doc.text = "edits".into();
        doc.mark_text_changed();
        assert!(doc.presentation_dirty());
        let revision = doc.revision();
        doc.save_to(&path).unwrap();
        assert_eq!(doc.revision(), revision);
        assert!(!doc.presentation_dirty());
        doc.set_encoding(Encoding::Utf16Le, true).unwrap();
        assert_eq!(doc.revision(), revision);
        assert!(doc.presentation_dirty());
        doc.save_to(&path).unwrap();
        assert!(!doc.presentation_dirty());
        assert_eq!(doc.text_statistics(), (1, 5));
        assert_eq!(Document::open(&path).unwrap().text, "edits");
    }

    #[test]
    fn presentation_cache_is_buffer_local_and_committed_baseline_invalidates_it() {
        let mut main = Document::from_text("same");
        assert_eq!(main.text_statistics(), (1, 4));
        assert!(!main.presentation_dirty());
        let mut other = main.clone();
        other.text = "副\n🙂".into();
        other.mark_text_changed();
        assert_eq!(other.text_statistics(), (2, 3));
        assert!(other.presentation_dirty());
        assert_eq!(main.text_statistics(), (1, 4));
        assert!(!main.presentation_dirty());
        main.accept_committed("worker\n".into(), b"worker\n".to_vec());
        assert_eq!(main.text_statistics(), (1, 7));
        assert!(!main.presentation_dirty());
    }
    #[test]
    fn python_codec_golden_files_match_original_and_edited_disk_bytes() {
        macro_rules! fixture {
            ($name:literal) => {
                (
                    include_bytes!(concat!("../tests/fixtures/encodings/", $name, ".txt"))
                        .as_slice(),
                    include_bytes!(concat!(
                        "../tests/fixtures/encodings/",
                        $name,
                        ".edited.txt"
                    ))
                    .as_slice(),
                )
            };
        }
        let manifest: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/fixtures/encodings/manifest.json"))
                .unwrap();
        let dir = tempfile::tempdir().unwrap();
        for case in manifest {
            let name = case["name"].as_str().unwrap();
            let format = match name {
                "utf8" => Encoding::Utf8,
                "utf8-bom" => Encoding::Utf8Bom,
                "utf16le" | "utf16le-no-bom" => Encoding::Utf16Le,
                "utf16be" | "utf16be-no-bom" => Encoding::Utf16Be,
                "gbk" => Encoding::Gbk,
                "gb18030" => Encoding::Gb18030,
                "big5" => Encoding::Big5,
                "shift-jis" => Encoding::ShiftJis,
                "euc-kr" => Encoding::EucKr,
                "windows1251" => Encoding::Windows1251,
                "windows1252" => Encoding::Windows1252,
                _ => unreachable!(),
            };
            let (original, edited) = match name {
                "utf8" => fixture!("utf8"),
                "utf8-bom" => fixture!("utf8-bom"),
                "utf16le" => fixture!("utf16le"),
                "utf16be" => fixture!("utf16be"),
                "utf16le-no-bom" => fixture!("utf16le-no-bom"),
                "utf16be-no-bom" => fixture!("utf16be-no-bom"),
                "gbk" => fixture!("gbk"),
                "gb18030" => fixture!("gb18030"),
                "big5" => fixture!("big5"),
                "shift-jis" => fixture!("shift-jis"),
                "euc-kr" => fixture!("euc-kr"),
                "windows1251" => fixture!("windows1251"),
                "windows1252" => fixture!("windows1252"),
                _ => unreachable!(),
            };
            let path = dir.path().join("golden.txt");
            fs::write(&path, original).unwrap();
            let mut doc = Document::open_with_encoding(&path, Some(format)).unwrap();
            assert_eq!(doc.text, case["text"].as_str().unwrap(), "{name}");
            assert_eq!(doc.bom, case["bom"].as_bool().unwrap(), "{name}");
            doc.save_to(&path).unwrap();
            assert_eq!(fs::read(&path).unwrap(), original, "{name}");
            doc.text.push_str("edit\r\n");
            doc.save_to(&path).unwrap();
            assert_eq!(fs::read(&path).unwrap(), edited, "{name}");
        }
    }
    #[test]
    fn all_encodings_preserve_bytes_then_edit_and_keep_encoding() {
        let dir = tempfile::tempdir().unwrap();
        for (format, text) in [
            (Encoding::Utf8, "中文 🦀"),
            (Encoding::Utf8Bom, "Русский 🦀"),
            (Encoding::Utf16Le, "中文 🦀"),
            (Encoding::Utf16Be, "日本語 🦀"),
            (Encoding::Gbk, "简体中文"),
            (Encoding::Gb18030, "中文𠀀🦀"),
            (Encoding::Big5, "繁體中文"),
            (Encoding::ShiftJis, "日本語テスト"),
            (Encoding::EucKr, "한국어 테스트"),
            (Encoding::Windows1251, "Русский текст"),
            (Encoding::Windows1252, "café € résumé"),
        ] {
            for bom in [false, true] {
                let path = dir.path().join("fixture.txt");
                let bom = bom
                    && matches!(
                        format,
                        Encoding::Utf8Bom | Encoding::Utf16Le | Encoding::Utf16Be
                    );
                let source = format!("{text}\r\nline\nlast\r");
                let bytes = encoding::encode(&source, format, bom).unwrap();
                fs::write(&path, &bytes).unwrap();
                let mut doc = Document::open_with_encoding(&path, Some(format)).unwrap();
                assert_eq!(doc.text, source);
                doc.save_to(&path).unwrap();
                assert_eq!(fs::read(&path).unwrap(), bytes, "unchanged {format:?}");
                doc.text.push_str("edit\r\n");
                doc.save_to(&path).unwrap();
                let reopened = Document::open_with_encoding(&path, Some(format)).unwrap();
                assert_eq!(reopened.text, doc.text);
                assert_eq!(reopened.bom, doc.bom);
            }
        }
    }
    #[test]
    fn detected_gb18030_four_byte_characters_survive_ordinary_edit_and_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gb18030.txt");
        let text = "这是简体中文编码测试文档。支持本地编码，保存和读取文件。𠀀🦀\r\n".repeat(8);
        fs::write(
            &path,
            encoding::encode(&text, Encoding::Gb18030, false).unwrap(),
        )
        .unwrap();
        let mut doc = Document::open(&path).unwrap();
        assert_eq!(doc.encoding, Encoding::Gb18030);
        assert!(doc.guessed);
        assert_eq!(doc.text, text);
        doc.text.push_str("edited\r\n");
        doc.save_to(&path).unwrap();
        assert_eq!(Document::open(&path).unwrap().text, doc.text);
        assert!(
            doc.guessed,
            "saving does not turn detection into a certainty"
        );
    }
    #[test]
    fn duplicate_big5_mapping_keeps_original_bytes_on_unchanged_save_as_well() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big5.txt");
        let bytes = [0xa2, 0xcc, b'\r', b'\n'];
        fs::write(&path, bytes).unwrap();
        let mut doc = Document::open_with_encoding(&path, Some(Encoding::Big5)).unwrap();
        doc.save_to(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let copy = dir.path().join("copy.txt");
        doc.save_to(&copy).unwrap();
        assert_eq!(fs::read(copy).unwrap(), bytes);
    }
    #[test]
    fn conversion_is_pending_dirty_atomic_and_fails_without_losing_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conversion.md");
        fs::write(&path, "中文 🦀\r\n").unwrap();
        let original = fs::read(&path).unwrap();
        let mut doc = Document::open(&path).unwrap();
        assert!(doc.set_encoding(Encoding::Windows1252, false).is_err());
        assert_eq!(doc.encoding, Encoding::Utf8);
        assert!(!doc.dirty());
        doc.set_encoding(Encoding::Utf16Be, true).unwrap();
        assert!(doc.dirty());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(doc.save_to(&dir.path().join("missing/out.md")).is_err());
        assert!(doc.dirty());
        doc.save_to(&path).unwrap();
        assert!(!doc.dirty());
        assert_eq!(&fs::read(&path).unwrap()[..2], &[0xfe, 0xff]);
        doc.text.push('🦀');
        fs::write(&path, "external change").unwrap();
        assert!(doc.save_to(&path).is_err());
        assert!(doc.dirty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external change");
        assert!(Document::open_with_encoding(&path, Some(Encoding::Utf16Be)).is_err());
        assert!(doc.text.ends_with('🦀'));
    }
    #[test]
    fn utf8_bom_crlf_roundtrip_and_dirty_undo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("中文.md");
        let original = "\u{feff}# 你好\r\n\r\nRust **works** 🦀\r\n";
        fs::write(&path, original).unwrap();
        let mut doc = Document::open(&path).unwrap();
        assert_eq!(doc.text, original.trim_start_matches('\u{feff}'));
        assert!(!doc.dirty());
        doc.text.push_str("新增段落\r\n");
        assert!(doc.dirty());
        doc.save_to(&path).unwrap();
        assert!(!doc.dirty());
        assert_eq!(Document::open(&path).unwrap().text, doc.text);
        let saved = doc.text.clone();
        doc.text.push('x');
        doc.text = saved;
        assert!(!doc.dirty());
    }
    #[test]
    fn six_languages_roundtrip_without_changing_bom_crlf_or_text() {
        let dir = tempfile::tempdir().unwrap();
        let text = "\u{feff}# 简体中文 繁體中文 English 한국어 日本語 Русский 🦀\r\n\r\n- [x] 保存 儲存 Save 저장 保存 Сохранить\r\n";
        for language in crate::localization::Language::ALL {
            let path = dir.path().join(language.text("Untitled.md"));
            std::fs::write(&path, text).unwrap();
            let mut doc = Document::open(&path).unwrap();
            assert_eq!(doc.text, text.trim_start_matches('\u{feff}'));
            assert_eq!(doc.name(language), language.text("Untitled.md"));
            doc.text.push_str("追加 추가 добавлено\r\n");
            doc.save_to(&path).unwrap();
            assert_eq!(
                std::fs::read(&path).unwrap(),
                [b"\xef\xbb\xbf".as_slice(), doc.text.as_bytes()].concat()
            );
            assert_eq!(Document::open(&path).unwrap().text, doc.text);
            assert!(!doc.dirty());
        }
    }
    #[test]
    fn failed_save_keeps_path_dirty_and_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("safe.md");
        fs::write(&path, "original").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.text = "edited".into();
        assert!(doc.save_to(&dir.path().join("missing/file.md")).is_err());
        assert!(doc.dirty());
        assert_eq!(doc.path.as_ref(), Some(&fs::canonicalize(&path).unwrap()));
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    }
    #[test]
    fn external_change_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "one").unwrap();
        let mut doc = Document::open(&path).unwrap();
        doc.text = "two".into();
        fs::write(&path, "external").unwrap();
        assert!(doc.save_to(&path).is_err());
        assert!(doc.dirty());
        assert_eq!(fs::read_to_string(path).unwrap(), "external");
    }
    #[test]
    fn invalid_utf8_is_rejected_without_lossy_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.md");
        fs::write(&path, [0xff, 0xfe, 0]).unwrap();
        assert_eq!(
            Document::open(&path).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn mainstream_text_roundtrips_and_binary_and_size_errors_keep_original() {
        let dir = tempfile::tempdir().unwrap();
        let original = "\u{feff}# literal **text** <script>\r\n中文 日本語 🦀\r\n";
        for name in [
            "note.TXT",
            "config.json",
            "data.csv",
            "main.rs",
            "README",
            ".env",
        ] {
            let path = dir.path().join(name);
            fs::write(&path, original).unwrap();
            let mut doc = Document::open(&path).unwrap();
            doc.text.push_str("edit\r\n");
            doc.save_to(&path).unwrap();
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                format!("\u{feff}{}", doc.text)
            );
        }
        let path = dir.path().join("binary.txt");
        fs::write(&path, b"valid UTF8 but\0binary").unwrap();
        assert!(Document::open(&path).is_err());
        fs::write(&path, [0xff]).unwrap();
        assert!(Document::open_with_encoding(&path, Some(Encoding::Utf8)).is_err());
        let file = fs::File::create(&path).unwrap();
        file.set_len(crate::text_files::MAX_FILE_BYTES + 1).unwrap();
        assert!(
            Document::open(&path)
                .err()
                .unwrap()
                .to_string()
                .contains("16 MiB")
        );
        assert!(Document::open(&dir.path().join("bad.exe")).is_err());
    }
    #[test]
    fn commonmark_outline_ignores_code_and_supports_setext() {
        assert_eq!(
            outline("# **你好**\n\n```md\n# hidden\n```\n\nChapter\n-------"),
            vec![(1, "你好".into()), (2, "Chapter".into())]
        );
    }
    #[test]
    fn gfm_fixture_contains_real_structures() {
        use pulldown_cmark::{Event, Options, Parser, Tag};
        let events: Vec<_> =
            Parser::new_ext(include_str!("../examples/Welcome.md"), Options::all()).collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Start(Tag::Table(_))))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::TaskListMarker(true)))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Start(Tag::CodeBlock(_))))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Start(Tag::Link { .. })))
        );
    }
}
