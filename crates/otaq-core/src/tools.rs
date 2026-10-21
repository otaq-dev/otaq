//! The first-run tools checklist (SET-02, d0021, d0017) and the permission-mode warning (d0011, d0016).
//!
//! `git` is required, and **at least one agent tool connected** (SET-02 `[M23]`, d0086: Claude Code, Codex,
//! Copilot or OpenCode, from [`crate::agent_tools::connected::ConnectedTools`], which the service adds to the
//! report); `gh` and `az` (with the `azure-devops` extension) are optional and only matter for opening PRs. `claude`
//! is no longer a row: it is an agent tool like the others, detected through its adapter. Every tool is looked up on `PATH` and asked for its version, for display only:
//! Otaq never installs, updates or version-gates a tool (PLT-03). A missing tool comes with the command
//! that installs it on the current OS, for the user to copy.
//!
//! The permission-mode check reads the user's Claude Code settings **read-only** and reports whether new sessions
//! will stop on permission prompts. Source: Claude Code docs, "Choose a permission mode → Which mode a session
//! starts in" and "Deploy managed settings" (code.claude.com/docs/en/permission-modes and /managed-settings,
//! read 2026-10-09):
//! - the starting mode is `permissions.defaultMode` from the settings files (managed settings win over the
//!   user's `~/.claude/settings.json`), else a built-in default that depends on the Claude Code version and plan;
//! - modes `default` (Manual; alias `manual`), `acceptEdits` and `plan` prompt; `auto`, `dontAsk` and
//!   `bypassPermissions` don't;
//! - `disableAutoMode: "disable"` in any settings file turns `auto` (and the built-in default) into Manual;
//! - managed settings live in `/Library/Application Support/ClaudeCode/` on macOS, `/etc/claude-code/` on Linux
//!   and `C:\Program Files\ClaudeCode\` on Windows (`managed-settings.json` plus `managed-settings.d/*.json`);
//!   `CLAUDE_CONFIG_DIR` moves `~/.claude`.
//!
//! Anything unreadable, an unknown mode, or no mode set at all (the built-in default can't be known without a
//! version check) gives [`PermissionVerdict::Unknown`], which shows no warning. Project settings are not read:
//! the check runs before any project exists. MDM profiles and server-managed settings are out of reach.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How long one tool may take to answer (`az` is slow).
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Git,
    Claude,
    Gh,
    Az,
    /// GitLab's CLI: only Flow onboarding asks whether it is signed in (ONB-03, d0108); it is no checklist row.
    Glab,
}

impl Tool {
    /// The checklist's rows. [`Tool::Claude`] isn't one: its adapter's detect checks it ([`check_one`]); nor is
    /// [`Tool::Glab`], which only Flow onboarding checks (ONB-03).
    pub const ALL: [Tool; 3] = [Tool::Git, Tool::Gh, Tool::Az];

    /// The executable's name.
    pub fn command(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Claude => "claude",
            Self::Gh => "gh",
            Self::Az => "az",
            Self::Glab => "glab",
        }
    }

    pub fn required(self) -> bool {
        matches!(self, Self::Git)
    }

    /// The command that installs the tool on this OS, shown for copying (SET-02). Otaq never runs it.
    pub fn install_command(self) -> &'static str {
        self.install_command_on(Os::current())
    }

    /// The install command on `os`: Homebrew on macOS, winget on Windows (frame *14 → gh and az missing ·
    /// non-blocking (Windows)*), apt on Linux (Debian and Ubuntu; a hint, other distributions use their own).
    pub fn install_command_on(self, os: Os) -> &'static str {
        match (os, self) {
            (_, Self::Claude) => "npm install -g @anthropic-ai/claude-code",
            (Os::Mac, Self::Git) => "xcode-select --install",
            (Os::Mac, Self::Gh) => "brew install gh",
            (Os::Mac, Self::Az) => "brew install azure-cli && az extension add --name azure-devops",
            (Os::Mac, Self::Glab) => "brew install glab",
            (Os::Windows, Self::Git) => "winget install Git.Git",
            (Os::Windows, Self::Gh) => "winget install GitHub.cli",
            (Os::Windows, Self::Glab) => "winget install GLab.GLab",
            (Os::Windows, Self::Az) => {
                "winget install Microsoft.AzureCLI  &&  az extension add --name azure-devops"
            }
            (Os::Linux, Self::Git) => "sudo apt install git",
            (Os::Linux, Self::Gh) => "sudo apt install gh",
            (Os::Linux, Self::Glab) => "sudo apt install glab",
            (Os::Linux, Self::Az) => {
                "curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash && az extension add --name azure-devops"
            }
        }
    }
}

