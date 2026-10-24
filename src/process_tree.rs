// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
//! Linux export-task cancellation. Signals target stable pidfds; completion means
//! every discovered process has exited (zombies are terminal, not live workers).
use std::{
    collections::HashMap,
    fs, io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
struct Identity {
    pid: u32,
    parent: u32,
    group: u32,
    start: u64,
    state: char,
}
fn identity(pid: u32) -> io::Result<Identity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let (_, fields) = stat
        .rsplit_once(") ")
        .ok_or_else(|| io::Error::other("Invalid process identity"))?;
    let fields: Vec<_> = fields.split_whitespace().collect();
    Ok(Identity {
        pid,
        parent: fields
            .get(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("Invalid process parent"))?,
        group: fields
            .get(2)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("Invalid process group"))?,
        start: fields
            .get(19)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other("Invalid process start time"))?,
        state: fields
            .first()
            .and_then(|s| s.chars().next())
            .ok_or_else(|| io::Error::other("Invalid process state"))?,
    })
}
struct Process {
    id: Identity,
    fd: OwnedFd,
}
impl Process {
    fn open(id: Identity) -> io::Result<Self> {
        // SAFETY: pidfd_open takes a numeric PID and zero flags; the returned FD
        // is owned exactly once and closed by OwnedFd.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, id.pid, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        if identity(id.pid)?.start != id.start {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Process identity changed before capture",
            ));
        }
        Ok(Self { id, fd })
    }
    fn exited(&self) -> io::Result<bool> {
        let mut poll = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll points to one initialized pollfd, with a nonblocking timeout.
        let result = unsafe { libc::poll(&mut poll, 1, 0) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(result > 0 && poll.revents & (libc::POLLIN | libc::POLLHUP) != 0)
    }
    fn signal(&self, signal: i32) -> io::Result<()> {
        // SAFETY: the stable pidfd is live; null siginfo requests an ordinary
        // signal. PID reuse cannot redirect this operation to a different task.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.fd.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        Ok(())
    }
    fn stopped_or_exited(&self) -> io::Result<bool> {
        if self.exited()? {
            return Ok(true);
        }
        let now = match identity(self.id.pid) {
            Ok(now) => now,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return self.exited(),
            Err(e) => return Err(e),
        };
        Ok(now.start == self.id.start && matches!(now.state, 'T' | 't' | 'Z' | 'X'))
    }
}
pub fn ensure_available() -> io::Result<()> {
    let process = Process::open(identity(std::process::id())?)?;
    process.signal(0)
}
#[cfg(test)]
pub fn cancel(pid: u32, timeout: Duration) -> io::Result<()> {
    cancel_observing(pid, timeout, Process::exited)
}
#[cfg(test)]
fn cancel_observing(
    pid: u32,
    timeout: Duration,
    observe_exit: impl Fn(&Process) -> io::Result<bool>,
) -> io::Result<()> {
    let root = match identity(pid) {
        Ok(id) => id,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    cancel_identities(vec![root], timeout, observe_exit)
}
/// Captured immediately after spawning with process_group(0). The leader's
/// identity survives reaping, allowing cleanup of children holding inherited
/// streams after the leader has naturally exited.
pub struct OwnedGroup {
    leader: Identity,
}
impl OwnedGroup {
    pub fn capture(pid: u32) -> io::Result<Self> {
        let leader = identity(pid)?;
        if leader.group != pid {
            return Err(io::Error::other(
                "Export process does not own its process group",
            ));
        }
        Ok(Self { leader })
    }
    pub fn cancel(&self, timeout: Duration) -> io::Result<()> {
        if let Ok(current) = identity(self.leader.pid)
            && current.start != self.leader.start
        {
            return Err(io::Error::other(
                "Export group leader identity changed; refusing to signal a reused PID",
            ));
        }
        let mut roots = Vec::new();
        for entry in fs::read_dir("/proc")?.flatten() {
            if let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse().ok())
                .and_then(|pid| identity(pid).ok())
                && id.group == self.leader.pid
                && id.start >= self.leader.start
            {
                roots.push(id);
            }
        }
        cancel_identities(roots, timeout, Process::exited)
    }
}
fn cancel_identities(
    roots: Vec<Identity>,
    timeout: Duration,
    observe_exit: impl Fn(&Process) -> io::Result<bool>,
) -> io::Result<()> {
    let mut owned = Vec::new();
    for root in roots {
        match Process::open(root) {
            Ok(process) => owned.push(process),
            Err(e)
                if e.kind() == io::ErrorKind::NotFound || e.raw_os_error() == Some(libc::ESRCH) => {
            }
            Err(e) => return Err(e),
        }
    }
    let deadline = Instant::now() + timeout;
    let capture: io::Result<()> = (|| {
        loop {
            for process in &owned {
                process.signal(libc::SIGSTOP)?;
            }
            loop {
                if owned
                    .iter()
                    .map(Process::stopped_or_exited)
                    .collect::<io::Result<Vec<_>>>()?
                    .into_iter()
                    .all(|stopped| stopped)
                {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Export processes did not stop before the cleanup deadline",
                    ));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            // Frozen parents cannot create new children while this snapshot is
            // traversed. Repeat until newly discovered descendants are frozen too.
            let mut known: HashMap<_, _> = owned.iter().map(|p| (p.id.pid, p.id.start)).collect();
            let edges: Vec<_> = fs::read_dir("/proc")?
                .flatten()
                .filter_map(|e| {
                    e.file_name()
                        .to_str()?
                        .parse()
                        .ok()
                        .and_then(|pid| identity(pid).ok())
                })
                .collect();
            let before = owned.len();
            loop {
                let count = owned.len();
                for id in &edges {
                    if !known.contains_key(&id.pid)
                        && known.get(&id.parent).is_some_and(|start| {
                            identity(id.parent).is_ok_and(|parent| parent.start == *start)
                        })
                    {
                        match Process::open(*id) {
                            Ok(process) => {
                                known.insert(id.pid, id.start);
                                owned.push(process);
                            }
                            Err(e)
                                if e.kind() == io::ErrorKind::NotFound
                                    || e.raw_os_error() == Some(libc::ESRCH) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
                if count == owned.len() {
                    break;
                }
            }
            if before == owned.len() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Export descendants could not be frozen before cleanup",
                ));
            }
        }
        Ok(())
    })();
    // Even capture failure must not leave already frozen tasks suspended forever.
    let mut failure = capture.err();
    for process in owned.iter().rev() {
        if let Err(e) = process.signal(libc::SIGKILL) {
            failure.get_or_insert(e);
        }
    }
    let exit_deadline = Instant::now() + timeout;
    loop {
        if owned
            .iter()
            .map(&observe_exit)
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .all(|exited| exited)
        {
            break;
        }
        if Instant::now() >= exit_deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Export cleanup did not confirm every process exit",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        process::{Command, Stdio},
    };
    #[test]
    fn cancel_multilevel_detached_tree_waits_for_terminal_and_supervisor_reaps_it() {
        ensure_available().unwrap();
        let script = r#"
import ctypes,subprocess,sys,time,os
assert ctypes.CDLL(None).prctl(36,1,0,0,0)==0
leaf="import os,subprocess,time; p=subprocess.Popen(['/usr/bin/setsid','/bin/sleep','30']); print(str(os.getpid())+' '+str(p.pid),flush=True); time.sleep(30)"
root_code="import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-u','-c',"+repr(leaf)+"],stdout=subprocess.PIPE,start_new_session=True); print(p.stdout.readline().decode().strip(),flush=True); time.sleep(30)"
root=subprocess.Popen([sys.executable,'-u','-c',root_code],stdout=subprocess.PIPE,start_new_session=True)
descendants=list(map(int,root.stdout.readline().split()))
print('TASK '+str(root.pid)+' '+' '.join(map(str,descendants)),flush=True)
assert root.wait()==-9
for pid in descendants:
    waited,status=os.waitpid(pid,0)
    assert waited==pid and os.WIFSIGNALED(status) and os.WTERMSIG(status)==9
print('REAPED',flush=True)
"#;
        let mut supervisor = Command::new("/usr/bin/python3")
            .args(["-u", "-c", script])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(supervisor.stdout.take().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("TASK "));
        let pids: Vec<u32> = line
            .split_whitespace()
            .skip(1)
            .map(|s| s.parse().unwrap())
            .collect();
        let mut unrelated = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let result = cancel(pids[0], Duration::from_secs(2));
        assert!(unrelated.try_wait().unwrap().is_none());
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
        result.unwrap();
        for pid in pids {
            assert!(identity(pid).map_or(true, |id| matches!(id.state, 'Z' | 'X')));
        }
        let mut rest = String::new();
        reader.read_to_string(&mut rest).unwrap();
        assert!(rest.contains("REAPED"));
        assert!(supervisor.wait().unwrap().success());
    }
    #[test]
    fn captured_group_survives_reaped_leader_and_closes_inherited_pipes() {
        let script = r#"
import ctypes,subprocess,sys,os
assert ctypes.CDLL(None).prctl(36,1,0,0,0)==0
root=subprocess.Popen(['/bin/sh','-c','/bin/sleep 30 & echo $!; exit 0'],stdout=subprocess.PIPE,start_new_session=True)
leaf=int(root.stdout.readline())
print(str(root.pid)+' '+str(leaf),flush=True)
sys.stdin.readline()
assert root.wait()==0
print('LEADER_EXITED',flush=True)
waited,status=os.waitpid(leaf,0)
assert waited==leaf and os.WIFSIGNALED(status) and os.WTERMSIG(status)==9
assert root.stdout.read()==b''
print('REAPED_AND_CLOSED',flush=True)
"#;
        let mut supervisor = Command::new("/usr/bin/python3")
            .args(["-u", "-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(supervisor.stdout.take().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let pids: Vec<u32> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let group = OwnedGroup::capture(pids[0]).unwrap();
        supervisor
            .stdin
            .take()
            .unwrap()
            .write_all(b"captured\n")
            .unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "LEADER_EXITED");
        assert!(identity(pids[0]).is_err());
        group.cancel(Duration::from_secs(2)).unwrap();
        let mut rest = String::new();
        reader.read_to_string(&mut rest).unwrap();
        assert!(rest.contains("REAPED_AND_CLOSED"));
        assert!(supervisor.wait().unwrap().success());
    }
    #[test]
    fn termination_timeout_is_an_error_even_if_signal_was_sent() {
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        // Fault-inject only unavailable exit acknowledgement, using a real killed
        // process. Never turn an unconfirmed terminal state into a successful cancel.
        let result = cancel_observing(child.id(), Duration::from_millis(20), |_| Ok(false));
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(!child.wait().unwrap().success());
    }
    #[test]
    fn already_reaped_root_cancellation_is_idempotent() {
        let mut child = Command::new("/bin/true").spawn().unwrap();
        let pid = child.id();
        assert!(child.wait().unwrap().success());
        cancel(pid, Duration::from_millis(100)).unwrap();
    }
}
