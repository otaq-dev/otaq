//! The data folder, `~/.otaq/` on every platform (d0055): the database, `wt/<repo-name>/<KEY>` worktrees,
//! `logs/` and the service socket. Nothing is ever written into the user's repositories (d0023).
//!
//! `OTAQ_HOME` points the whole folder somewhere else. Tests always set it to a temporary directory, so
//! they never read or write the real `~/.otaq/`.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::naming;

/// Where Otaq keeps its data. Only builds paths; folders are created with [`DataDir::ensure_dir`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// A data folder at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `OTAQ_HOME` if it is set and not empty, otherwise `~/.otaq`.
    pub fn from_env() -> Result<Self, DataDirError> {
        Self::resolve(std::env::var_os(Self::env_var()), std::env::home_dir())
    }

    fn resolve(
        override_dir: Option<OsString>,
        user_home: Option<PathBuf>,
    ) -> Result<Self, DataDirError> {
        if let Some(dir) = override_dir.filter(|v| !v.is_empty()) {
            return Ok(Self::new(dir));
        }
        let user_home = user_home.ok_or(DataDirError::NoHomeFolder)?;
        Ok(Self::new(user_home.join(naming::data_dir_name())))
    }

    /// The variable that overrides the data folder: `OTAQ_HOME`.
    pub fn env_var() -> String {
        naming::env_var("HOME")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The SQLite database file. The file name is an implementation choice (SDD §26).
    pub fn database_path(&self) -> PathBuf {
        self.root.join(format!("{}.db", naming::COMMAND_NAME))
    }

    /// The folder holding every worktree, grouped by repository.
    pub fn worktrees_dir(&self) -> PathBuf {
        self.root.join("wt")
    }

    /// A ticket's worktree: `wt/<repo-name>/<KEY>`, for example `wt/shop-web/SHOP-40.1`.
    pub fn worktree_path(&self, repo_name: &str, key: &str) -> Result<PathBuf, DataDirError> {
        let repo = path_component(repo_name)?;
        let key = path_component(key)?;
        Ok(self.worktrees_dir().join(repo).join(key))
    }

    /// Otaq's own scratch worktree for a repository (`scratch/<repo-name>`), where Local merges happen,
    /// never in the user's folder (d0038).
    pub fn scratch_path(&self, repo_name: &str) -> Result<PathBuf, DataDirError> {
        let repo = path_component(repo_name)?;
        Ok(self.root.join("scratch").join(repo))
    }

    /// The log folder; files rotate daily (see [`crate::logging`]).
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// A ticket's setup-command output (SES-01 step 5): `logs/setup/<KEY>.log`, replaced on every start.
    pub fn setup_log_path(&self, key: &str) -> Result<PathBuf, DataDirError> {
        let key = path_component(key)?;
        Ok(self.logs_dir().join("setup").join(format!("{key}.log")))
    }

    /// The service socket (a Unix domain socket on macOS and Linux).
    pub fn socket_path(&self) -> PathBuf {
        self.root.join(format!("{}.sock", naming::COMMAND_NAME))
    }

    /// The service's lock file: locked while a service runs (so only one can), holding its PID.
    pub fn service_lock_path(&self) -> PathBuf {
        self.root.join("service.lock")
    }

    /// Creates `path` and any missing parents. New folders are readable only by the current user.
    pub fn ensure_dir(path: &Path) -> io::Result<()> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path)
    }
}

/// A repository name or ticket key must be one plain path component, so a worktree path never escapes `wt/`.
fn path_component(part: &str) -> Result<&str, DataDirError> {
    let invalid = part.is_empty()
        || part == "."
        || part == ".."
        || part.contains(['/', '\\'])
        || part.contains('\0');
    if invalid {
        Err(DataDirError::InvalidPathPart(part.to_owned()))
    } else {
        Ok(part)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataDirError {
    /// No home folder could be found and the override variable isn't set.
    NoHomeFolder,
    /// A repository name or key that isn't a single path component.
    InvalidPathPart(String),
}

impl fmt::Display for DataDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHomeFolder => write!(
                f,
                "no home folder found; set {} to choose the data folder",
                DataDir::env_var()
            ),
            Self::InvalidPathPart(part) => write!(f, "{part:?} can't be used as a folder name"),
        }
    }
}

impl std::error::Error for DataDirError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_data_dir() -> (tempfile::TempDir, DataDir) {
        let tmp = tempfile::tempdir().unwrap();
        let data = DataDir::new(tmp.path().join("home"));
        (tmp, data)
    }

    #[test]
    fn paths_live_under_the_root() {
        let (tmp, data) = temp_data_dir();
        let root = tmp.path().join("home");
        assert_eq!(data.root(), root);
        let name = naming::COMMAND_NAME;
        assert_eq!(data.database_path(), root.join(format!("{name}.db")));
        assert_eq!(data.worktrees_dir(), root.join("wt"));
        assert_eq!(data.logs_dir(), root.join("logs"));
        assert_eq!(data.socket_path(), root.join(format!("{name}.sock")));
        assert_eq!(data.service_lock_path(), root.join("service.lock"));
    }

    #[test]
    fn worktree_paths_use_repo_name_and_key() {
        let (tmp, data) = temp_data_dir();
        let wt = tmp.path().join("home").join("wt");
        assert_eq!(
            data.worktree_path("shop-web", "SHOP-18").unwrap(),
            wt.join("shop-web").join("SHOP-18")
        );
        assert_eq!(
            data.worktree_path("shop-web", "SHOP-40.1").unwrap(),
            wt.join("shop-web").join("SHOP-40.1")
        );
    }

    #[test]
    fn worktree_paths_never_escape_the_folder() {
        let (_tmp, data) = temp_data_dir();
        for bad in ["", ".", "..", "a/b", "a\\b", "x\0y"] {
            assert_eq!(
                data.worktree_path(bad, "SHOP-1"),
                Err(DataDirError::InvalidPathPart(bad.to_owned()))
            );
            assert!(data.worktree_path("shop-web", bad).is_err());
        }
    }

    #[test]
    fn ensure_dir_creates_user_only_folders() {
        let (_tmp, data) = temp_data_dir();
        let wt = data.worktree_path("shop-web", "SHOP-18").unwrap();
        DataDir::ensure_dir(&wt).unwrap();
        assert!(wt.is_dir());
        // Creating it again is fine.
        DataDir::ensure_dir(&wt).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for dir in [data.root().to_path_buf(), data.worktrees_dir(), wt] {
                let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o700, "{}", dir.display());
            }
        }
    }

    #[test]
    fn override_variable_name() {
        assert_eq!(DataDir::env_var(), naming::env_var("HOME"));
    }

    #[test]
    fn the_override_wins_over_the_home_folder() {
        let data = DataDir::resolve(Some("/tmp/cb".into()), Some("/Users/me".into())).unwrap();
        assert_eq!(data.root(), Path::new("/tmp/cb"));
    }

    #[test]
    fn default_is_the_hidden_folder_in_home() {
        let expected = Path::new("/Users/me").join(naming::data_dir_name());
        for empty in [None, Some(OsString::new())] {
            let data = DataDir::resolve(empty, Some("/Users/me".into())).unwrap();
            assert_eq!(data.root(), expected);
        }
        assert_eq!(
            DataDir::resolve(None, None),
            Err(DataDirError::NoHomeFolder)
        );
    }
}
