//! Otaq's transparent status line (USG-04, SES-03; d0020, d0012).
//!
//! Agents launch with `--settings <file>`, a file Otaq writes in its data folder (never in the worktree or
//! the user's repository) whose only key is `statusLine`: `crewboard statusline`. That command records the plan
//! windows from the status-line JSON, then runs the **user's own** status-line command with the same input and
//! passes its output through unchanged, so the user's status line looks the same.
//!
//! Source: Claude Code docs (read 2026-10-10; Claude Code 2.1.296): "Settings files and precedence"
//! (code.claude.com/docs/en/settings): `--settings` takes a file or JSON and applies **above** the user, project and
//! local files and below managed settings, key by key, for that session only; and "Customize your status line"
//! (code.claude.com/docs/en/statusline): `statusLine` is `{"type": "command", "command": …, "padding"?,
//! "refreshInterval"?}`, the command runs in a shell with the JSON on stdin and its stdout shown.
//!
//! The user's status line is found the way Claude Code would without Otaq's file, read-only: the project
//! local file (`.claude/settings.local.json`; in a git worktree, the main checkout's), the shared project file
//! (`.claude/settings.json` in the session's folder), then the user's `settings.json`. A managed `statusLine`
//! overrides Otaq's too, so the user's managed one simply keeps running. The user's `padding` and
//! `refreshInterval` are copied into Otaq's file so their status line keeps its spacing and timer.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::data_dir::DataDir;
use crate::domain::TicketKey;
use crate::naming;

/// The `crewboard` subcommand Claude Code runs as the status line.
pub const SUBCOMMAND: &str = "statusline";

/// Set while the user's command runs, so a status line that calls Otaq's again prints nothing instead of
/// looping: `CREWBOARD_STATUSLINE`.
pub fn running_env_var() -> String {
    naming::env_var("STATUSLINE")
}

/// The command line Claude Code runs: `'<bin>/crewboard' statusline`, quoted for the shell. On Windows Claude
/// Code runs it with Git Bash, so the program is `crewboard.exe` with forward slashes (`'C:/…/crewboard.exe'`).
pub fn command_line(bin_dir: &Path) -> String {
    let program = bin_dir.join(format!(
        "{}{}",
        naming::COMMAND_NAME,
        std::env::consts::EXE_SUFFIX
    ));
    let mut program = program.display().to_string();
    if cfg!(windows) {
        program = program.replace('\\', "/");
    }
    format!("{} {SUBCOMMAND}", shell_quote(&program))
}

/// `'…'` with any `'` inside escaped, for `sh`.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The settings files that could hold the user's `statusLine` for a session in `cwd`, highest first.
pub fn candidate_files(claude_dir: Option<&Path>, cwd: &Path) -> Vec<PathBuf> {
    let local_root = main_checkout(cwd).unwrap_or_else(|| cwd.to_owned());
    let mut files = vec![
        local_root.join(".claude").join("settings.local.json"),
        cwd.join(".claude").join("settings.json"),
    ];
    files.extend(claude_dir.map(|d| d.join("settings.json")));
    files
}

/// The user's own `statusLine` (a `command` one) for a session in `cwd`. Only reads.
pub fn user_status_line(claude_dir: Option<&Path>, cwd: &Path) -> Option<Value> {
    candidate_files(claude_dir, cwd).iter().find_map(|file| {
        let text = std::fs::read_to_string(file).ok()?;
        let settings: Value = serde_json::from_str(&text).ok()?;
        let line = settings.get("statusLine")?;
        let is_command = line.get("type").and_then(Value::as_str) == Some("command")
            && line
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|c| !c.trim().is_empty());
        is_command.then(|| line.clone())
    })
}

