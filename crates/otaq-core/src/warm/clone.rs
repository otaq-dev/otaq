//! Copy-on-write cloning, and whether the data folder's disk can clone (WRM-01, d0082).
//!
//! - [`clone_tree`] clones a folder, or the listed paths inside it, keeping the layout, modified times, permissions and
//!   symlinks as symlinks. macOS: one `clonefile(2)` per listed path (a whole folder is one call, never file by file).
//!   Linux: a walk that clones each file with `ioctl(FICLONE)` (btrfs, XFS) and recreates folders and symlinks.
//!   Windows: a walk that block-clones each file with `FSCTL_DUPLICATE_EXTENTS_TO_FILE` (ReFS, Dev Drive) and
//!   recreates folders and symlinks; NTFS can't clone.
//! - **A plain copy is never made.** The first file that can't be cloned stops the clone, what was made so far is
//!   removed, and [`CloneError::Unsupported`] comes back. Another disk (`EXDEV`) is unsupported too. Hard links are
//!   never made: build tools write into their outputs.
//! - [`move_tree`] renames paths from one folder into another on the same disk; another disk is an error, never a copy.
//! - [`probe`] clones a small file inside `<app-data>/warm/.probe` to learn whether the data folder's disk can clone.
//!   It decides alone whether warm builds are available; [`CloneProbe`] keeps its answer for the life of the service.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::data_dir::DataDir;

/// Whether the disk under the data folder can clone, and which disk it is (shown when it can't).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneSupport {
    pub available: bool,
    /// The file system as the operating system names it: `apfs`, `exfat` (macOS), `btrfs`, `ext4` (Linux), `NTFS`,
    /// `ReFS` (Windows). Empty when it couldn't be read.
    pub file_system: String,
    /// Where that disk is mounted: `/`, `/home`, `C:`. Empty when it couldn't be read.
    pub volume: String,
}

#[derive(Debug)]
pub enum CloneError {
    /// The disk can't clone `path`, or the two folders are on different disks. Nothing is copied instead.
    Unsupported {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        error: io::Error,
    },
}

impl fmt::Display for CloneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { path } => write!(
                f,
                "{} can't be cloned copy-on-write here (another disk, or a disk that can't clone); it isn't copied",
                path.display()
            ),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for CloneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unsupported { .. } => None,
            Self::Io { error, .. } => Some(error),
        }
    }
}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> CloneError + '_ {
    move |error| CloneError::Io {
        path: path.to_owned(),
        error,
    }
}

/// What does the cloning, so the warm-build tests can stand in for it where the test disk can't clone.
pub trait Cloner: fmt::Debug + Send + Sync {
    fn clone_tree(
        &self,
        from: &Path,
        to: &Path,
        only: Option<&[PathBuf]>,
    ) -> Result<(), CloneError>;
}

/// The file system's own cloning: [`clone_tree`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FsCloner;

impl Cloner for FsCloner {
    fn clone_tree(
        &self,
        from: &Path,
        to: &Path,
        only: Option<&[PathBuf]>,
    ) -> Result<(), CloneError> {
        clone_tree(from, to, only)
    }
}

/// Clones `from` into `to`, or with `only`, just those paths (relative to `from`) into the same places under `to`.
///
/// Without `only`, `to` must not exist. With `only`, `to` may exist (a fresh worktree) but none of the listed paths
/// may. Missing parent folders are created. On any failure everything this call created is removed, so a clone that
/// fails halfway leaves nothing behind.
pub fn clone_tree(from: &Path, to: &Path, only: Option<&[PathBuf]>) -> Result<(), CloneError> {
    let mut made = Vec::new();
    let result = clone_into(from, to, only, &mut made);
    if result.is_err() {
        for path in made.iter().rev() {
            remove_tree(path);
        }
    }
    result
}

