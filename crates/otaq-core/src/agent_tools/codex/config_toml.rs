//! Otaq's own entries in Codex's `config.toml` (T-0351): re-read before writing, keep everything else, atomic
//! replace, never overwrite a file it can't parse (d0085).
//!
//! The file is the user's and Codex's: their model, approval policy, other projects, other tools' hook trust and
//! comments. Otaq edits it with `toml_edit`, which keeps every other line, comment and order as it was, and changes
//! only its own entries: `[projects."<repo>"] trust_level` (folder trust) and, on disconnect, the
//! `[hooks.state."<Otaq's hook>"]` tables and its plugin's `[plugins."…"]` / `[marketplaces.…]` tables when Codex
//! itself can't remove them. The new text goes through a temporary file in the same folder and a rename, and only
//! when the file is still as it was read; a file that doesn't parse as TOML is never written.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Table, value};

use super::DISPLAY_NAME;
use crate::naming;

/// Codex's config file in its home folder.
pub(super) const FILE: &str = "config.toml";

/// The value Codex's "Trust this folder?" screen writes.
const TRUSTED: &str = "trusted";

/// How many times a write is tried when another writer changes the file in between.
const ATTEMPTS: usize = 2;

/// Why the file couldn't be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConfigError {
    /// It couldn't be read, isn't TOML, or an entry Otaq edits isn't shaped as Codex writes it.
    Unreadable,
    /// It or its folder couldn't be written.
    Unwritable,
    /// Another writer changed it on every attempt.
    KeptChanging,
}

impl ConfigError {
    /// The plain sentence the page shows (no final period).
    pub(super) fn reason(self) -> String {
        match self {
            Self::Unreadable => format!("{DISPLAY_NAME}'s config file couldn't be read"),
            Self::Unwritable => format!("{DISPLAY_NAME}'s config file couldn't be written"),
            Self::KeptChanging => {
                format!("{DISPLAY_NAME}'s config file kept changing while it was being written")
            }
        }
    }
}

/// `config.toml` in `home`.
pub(super) fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

/// The file as a document; `None` when there is none yet.
pub(super) fn read(file: &Path) -> Result<Option<DocumentMut>, ConfigError> {
    match read_bytes(file)? {
        Some(bytes) => parse(&bytes).map(Some),
        None => Ok(None),
    }
}

/// Reads the file, applies `change` (true when it changed something), and replaces the file when it did. True
/// when the file was written. Nothing is written when `change` fails or changes nothing.
pub(super) fn edit(
    file: &Path,
    change: &mut dyn FnMut(&mut DocumentMut) -> Result<bool, ConfigError>,
) -> Result<bool, ConfigError> {
    edit_with(file, change, &mut || {})
}

/// [`edit`] with `before_rename` called between the read and the rename (a test seam).
fn edit_with(
    file: &Path,
    change: &mut dyn FnMut(&mut DocumentMut) -> Result<bool, ConfigError>,
    before_rename: &mut dyn FnMut(),
) -> Result<bool, ConfigError> {
    let target = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    for _ in 0..ATTEMPTS {
        let original = read_bytes(&target)?;
        let mut doc = match &original {
            Some(bytes) => parse(bytes)?,
            None => DocumentMut::new(),
        };
        if !change(&mut doc)? {
            return Ok(false);
        }
        if replace(
            &target,
            &original,
            doc.to_string().as_bytes(),
            before_rename,
        )? {
            return Ok(true);
        }
    }
    Err(ConfigError::KeptChanging)
}

fn parse(bytes: &[u8]) -> Result<DocumentMut, ConfigError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse::<DocumentMut>().ok())
        .ok_or(ConfigError::Unreadable)
}

// ---------------------------------------------------------------------------------------------------------------
// Folder trust