/// The operating systems with their own install commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// One row of the checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    pub tool: Tool,
    pub required: bool,
    pub found: bool,
    /// The version as the frame shows it (`2.46.0`, `Claude Code 2.1.4`); display only.
    pub version: Option<String>,
    /// gh, glab and az only: whether `gh auth status` / `glab auth status` / `az account show` succeed.
    pub logged_in: Option<bool>,
    /// az only: whether the `azure-devops` extension is installed.
    pub extension_installed: Option<bool>,
    /// What to run to fix what is missing (the tool, or az's extension); `None` when nothing is missing.
    pub install_command: Option<String>,
    /// What to run to log in, when the tool is found but not logged in.
    pub login_command: Option<String>,
}

/// Whether new Claude Code sessions will stop on permission prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionVerdict {
    Prompting,
    NotPrompting,
    /// Unreadable or undecidable: no warning.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionCheck {
    pub verdict: PermissionVerdict,
    /// The `permissions.defaultMode` that decided it, if one is set.
    pub mode: Option<String>,
    /// The settings file it came from.
    pub source: Option<String>,
}

/// The answer to `tools.check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolsReport {
    pub tools: Vec<ToolStatus>,
    /// Claude Code's permission mode, as the checklist evaluated it (d0011, d0016).
    pub permission: PermissionCheck,
    /// Every agent tool's row, checked again for the report, each with its own permission warning (SET-02,
    /// d0086). Empty from [`check`] alone; the service fills it.
    #[serde(default)]
    pub agent_tools: Vec<crate::agent_tools::connected::ToolStatus>,
}

impl ToolsReport {
    /// Whether onboarding can go on: every required tool found and at least one agent tool connected (SET-02).
    pub fn required_found(&self) -> bool {
        self.tools.iter().all(|t| t.found || !t.required) && self.agent_tool_connected()
    }

    /// At least one agent tool is connected (d0086: "at least one connected tool instead of Claude Code").
    pub fn agent_tool_connected(&self) -> bool {
        self.agent_tools.iter().any(|t| t.state.is_connected())
    }
}

/// Where to look. [`CheckEnv::from_process`] uses this process's environment; tests point it at fixtures.
#[derive(Debug, Clone)]
pub struct CheckEnv {
    /// The `PATH` to search, also passed to the tools.
    pub path: OsString,
    /// The user's Claude Code folder (`$CLAUDE_CONFIG_DIR` or `~/.claude`).
    pub claude_config_dir: Option<PathBuf>,
    /// The managed settings folder.
    pub managed_settings_dir: PathBuf,
}

impl CheckEnv {
    pub fn from_process() -> Self {
        let claude_config_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::home_dir().map(|h| h.join(".claude")));
        Self {
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude_config_dir,
            managed_settings_dir: managed_settings_dir(),
        }
    }
}

/// Runs the checklist's rows (git, gh, az). Only ever reads: runs `--version`, `auth status`, `account show` and
/// reads settings. The agent tools' rows are the service's ([`ToolsReport::agent_tools`]).
pub async fn check(env: &CheckEnv) -> ToolsReport {
    let (git, gh, az) = tokio::join!(
        check_tool(Tool::Git, env),
        check_tool(Tool::Gh, env),
        check_tool(Tool::Az, env),
    );
    ToolsReport {
        tools: vec![git, gh, az],
        permission: check_permission_mode(env),
        agent_tools: Vec::new(),
    }
}

/// Claude Code's managed settings folder on this OS (its docs, "Deploy managed settings").
fn managed_settings_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Program Files\ClaudeCode")
    } else if cfg!(target_os = "linux") {
        PathBuf::from("/etc/claude-code")
    } else {
        PathBuf::from("/Library/Application Support/ClaudeCode")
    }
}

/// Finds `name` in the directories of `path`, like the shell would. On Windows a name without an extension
/// also matches `name` plus each extension of `PATHEXT` (`gh.exe`, `az.cmd`).
pub fn find_on_path(name: &str, path: &OsStr) -> Option<PathBuf> {
    let names = candidate_names(name);
    std::env::split_paths(path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|candidate| is_executable(candidate))
}