fn clone_into(
    from: &Path,
    to: &Path,
    only: Option<&[PathBuf]>,
    made: &mut Vec<PathBuf>,
) -> Result<(), CloneError> {
    let Some(paths) = only else {
        return clone_one(from, to, made);
    };
    for rel in paths {
        let rel = relative(rel)?;
        clone_one(&from.join(rel), &to.join(rel), made)?;
    }
    Ok(())
}

fn clone_one(src: &Path, dst: &Path, made: &mut Vec<PathBuf>) -> Result<(), CloneError> {
    refuse_existing(dst)?;
    create_parents(dst, made)?;
    made.push(dst.to_owned());
    platform::clone_entry(src, dst)
}

/// Renames `from` to `to`, or with `only`, just those paths (relative to `from`) into the same places under `to`,
/// creating parent folders (removed again if the rename fails). A destination that exists is an error. Another disk is [`CloneError::Unsupported`],
/// never a copy. Stops at the first failure; what was moved before it stays moved.
pub fn move_tree(from: &Path, to: &Path, only: Option<&[PathBuf]>) -> Result<(), CloneError> {
    let Some(paths) = only else {
        return move_one(from, to);
    };
    for rel in paths {
        let rel = relative(rel)?;
        move_one(&from.join(rel), &to.join(rel))?;
    }
    Ok(())
}

fn move_one(src: &Path, dst: &Path) -> Result<(), CloneError> {
    refuse_existing(dst)?;
    let mut made = Vec::new();
    let result = create_parents(dst, &mut made).and_then(|()| rename(src, dst));
    if result.is_err() {
        for dir in made.iter().rev() {
            let _ = fs::remove_dir(dir);
        }
    }
    result
}

fn rename(src: &Path, dst: &Path) -> Result<(), CloneError> {
    fs::rename(src, dst).map_err(|error| {
        if error.kind() == io::ErrorKind::CrossesDevices {
            CloneError::Unsupported {
                path: src.to_owned(),
            }
        } else {
            CloneError::Io {
                path: src.to_owned(),
                error,
            }
        }
    })
}

/// A listed path must stay inside its folder.
fn relative(path: &Path) -> Result<&Path, CloneError> {
    let inside = !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if inside {
        Ok(path)
    } else {
        Err(CloneError::Io {
            path: path.to_owned(),
            error: io::Error::new(io::ErrorKind::InvalidInput, "not a path inside the folder"),
        })
    }
}

fn refuse_existing(dst: &Path) -> Result<(), CloneError> {
    if fs::symlink_metadata(dst).is_ok() {
        return Err(CloneError::Io {
            path: dst.to_owned(),
            error: io::Error::from(io::ErrorKind::AlreadyExists),
        });
    }
    Ok(())
}

/// Creates the missing parents of `path`, outermost first, recording each so a failure can remove them.
fn create_parents(path: &Path, made: &mut Vec<PathBuf>) -> Result<(), CloneError> {
    let missing: Vec<&Path> = path
        .ancestors()
        .skip(1)
        .filter(|dir| !dir.as_os_str().is_empty())
        .take_while(|dir| fs::symlink_metadata(dir).is_err())
        .collect();
    for dir in missing.into_iter().rev() {
        fs::create_dir(dir).map_err(io_at(dir))?;
        made.push(dir.to_owned());
    }
    Ok(())
}

/// Removes a file, symlink or folder, quietly. A cloned read-only folder is made writable first, so its contents can
/// go.
pub(crate) fn remove_tree(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        let _ = fs::remove_file(path);
        return;
    }
    if fs::remove_dir_all(path).is_err() {
        make_writable(path);
        let _ = fs::remove_dir_all(path);
    }
}

fn make_writable(dir: &Path) {
    let Ok(meta) = fs::symlink_metadata(dir) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let mut perms = meta.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(perms.mode() | 0o700);
    }
    #[cfg(not(unix))]
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    let _ = fs::set_permissions(dir, perms);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            make_writable(&entry.path());
        }
    }
}

