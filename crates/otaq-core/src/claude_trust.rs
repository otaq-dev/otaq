//! Claude Code's folder trust (PRJ-06, d0085): adding a project records `projects["<repo>"].hasTrustDialogAccepted`
//! in Claude Code's config file, so its agents never wait on the "Quick safety check" screen.
//!
//! The file is Claude Code's, rewritten often by every running `claude`, and holds its login and settings. Only that
//! one key is ever changed: the file is re-read just before writing, every other value is kept, and the new content
//! goes through a temporary file in the same folder and a rename. A file that doesn't parse is never overwritten.
//! The logic takes the file's path from [`ClaudeConfig`]; only [`ClaudeConfig::from_env`] reads the environment.
//!
//! [`Trust`] is the service's side: the trust is recorded when a project is added and, in a startup pass, for the
//! projects added earlier (PRJ-08), with the one-time note and the projects whose trust couldn't be recorded.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::watch;

use crate::store::settings::{TRUST_NOTE_DISMISSED, TRUST_NOTE_PENDING};
use crate::store::{Store, StoreError};
use crate::{naming, paths};

/// The key inside a `projects` entry that Claude Code's trust screen sets.
const TRUST_FIELD: &str = "hasTrustDialogAccepted";

/// How many times a write is tried when another writer changes the file in between.
const ATTEMPTS: usize = 2;

/// Where Claude Code keeps its per-folder state, and the home folder it never keeps trust for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeConfig {
    /// Claude Code's config file (`.claude.json`, or the legacy `.config.json`).
    pub file: PathBuf,
    /// The user's home folder, links resolved. Claude Code keeps trust for it per session only.
    pub home: Option<PathBuf>,
}

/// What [`ClaudeConfig::record_trust`] did for one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    /// Claude Code already trusted the repo; the file wasn't touched.
    AlreadyTrusted,
    /// The trust was written.
    Recorded,
    /// The trust couldn't be recorded. `reason` is a short sentence shown as it is (no final period).
    Failed { reason: String },
}

/// Why the trust couldn't be read or recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustError {
    /// The repo is the home folder.
    HomeFolder,
    /// The file couldn't be read, isn't JSON, or isn't shaped like Claude Code's config.
    Unreadable,
    /// The file or its folder couldn't be written.
    Unwritable,
    /// Another writer changed the file on every attempt.
    KeptChanging,
}

impl TrustError {
    /// The plain sentence the UI shows.
    pub fn reason(self) -> &'static str {
        match self {
            Self::HomeFolder => "Claude Code never keeps trust for your home folder",
            Self::Unreadable => "Claude Code's config file couldn't be read",
            Self::Unwritable => "Claude Code's config file couldn't be written",
            Self::KeptChanging => {
                "Claude Code's config file kept changing while it was being written"
            }
        }
    }
}

impl fmt::Display for TrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for TrustError {}

/// One write attempt's result.
enum Step {
    AlreadyTrusted,
    Written,
    /// The file changed between the read and the rename; nothing was written.
    Changed,
}

impl ClaudeConfig {
    pub fn new(file: impl Into<PathBuf>, home: Option<PathBuf>) -> Self {
        Self {
            file: file.into(),
            home,
        }
    }

    /// The config file this process's Claude Code uses (`CLAUDE_CONFIG_DIR`, the home folder).
    pub fn from_env() -> Option<Self> {
        Self::locate(std::env::var_os("CLAUDE_CONFIG_DIR"), std::env::home_dir())
    }

    /// Claude Code's rule (checked on 2.1.301): `$CLAUDE_CONFIG_DIR/.claude.json` when the variable is set and not
    /// empty, otherwise `<home>/.claude.json`. The legacy `<config folder>/.config.json` (`$CLAUDE_CONFIG_DIR` or
    /// `<home>/.claude`) wins when it is a non-empty file, or an empty writable one with no `.claude.json` beside
    /// the main location. `None` with neither a config folder nor a home folder.
    pub fn locate(claude_config_dir: Option<OsString>, home: Option<PathBuf>) -> Option<Self> {
        let config_dir = claude_config_dir
            .filter(|d| !d.is_empty())
            .map(PathBuf::from);
        let main = config_dir.as_ref().or(home.as_ref())?.join(".claude.json");
        let legacy_dir = config_dir.or_else(|| home.as_ref().map(|h| h.join(".claude")));
        let file = match legacy_dir.map(|d| d.join(".config.json")) {
            Some(legacy) if legacy_wins(&legacy, &main) => legacy,
            _ => main,
        };
        let home = home.map(|h| paths::canonical(&h).unwrap_or_else(|_| paths::ordinary(&h)));
        Some(Self::new(file, home))
    }

