// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Same interval for baseline and candidate: first App::update statement to
//! UpdateGuard::drop's first elapsed expression. Its own queue/publisher is outside.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};
#[derive(Clone, serde::Serialize)]
struct Sample {
    frame: u64,
    pass: u64,
    wheel_input: bool,
    elapsed_wall_ms: f64,
    observed_monotonic_ns: u128,
}
static START: OnceLock<Instant> = OnceLock::new();
static SENDER: OnceLock<Option<Observer>> = OnceLock::new();
type Outcome = Result<(), String>;
struct Counts {
    submitted: AtomicU64,
    dropped: AtomicU64,
    stopping: AtomicBool,
    final_result: Mutex<Option<Outcome>>,
}
enum Message {
    Sample(Sample),
    Finish {
        ack: mpsc::SyncSender<Outcome>,
        retry_until: Instant,
    },
}
struct Observer {
    tx: mpsc::SyncSender<Message>,
    counts: Arc<Counts>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}
impl Observer {
    fn new(path: std::path::PathBuf) -> Self {
        Self::spawn(path, 4096, 8192, Duration::from_millis(100), atomic_publish)
    }
    fn spawn(
        path: std::path::PathBuf,
        capacity: usize,
        retained_limit: usize,
        interval: Duration,
        publish: impl FnMut(&std::path::Path, &[u8]) -> std::io::Result<()> + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::sync_channel(capacity);
        let counts = Arc::new(Counts {
            submitted: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            stopping: AtomicBool::new(false),
            final_result: Mutex::new(None),
        });
        let worker_counts = counts.clone();
        let thread = std::thread::spawn(move || {
            writer_loop(rx, path, worker_counts, retained_limit, interval, publish)
        });
        Self {
            tx,
            counts,
            thread: Mutex::new(Some(thread)),
        }
    }
    fn submit(&self, sample: Sample) {
        self.counts.submitted.fetch_add(1, Ordering::Relaxed);
        if self.counts.stopping.load(Ordering::Acquire)
            || self.tx.try_send(Message::Sample(sample)).is_err()
        {
            self.counts.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn finish(&self, timeout: Duration) -> Outcome {
        let deadline = Instant::now() + timeout;
        let mut thread = self
            .thread
            .lock()
            .map_err(|_| "Observer join lock poisoned")?;
        if thread.as_ref().is_some_and(|h| h.is_finished()) {
            if thread.take().unwrap().join().is_err() {
                return Err("Observer worker panicked".into());
            }
            return self
                .counts
                .final_result
                .lock()
                .map_err(|_| "Observer result lock poisoned")?
                .clone()
                .unwrap_or_else(|| Err("Observer exited without final acknowledgment".into()));
        }
        if thread.is_none() {
            return self
                .counts
                .final_result
                .lock()
                .map_err(|_| "Observer result lock poisoned")?
                .clone()
                .unwrap_or_else(|| Err("Observer has no final acknowledgment".into()));
        }
        self.counts.stopping.store(true, Ordering::Release);
        let (ack, received) = mpsc::sync_channel(1);
        let mut message = Message::Finish {
            ack,
            retry_until: deadline
                .checked_sub(Duration::from_millis(10))
                .unwrap_or(deadline),
        };
        loop {
            match self.tx.try_send(message) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) => message = returned,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("Observer disconnected before final acknowledgment".into());
                }
            }
            if Instant::now() >= deadline {
                return Err("Observer finish timed out enqueueing barrier".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let outcome = received
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|e| format!("Observer final acknowledgment failed: {e}"))?;
        while !thread.as_ref().unwrap().is_finished() {
            if Instant::now() >= deadline {
                return Err("Observer finish timed out waiting for worker exit".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        thread
            .take()
            .unwrap()
            .join()
            .map_err(|_| "Observer worker panicked".to_owned())?;
        outcome
    }
}
fn atomic_publish(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(m) => {
            if !m.is_file() || m.file_type().is_symlink() || m.permissions().readonly() {
                return Err(std::io::Error::other(
                    "Observer path is not writable regular file",
                ));
            }
            Some(m)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    if let Some(metadata) = metadata {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.write_all(bytes)?;
    // Atomic diagnostic completeness, not power-loss durability.
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}
fn writer_loop(
    rx: mpsc::Receiver<Message>,
    path: std::path::PathBuf,
    counts: Arc<Counts>,
    retained_limit: usize,
    interval: Duration,
    mut publish: impl FnMut(&std::path::Path, &[u8]) -> std::io::Result<()>,
) {
    let mut samples = VecDeque::new();
    let mut total = 0u64;
    let mut publication_failures = 0u64;
    let mut last_publication_error = None::<String>;
    loop {
        let first = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(m) => m,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        let mut finish = None;
        let mut message = Some(first);
        while let Some(next) = message {
            match next {
                Message::Sample(sample) => {
                    samples.push_back(sample);
                    total += 1;
                }
                Message::Finish { ack, retry_until } => {
                    finish = Some((ack, retry_until));
                    break;
                }
            }
            message = rx.try_recv().ok();
        }
        while samples.len() > retained_limit {
            samples.pop_front();
        }
        let mut outcome;
        loop {
            let submitted = counts.submitted.load(Ordering::Relaxed);
            let dropped = counts.dropped.load(Ordering::Relaxed);
            let complete = finish.is_some()
                && submitted == total
                && dropped == 0
                && total == samples.len() as u64;
            let report = serde_json::json!({"scope":"App::update first-statement to final-local-guard drop; function wall, existing diagnostics included; own observer enqueue/JSON writer excluded","unit":"milliseconds","total_samples":total,"processed_samples":total,"submitted_samples":submitted,"queue_dropped_samples":dropped,"retention_dropped_samples":total.saturating_sub(samples.len()as u64),"retained_limit":retained_limit,"flush_complete":complete,"publication_failures":publication_failures,"last_publication_error":last_publication_error,"samples":samples});
            outcome = serde_json::to_vec(&report)
                .map_err(std::io::Error::other)
                .and_then(|bytes| publish(&path, &bytes))
                .map_err(|error| error.to_string());
            if let Err(error) = &outcome {
                publication_failures += 1;
                last_publication_error = Some(error.clone());
            } else if finish.is_some() && !complete {
                outcome = Err(format!(
                    "Observer incomplete: submitted {submitted}, processed {total}, queue dropped {dropped}, retained {}",
                    samples.len()
                ));
                break;
            }
            if outcome.is_ok()
                || finish
                    .as_ref()
                    .is_none_or(|(_, deadline)| Instant::now() >= *deadline)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if let Some((ack, _)) = finish {
            if let Ok(mut status) = counts.final_result.lock() {
                *status = Some(outcome.clone());
            }
            let _ = ack.send(outcome);
            return;
        }
        std::thread::sleep(interval);
    }
}
fn sender() -> Option<&'static Observer> {
    SENDER
        .get_or_init(|| {
            std::env::var_os("MDCARD_FULL_UPDATE_REPORT").map(|path| Observer::new(path.into()))
        })
        .as_ref()
}
/// Only after run_native returns: outside every measured update.
pub fn finish() -> Outcome {
    SENDER
        .get()
        .and_then(Option::as_ref)
        .map_or(Ok(()), |observer| observer.finish(Duration::from_secs(3)))
}

pub struct UpdateGuard {
    started: Instant,
    enabled: bool,
    frame: u64,
    pass: u64,
    wheel_input: bool,
}
impl UpdateGuard {
    pub fn start() -> Self {
        let started = Instant::now();
        START.get_or_init(Instant::now);
        Self { started, enabled: sender().is_some(), frame: 0, pass: 0, wheel_input: false }
    }
    pub fn tag(&mut self, frame: u64, pass: u64, wheel_input: bool) {
        self.frame = frame;
        self.pass = pass;
        self.wheel_input = wheel_input;
    }
}
impl Drop for UpdateGuard {
    fn drop(&mut self) {
        let elapsed_wall_ms = self.started.elapsed().as_secs_f64() * 1000.;
        if !self.enabled {
            return;
        }
        let sample = Sample {
            frame: self.frame,
            pass: self.pass,
            wheel_input: self.wheel_input,
            elapsed_wall_ms,
            observed_monotonic_ns: START.get().unwrap().elapsed().as_nanos(),
        };
        sender().unwrap().submit(sample);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    fn sample(frame: u64) -> Sample {
        Sample {
            frame,
            pass: frame,
            wheel_input: frame.is_multiple_of(2),
            elapsed_wall_ms: 0.25,
            observed_monotonic_ns: 0,
        }
    }
    #[test]
    fn final_fifo_barrier_includes_last_sample_and_exact_counts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let observer = Observer::new(path.clone());
        for i in 0..300 {
            observer.submit(sample(i));
        }
        observer.finish(Duration::from_secs(3)).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(v["flush_complete"], true);
        assert_eq!(v["submitted_samples"], 300);
        assert_eq!(v["processed_samples"], 300);
        assert_eq!(v["total_samples"], 300);
        assert_eq!(v["samples"].as_array().unwrap().len(), 300);
        assert_eq!(v["samples"][299]["frame"], 299);
        assert_eq!(v["queue_dropped_samples"], 0);
        observer.finish(Duration::from_secs(1)).unwrap();
    }
    #[test]
    fn readonly_failure_preserves_previous_json_and_final_ack_reports_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let old = "{\"text\":\"中文 👩🏽‍💻 e\u{301}\"}".as_bytes();
        atomic_publish(&path, old).unwrap();
        let permissions = std::fs::metadata(&path).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        std::fs::set_permissions(&path, readonly).unwrap();
        assert!(atomic_publish(&path, b"{\"new\":true}").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        let observer = Observer::new(path.clone());
        observer.submit(sample(7));
        assert!(observer.finish(Duration::from_millis(200)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        std::fs::set_permissions(path, permissions).unwrap();
    }
    #[test]
    fn timeout_and_full_queue_are_explicit_without_truncating_previous_json() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        atomic_publish(&path, b"{\"old\":true}").unwrap();
        let (entered, started) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        let mut first = true;
        let observer = Observer::spawn(path.clone(), 1, 8192, Duration::ZERO, move |p, bytes| {
            if first {
                first = false;
                entered.send(()).unwrap();
                wait.recv().unwrap();
            }
            atomic_publish(p, bytes)
        });
        observer.submit(sample(1));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        for i in 2..8 {
            observer.submit(sample(i));
        }
        assert!(observer.finish(Duration::from_millis(30)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"old\":true}");
        release.send(()).unwrap();
        assert!(observer.finish(Duration::from_secs(3)).is_err());
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(v["flush_complete"], false);
        assert!(v["queue_dropped_samples"].as_u64().unwrap() > 0);
        assert_eq!(v["submitted_samples"], 7);
    }
    #[test]
    fn concurrent_reader_only_observes_complete_old_or_new_json() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let old = serde_json::to_vec(&serde_json::json!({"old":"中文".repeat(300)})).unwrap();
        let new = serde_json::to_vec(&serde_json::json!({"new":"👩🏽‍💻".repeat(400)})).unwrap();
        atomic_publish(&path, &old).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let p = path.clone();
        let a = old.clone();
        let b = new.clone();
        let ready = Arc::new(std::sync::Barrier::new(2));
        let reader_ready = ready.clone();
        let reader = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut reads = 0;
            reader_ready.wait();
            while (!stopping.load(Ordering::Relaxed) || reads < 100) && Instant::now() < deadline {
                if let Ok(bytes) = std::fs::read(&p) {
                    assert!(bytes == a || bytes == b);
                    let _: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    reads += 1;
                }
            }
            reads
        });
        ready.wait();
        let mut successes = 0;
        for i in 0..100 {
            if atomic_publish(&path, if i % 2 == 0 { &old } else { &new }).is_ok() {
                successes += 1;
            }
        }
        stop.store(true, Ordering::Relaxed);
        assert!(reader.join().unwrap() >= 100);
        assert!(successes > 0);
        atomic_publish(&path, &new).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), new);
    }
    #[test]
    fn retained_window_is_never_labeled_complete_raw_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let observer = Observer::spawn(path.clone(), 4096, 2, Duration::ZERO, atomic_publish);
        for i in 0..4 {
            observer.submit(sample(i));
        }
        assert!(observer.finish(Duration::from_secs(3)).is_err());
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["queue_dropped_samples"], 0);
        assert_eq!(value["retention_dropped_samples"], 2);
        assert_eq!(value["total_samples"], 4);
        assert_eq!(value["flush_complete"], false);
    }
    #[test]
    fn queued_finish_ack_timeout_is_bounded_and_worker_can_exit_after_release() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        atomic_publish(&path, b"{\"old\":true}").unwrap();
        let (entered, started) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        let observer = Arc::new(Observer::spawn(
            path.clone(),
            4096,
            8192,
            Duration::ZERO,
            move |p, bytes| {
                let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                if value["flush_complete"] == true {
                    entered.send(()).unwrap();
                    wait.recv().unwrap();
                }
                atomic_publish(p, bytes)
            },
        ));
        observer.submit(sample(11));
        let finishing = observer.clone();
        let caller = std::thread::spawn(move || finishing.finish(Duration::from_millis(60)));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(caller.join().unwrap().is_err());
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if observer
                .thread
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_finished()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        observer.finish(Duration::from_secs(1)).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(value["flush_complete"], true);
        assert_eq!(value["samples"][0]["frame"], 11);
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn symbolic_link_cannot_replace_or_modify_the_old_report() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("old.json");
        let link = directory.path().join("link.json");
        atomic_publish(&target, b"{\"old\":true}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(atomic_publish(&link, b"bad").is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"{\"old\":true}");
    }
    #[cfg(windows)]
    #[test]
    fn actual_windows_replace_lock_preserves_previous_snapshot() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        atomic_publish(&path, b"{\"old\":true}").unwrap();
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        assert!(atomic_publish(&path, b"{\"new\":true}").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{\"old\":true}");
        drop(handle);
        atomic_publish(&path, b"{\"new\":true}").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"{\"new\":true}");
    }
}