/// The `projects` key Codex uses for a repo: its path as Otaq stores it (links already resolved), with no trailing
/// separator.
pub(super) fn project_key(repo: &Path) -> String {
    let mut key = crate::paths::ordinary(repo).to_string_lossy().into_owned();
    while key.len() > 1 && key.ends_with('/') {
        key.pop();
    }
    key
}

/// Whether `[projects."<key>"] trust_level = "trusted"`.
pub(super) fn project_trusted(doc: &DocumentMut, key: &str) -> bool {
    doc.get("projects")
        .and_then(|projects| projects.get(key))
        .and_then(|project| project.get("trust_level"))
        .and_then(Item::as_str)
        == Some(TRUSTED)
}

/// Sets `[projects."<key>"] trust_level = "trusted"`, keeping the entry's other keys. A `projects` or an entry that
/// isn't a table is left alone (an error).
pub(super) fn trust_project(doc: &mut DocumentMut, key: &str) -> Result<(), ConfigError> {
    let projects = doc
        .as_table_mut()
        .entry("projects")
        .or_insert_with(implicit_table)
        .as_table_like_mut()
        .ok_or(ConfigError::Unreadable)?;
    if projects.get(key).is_none() {
        projects.insert(key, Item::Table(Table::new()));
    }
    projects
        .get_mut(key)
        .and_then(Item::as_table_like_mut)
        .ok_or(ConfigError::Unreadable)?
        .insert("trust_level", value(TRUSTED));
    Ok(())
}

/// A table that prints only through its children (`[projects."/a"]`, never a bare `[projects]`).
fn implicit_table() -> Item {
    let mut table = Table::new();
    table.set_implicit(true);
    Item::Table(table)
}

// ---------------------------------------------------------------------------------------------------------------
// Otaq's plugin and its hooks' trust

/// The keys under `[hooks.state]` that start with `prefix` and have a `trusted_hash`.
pub(super) fn trusted_hooks(doc: &DocumentMut, prefix: &str) -> Vec<String> {
    let Some(state) = doc
        .get("hooks")
        .and_then(|hooks| hooks.get("state"))
        .and_then(Item::as_table_like)
    else {
        return Vec::new();
    };
    state
        .iter()
        .filter(|(key, entry)| {
            key.starts_with(prefix)
                && entry
                    .get("trusted_hash")
                    .and_then(Item::as_str)
                    .is_some_and(|hash| !hash.is_empty())
        })
        .map(|(key, _)| key.to_owned())
        .collect()
}

/// Removes the `[hooks.state]` entries whose key starts with `prefix`; true when there were any.
pub(super) fn remove_hooks(doc: &mut DocumentMut, prefix: &str) -> bool {
    let Some(state) = doc
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("state"))
        .and_then(Item::as_table_like_mut)
    else {
        return false;
    };
    let keys: Vec<String> = state
        .iter()
        .map(|(key, _)| key.to_owned())
        .filter(|key| key.starts_with(prefix))
        .collect();
    for key in &keys {
        state.remove(key);
    }
    !keys.is_empty()
}

/// `[marketplaces.<name>] source`: the folder a local marketplace was added from.
pub(super) fn marketplace_source(doc: &DocumentMut, name: &str) -> Option<String> {
    doc.get("marketplaces")?
        .get(name)?
        .get("source")?
        .as_str()
        .map(str::to_owned)
}

/// Whether `[plugins."<id>"]` is there (`codex plugin add` writes it, with `enabled = true`).
pub(super) fn plugin_installed(doc: &DocumentMut, id: &str) -> bool {
    doc.get("plugins")
        .and_then(|plugins| plugins.get(id))
        .is_some_and(|plugin| plugin.get("enabled").and_then(Item::as_bool) != Some(false))
}

/// Removes `[plugins."<id>"]` and `[marketplaces.<name>]`; true when either was there.
pub(super) fn remove_plugin(doc: &mut DocumentMut, id: &str, marketplace: &str) -> bool {
    let mut removed = false;
    for (table, key) in [("plugins", id), ("marketplaces", marketplace)] {
        if let Some(entries) = doc.get_mut(table).and_then(Item::as_table_like_mut) {
            removed |= entries.remove(key).is_some();
        }
    }
    removed
}