    /// Whether Claude Code trusts the folder with this [`trust_key`]. A missing file is "not trusted".
    pub fn is_trusted(&self, key: &str) -> Result<bool, TrustError> {
        let Some(bytes) = read_if_exists(&self.target())? else {
            return Ok(false);
        };
        let config: Value = serde_json::from_slice(&bytes).map_err(|_| TrustError::Unreadable)?;
        Ok(entry_trusted(&config, key))
    }

    /// Records the trust for an added project's repo (`repo_path` as stored, from [`paths::canonical`]) unless
    /// Claude Code already trusts it. Synchronous and short: nothing waits between the read and the rename.
    pub fn record_trust(&self, repo_path: &Path) -> Outcome {
        self.record_with(repo_path, &mut || {})
    }

    /// [`Self::record_trust`] with `before_rename` called between the read and the rename (a test seam).
    fn record_with(&self, repo_path: &Path, before_rename: &mut dyn FnMut()) -> Outcome {
        let key = trust_key(repo_path);
        let outcome = match self.try_record(&key, before_rename) {
            Ok(outcome) => outcome,
            Err(error) => Outcome::Failed {
                reason: error.reason().to_string(),
            },
        };
        let file = self.file.display();
        match &outcome {
            Outcome::AlreadyTrusted => {}
            Outcome::Recorded => tracing::info!(key, %file, "recorded Claude Code's trust"),
            Outcome::Failed { reason } => {
                tracing::warn!(key, %file, reason, "couldn't record Claude Code's trust")
            }
        }
        outcome
    }

    fn try_record(
        &self,
        key: &str,
        before_rename: &mut dyn FnMut(),
    ) -> Result<Outcome, TrustError> {
        if self
            .home
            .as_deref()
            .is_some_and(|home| trust_key(home) == key)
        {
            return Err(TrustError::HomeFolder);
        }
        for attempt in 0..ATTEMPTS {
            match self.write_once(key, before_rename)? {
                Step::AlreadyTrusted if attempt == 0 => return Ok(Outcome::AlreadyTrusted),
                Step::AlreadyTrusted => return Ok(Outcome::Recorded),
                Step::Changed => continue,
                // A running `claude` may have written its older copy over ours: read once more.
                Step::Written if self.is_trusted(key)? => return Ok(Outcome::Recorded),
                Step::Written => continue,
            }
        }
        Err(TrustError::KeptChanging)
    }

    /// Reads the file, sets the one key, and replaces the file through a temporary file in the same folder.
    fn write_once(&self, key: &str, before_rename: &mut dyn FnMut()) -> Result<Step, TrustError> {
        let target = self.target();
        let original = read_if_exists(&target)?;
        let mut config = match &original {
            Some(bytes) => serde_json::from_slice(bytes).map_err(|_| TrustError::Unreadable)?,
            None => Value::Object(Map::new()),
        };
        if entry_trusted(&config, key) {
            return Ok(Step::AlreadyTrusted);
        }
        set_trusted(&mut config, key)?;
        let text = serde_json::to_string_pretty(&config).map_err(|_| TrustError::Unwritable)?;

        let permissions = match original {
            Some(_) => Some(
                fs::metadata(&target)
                    .map_err(|_| TrustError::Unreadable)?
                    .permissions(),
            ),
            None => None,
        };
        // A read-only file stays as the user left it, even where its folder would allow a rename.
        if permissions.as_ref().is_some_and(|p| p.readonly()) {
            return Err(TrustError::Unwritable);
        }
        let temp = temp_path(&target);
        let result = write_new(&temp, text.as_bytes(), permissions).and_then(|()| {
            before_rename();
            if read_if_exists(&target)? != original {
                return Ok(Step::Changed);
            }
            fs::rename(&temp, &target).map_err(|_| TrustError::Unwritable)?;
            Ok(Step::Written)
        });
        if !matches!(result, Ok(Step::Written)) {
            let _ = fs::remove_file(&temp);
        }
        result
    }

