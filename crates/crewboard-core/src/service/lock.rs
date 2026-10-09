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
            match file.try_lock() {
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

#[cfg(not(unix))]
fn same_file(_file: &File, path: &Path) -> bool {
    path.exists()
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
    fn a_stale_file_is_taken_over() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("service.lock");
        std::fs::write(&path, "999999\n").unwrap();
        let lock = ServiceLock::acquire(&path).unwrap();
        assert_eq!(read_pid(&path), Some(std::process::id()));
        drop(lock);
    }
}