/// Learns whether the data folder's disk can clone: clones a small file inside `<app-data>/warm/.probe`, then
/// removes the probe folder. Any failure answers unavailable; the file system and volume are read either way.
pub fn probe(data: &DataDir) -> CloneSupport {
    let dir = data.warm_dir().join(".probe");
    remove_tree(&dir);
    let available = DataDir::ensure_dir(&dir).is_ok() && probe_clone(&dir);
    let (file_system, volume) = platform::volume_of(nearest_existing(&dir));
    remove_tree(&dir);
    CloneSupport {
        available,
        file_system,
        volume,
    }
}

fn probe_clone(dir: &Path) -> bool {
    const CONTENT: &[u8] = b"copy-on-write probe\n";
    let source = dir.join("source");
    let copy = dir.join("copy");
    fs::write(&source, CONTENT).is_ok()
        && clone_tree(&source, &copy, None).is_ok()
        && fs::read(&copy).is_ok_and(|read| read == CONTENT)
}

fn nearest_existing(path: &Path) -> &Path {
    path.ancestors().find(|dir| dir.exists()).unwrap_or(path)
}

/// The probe's answer, kept for the life of the service: probed once (at service start) and then reused. Tests
/// build it [`forced`](Self::forced) to a value.
#[derive(Debug, Default)]
pub struct CloneProbe {
    kept: OnceLock<CloneSupport>,
}

impl CloneProbe {
    /// Probes on the first [`support`](Self::support) call.
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers `support` without probing.
    pub fn forced(support: CloneSupport) -> Self {
        let kept = OnceLock::new();
        let _ = kept.set(support);
        Self { kept }
    }

    /// The kept answer, probing `data` the first time.
    pub fn support(&self, data: &DataDir) -> &CloneSupport {
        self.kept.get_or_init(|| probe(data))
    }
}

#[cfg(unix)]
fn c_path(path: &Path) -> Result<std::ffi::CString, CloneError> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| CloneError::Io {
        path: path.to_owned(),
        error: io::Error::new(io::ErrorKind::InvalidInput, "a null character in the path"),
    })
}

/// Sets the access and modified times of `path` itself (a symlink is not followed).
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn set_times(path: &Path, meta: &fs::Metadata) -> Result<(), CloneError> {
    use std::os::unix::fs::MetadataExt;
    let c = c_path(path)?;
    // SAFETY: `timespec` is plain data; every field the call reads is set below.
    let mut times: [libc::timespec; 2] = unsafe { std::mem::zeroed() };
    times[0].tv_sec = meta.atime() as libc::time_t;
    times[0].tv_nsec = meta.atime_nsec() as _;
    times[1].tv_sec = meta.mtime() as libc::time_t;
    times[1].tv_nsec = meta.mtime_nsec() as _;
    // SAFETY: `c` is a valid null-terminated path and `times` holds two entries.
    let done = unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            c.as_ptr(),
            times.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if done == 0 {
        Ok(())
    } else {
        Err(CloneError::Io {
            path: path.to_owned(),
            error: io::Error::last_os_error(),
        })
    }
}

