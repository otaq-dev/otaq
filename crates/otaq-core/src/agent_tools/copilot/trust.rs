//! Copilot's one-time setup (ADP-08, PRJ-06, d0085; T-0353): folder trust in its `config.json` `trustedFolders`, its
//! only setup (hooks and skills come with every launch, `--plugin-dir`). Folder trust stays on disconnect, so removal
//! removes nothing.
//!
//! `$COPILOT_HOME/config.json` (else `~/.copilot/config.json`) is Copilot's own file: its machine-wide state (the
//! signed-in accounts, trusted folders, first launch…), which it rewrites itself. Checked with Copilot 1.0.98 in a
//! scratch `COPILOT_HOME` on 2026-10-18:
//! - it starts with two `//` comment lines ([`HEADER`]), then one JSON object;
//! - Copilot rewrites it through `config.json.tmp.<uuid>` and a rename, keeping the comment lines and keys it doesn't
//!   know; it took no lock file for that, and a `config.json.lock` beside it (empty, or held with `flock`/`lockf`)
//!   didn't make it wait;
//! - the keys are camelCase (`trustedFolders`, `loggedInUsers`, `lastLoggedInUser`); 0.0.x wrote the accounts in
//!   snake_case (`logged_in_users`), which 1.0.98 still reads.
//!
//! So Otaq changes only `trustedFolders`: it re-reads the file just before writing, keeps the comment lines and every
//! other key, adds the repo root when it isn't there, and replaces the file through a temporary file and a rename,
//! retried when the file changed in between. A file that doesn't parse is never overwritten. A `config.json.lock`
//! (what older versions left beside it) is respected all the same: a recent one is waited for. Copilot trusts a
//! trusted folder's subfolders and a worktree of a trusted repo (the hands-on check, row 5), so the repo root is
//! enough for every ticket's worktree.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Map, Value};

use super::{CopilotContext, SHORT_NAME};
use crate::agent_tools::{SetupEntry, SetupOutcome, SetupReport};
use crate::{naming, paths};

/// Copilot's state file in its home folder.
pub(super) const CONFIG_FILE: &str = "config.json";

/// The comment lines Copilot writes at the top of a new `config.json`.
const HEADER: &str =
    "// User settings belong in settings.json.\n// This file is managed automatically.\n";

/// The key Copilot's "Yes, and remember" writes.
const TRUSTED_FOLDERS: &str = "trustedFolders";

/// The lock file's suffix (`config.json.lock`).
const LOCK_SUFFIX: &str = ".lock";

/// How long a lock file is waited for, and how often it is looked at.
const LOCK_WAIT: Duration = Duration::from_secs(2);
const LOCK_POLL: Duration = Duration::from_millis(50);

/// A lock file older than this was left behind by a writer that stopped: it is ignored.
const LOCK_STALE: Duration = Duration::from_secs(30);

/// How many times a write is tried when another writer changes the file in between.
const ATTEMPTS: usize = 2;

/// Copilot's `config.json` in `home` (its home folder).
pub(super) fn config_file(home: &Path) -> PathBuf {
    home.join(CONFIG_FILE)
}

/// A `config.json` split into its leading comment lines and its object; `None` when the rest isn't a JSON object.
pub(super) fn parse(text: &str) -> Option<(String, Map<String, Value>)> {
    let mut header_end = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if !(trimmed.is_empty() || trimmed.starts_with("//")) {
            break;
        }
        header_end += line.len();
    }
    let (header, body) = text.split_at(header_end);
    match serde_json::from_str(body).ok()? {
        Value::Object(map) => Some((header.to_owned(), map)),
        _ => None,
    }
}

/// Why the trust couldn't be recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrustError {
    /// The file couldn't be read, isn't a JSON object after its comment lines, or its `trustedFolders` isn't a list.
    Unreadable,
    /// The file or its folder couldn't be written.
    Unwritable,
    /// A recent lock file stayed beside it.
    Locked,
    /// Another writer changed the file on every attempt.
    KeptChanging,
}

