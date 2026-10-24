// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Bounded synchronous diagnostic output. This never writes manuscript or reading state.
use std::{io, path::Path};

#[derive(Clone, Copy)]
pub enum Kind {
    Reading,
    Ui,
}

#[derive(Default)]
pub struct DiagnosticSinks {
    #[cfg(target_os = "linux")]
    slots: [Option<linux::Handle>; 2],
}

impl DiagnosticSinks {
    pub fn write(&mut self, kind: Kind, path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            let slot = match kind {
                Kind::Reading => &mut self.slots[0],
                Kind::Ui => &mut self.slots[1],
            };
            // On any error the local handle is dropped, so the next call must reopen.
            let handle = linux::publish(slot.take(), path.as_ref(), bytes)?;
            *slot = Some(handle);
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = kind;
            std::fs::write(path, bytes)
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        fs::{self, File, Metadata, OpenOptions},
        io::{self, Seek, SeekFrom, Write},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
        path::{Path, PathBuf},
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Identity {
        device: u64,
        inode: u64,
    }
    fn identity(metadata: &Metadata) -> Identity {
        Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    fn validate(metadata: &Metadata) -> io::Result<()> {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::other(
                "Diagnostic path must be a regular non-symlink file",
            ));
        }
        if metadata.permissions().readonly() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Diagnostic path is readonly",
            ));
        }
        Ok(())
    }

    pub(super) struct Handle {
        path: PathBuf,
        file: File,
        identity: Identity,
    }

    pub(super) fn publish(
        previous: Option<Handle>,
        path: &Path,
        bytes: &[u8],
    ) -> io::Result<Handle> {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                validate(&metadata)?;
                Some(metadata)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let reusable = previous.as_ref().is_some_and(|handle| {
            handle.path == path
                && metadata
                    .as_ref()
                    .is_some_and(|metadata| identity(metadata) == handle.identity)
        });
        let mut handle = if reusable {
            previous.unwrap()
        } else {
            drop(previous);
            // NONBLOCK prevents a precheck/open race with a FIFO from blocking.
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&path)?;
            let opened = file.metadata()?;
            validate(&opened)?;
            let opened_identity = identity(&opened);
            if metadata
                .as_ref()
                .is_some_and(|metadata| identity(metadata) != opened_identity)
            {
                return Err(io::Error::other("Diagnostic path changed while reopening"));
            }
            Handle {
                path: path.clone(),
                file,
                identity: opened_identity,
            }
        };
        handle.file.seek(SeekFrom::Start(0))?;
        handle.file.write_all(bytes)?;
        if metadata
            .as_ref()
            .is_some_and(|metadata| metadata.len() > bytes.len() as u64)
        {
            handle.file.set_len(bytes.len() as u64)?;
        }
        let after = fs::symlink_metadata(&path)?;
        validate(&after)?;
        if identity(&after) != handle.identity || after.len() != bytes.len() as u64 {
            return Err(io::Error::other(
                "Diagnostic path changed during publication",
            ));
        }
        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn complete_unicode_json_can_shrink_and_change_path() {
        let directory = tempfile::tempdir().unwrap();
        let a = directory.path().join("a.json");
        let b = directory.path().join("b.json");
        let mut sinks = DiagnosticSinks::default();
        let long = serde_json::to_vec(
            &serde_json::json!({"text":"中文 👩🏽‍💻 e\u{301}","long":"x".repeat(512)}),
        )
        .unwrap();
        let short = br#"{"x":1}"#;
        sinks.write(Kind::Reading, &a, &long).unwrap();
        sinks.write(Kind::Reading, &a, short).unwrap();
        assert_eq!(fs::read(&a).unwrap(), short);
        sinks.write(Kind::Reading, &b, &long).unwrap();
        assert_eq!(fs::read(&a).unwrap(), short);
        assert_eq!(fs::read(&b).unwrap(), long);
        sinks.write(Kind::Ui, &a, short).unwrap();
        #[cfg(target_os = "linux")]
        assert_eq!(sinks.slots.iter().filter(|slot| slot.is_some()).count(), 2);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn external_delete_replace_and_readonly_invalidate_the_actual_inode() {
        use std::{io::Read, os::unix::fs::PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let replacement = directory.path().join("replacement.json");
        let mut sinks = DiagnosticSinks::default();
        sinks.write(Kind::Reading, &path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        sinks.write(Kind::Reading, &path, b"old").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
        assert!(sinks.write(Kind::Reading, &path, b"changed").is_err());
        assert!(sinks.slots[0].is_none());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        sinks.write(Kind::Reading, &path, b"old").unwrap();
        let mut old_inode = fs::File::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        sinks.write(Kind::Reading, &path, b"created").unwrap();
        let mut bytes = Vec::new();
        old_inode.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"old");
        let mut old_inode = fs::File::open(&path).unwrap();
        fs::write(&replacement, b"replacement").unwrap();
        fs::rename(&replacement, &path).unwrap();
        sinks.write(Kind::Reading, &path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        let mut bytes = Vec::new();
        old_inode.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"created");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn symlink_fifo_and_failed_open_close_the_slot_and_allow_recovery() {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let link = directory.path().join("link.json");
        let fifo = directory.path().join("fifo");
        let mut sinks = DiagnosticSinks::default();
        sinks.write(Kind::Reading, &path, b"old").unwrap();
        symlink(&path, &link).unwrap();
        assert!(sinks.write(Kind::Reading, &link, b"bad").is_err());
        assert!(sinks.slots[0].is_none());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let started = std::time::Instant::now();
        assert!(sinks.write(Kind::Reading, &fifo, b"bad").is_err());
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
        assert!(sinks.slots[0].is_none());
        assert!(
            sinks
                .write(Kind::Reading, directory.path().join("absent/a"), b"bad")
                .is_err()
        );
        assert!(sinks.slots[0].is_none());
        sinks.write(Kind::Reading, &path, b"recovered").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"recovered");
    }
}