fn candidate_names(name: &str) -> Vec<String> {
    if !cfg!(windows) || Path::new(name).extension().is_some() {
        return vec![name.to_owned()];
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
    pathext
        .split(';')
        .filter(|ext| !ext.is_empty())
        .map(|ext| format!("{name}{}", ext.to_ascii_lowercase()))
        .collect()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Windows has no executable bit: the extension (from `PATHEXT`) decides.
#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

struct Run {
    success: bool,
    stdout: String,
}

/// Runs a tool with no input and a time limit; `None` if it couldn't run or timed out.
async fn run(program: &Path, args: &[&str], env: &CheckEnv) -> Option<Run> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .env("PATH", &env.path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(COMMAND_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    Some(Run {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    })
}

/// Checks one tool, as the whole check would (`--version`, and the login for `gh` / `glab` / `az`).
pub async fn check_one(tool: Tool, env: &CheckEnv) -> ToolStatus {
    check_tool(tool, env).await
}

async fn check_tool(tool: Tool, env: &CheckEnv) -> ToolStatus {
    let mut status = ToolStatus {
        tool,
        required: tool.required(),
        found: false,
        version: None,
        logged_in: None,
        extension_installed: None,
        install_command: Some(tool.install_command().to_owned()),
        login_command: None,
    };
    let Some(program) = find_on_path(tool.command(), &env.path) else {
        return status;
    };
    status.found = true;
    status.install_command = None;
    match tool {
        Tool::Git | Tool::Claude | Tool::Gh | Tool::Glab => {
            if let Some(out) = run(&program, &["--version"], env).await {
                status.version = parse_version(tool, &out.stdout);
            }
        }
        Tool::Az => {
            let versions = run(&program, &["version", "--output", "json"], env).await;
            let parsed: Option<Value> = versions.and_then(|o| serde_json::from_str(&o.stdout).ok());
            status.version = parsed
                .as_ref()
                .and_then(|v| v.get("azure-cli")?.as_str().map(str::to_owned));
            let extension = parsed
                .as_ref()
                .and_then(|v| v.get("extensions")?.get("azure-devops"))
                .is_some();
            status.extension_installed = Some(extension);
            if !extension {
                status.install_command = Some("az extension add --name azure-devops".to_owned());
            }
        }
    }
    let login = match tool {
        Tool::Gh => Some((&["auth", "status"][..], "gh auth login")),
        Tool::Glab => Some((&["auth", "status"][..], "glab auth login")),
        Tool::Az => Some((&["account", "show"][..], "az login")),
        Tool::Git | Tool::Claude => None,
    };
    if let Some((args, login_command)) = login {
        let logged_in = run(&program, args, env).await.is_some_and(|o| o.success);
        status.logged_in = Some(logged_in);
        if !logged_in {
            status.login_command = Some(login_command.to_owned());
        }
    }
    status
}

/// The version as shown in the checklist, from the tool's `--version` output.
pub fn parse_version(tool: Tool, output: &str) -> Option<String> {
    let first = output.lines().next()?.trim();
    let token = match tool {
        // "git version 2.46.0"
        Tool::Git => first
            .strip_prefix("git version ")?
            .split_whitespace()
            .next()?,
        // "2.1.4 (Claude Code)"
        Tool::Claude => return Some(format!("Claude Code {}", first.split_whitespace().next()?)),
        // "gh version 2.62.0 (2024-11-14)"
        Tool::Gh => first
            .strip_prefix("gh version ")?
            .split_whitespace()
            .next()?,
        Tool::Az => first.split_whitespace().last()?,
        // "glab 1.46.1 (2024-09-12)", older "glab version 1.36.0 (2023-12-19)"
        Tool::Glab => {
            let rest = first.strip_prefix("glab ")?;
            rest.strip_prefix("version ")
                .unwrap_or(rest)
                .split_whitespace()
                .next()?
        }
    };
    Some(token.to_owned())
}

/// A settings file as read for the check.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsFile {
    Missing,
    Unreadable,
    Parsed(Value),
}

impl SettingsFile {
    pub fn read(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::Missing,
            Err(_) => Self::Unreadable,
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(value) if value.is_object() => Self::Parsed(value),
                _ => Self::Unreadable,
            },
        }
    }

    fn value(&self) -> Option<&Value> {
        match self {
            Self::Parsed(v) => Some(v),
            _ => None,
        }
    }
}