impl TrustError {
    /// The plain sentence the UI shows.
    fn reason(self) -> String {
        match self {
            Self::Unreadable => format!("{SHORT_NAME}'s config file couldn't be read"),
            Self::Unwritable => format!("{SHORT_NAME}'s config file couldn't be written"),
            Self::Locked => format!("{SHORT_NAME}'s config file stayed locked"),
            Self::KeptChanging => {
                format!("{SHORT_NAME}'s config file kept changing while it was being written")
            }
        }
    }
}

/// One write attempt's result.
enum Step {
    AlreadyTrusted,
    Written,
    /// The file changed between the read and the rename; nothing was written.
    Changed,
}

/// Records the trust for an added project's repo (as stored, from [`paths::canonical`]). `None` without Copilot's
/// home folder: Copilot never ran here (the tool's config can't be located), so nothing is created for it; it is
/// recorded when Copilot becomes Connected.
pub(super) fn record_project(context: &CopilotContext, repo: &Path) -> Option<SetupOutcome> {
    let home = context.copilot_home.as_deref()?;
    if !home.is_dir() {
        tracing::debug!(home = %home.display(), "no Copilot home folder: its folder trust isn't recorded");
        return None;
    }
    Some(record_with(&config_file(home), repo, &mut || {}))
}

/// [`record_project`] on `file`, with `before_rename` called between the read and the rename (a test seam).
fn record_with(file: &Path, repo: &Path, before_rename: &mut dyn FnMut()) -> SetupOutcome {
    let key = trust_key(repo);
    let outcome = match try_record(file, &key, before_rename) {
        Ok(outcome) => outcome,
        Err(error) => SetupOutcome::Failed {
            reason: error.reason(),
        },
    };
    let file = file.display();
    match &outcome {
        SetupOutcome::AlreadyTrusted => {}
        SetupOutcome::Recorded => tracing::info!(key, %file, "recorded Copilot's folder trust"),
        SetupOutcome::Failed { reason } => {
            tracing::warn!(key, %file, reason, "couldn't record Copilot's folder trust")
        }
    }
    outcome
}

fn try_record(
    file: &Path,
    key: &str,
    before_rename: &mut dyn FnMut(),
) -> Result<SetupOutcome, TrustError> {
    let target = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    for attempt in 0..ATTEMPTS {
        wait_for_lock(&target)?;
        match write_once(&target, key, before_rename)? {
            Step::AlreadyTrusted if attempt == 0 => return Ok(SetupOutcome::AlreadyTrusted),
            Step::AlreadyTrusted => return Ok(SetupOutcome::Recorded),
            Step::Changed => continue,
            // A running `copilot` may have written its older copy over ours: read once more.
            Step::Written if is_trusted(&target, key)? => return Ok(SetupOutcome::Recorded),
            Step::Written => continue,
        }
    }
    Err(TrustError::KeptChanging)
}

/// Waits up to [`LOCK_WAIT`] while a lock file younger than [`LOCK_STALE`] is beside `target`.
fn wait_for_lock(target: &Path) -> Result<(), TrustError> {
    let mut lock = target.as_os_str().to_owned();
    lock.push(LOCK_SUFFIX);
    let lock = PathBuf::from(lock);
    let held = || {
        fs::metadata(&lock)
            .and_then(|m| m.modified())
            .is_ok_and(|at| {
                SystemTime::now()
                    .duration_since(at)
                    .is_ok_and(|age| age < LOCK_STALE)
                    // A time ahead of the clock is a recent lock too.
                    || at > SystemTime::now()
            })
    };
    let waited = std::time::Instant::now();
    while held() {
        if waited.elapsed() >= LOCK_WAIT {
            return Err(TrustError::Locked);
        }
        std::thread::sleep(LOCK_POLL);
    }
    Ok(())
}

