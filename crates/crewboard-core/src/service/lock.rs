//! The service lock file (`service.lock` in the data folder). A running service holds an exclusive lock on it
//! and writes its PID into it, so only one service runs per data folder.
//!
//! The OS drops the lock when the process ends, even after a crash, so a leftover file from a crash is simply
//! locked again by the next service ("stale lock replaced"). A clean stop removes the file.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ServiceLock {
    file: File,
    path: PathBuf,
}

#[derive(Debug)]
pub enum LockError {
    /// Another service holds the lock.
    Held {
        pid: Option<u32>,
    },
    Io(io::Error),
}

impl ServiceLock {
    /// Takes the lock and writes this process's PID into the file.
    pub fn acquire(path: &Path) -> Result<Self, LockError> {
        loop {
            let mut options = OpenOptions::new();
            options.read(true).write(true).create(true).truncate(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(path).map_err(LockError::Io)?;
            match try_lock(&file) {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => {
                    return Err(LockError::Held {
                        pid: read_pid(path),
                    });
                }
                Err(TryLockError::Error(e)) => return Err(LockError::Io(e)),
            }
            // A stopping service may have removed the file between our open and our lock; then we hold a lock
            // on a file nobody else can see. Start again with the file now at `path`.
            if !same_file(&file, path) {
                continue;
            }
            file.set_len(0).map_err(LockError::Io)?;
            file.seek(SeekFrom::Start(0)).map_err(LockError::Io)?;
            writeln!(file, "{}", std::process::id()).map_err(LockError::Io)?;
            file.flush().map_err(LockError::Io)?;
            return Ok(Self {
                file,
                path: path.to_owned(),
            });
        }
    }
}

impl Drop for ServiceLock {
    /// Removes the file while still holding the lock; the lock itself ends when the file is closed.
    fn drop(&mut self) {
        if same_file(&self.file, &self.path) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Takes the exclusive lock without waiting. macOS and Linux lock the whole file (advisory, so the PID stays
/// readable). Windows locks are mandatory, so a lock over the file would hide the PID from everyone else: the
/// service locks one byte far past the end instead, which no reader touches.
#[cfg(unix)]
fn try_lock(file: &File) -> Result<(), TryLockError> {
    file.try_lock()
}

#[cfg(windows)]
fn try_lock(file: &File) -> Result<(), TryLockError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_IO_PENDING, ERROR_LOCK_VIOLATION};
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    // Byte 2^32 + 0: past any PID the file will ever hold.
    overlapped.Anonymous.Anonymous.OffsetHigh = 1;
    // SAFETY: a valid file handle and a zeroed OVERLAPPED that lives for the (immediate) call.
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle().cast(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if ok != 0 {
        return Ok(());
    }
    let e = io::Error::last_os_error();
    match e.raw_os_error().and_then(|c| u32::try_from(c).ok()) {
        Some(ERROR_LOCK_VIOLATION | ERROR_IO_PENDING) => Err(TryLockError::WouldBlock),
        _ => Err(TryLockError::Error(e)),
    }
}

/// Whether a service process is alive for this lock file: the file names a PID and that process exists. Used to
/// tell "reconnecting" (the service runs, the connection dropped) from "stopped" (the service has ended). It never
/// takes the lock, so it can't get in the way of a service that is starting.
pub fn service_alive(path: &Path) -> bool {
    read_pid(path).is_some_and(process_alive)
}

fn process_alive(pid: u32) -> bool {
    crate::process::alive(pid)
}

/// The PID written in a lock file, if it holds one.
pub fn read_pid(path: &Path) -> Option<u32> {
    let mut text = String::new();
    File::open(path).ok()?.read_to_string(&mut text).ok()?;
    text.trim().parse().ok()
}

#[cfg(unix)]
fn same_file(file: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (file.metadata(), std::fs::metadata(path)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Windows: the volume serial number and file index of the open file and of the file at `path`.
#[cfg(windows)]
fn same_file(file: &File, path: &Path) -> bool {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    fn identity(file: &File) -> Option<(u32, u32, u32)> {
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: a valid open file handle and an out parameter.
        let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) };
        (ok != 0).then_some((
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
        ))
    }
    match (
        identity(file),
        File::open(path).ok().as_ref().and_then(identity),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_holder() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("service.lock");
        let lock = ServiceLock::acquire(&path).unwrap();
        assert_eq!(read_pid(&path), Some(std::process::id()));
        match ServiceLock::acquire(&path) {
            Err(LockError::Held { pid }) => assert_eq!(pid, Some(std::process::id())),
            other => panic!("{other:?}"),
        }
        drop(lock);
        assert!(!path.exists(), "a clean release removes the file");
        let again = ServiceLock::acquire(&path).unwrap();
        drop(again);
    }

    #[test]
    fn alive_only_while_the_pid_runs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("service.lock");
        assert!(!service_alive(&path), "no file");
        let lock = ServiceLock::acquire(&path).unwrap();
        assert!(service_alive(&path), "this process holds it");
        drop(lock);
        // A crash leaves the file with a PID that no longer runs.
        let mut child = crate::process::testing::quick_command().spawn().unwrap();
        let dead = child.id();
        child.wait().unwrap();
        std::fs::write(&path, format!("{dead}\n")).unwrap();
        assert!(!service_alive(&path));
    }

    #[test]
    fn a_stale_file_is_taken_over() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("service.lock");
        std::fs::write(&path, "999999\n").unwrap();
        let lock = ServiceLock::acquire(&path).unwrap();
        assert_eq!(read_pid(&path), Some(std::process::id()));
        drop(lock);
    }
}