// ---------------------------------------------------------------------------------------------------------------
// The user's approval settings (read only)

/// What the user's config says about approvals, top level or in the profile it selects (`profile = "<name>"`,
/// `[profiles.<name>]`): `approval_policy` (`untrusted`, `on-request`, `never`, or a `granular` table) and
/// `approvals_reviewer` (`user`, `auto_review`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Approvals {
    /// The policy as text; a `granular` table is `"granular"`.
    pub policy: Option<String>,
    pub reviewer: Option<String>,
}

pub(super) fn approvals(doc: &DocumentMut) -> Approvals {
    let read = |table: &Item| Approvals {
        policy: table.get("approval_policy").and_then(|policy| {
            policy
                .as_str()
                .map(str::to_owned)
                .or_else(|| policy.get("granular").map(|_| "granular".to_owned()))
        }),
        reviewer: table
            .get("approvals_reviewer")
            .and_then(Item::as_str)
            .map(str::to_owned),
    };
    let top = read(doc.as_item());
    let profile = doc
        .get("profile")
        .and_then(Item::as_str)
        .and_then(|name| doc.get("profiles")?.get(name))
        .map(read)
        .unwrap_or_default();
    Approvals {
        policy: profile.policy.or(top.policy),
        reviewer: profile.reviewer.or(top.reviewer),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Files

pub(super) fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, ConfigError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ConfigError::Unreadable),
    }
}

/// Replaces `target` with `bytes` through a temporary file beside it, when it still holds `original`; false when
/// it changed in between (nothing written). A read-only file stays as the user left it.
pub(super) fn replace(
    target: &Path,
    original: &Option<Vec<u8>>,
    bytes: &[u8],
    before_rename: &mut dyn FnMut(),
) -> Result<bool, ConfigError> {
    let permissions = match original {
        Some(_) => Some(
            fs::metadata(target)
                .map_err(|_| ConfigError::Unreadable)?
                .permissions(),
        ),
        None => None,
    };
    if permissions.as_ref().is_some_and(|p| p.readonly()) {
        return Err(ConfigError::Unwritable);
    }
    let temp = temp_path(target);
    let result = write_new(&temp, bytes, permissions).and_then(|()| {
        before_rename();
        if read_bytes(target)? != *original {
            return Ok(false);
        }
        fs::rename(&temp, target).map_err(|_| ConfigError::Unwritable)?;
        Ok(true)
    });
    if !matches!(result, Ok(true)) {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// `.config.toml.<command>-<id>.tmp` beside the file, so the rename stays on one disk.
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
) -> Result<(), ConfigError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path).map_err(|_| ConfigError::Unwritable)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| match permissions {
            Some(p) => fs::set_permissions(path, p),
            None => Ok(()),
        });
    drop(file);
    written.map_err(|_| ConfigError::Unwritable)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A user's config as Codex and its user leave it: comments, the pinned model, the approval policy, another
    /// project, another hook's trust, a profile.
    pub(in crate::agent_tools::codex) const USER_CONFIG: &str = r#"# My Codex settings
model = "gpt-5.6-terra"   # pinned: don't upgrade me
approval_policy = "on-request"

[tui]
screen_reader_detection_done = true

# work laptop
[projects."/Users/me/other"]
trust_level = "trusted"
note = "keep"

[projects."/Users/me/untrusted"]
trust_level = "untrusted"

[hooks.state."/Users/me/.codex/hooks.json:stop:0:0"]
trusted_hash = "sha256:1111"