/// A C `char` array filled by the system, up to its first null.
#[cfg(target_os = "macos")]
fn c_chars(chars: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    /// `clonefile(2)`'s flag: clone a symlink itself, not what it points to.
    const CLONE_NOFOLLOW: u32 = 0x0001;

    /// One `clonefile(2)` call: a file, a symlink, or a whole folder at once (APFS clones the folder's hierarchy,
    /// keeping the files' times and permissions). Filling a cloned folder refreshes its own modified time, so the
    /// folders' times are set back afterwards.
    pub(super) fn clone_entry(src: &Path, dst: &Path) -> Result<(), CloneError> {
        let from = c_path(src)?;
        let to = c_path(dst)?;
        // SAFETY: both are valid null-terminated paths that outlive the call.
        if unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), CLONE_NOFOLLOW) } == 0 {
            return folder_times(src, dst);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ENOTSUP | libc::EOPNOTSUPP | libc::EXDEV) => Err(CloneError::Unsupported {
                path: src.to_owned(),
            }),
            _ => Err(CloneError::Io {
                path: src.to_owned(),
                error,
            }),
        }
    }

    /// Sets every cloned folder's times to its source's, innermost first. Nothing else is walked or touched.
    fn folder_times(src: &Path, dst: &Path) -> Result<(), CloneError> {
        let meta = fs::symlink_metadata(src).map_err(io_at(src))?;
        if !meta.is_dir() {
            return Ok(());
        }
        for entry in fs::read_dir(src).map_err(io_at(src))? {
            let entry = entry.map_err(io_at(src))?;
            if entry.file_type().map_err(io_at(src))?.is_dir() {
                let name = entry.file_name();
                folder_times(&src.join(&name), &dst.join(&name))?;
            }
        }
        set_times(dst, &meta)
    }

    /// The file system type (`apfs`, `exfat`) and mount point from `statfs`.
    pub(super) fn volume_of(path: &Path) -> (String, String) {
        let Ok(c) = c_path(path) else {
            return Default::default();
        };
        // SAFETY: `statfs` is plain data, filled by the call; `c` is a valid null-terminated path.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return Default::default();
        }
        (c_chars(&st.f_fstypename), c_chars(&st.f_mntonname))
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    use super::*;

    /// Recreates folders and symlinks, clones each file with `ioctl(FICLONE)`, and sets the times last.
    pub(super) fn clone_entry(src: &Path, dst: &Path) -> Result<(), CloneError> {
        let meta = fs::symlink_metadata(src).map_err(io_at(src))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let target = fs::read_link(src).map_err(io_at(src))?;
            std::os::unix::fs::symlink(target, dst).map_err(io_at(dst))?;
        } else if kind.is_dir() {
            fs::create_dir(dst).map_err(io_at(dst))?;
            for entry in fs::read_dir(src).map_err(io_at(src))? {
                let name = entry.map_err(io_at(src))?.file_name();
                clone_entry(&src.join(&name), &dst.join(&name))?;
            }
            // After the contents, so a read-only folder can still be filled.
            fs::set_permissions(dst, meta.permissions()).map_err(io_at(dst))?;
        } else if kind.is_file() {
            clone_file(src, dst, &meta)?;
        } else {
            // A socket, pipe or device can't be cloned.
            return Err(CloneError::Unsupported {
                path: src.to_owned(),
            });
        }
        set_times(dst, &meta)
    }

    fn clone_file(src: &Path, dst: &Path, meta: &fs::Metadata) -> Result<(), CloneError> {
        let from = fs::File::open(src).map_err(io_at(src))?;
        let to = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dst)
            .map_err(io_at(dst))?;
        // SAFETY: both descriptors are open for the whole call.
        if unsafe { libc::ioctl(to.as_raw_fd(), libc::FICLONE, from.as_raw_fd()) } != 0 {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(
                    libc::EOPNOTSUPP | libc::EXDEV | libc::EINVAL | libc::ENOTTY | libc::ENOSYS,
                ) => Err(CloneError::Unsupported {
                    path: src.to_owned(),
                }),
                _ => Err(CloneError::Io {
                    path: src.to_owned(),
                    error,
                }),
            };
        }
        to.set_permissions(meta.permissions()).map_err(io_at(dst))
    }

    /// The file system type from `statfs.f_type`, and the mount point (the outermost folder on the same device).
    pub(super) fn volume_of(path: &Path) -> (String, String) {
        let Ok(c) = c_path(path) else {
            return Default::default();
        };
        // SAFETY: `statfs` is plain data, filled by the call; `c` is a valid null-terminated path.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return Default::default();
        }
        // `f_type`'s integer type differs between targets; the magic numbers are 32 bits.
        #[allow(clippy::unnecessary_cast)]
        let magic = st.f_type as u32;
        (file_system_name(magic), mount_point(path))
    }

    fn file_system_name(magic: u32) -> String {
        let name = match magic {
            0x9123_683E => "btrfs",
            0x5846_5342 => "xfs",
            0xEF53 => "ext4",
            0xCA45_1A4E => "bcachefs",
            0xF2F5_2010 => "f2fs",
            0x2FC1_2FC1 => "zfs",
            0x0102_1994 => "tmpfs",
            0x794C_7630 => "overlayfs",
            0x2011_BAB0 => "exfat",
            0x4D44 => "vfat",
            0x5346_544E | 0x7366_746E => "ntfs",
            0x6969 => "nfs",
            0xFF53_4D42 | 0xFE53_4D42 => "smb",
            0x0102_1997 => "9p",
            0x6573_5546 => "fuse",
            0x7371_7368 => "squashfs",
            0xF15F => "ecryptfs",
            other => return format!("0x{other:x}"),
        };
        name.to_owned()
    }

    fn mount_point(path: &Path) -> String {
        let Ok(path) = fs::canonicalize(path) else {
            return String::new();
        };
        let device = |dir: &Path| fs::metadata(dir).ok().map(|meta| meta.dev());
        let Some(dev) = device(&path) else {
            return String::new();
        };
        let mut mount = path.as_path();
        while let Some(parent) = mount.parent() {
            if device(parent) != Some(dev) {
                break;
            }
            mount = parent;
        }
        mount.display().to_string()
    }
}