    /// The file to read and replace: a linked config file is written where the link points, so the link stays.
    fn target(&self) -> PathBuf {
        fs::canonicalize(&self.file).unwrap_or_else(|_| self.file.clone())
    }
}

/// The `projects` key Claude Code uses for a folder: its real path (the caller resolves links), with no trailing
/// separator and, on Windows, `/` for `\` (`C:\Users\me\shop-web` → `C:/Users/me/shop-web`).
pub fn trust_key(repo_path: &Path) -> String {
    key_text(&repo_path.to_string_lossy(), cfg!(windows))
}

fn key_text(path: &str, windows: bool) -> String {
    let path = paths::ordinary(Path::new(path));
    let mut key = path.to_string_lossy().into_owned();
    if windows {
        key = key.replace('\\', "/");
    }
    // A root (`/`, `C:/`) keeps its separator, as Node's `path.normalize` does.
    while key.len() > 1
        && key.ends_with('/')
        && !(windows && key.len() == 3 && key.as_bytes()[1] == b':')
    {
        key.pop();
    }
    key
}

fn legacy_wins(legacy: &Path, main: &Path) -> bool {
    match fs::metadata(legacy) {
        Ok(meta) if meta.is_file() => {
            meta.len() > 0 || (!meta.permissions().readonly() && !main.exists())
        }
        _ => false,
    }
}

fn entry_trusted(config: &Value, key: &str) -> bool {
    config
        .get("projects")
        .and_then(|p| p.get(key))
        .and_then(|e| e.get(TRUST_FIELD))
        == Some(&Value::Bool(true))
}

/// Sets `projects[key].hasTrustDialogAccepted = true`, creating `projects` and the entry when they are missing. A
/// file whose shape isn't Claude Code's (an array, a `projects` that isn't an object…) is left alone.
fn set_trusted(config: &mut Value, key: &str) -> Result<(), TrustError> {
    let projects = config
        .as_object_mut()
        .ok_or(TrustError::Unreadable)?
        .entry("projects")
        .or_insert_with(|| Value::Object(Map::new()));
    let entry = projects
        .as_object_mut()
        .ok_or(TrustError::Unreadable)?
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()));
    entry
        .as_object_mut()
        .ok_or(TrustError::Unreadable)?
        .insert(TRUST_FIELD.to_string(), Value::Bool(true));
    Ok(())
}

fn read_if_exists(path: &Path) -> Result<Option<Vec<u8>>, TrustError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(TrustError::Unreadable),
    }
}

/// `.claude.json.<command>-<id>.tmp` beside the file, so the rename stays on one disk.
fn temp_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    target.with_file_name(format!(
        ".{name}.{}-{}.tmp",
        naming::COMMAND_NAME,
        uuid::Uuid::now_v7().simple()
    ))
}

/// Writes a new file: `0600` on Unix, then the original file's permissions when there is one.
fn write_new(
    path: &Path,
    bytes: &[u8],
    permissions: Option<fs::Permissions>,
) -> Result<(), TrustError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path).map_err(|_| TrustError::Unwritable)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| match permissions {
            Some(p) => fs::set_permissions(path, p),
            None => Ok(()),
        });
    drop(file);
    if written.is_err() {
        let _ = fs::remove_file(path);
        return Err(TrustError::Unwritable);
    }
    Ok(())
}

/// The service's folder trust (PRJ-06, PRJ-08): the config file located once at service start, and the projects
/// whose trust couldn't be recorded (each project's latest outcome, in memory). The board's clones share it; without
/// a config file (the default, and in tests that don't pass a temporary one) nothing is read or written.
#[derive(Debug, Clone, Default)]
pub struct Trust {
    inner: Arc<TrustInner>,
}

#[derive(Debug, Default)]
struct TrustInner {
    config: OnceLock<ClaudeConfig>,
    /// Project key → why its trust couldn't be recorded.
    failed: Mutex<BTreeMap<String, String>>,
    /// Set once the startup pass is over.
    pass_done: watch::Sender<bool>,
}