/// Whether the file at `target` trusts `key`. A missing file trusts nothing.
fn is_trusted(target: &Path, key: &str) -> Result<bool, TrustError> {
    let Some(bytes) = read_if_exists(target)? else {
        return Ok(false);
    };
    let (_, config) = parse_bytes(&bytes)?;
    Ok(trusted(&config, key))
}

/// Reads the file, adds the one folder, and replaces the file through a temporary file in the same folder.
fn write_once(
    target: &Path,
    key: &str,
    before_rename: &mut dyn FnMut(),
) -> Result<Step, TrustError> {
    let original = read_if_exists(target)?;
    let (header, mut config) = match &original {
        Some(bytes) => parse_bytes(bytes)?,
        None => (HEADER.to_owned(), Map::new()),
    };
    if trusted(&config, key) {
        return Ok(Step::AlreadyTrusted);
    }
    match config
        .entry(TRUSTED_FOLDERS)
        .or_insert_with(|| Value::Array(Vec::new()))
    {
        Value::Array(folders) => folders.push(Value::String(key.to_owned())),
        _ => return Err(TrustError::Unreadable),
    }
    let body =
        serde_json::to_string_pretty(&Value::Object(config)).map_err(|_| TrustError::Unwritable)?;
    let text = format!("{header}{body}\n");

    let permissions = match original {
        Some(_) => Some(
            fs::metadata(target)
                .map_err(|_| TrustError::Unreadable)?
                .permissions(),
        ),
        None => None,
    };
    // A read-only file stays as the user left it, even where its folder would allow a rename.
    if permissions.as_ref().is_some_and(|p| p.readonly()) {
        return Err(TrustError::Unwritable);
    }
    let temp = temp_path(target);
    let result = write_new(&temp, text.as_bytes(), permissions).and_then(|()| {
        before_rename();
        if read_if_exists(target)? != original {
            return Ok(Step::Changed);
        }
        fs::rename(&temp, target).map_err(|_| TrustError::Unwritable)?;
        Ok(Step::Written)
    });
    if !matches!(result, Ok(Step::Written)) {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn parse_bytes(bytes: &[u8]) -> Result<(String, Map<String, Value>), TrustError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(parse)
        .ok_or(TrustError::Unreadable)
}

/// Whether `trustedFolders` has `key` (a trailing separator aside).
fn trusted(config: &Map<String, Value>, key: &str) -> bool {
    config
        .get(TRUSTED_FOLDERS)
        .and_then(Value::as_array)
        .is_some_and(|folders| {
            folders
                .iter()
                .filter_map(Value::as_str)
                .any(|folder| trust_key(Path::new(folder)) == key)
        })
}

/// The folder as `trustedFolders` holds it: the absolute path with no trailing separator (a root keeps its own).
fn trust_key(repo: &Path) -> String {
    let mut key = paths::ordinary(repo).to_string_lossy().into_owned();
    while key.len() > 1 && (key.ends_with('/') || key.ends_with('\\')) && !key.ends_with(":\\") {
        key.pop();
    }
    key
}

fn read_if_exists(path: &Path) -> Result<Option<Vec<u8>>, TrustError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(TrustError::Unreadable),
    }
}

/// `.config.json.<command>-<id>.tmp` beside the file, so the rename stays on one disk.
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

/// Writes a new file: `0600` on Unix (as Copilot keeps it), then the original file's permissions when there is one.
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
    written.map_err(|_| {
        let _ = fs::remove_file(path);
        TrustError::Unwritable
    })
}

/// Nothing tool-wide: hooks and skills come with every launch (`--plugin-dir`), and folder trust per project.
pub(super) fn set_up_tool() -> SetupReport {
    SetupReport::default()
}

/// "trust for your projects", which stays on disconnect (d0086).
pub(super) fn setup_entries() -> Vec<SetupEntry> {
    vec![SetupEntry {
        text: "trust for your projects".to_owned(),
        removed_on_disconnect: false,
    }]
}