#[cfg(windows)]
mod platform {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::{FileTimesExt, FileTypeExt, MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::{
        ERROR_BLOCK_TOO_MANY_REFERENCES, ERROR_INVALID_FUNCTION, ERROR_NOT_SAME_DEVICE,
        ERROR_NOT_SUPPORTED, HANDLE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL,
        FILE_ATTRIBUTE_NOT_CONTENT_INDEXED, FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SPARSE_FILE,
        FILE_ATTRIBUTE_SYSTEM, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_WRITE_ATTRIBUTES, GetVolumeInformationW, GetVolumePathNameW, SetFileAttributesW,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::{
        DUPLICATE_EXTENTS_DATA, FSCTL_DUPLICATE_EXTENTS_TO_FILE, FSCTL_GET_INTEGRITY_INFORMATION,
        FSCTL_GET_INTEGRITY_INFORMATION_BUFFER, FSCTL_SET_INTEGRITY_INFORMATION,
        FSCTL_SET_INTEGRITY_INFORMATION_BUFFER, FSCTL_SET_SPARSE,
    };

    use super::*;

    /// The attributes a clone carries over. Sparse is set by its own call; the rest belong to the disk.
    const KEPT_ATTRIBUTES: u32 = FILE_ATTRIBUTE_READONLY
        | FILE_ATTRIBUTE_HIDDEN
        | FILE_ATTRIBUTE_SYSTEM
        | FILE_ATTRIBUTE_ARCHIVE
        | FILE_ATTRIBUTE_NOT_CONTENT_INDEXED;

    /// One block-clone call must stay under 4 GB; 1 GB is a whole number of clusters of any size.
    const CHUNK: u64 = 1 << 30;

    /// Recreates folders and symlinks, block-clones each file with `FSCTL_DUPLICATE_EXTENTS_TO_FILE` (ReFS, Dev
    /// Drive), and sets the times and attributes last. NTFS and other disks answer [`CloneError::Unsupported`].
    pub(super) fn clone_entry(src: &Path, dst: &Path) -> Result<(), CloneError> {
        let meta = fs::symlink_metadata(src).map_err(io_at(src))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            let target = fs::read_link(src).map_err(io_at(src))?;
            if kind.is_symlink_dir() {
                std::os::windows::fs::symlink_dir(target, dst).map_err(io_at(dst))?;
            } else {
                std::os::windows::fs::symlink_file(target, dst).map_err(io_at(dst))?;
            }
            // A symlink keeps its own times; its attributes follow its kind.
            return set_times(
                dst,
                &meta,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            );
        }
        if kind.is_dir() {
            fs::create_dir(dst).map_err(io_at(dst))?;
            for entry in fs::read_dir(src).map_err(io_at(src))? {
                let name = entry.map_err(io_at(src))?.file_name();
                clone_entry(&src.join(&name), &dst.join(&name))?;
            }
            // After the contents, so filling the folder doesn't refresh its time.
            set_times(dst, &meta, FILE_FLAG_BACKUP_SEMANTICS)?;
        } else {
            clone_file(src, dst, &meta)?;
            set_times(dst, &meta, 0)?;
        }
        set_attributes(dst, meta.file_attributes())
    }