[profiles.fast]
approval_policy = "never"
"#;

    /// [`USER_CONFIG`] with `/tmp/m23/shop-web` trusted: the new table goes after the other projects.
    pub(in crate::agent_tools::codex) fn with_shop_web() -> String {
        USER_CONFIG.replace(
            "[hooks.state.",
            "[projects.\"/tmp/m23/shop-web\"]\ntrust_level = \"trusted\"\n\n[hooks.state.",
        )
    }

    fn doc(text: &str) -> DocumentMut {
        text.parse().unwrap()
    }

    #[test]
    fn a_new_project_goes_after_the_others_and_nothing_else_changes() {
        let mut d = doc(USER_CONFIG);
        assert!(project_trusted(&d, "/Users/me/other"));
        assert!(!project_trusted(&d, "/Users/me/untrusted"));
        assert!(!project_trusted(&d, "/tmp/m23/shop-web"));
        trust_project(&mut d, "/tmp/m23/shop-web").unwrap();
        assert_eq!(d.to_string(), with_shop_web());
        assert!(project_trusted(&d, "/tmp/m23/shop-web"));
    }

    #[test]
    fn an_existing_entry_keeps_its_place_and_other_keys() {
        let mut d = doc(USER_CONFIG);
        trust_project(&mut d, "/Users/me/untrusted").unwrap();
        assert_eq!(
            d.to_string(),
            USER_CONFIG.replace(
                "[projects.\"/Users/me/untrusted\"]\ntrust_level = \"untrusted\"",
                "[projects.\"/Users/me/untrusted\"]\ntrust_level = \"trusted\""
            )
        );
    }

    #[test]
    fn an_empty_file_gets_one_table() {
        let mut d = DocumentMut::new();
        trust_project(&mut d, "/r").unwrap();
        assert_eq!(
            d.to_string(),
            "[projects.\"/r\"]\ntrust_level = \"trusted\"\n"
        );
    }

    #[test]
    fn projects_of_another_shape_are_left_alone() {
        let mut d = doc("projects = 3\n");
        assert_eq!(trust_project(&mut d, "/r"), Err(ConfigError::Unreadable));
        let mut d = doc("[projects]\n\"/r\" = \"yes\"\n");
        assert_eq!(trust_project(&mut d, "/r"), Err(ConfigError::Unreadable));
    }

    #[test]
    fn keys_are_the_path_without_a_trailing_separator() {
        assert_eq!(
            project_key(Path::new("/tmp/m23/shop-web/")),
            "/tmp/m23/shop-web"
        );
        assert_eq!(project_key(Path::new("/")), "/");
    }

    #[test]
    fn only_hooks_with_the_prefix_are_found_and_removed() {
        let prefix = format!("{}@x:", naming::PLUGIN_NAMESPACE);
        let ours = format!("{prefix}plugin.json#hooks[0]:stop:0:0");
        let text =
            format!("{USER_CONFIG}\n[hooks.state.\"{ours}\"]\ntrusted_hash = \"sha256:2222\"\n");
        let mut d = doc(&text);
        assert_eq!(trusted_hooks(&d, &prefix), [ours]);
        assert!(remove_hooks(&mut d, &prefix));
        assert_eq!(d.to_string(), USER_CONFIG);
        assert!(!remove_hooks(&mut d, &prefix));
        assert!(trusted_hooks(&d, &prefix).is_empty());
    }

    #[test]
    fn the_plugin_tables() {
        let text = format!(
            "{USER_CONFIG}\n[marketplaces.m]\nsource_type = \"local\"\nsource = \"/opt/p\"\n\n[plugins.\"p@m\"]\nenabled = true\n"
        );
        let mut d = doc(&text);
        assert_eq!(marketplace_source(&d, "m").as_deref(), Some("/opt/p"));
        assert!(plugin_installed(&d, "p@m"));
        assert!(remove_plugin(&mut d, "p@m", "m"));
        assert_eq!(d.to_string(), USER_CONFIG);
        assert_eq!(marketplace_source(&d, "m"), None);
        assert!(!plugin_installed(&d, "p@m"));
        assert!(!remove_plugin(&mut d, "p@m", "m"));
        assert!(!plugin_installed(
            &doc("[plugins.\"p@m\"]\nenabled = false\n"),
            "p@m"
        ));
    }

    #[test]
    fn approvals_top_level_then_the_selected_profile() {
        assert_eq!(
            approvals(&doc(USER_CONFIG)),
            Approvals {
                policy: Some("on-request".into()),
                reviewer: None
            }
        );
        let fast = format!("profile = \"fast\"\n{USER_CONFIG}");
        assert_eq!(approvals(&doc(&fast)).policy.as_deref(), Some("never"));
        assert_eq!(approvals(&DocumentMut::new()), Approvals::default());
        let granular = "approval_policy = { granular = { mcp_elicitations = true, rules = true, sandbox_approval = false } }\napprovals_reviewer = \"auto_review\"\n";
        assert_eq!(
            approvals(&doc(granular)),
            Approvals {
                policy: Some("granular".into()),
                reviewer: Some("auto_review".into())
            }
        );
    }

    #[test]
    fn an_edit_keeps_the_file_byte_for_byte_but_its_own_lines() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        fs::write(&file, USER_CONFIG).unwrap();
        let written = edit(&file, &mut |d| {
            trust_project(d, "/tmp/m23/shop-web")?;
            Ok(true)
        })
        .unwrap();
        assert!(written);
        assert_eq!(fs::read_to_string(&file).unwrap(), with_shop_web());
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no temporary file left"
        );
        // Nothing to change: not written.
        assert!(!edit(&file, &mut |_| Ok(false)).unwrap());
    }

    #[test]
    fn a_file_that_does_not_parse_is_never_written() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        let broken = "model = \"gpt-5.6-terra\"\n[projects.\"/a\"\ntrust_level = \n";
        fs::write(&file, broken).unwrap();
        let mut called = false;
        assert_eq!(
            edit(&file, &mut |_| {
                called = true;
                Ok(true)
            }),
            Err(ConfigError::Unreadable)
        );
        assert!(!called);
        assert_eq!(fs::read_to_string(&file).unwrap(), broken);
        assert_eq!(read(&file).unwrap_err(), ConfigError::Unreadable);
    }

    #[test]
    fn a_file_changed_during_the_write_is_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        fs::write(&file, USER_CONFIG).unwrap();
        let mut writes = 0;
        let result = edit_with(
            &file,
            &mut |d| {
                trust_project(d, "/r")?;
                Ok(true)
            },
            &mut || {
                writes += 1;
                if writes == 1 {
                    // Codex saves the user's /model pick meanwhile.
                    let text = fs::read_to_string(&file).unwrap();
                    fs::write(&file, text.replace("gpt-5.6-terra", "gpt-6-sol")).unwrap();
                }
            },
        );
        assert_eq!(result, Ok(true));
        let after = fs::read_to_string(&file).unwrap();
        assert!(after.contains("model = \"gpt-6-sol\""), "{after}");
        assert!(after.contains("[projects.\"/r\"]"));
    }

    #[test]
    fn a_file_that_keeps_changing_is_given_up() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        fs::write(&file, USER_CONFIG).unwrap();
        let mut n = 0;
        let result = edit_with(
            &file,
            &mut |d| {
                trust_project(d, "/r")?;
                Ok(true)
            },
            &mut || {
                n += 1;
                fs::write(&file, format!("{USER_CONFIG}# {n}\n")).unwrap();
            },
        );
        assert_eq!(result, Err(ConfigError::KeptChanging));
        assert!(!fs::read_to_string(&file).unwrap().contains("\"/r\""));
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_file_stays_as_it_is() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        fs::write(&file, USER_CONFIG).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            edit(&file, &mut |d| {
                trust_project(d, "/r")?;
                Ok(true)
            }),
            Err(ConfigError::Unwritable)
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), USER_CONFIG);
    }
}