/// The user's status-line command, if they have one.
pub fn user_command(claude_dir: Option<&Path>, cwd: &Path) -> Option<String> {
    user_status_line(claude_dir, cwd)?
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Otaq's settings: only `statusLine`, running `command`, with the user's `padding` and `refreshInterval`.
pub fn settings_json(user: Option<&Value>, command: &str) -> Value {
    let mut line = json!({ "type": "command", "command": command });
    for key in ["padding", "refreshInterval"] {
        if let Some(value) = user.and_then(|u| u.get(key)) {
            line[key] = value.clone();
        }
    }
    json!({ "statusLine": line })
}

/// Where a ticket's settings file lives: `<data>/claude/<KEY>.settings.json`.
pub fn settings_path(data: &DataDir, ticket: &TicketKey) -> PathBuf {
    data.root()
        .join("claude")
        .join(format!("{ticket}.settings.json"))
}

/// Where a project's Project agents' settings file lives: `<data>/claude/<PROJECT>.project-agent.settings.json`
/// (they all run in the project's folder, so they share it).
pub fn project_agent_settings_path(data: &DataDir, project: &str) -> PathBuf {
    data.root()
        .join("claude")
        .join(format!("{project}.project-agent.settings.json"))
}

/// Writes the ticket's settings file for a session in `cwd` and returns its path.
pub fn write_settings(
    data: &DataDir,
    ticket: &TicketKey,
    cwd: &Path,
    claude_dir: Option<&Path>,
    bin_dir: &Path,
) -> io::Result<PathBuf> {
    write_settings_at(settings_path(data, ticket), cwd, claude_dir, bin_dir)
}

/// Writes the settings file at `path` for a session in `cwd` and returns the path.
pub fn write_settings_at(
    path: PathBuf,
    cwd: &Path,
    claude_dir: Option<&Path>,
    bin_dir: &Path,
) -> io::Result<PathBuf> {
    if let Some(dir) = path.parent() {
        DataDir::ensure_dir(dir)?;
    }
    let user = user_status_line(claude_dir, cwd);
    let settings = settings_json(user.as_ref(), &command_line(bin_dir));
    std::fs::write(&path, serde_json::to_vec_pretty(&settings)?)?;
    Ok(path)
}

/// The main checkout of a git worktree in `cwd` (its `.git` file says `gitdir: <main>/.git/worktrees/<name>`).
fn main_checkout(cwd: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(cwd.join(".git")).ok()?;
    let gitdir = PathBuf::from(text.strip_prefix("gitdir:")?.trim());
    let worktrees = gitdir.parent()?;
    if worktrees.file_name()? != "worktrees" {
        return None;
    }
    Some(worktrees.parent()?.parent()?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_is_quoted_for_the_shell() {
        assert_eq!(
            command_line(Path::new("/opt/My Apps/bin")),
            format!(
                "'/opt/My Apps/bin/{}{}' {SUBCOMMAND}",
                naming::COMMAND_NAME,
                std::env::consts::EXE_SUFFIX
            )
        );
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn the_settings_hold_only_the_status_line_with_the_users_spacing() {
        let user = json!({"type": "command", "command": "~/.claude/statusline.sh", "padding": 2,
                          "refreshInterval": 5});
        assert_eq!(
            settings_json(Some(&user), "cb statusline"),
            json!({"statusLine": {"type": "command", "command": "cb statusline", "padding": 2,
                                  "refreshInterval": 5}})
        );
        assert_eq!(
            settings_json(None, "cb statusline"),
            json!({"statusLine": {"type": "command", "command": "cb statusline"}})
        );
    }

    #[test]
    fn the_users_status_line_follows_claude_codes_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let user_dir = dir.path().join("claude");
        let main = dir.path().join("repo");
        let wt = dir.path().join("wt");
        for d in [&user_dir, &main.join(".claude"), &wt.join(".claude")] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::create_dir_all(main.join(".git").join("worktrees").join("SHOP-1")).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", main.join(".git/worktrees/SHOP-1").display()),
        )
        .unwrap();
        let line = |c: &str| json!({"statusLine": {"type": "command", "command": c}}).to_string();
        assert_eq!(user_command(Some(&user_dir), &wt), None);
        std::fs::write(user_dir.join("settings.json"), line("user")).unwrap();
        assert_eq!(user_command(Some(&user_dir), &wt).as_deref(), Some("user"));
        std::fs::write(wt.join(".claude/settings.json"), line("project")).unwrap();
        assert_eq!(
            user_command(Some(&user_dir), &wt).as_deref(),
            Some("project")
        );
        std::fs::write(main.join(".claude/settings.local.json"), line("local")).unwrap();
        assert_eq!(user_command(Some(&user_dir), &wt).as_deref(), Some("local"));
        // Not a command status line, or unreadable: the next file decides.
        std::fs::write(main.join(".claude/settings.local.json"), "{not json").unwrap();
        assert_eq!(
            user_command(Some(&user_dir), &wt).as_deref(),
            Some("project")
        );
    }
}