    /// Creates `dst` with `src`'s integrity setting, sparseness and size, then clones every cluster of `src` into it.
    fn clone_file(src: &Path, dst: &Path, meta: &fs::Metadata) -> Result<(), CloneError> {
        let from = fs::File::open(src).map_err(io_at(src))?;
        let mut integrity = FSCTL_GET_INTEGRITY_INFORMATION_BUFFER::default();
        // ReFS answers the cluster size and the integrity setting; NTFS answers an error (it can't clone).
        control(&from, FSCTL_GET_INTEGRITY_INFORMATION, &(), &mut integrity)
            .map_err(unsupported_or_io(src))?;
        let to = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dst)
            .map_err(io_at(dst))?;
        // A block clone needs both files to have the same integrity setting (the new file takes its folder's), and
        // a sparse source a sparse target.
        let mut target = FSCTL_GET_INTEGRITY_INFORMATION_BUFFER::default();
        control(&to, FSCTL_GET_INTEGRITY_INFORMATION, &(), &mut target)
            .map_err(unsupported_or_io(dst))?;
        if (target.ChecksumAlgorithm, target.Flags)
            != (integrity.ChecksumAlgorithm, integrity.Flags)
        {
            let set = FSCTL_SET_INTEGRITY_INFORMATION_BUFFER {
                ChecksumAlgorithm: integrity.ChecksumAlgorithm,
                Reserved: 0,
                Flags: integrity.Flags,
            };
            control(&to, FSCTL_SET_INTEGRITY_INFORMATION, &set, &mut ())
                .map_err(unsupported_or_io(dst))?;
        }
        if meta.file_attributes() & FILE_ATTRIBUTE_SPARSE_FILE != 0 {
            control(&to, FSCTL_SET_SPARSE, &(), &mut ()).map_err(unsupported_or_io(dst))?;
        }
        // The target's size first; then the ranges, cluster-aligned (the last one rounded up past the end).
        let len = meta.len();
        to.set_len(len).map_err(io_at(dst))?;
        let cluster = u64::from(integrity.ClusterSizeInBytes.max(1));
        let end = len.div_ceil(cluster) * cluster;
        let mut offset = 0;
        while offset < end {
            let count = CHUNK.min(end - offset);
            let range = DUPLICATE_EXTENTS_DATA {
                FileHandle: from.as_raw_handle() as HANDLE,
                SourceFileOffset: offset as i64,
                TargetFileOffset: offset as i64,
                ByteCount: count as i64,
            };
            control(&to, FSCTL_DUPLICATE_EXTENTS_TO_FILE, &range, &mut ())
                .map_err(unsupported_or_io(src))?;
            offset += count;
        }
        Ok(())
    }