/// Copilot has no Otaq-only entries: folder trust stays.
pub(super) fn remove_setup() -> SetupReport {
    SetupReport::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::AgentTool;
    use crate::agent_tools::copilot::{Copilot, CopilotContext};
    use crate::agent_tools::test_support::context;

    /// A real-shaped `config.json` (`tests/fixtures/copilot/config/config.json`): the comment lines, the accounts,
    /// another trusted folder, keys Otaq doesn't know.
    fn fixture() -> String {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "tests",
            "fixtures",
            "copilot",
            "config",
            "config.json",
        ]
        .iter()
        .collect();
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// An adapter whose Copilot home is `home`.
    fn copilot(home: &Path) -> Copilot {
        let bin = home.join("bin");
        Copilot::with_context(CopilotContext {
            tool: context(&bin, Some(home)),
            copilot_home: Some(home.to_path_buf()),
        })
    }

    fn folders(text: &str) -> Vec<String> {
        let (_, config) = parse(text).unwrap();
        config[TRUSTED_FOLDERS]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap().to_owned())
            .collect()
    }

    /// Only the repo root is added: the comment lines, the accounts and every other key stay; a second time changes
    /// nothing; a trailing separator is the same folder.
    #[test]
    fn only_the_folder_is_added() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(CONFIG_FILE);
        fs::write(&file, fixture()).unwrap();
        let adapter = copilot(home.path());

        assert_eq!(
            adapter.record_project(Path::new("/Users/me/shop-web")),
            Some(SetupOutcome::Recorded)
        );
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.starts_with(HEADER), "{text}");
        assert_eq!(
            folders(&text),
            ["/Users/me/old-project", "/Users/me/shop-web"]
        );
        let (_, before) = parse(&fixture()).unwrap();
        let (_, after) = parse(&text).unwrap();
        for (key, value) in &before {
            if key != TRUSTED_FOLDERS {
                assert_eq!(after.get(key), Some(value), "{key}");
            }
        }
        assert_eq!(after.len(), before.len());

        assert_eq!(
            adapter.record_project(Path::new("/Users/me/shop-web/")),
            Some(SetupOutcome::AlreadyTrusted)
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), text);
        // No temporary file is left behind.
        let names: Vec<_> = fs::read_dir(home.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, [CONFIG_FILE]);
    }

    /// A file that doesn't parse, or whose `trustedFolders` isn't a list, is never overwritten.
    #[test]
    fn an_unparseable_file_is_left_alone() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(CONFIG_FILE);
        let adapter = copilot(home.path());
        for broken in [
            "// User settings belong in settings.json.\n{\"trustedFolders\": [",
            "[1, 2]",
            "{\"trustedFolders\": \"/Users/me\"}",
        ] {
            fs::write(&file, broken).unwrap();
            assert_eq!(
                adapter.record_project(Path::new("/Users/me/shop-web")),
                Some(SetupOutcome::Failed {
                    reason: "Copilot's config file couldn't be read".into()
                }),
                "{broken}"
            );
            assert_eq!(fs::read_to_string(&file).unwrap(), broken);
        }
    }

    /// Without Copilot's home folder nothing is created; a home folder without the file gets one with Copilot's
    /// comment lines.
    #[test]
    fn a_missing_home_is_left_alone_and_a_missing_file_is_created() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join(".copilot");
        let adapter = copilot(&home);
        assert_eq!(adapter.record_project(Path::new("/r/shop-web")), None);
        assert!(!home.exists());

        fs::create_dir(&home).unwrap();
        assert_eq!(
            adapter.record_project(Path::new("/r/shop-web")),
            Some(SetupOutcome::Recorded)
        );
        let text = fs::read_to_string(home.join(CONFIG_FILE)).unwrap();
        assert_eq!(
            text,
            format!("{HEADER}{{\n  \"trustedFolders\": [\n    \"/r/shop-web\"\n  ]\n}}\n")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(home.join(CONFIG_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // No home folder known at all (tests, parts of the service built on their own).
        let bin = root.path().join("bin");
        assert_eq!(
            Copilot::new(context(&bin, None)).record_project(Path::new("/r/shop-web")),
            None
        );
    }

    /// A recent lock file is waited for, and the write gives up while it stays; an old one is ignored.
    #[test]
    fn the_lock_is_respected() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(CONFIG_FILE);
        fs::write(&file, fixture()).unwrap();
        let lock = home.path().join("config.json.lock");
        fs::write(&lock, "").unwrap();
        let adapter = copilot(home.path());

        let started = std::time::Instant::now();
        assert_eq!(
            adapter.record_project(Path::new("/Users/me/shop-web")),
            Some(SetupOutcome::Failed {
                reason: "Copilot's config file stayed locked".into()
            })
        );
        assert!(started.elapsed() >= LOCK_WAIT);
        assert_eq!(fs::read_to_string(&file).unwrap(), fixture());

        // Released while Otaq waits: written.
        let release = {
            let lock = lock.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                fs::remove_file(lock).unwrap();
            })
        };
        assert_eq!(
            adapter.record_project(Path::new("/Users/me/shop-web")),
            Some(SetupOutcome::Recorded)
        );
        release.join().unwrap();

        // Left behind long ago: ignored.
        fs::write(&lock, "").unwrap();
        let old = SystemTime::now() - Duration::from_secs(120);
        fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(
            adapter.record_project(Path::new("/Users/me/other")),
            Some(SetupOutcome::Recorded)
        );
    }

    /// A writer that changes the file between Otaq's read and its rename: Otaq reads again and keeps that change.
    #[test]
    fn a_change_in_between_is_kept() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join(CONFIG_FILE);
        fs::write(&file, fixture()).unwrap();
        let mut once = true;
        let outcome = record_with(&file, Path::new("/Users/me/shop-web"), &mut || {
            if std::mem::take(&mut once) {
                let text = fixture().replace("\"banner\": \"always\"", "\"banner\": \"never\"");
                fs::write(&file, text).unwrap();
            }
        });
        assert_eq!(outcome, SetupOutcome::Recorded);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"banner\": \"never\""), "{text}");
        assert_eq!(
            folders(&text),
            ["/Users/me/old-project", "/Users/me/shop-web"]
        );

        let always = record_with(&file, Path::new("/Users/me/third"), &mut || {
            let now = fs::read_to_string(&file).unwrap();
            fs::write(&file, now.replace("\"banner\"", "\"banner\" ")).unwrap();
        });
        assert_eq!(
            always,
            SetupOutcome::Failed {
                reason: "Copilot's config file kept changing while it was being written".into()
            }
        );
    }

    /// The Agent tools page's line; it stays on disconnect, and removal and the tool-wide setup do nothing.
    #[tokio::test]
    async fn the_setup_is_folder_trust_only() {
        let home = tempfile::tempdir().unwrap();
        let adapter = copilot(home.path());
        assert_eq!(
            adapter.setup_entries(),
            [SetupEntry {
                text: "trust for your projects".into(),
                removed_on_disconnect: false
            }]
        );
        assert_eq!(adapter.set_up_tool().await, SetupReport::default());
        assert_eq!(adapter.remove_setup(), SetupReport::default());
    }

    #[test]
    fn comment_lines_are_split_from_the_object() {
        let (header, config) = parse(&fixture()).unwrap();
        assert_eq!(header, HEADER);
        assert_eq!(config["lastLoggedInUser"]["login"], "octo-me");
        assert!(parse("// only a comment\n").is_none());
        let (header, config) = parse("{\"a\": 1}").unwrap();
        assert_eq!((header.as_str(), config.len()), ("", 1));
    }
}