/// A project whose folder trust couldn't be recorded: its warning banner (PRJ-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustFailure {
    pub project_key: String,
    /// A short sentence shown as it is (no final period).
    pub reason: String,
}

impl Trust {
    /// The config file the service's agents use. Set once, at service start; a second call changes nothing.
    pub fn set_config(&self, config: ClaudeConfig) {
        let _ = self.inner.config.set(config);
    }

    /// Records the trust for a project's repo, off the async threads. `None` without a config file. A `Failed`
    /// outcome lists the project until a later attempt records it.
    pub async fn record(&self, project_key: &str, repo_path: &str) -> Option<Outcome> {
        let config = self.inner.config.get()?.clone();
        let repo = PathBuf::from(repo_path);
        let outcome = tokio::task::spawn_blocking(move || config.record_trust(&repo))
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(project = project_key, error = %e, "recording Claude Code's trust stopped");
                Outcome::Failed {
                    reason: TrustError::Unwritable.reason().to_string(),
                }
            });
        let mut failed = self.inner.failed.lock().unwrap_or_else(|e| e.into_inner());
        match &outcome {
            Outcome::Failed { reason } => failed.insert(project_key.to_owned(), reason.clone()),
            Outcome::AlreadyTrusted | Outcome::Recorded => failed.remove(project_key),
        };
        Some(outcome)
    }

    /// The projects whose latest attempt failed, by key.
    pub fn failed(&self) -> Vec<TrustFailure> {
        self.inner
            .failed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(key, reason)| TrustFailure {
                project_key: key.clone(),
                reason: reason.clone(),
            })
            .collect()
    }

    /// The startup pass (PRJ-08): records the trust for every stored project, oldest first. A repo folder that is
    /// gone is skipped; the first pass that records anything sets the one-time note, unless it was ever dismissed.
    /// Never fails: every problem is an outcome or a log line. True when the notices changed (the note was set or a
    /// project failed).
    pub async fn startup_pass(&self, store: &Store) -> bool {
        let changed = self.pass(store).await;
        self.inner.pass_done.send_replace(true);
        changed
    }

    /// Resolves once the startup pass is over.
    pub async fn pass_finished(&self) {
        let mut done = self.inner.pass_done.subscribe();
        let _ = done.wait_for(|d| *d).await;
    }

    async fn pass(&self, store: &Store) -> bool {
        if self.inner.config.get().is_none() {
            tracing::debug!("no Claude Code config file: folder trust not checked");
            return false;
        }
        let projects = match store.projects().await {
            Ok(projects) => projects,
            Err(e) => {
                tracing::warn!(error = %e, "can't list the projects to record Claude Code's trust");
                return false;
            }
        };
        let mut recorded = false;
        for project in projects {
            let is_dir = tokio::fs::metadata(&project.repo_path)
                .await
                .is_ok_and(|m| m.is_dir());
            if !is_dir {
                tracing::info!(
                    project = %project.key,
                    repo = %project.repo_path,
                    "the project's folder is gone: Claude Code's trust not recorded"
                );
                continue;
            }
            let outcome = self.record(&project.key, &project.repo_path).await;
            recorded |= outcome == Some(Outcome::Recorded);
        }
        let note = recorded && set_note_pending(store).await;
        note || !self.failed().is_empty()
    }
}

/// Sets the one-time note unless it was ever dismissed; true when it is set.
async fn set_note_pending(store: &Store) -> bool {
    let result = async {
        if setting_true(store, TRUST_NOTE_DISMISSED).await? {
            return Ok(false);
        }
        store.set_setting(TRUST_NOTE_PENDING, "true").await?;
        Ok::<_, StoreError>(true)
    }
    .await;
    result.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "can't store the folder trust note");
        false
    })
}

/// Whether the one-time note shows: set by a startup pass, cleared for good by its dismissal.
pub async fn note_pending(store: &Store) -> Result<bool, StoreError> {
    setting_true(store, TRUST_NOTE_PENDING).await
}