/// Whether new Claude Code sessions will stop on permission prompts, from its settings files (read-only).
pub fn check_permission_mode(env: &CheckEnv) -> PermissionCheck {
    let mut managed = vec![env.managed_settings_dir.join("managed-settings.json")];
    if let Ok(entries) = std::fs::read_dir(env.managed_settings_dir.join("managed-settings.d")) {
        let mut drop_ins: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        drop_ins.sort();
        managed.extend(drop_ins);
    }
    let mut files: Vec<(String, SettingsFile)> = managed
        .into_iter()
        .map(|p| (p.display().to_string(), SettingsFile::read(&p)))
        .collect();
    if let Some(dir) = &env.claude_config_dir {
        let user = dir.join("settings.json");
        files.push((user.display().to_string(), SettingsFile::read(&user)));
    }
    evaluate_permission_mode(&files)
}

/// Decides the verdict from settings files in precedence order (highest first).
pub fn evaluate_permission_mode(files: &[(String, SettingsFile)]) -> PermissionCheck {
    let unknown = |mode: Option<String>, source: Option<String>| PermissionCheck {
        verdict: PermissionVerdict::Unknown,
        mode,
        source,
    };
    let mut decided = None;
    for (source, file) in files {
        match file {
            // An unreadable file could hold the deciding value.
            SettingsFile::Unreadable if decided.is_none() => {
                return unknown(None, Some(source.clone()));
            }
            _ => {}
        }
        let mode = file
            .value()
            .and_then(|v| v.get("permissions")?.get("defaultMode")?.as_str());
        if let (None, Some(mode)) = (&decided, mode) {
            decided = Some((mode.to_owned(), source.clone()));
        }
    }
    let auto_disabled = files.iter().any(|(_, f)| {
        f.value()
            .and_then(|v| v.get("disableAutoMode")?.as_str())
            .is_some_and(|v| v == "disable")
    });
    let Some((mode, source)) = decided else {
        // No mode set: the built-in default depends on the claude version and plan, except that disabling auto
        // mode makes it Manual.
        return if auto_disabled {
            PermissionCheck {
                verdict: PermissionVerdict::Prompting,
                mode: None,
                source: None,
            }
        } else {
            unknown(None, None)
        };
    };
    let verdict = match mode.as_str() {
        "default" | "manual" | "acceptEdits" | "plan" => PermissionVerdict::Prompting,
        "auto" if auto_disabled => PermissionVerdict::Prompting,
        "auto" | "dontAsk" | "bypassPermissions" => PermissionVerdict::NotPrompting,
        _ => PermissionVerdict::Unknown,
    };
    PermissionCheck {
        verdict,
        mode: Some(mode),
        source: Some(source),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn versions_as_the_checklist_shows_them() {
        assert_eq!(
            parse_version(Tool::Git, "git version 2.46.0\n").as_deref(),
            Some("2.46.0")
        );
        assert_eq!(
            parse_version(Tool::Git, "git version 2.39.5 (Apple Git-154)\n").as_deref(),
            Some("2.39.5")
        );
        assert_eq!(
            parse_version(Tool::Claude, "2.1.4 (Claude Code)\n").as_deref(),
            Some("Claude Code 2.1.4")
        );
        assert_eq!(
            parse_version(
                Tool::Gh,
                "gh version 2.62.0 (2024-11-14)\nhttps://github.com/cli/cli/releases/tag/v2.62.0\n"
            )
            .as_deref(),
            Some("2.62.0")
        );
        assert_eq!(
            parse_version(Tool::Glab, "glab 1.46.1 (2024-09-12)\n").as_deref(),
            Some("1.46.1")
        );
        assert_eq!(
            parse_version(Tool::Glab, "glab version 1.36.0 (2023-12-19)\n").as_deref(),
            Some("1.36.0")
        );
        assert_eq!(parse_version(Tool::Git, ""), None);
        assert_eq!(parse_version(Tool::Gh, "garbage"), None);
    }

    fn parsed(v: Value) -> SettingsFile {
        SettingsFile::Parsed(v)
    }

    fn verdict(files: &[(&str, SettingsFile)]) -> PermissionVerdict {
        let files: Vec<_> = files
            .iter()
            .map(|(s, f)| (s.to_string(), f.clone()))
            .collect();
        evaluate_permission_mode(&files).verdict
    }

    #[test]
    fn prompting_modes() {
        for mode in ["default", "manual", "acceptEdits", "plan"] {
            let user = parsed(json!({"permissions": {"defaultMode": mode}}));
            assert_eq!(
                verdict(&[("user", user)]),
                PermissionVerdict::Prompting,
                "{mode}"
            );
        }
    }

    #[test]
    fn non_prompting_modes() {
        for mode in ["auto", "dontAsk", "bypassPermissions"] {
            let user = parsed(json!({"permissions": {"defaultMode": mode}}));
            assert_eq!(
                verdict(&[("user", user)]),
                PermissionVerdict::NotPrompting,
                "{mode}"
            );
        }
    }

    #[test]
    fn managed_settings_win() {
        let managed = parsed(json!({"permissions": {"defaultMode": "default"}}));
        let user = parsed(json!({"permissions": {"defaultMode": "auto"}}));
        let files = [("managed".to_owned(), managed), ("user".to_owned(), user)];
        let check = evaluate_permission_mode(&files);
        assert_eq!(check.verdict, PermissionVerdict::Prompting);
        assert_eq!(check.source.as_deref(), Some("managed"));
        assert_eq!(check.mode.as_deref(), Some("default"));
    }

    #[test]
    fn disabling_auto_mode_means_manual() {
        let managed = parsed(json!({"disableAutoMode": "disable"}));
        let user = parsed(json!({"permissions": {"defaultMode": "auto"}}));
        assert_eq!(
            verdict(&[("managed", managed.clone()), ("user", user)]),
            PermissionVerdict::Prompting
        );
        assert_eq!(
            verdict(&[("managed", managed), ("user", SettingsFile::Missing)]),
            PermissionVerdict::Prompting
        );
    }

    #[test]
    fn unknown_cases_show_no_warning() {
        // Nothing set: the built-in default depends on the version and plan.
        assert_eq!(
            verdict(&[
                ("managed", SettingsFile::Missing),
                ("user", parsed(json!({})))
            ]),
            PermissionVerdict::Unknown
        );
        assert_eq!(verdict(&[]), PermissionVerdict::Unknown);
        // Unreadable file before any decision.
        assert_eq!(
            verdict(&[("user", SettingsFile::Unreadable)]),
            PermissionVerdict::Unknown
        );
        // A mode this build doesn't know.
        let user = parsed(json!({"permissions": {"defaultMode": "telepathy"}}));
        assert_eq!(verdict(&[("user", user)]), PermissionVerdict::Unknown);
        // Wrong shapes are ignored.
        let user = parsed(json!({"permissions": {"defaultMode": 3}}));
        assert_eq!(verdict(&[("user", user)]), PermissionVerdict::Unknown);
    }

    #[test]
    fn an_unreadable_lower_file_does_not_matter_once_decided() {
        let managed = parsed(json!({"permissions": {"defaultMode": "acceptEdits"}}));
        assert_eq!(
            verdict(&[("managed", managed), ("user", SettingsFile::Unreadable)]),
            PermissionVerdict::Prompting
        );
    }

    #[test]
    fn install_commands_per_os() {
        assert_eq!(Tool::Gh.install_command_on(Os::Mac), "brew install gh");
        assert_eq!(
            Tool::Gh.install_command_on(Os::Windows),
            "winget install GitHub.cli"
        );
        assert_eq!(
            Tool::Az.install_command_on(Os::Windows),
            "winget install Microsoft.AzureCLI  &&  az extension add --name azure-devops"
        );
        assert_eq!(
            Tool::Git.install_command_on(Os::Linux),
            "sudo apt install git"
        );
        assert_eq!(
            Tool::Gh.install_command_on(Os::Linux),
            "sudo apt install gh"
        );
        for os in [Os::Mac, Os::Windows, Os::Linux] {
            assert!(
                Tool::ALL
                    .iter()
                    .all(|t| !t.install_command_on(os).is_empty())
            );
            // The extension az needs comes with every az command.
            assert!(
                Tool::Az
                    .install_command_on(os)
                    .ends_with("az extension add --name azure-devops")
            );
        }
        assert_eq!(
            Tool::Gh.install_command(),
            Tool::Gh.install_command_on(Os::current())
        );
        // `claude` is an agent tool now: one connected agent tool is the requirement (SET-02, d0086).
        assert!(Tool::Git.required() && !Tool::Claude.required());
        assert!(!Tool::Gh.required() && !Tool::Az.required());
        assert!(!Tool::ALL.contains(&Tool::Claude));
    }
}