    /// One `DeviceIoControl` call on `file`, with a fixed-size input and output (`()` for none).
    fn control<I, O>(file: &fs::File, code: u32, input: &I, output: &mut O) -> io::Result<()> {
        let size = |n: usize| u32::try_from(n).expect("a small buffer");
        let in_ptr = if size_of::<I>() == 0 {
            std::ptr::null()
        } else {
            (input as *const I).cast()
        };
        let out_ptr = if size_of::<O>() == 0 {
            std::ptr::null_mut()
        } else {
            (output as *mut O).cast()
        };
        let mut returned = 0u32;
        // SAFETY: the handle is open for the call; the buffers are valid for the sizes given, or null with size 0.
        let done = unsafe {
            DeviceIoControl(
                file.as_raw_handle() as HANDLE,
                code,
                in_ptr,
                size(size_of::<I>()),
                out_ptr,
                size(size_of::<O>()),
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if done == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// A disk that can't block-clone (NTFS, FAT), another disk, or a cluster shared too often is
    /// [`CloneError::Unsupported`]; anything else is an I/O error.
    fn unsupported_or_io(path: &Path) -> impl FnOnce(io::Error) -> CloneError + '_ {
        move |error| {
            let code = error.raw_os_error().map(|code| code as u32);
            match code {
                Some(
                    ERROR_INVALID_FUNCTION
                    | ERROR_NOT_SUPPORTED
                    | ERROR_NOT_SAME_DEVICE
                    | ERROR_BLOCK_TOO_MANY_REFERENCES,
                ) => CloneError::Unsupported {
                    path: path.to_owned(),
                },
                _ => CloneError::Io {
                    path: path.to_owned(),
                    error,
                },
            }
        }
    }

    /// Sets the created, accessed and modified times of `path` itself, opened with `flags`.
    fn set_times(path: &Path, meta: &fs::Metadata, flags: u32) -> Result<(), CloneError> {
        let mut times = fs::FileTimes::new();
        if let Ok(time) = meta.accessed() {
            times = times.set_accessed(time);
        }
        if let Ok(time) = meta.modified() {
            times = times.set_modified(time);
        }
        if let Ok(time) = meta.created() {
            times = times.set_created(time);
        }
        let file = fs::OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES)
            .custom_flags(flags)
            .open(path)
            .map_err(io_at(path))?;
        file.set_times(times).map_err(io_at(path))
    }

    /// Carries over read-only, hidden, system, archive and not-indexed. Last, so a read-only file was still
    /// writable while it was made.
    fn set_attributes(path: &Path, attributes: u32) -> Result<(), CloneError> {
        let kept = attributes & KEPT_ATTRIBUTES;
        let current = fs::symlink_metadata(path)
            .map_err(io_at(path))?
            .file_attributes();
        if current & KEPT_ATTRIBUTES == kept {
            return Ok(());
        }
        let wanted = match (current & !KEPT_ATTRIBUTES) | kept {
            0 => FILE_ATTRIBUTE_NORMAL,
            wanted => wanted,
        };
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a null-terminated path.
        if unsafe { SetFileAttributesW(wide.as_ptr(), wanted) } == 0 {
            return Err(CloneError::Io {
                path: path.to_owned(),
                error: io::Error::last_os_error(),
            });
        }
        Ok(())
    }

    /// The file system name (`NTFS`, `ReFS`) from `GetVolumeInformationW`, and the volume (`C:`).
    pub(super) fn volume_of(path: &Path) -> (String, String) {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut root = [0u16; 261];
        // SAFETY: `wide` is null-terminated and `root` is writable for the length given.
        if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
            return Default::default();
        }
        let mut name = [0u16; 261];
        // SAFETY: `root` holds a null-terminated volume path; the optional outputs are null; `name` is writable.
        let read = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                name.as_mut_ptr(),
                name.len() as u32,
            )
        };
        let volume = from_wide(&root);
        let volume = volume.strip_suffix('\\').unwrap_or(&volume).to_owned();
        let file_system = if read == 0 {
            String::new()
        } else {
            from_wide(&name)
        };
        (file_system, volume)
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod platform {
    use super::*;

    pub(super) fn clone_entry(src: &Path, _dst: &Path) -> Result<(), CloneError> {
        Err(CloneError::Unsupported {
            path: src.to_owned(),
        })
    }

    pub(super) fn volume_of(_path: &Path) -> (String, String) {
        Default::default()
    }
}