/// The user dismissed the one-time note (`notices.dismiss_trust_note`): it never comes back.
pub async fn dismiss_note(store: &Store) -> Result<(), StoreError> {
    store.set_setting(TRUST_NOTE_DISMISSED, "true").await?;
    store.set_setting(TRUST_NOTE_PENDING, "false").await
}

async fn setting_true(store: &Store, key: &str) -> Result<bool, StoreError> {
    Ok(store.setting(key).await?.is_some_and(|v| v == "true"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const REPO: &str = "/Users/me/pocket-notes";

    fn config_in(dir: &Path) -> ClaudeConfig {
        ClaudeConfig::new(dir.join(".claude.json"), None)
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn write_json(path: &Path, value: &Value) {
        fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    /// Every file in `dir` that isn't `keep`: a temporary file left behind shows up here.
    fn others(dir: &Path, keep: &str) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != keep)
            .collect()
    }

    fn key() -> String {
        trust_key(Path::new(REPO))
    }

    // --- locate ---

    #[test]
    fn locate_uses_claude_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("claude");
        let home = tmp.path().join("home");
        let found = ClaudeConfig::locate(Some(dir.clone().into()), Some(home)).unwrap();
        assert_eq!(found.file, dir.join(".claude.json"));
    }

    #[test]
    fn locate_without_or_with_empty_claude_config_dir_uses_the_home_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().to_path_buf();
        for dir in [None, Some(OsString::new())] {
            let found = ClaudeConfig::locate(dir, Some(home.clone())).unwrap();
            // The home folder itself, not `~/.claude/`.
            assert_eq!(found.file, home.join(".claude.json"));
            assert_eq!(found.home, Some(paths::canonical(&home).unwrap()));
        }
    }

    #[test]
    fn locate_without_any_folder_finds_nothing() {
        assert_eq!(ClaudeConfig::locate(None, None), None);
    }

    #[test]
    fn a_non_empty_legacy_file_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("claude");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(".claude.json"), "{}").unwrap();
        fs::write(dir.join(".config.json"), "{}").unwrap();
        let found = ClaudeConfig::locate(Some(dir.clone().into()), None).unwrap();
        assert_eq!(found.file, dir.join(".config.json"));

        // Without CLAUDE_CONFIG_DIR, the legacy file is in `~/.claude`.
        let home = tmp.path().join("home");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::write(home.join(".claude").join(".config.json"), "{}").unwrap();
        let found = ClaudeConfig::locate(None, Some(home.clone())).unwrap();
        assert_eq!(found.file, home.join(".claude").join(".config.json"));
    }

    #[test]
    fn an_empty_or_absent_legacy_file_does_not_win() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let found = ClaudeConfig::locate(Some(dir.clone().into()), None).unwrap();
        assert_eq!(found.file, dir.join(".claude.json"));

        fs::write(dir.join(".claude.json"), "{}").unwrap();
        fs::write(dir.join(".config.json"), "").unwrap();
        let found = ClaudeConfig::locate(Some(dir.clone().into()), None).unwrap();
        assert_eq!(found.file, dir.join(".claude.json"));
    }

    #[test]
    fn an_empty_writable_legacy_file_wins_when_there_is_no_claude_json() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        fs::write(dir.join(".config.json"), "").unwrap();
        let found = ClaudeConfig::locate(Some(dir.clone().into()), None).unwrap();
        assert_eq!(found.file, dir.join(".config.json"));
    }

    // --- the key ---

    #[test]
    fn the_key_on_macos_and_linux() {
        // macOS: the caller has already resolved `/tmp` to `/private/tmp`.
        assert_eq!(
            key_text("/private/tmp/m19/pocket-notes", false),
            "/private/tmp/m19/pocket-notes"
        );
        assert_eq!(key_text("/home/me/shop-web/", false), "/home/me/shop-web");
        assert_eq!(key_text("/", false), "/");
    }

    #[test]
    fn the_key_on_windows() {
        assert_eq!(
            key_text(r"C:\Users\me\shop-web", true),
            "C:/Users/me/shop-web"
        );
        assert_eq!(
            key_text(r"\\?\C:\Users\me\shop-web", true),
            "C:/Users/me/shop-web"
        );
        assert_eq!(
            key_text(r"C:\Users\me\shop-web\", true),
            "C:/Users/me/shop-web"
        );
        assert_eq!(key_text(r"C:\", true), "C:/");
        assert_eq!(
            key_text(r"\\server\share\shop-web", true),
            "//server/share/shop-web"
        );
    }

    #[test]
    fn the_key_of_a_real_folder_on_this_platform() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = paths::canonical(tmp.path()).unwrap();
        let key = trust_key(&repo);
        assert!(!key.contains('\\') || !cfg!(windows));
        assert!(!key.ends_with('/'));
        assert!(!key.starts_with(r"\\?\") && !key.starts_with("//?/"));
    }

    // --- reading ---

    #[test]
    fn a_missing_file_is_not_trusted() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(config_in(tmp.path()).is_trusted(&key()), Ok(false));
    }

    #[test]
    fn a_file_that_is_not_json_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".claude.json"), "{ not json").unwrap();
        assert_eq!(
            config_in(tmp.path()).is_trusted(&key()),
            Err(TrustError::Unreadable)
        );
    }

    // --- recording ---

    #[test]
    fn a_missing_file_is_created_with_only_the_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        assert_eq!(config.record_trust(Path::new(REPO)), Outcome::Recorded);
        assert_eq!(
            read_json(&config.file),
            json!({ "projects": { key(): { "hasTrustDialogAccepted": true } } })
        );
        assert_eq!(config.is_trusted(&key()), Ok(true));
        assert!(others(tmp.path(), ".claude.json").is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&config.file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn everything_else_in_the_file_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        let before = json!({
            "numStartups": 42,
            "oauthAccount": { "emailAddress": "me@example.com", "nested": [1, 2.5, null, "x"] },
            "theme": "dark",
            "projects": {
                "/Users/me/other": { "hasTrustDialogAccepted": true, "allowedTools": ["Bash"] },
                key(): {
                    "allowedTools": [],
                    "mcpServers": { "s": { "command": "x" } },
                    "lastSessionId": "abc",
                    "lastCost": 0.12345678901234,
                },
            },
        });
        write_json(&config.file, &before);
        assert_eq!(config.record_trust(Path::new(REPO)), Outcome::Recorded);

        let mut expected = before.clone();
        expected["projects"][key()]["hasTrustDialogAccepted"] = json!(true);
        assert_eq!(read_json(&config.file), expected);
    }

    #[test]
    fn already_trusted_leaves_the_file_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        write_json(
            &config.file,
            &json!({ "projects": { key(): { "hasTrustDialogAccepted": true } } }),
        );
        let bytes = fs::read(&config.file).unwrap();
        let modified = fs::metadata(&config.file).unwrap().modified().unwrap();

        assert_eq!(
            config.record_trust(Path::new(REPO)),
            Outcome::AlreadyTrusted
        );
        assert_eq!(fs::read(&config.file).unwrap(), bytes);
        assert_eq!(
            fs::metadata(&config.file).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn false_is_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        write_json(
            &config.file,
            &json!({ "projects": { key(): { "hasTrustDialogAccepted": false } } }),
        );
        assert_eq!(config.record_trust(Path::new(REPO)), Outcome::Recorded);
        assert_eq!(config.is_trusted(&key()), Ok(true));
    }

    #[test]
    fn invalid_json_is_never_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        for text in ["{ \"projects\": ", "[1, 2]", "{ \"projects\": [] }"] {
            fs::write(&config.file, text).unwrap();
            assert_eq!(
                config.record_trust(Path::new(REPO)),
                Outcome::Failed {
                    reason: "Claude Code's config file couldn't be read".into()
                }
            );
            assert_eq!(fs::read(&config.file).unwrap(), text.as_bytes());
            assert!(others(tmp.path(), ".claude.json").is_empty());
        }
    }

    #[test]
    fn a_read_only_file_fails_and_leaves_nothing_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        write_json(&config.file, &json!({ "projects": {} }));
        let bytes = fs::read(&config.file).unwrap();
        let mut permissions = fs::metadata(&config.file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&config.file, permissions.clone()).unwrap();

        let outcome = config.record_trust(Path::new(REPO));
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&config.file, permissions).unwrap();

        assert_eq!(
            outcome,
            Outcome::Failed {
                reason: "Claude Code's config file couldn't be written".into()
            }
        );
        assert_eq!(fs::read(&config.file).unwrap(), bytes);
        assert!(others(tmp.path(), ".claude.json").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_folder_fails_and_leaves_nothing_behind() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("claude");
        fs::create_dir(&dir).unwrap();
        let config = config_in(&dir);
        write_json(&config.file, &json!({ "projects": {} }));
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

        let outcome = config.record_trust(Path::new(REPO));
        let left = others(&dir, ".claude.json");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(
            outcome,
            Outcome::Failed {
                reason: "Claude Code's config file couldn't be written".into()
            }
        );
        assert_eq!(read_json(&config.file), json!({ "projects": {} }));
        assert!(left.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn the_file_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        write_json(&config.file, &json!({}));
        fs::set_permissions(&config.file, fs::Permissions::from_mode(0o640)).unwrap();
        assert_eq!(config.record_trust(Path::new(REPO)), Outcome::Recorded);
        assert_eq!(
            fs::metadata(&config.file).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_file_is_written_where_the_link_points() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("dotfiles");
        fs::create_dir(&real).unwrap();
        write_json(&real.join("claude.json"), &json!({ "theme": "dark" }));
        let config = config_in(tmp.path());
        std::os::unix::fs::symlink(real.join("claude.json"), &config.file).unwrap();

        assert_eq!(config.record_trust(Path::new(REPO)), Outcome::Recorded);
        assert!(
            fs::symlink_metadata(&config.file)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            read_json(&real.join("claude.json"))["projects"][key()]["hasTrustDialogAccepted"],
            json!(true)
        );
    }

    #[test]
    fn the_home_folder_fails_without_touching_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let home = paths::canonical(tmp.path()).unwrap();
        let config = ClaudeConfig::new(home.join(".claude.json"), Some(home.clone()));
        assert_eq!(
            config.record_trust(&home),
            Outcome::Failed {
                reason: "Claude Code never keeps trust for your home folder".into()
            }
        );
        assert!(!config.file.exists());
    }

    #[test]
    fn a_change_between_the_read_and_the_rename_is_caught() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        write_json(&config.file, &json!({ "numStartups": 1 }));

        // A running `claude` writes once while Otaq is writing: Otaq tries again on the new content and keeps it.
        let file = config.file.clone();
        let mut writes = 0;
        let outcome = config.record_with(Path::new(REPO), &mut || {
            if writes == 0 {
                write_json(&file, &json!({ "numStartups": 2, "theme": "light" }));
            }
            writes += 1;
        });
        assert_eq!(outcome, Outcome::Recorded);
        assert_eq!(
            read_json(&config.file),
            json!({ "numStartups": 2, "theme": "light", "projects": { key(): { "hasTrustDialogAccepted": true } } })
        );
        assert!(others(tmp.path(), ".claude.json").is_empty());
    }

    #[test]
    fn a_file_that_keeps_changing_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let config = config_in(tmp.path());
        write_json(&config.file, &json!({ "numStartups": 0 }));

        let file = config.file.clone();
        let mut n = 0;
        let outcome = config.record_with(Path::new(REPO), &mut || {
            n += 1;
            write_json(&file, &json!({ "numStartups": n }));
        });
        assert_eq!(
            outcome,
            Outcome::Failed {
                reason: "Claude Code's config file kept changing while it was being written".into()
            }
        );
        assert_eq!(read_json(&config.file), json!({ "numStartups": ATTEMPTS }));
        assert!(others(tmp.path(), ".claude.json").is_empty());
    }

    #[test]
    fn the_outcome_is_plain_data() {
        assert_eq!(
            serde_json::to_value(Outcome::Recorded).unwrap(),
            json!({ "kind": "recorded" })
        );
        assert_eq!(
            serde_json::to_value(Outcome::AlreadyTrusted).unwrap(),
            json!({ "kind": "already_trusted" })
        );
        assert_eq!(
            serde_json::to_value(Outcome::Failed { reason: "r".into() }).unwrap(),
            json!({ "kind": "failed", "reason": "r" })
        );
    }
}
