//! The agent's command line (SES-03, SES-04; d0009, d0011, d0031).
//!
//! A ticket's agent is the real interactive `claude` CLI, started in the ticket's worktree:
//!
//! ```text
//! OTAQ_TICKET=SHOP-18  PORT=<agent port>  PATH=<otaq bin dir>:$PATH
//! claude --plugin-dir <otaq plugin> --allowedTools "Bash(otaq:*)" --settings <status-line settings>
//!        --session-id <uuid> "/otaq:work-ticket"
//! ```
//!
//! **No permission mode or other permission flag**: the user's own Claude Code settings apply (d0011). The only
//! pre-approval is `Bash(otaq:*)`. `--settings` points at Otaq's file in its data folder whose only key is
//! `statusLine` ([`super::statusline`], USG-04); it is left out when that file couldn't be written.
//!
//! The Claude session ID is chosen by Otaq and passed with `--session-id` (the installed CLI supports it,
//! checked with `claude --help`, 2.1.296), so it is known and stored before the session starts; Resume, Park and
//! "Before the restart" use it (d0045). [`resume_agent`] builds `claude --resume <id>` with the same environment
//! and flags.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::domain::TicketKey;
use crate::naming;

/// The skill every agent session starts with.
pub const WORK_TICKET_SKILL: &str = "work-ticket";

/// The skill every Project agent starts with (d0068).
pub const PROJECT_AGENT_SKILL: &str = "project-agent";

/// Where things are on this machine. [`LaunchContext::from_process`] finds them for the running service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContext {
    /// The program and any leading arguments that stand for `claude` (`["claude"]`; tests substitute an
    /// ordinary command).
    pub claude: Vec<String>,
    /// The folder of the `otaq` command, put first on the agent's PATH.
    pub bin_dir: PathBuf,
    /// The Otaq plugin folder for `--plugin-dir`.
    pub plugin_dir: PathBuf,
    /// The PATH the agent extends (the service's).
    pub path: OsString,
    /// The user's Claude Code folder (`$CLAUDE_CONFIG_DIR` or `~/.claude`), read only to find their status line
    /// (USG-04). `None` reads no user settings (tests).
    pub claude_config_dir: Option<PathBuf>,
}

impl LaunchContext {
    /// `claude` from PATH, the running binary's folder, and the plugin next to it (or the repository's in a
    /// dev build). `None` when no plugin folder is found.
    pub fn from_process() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        let bin_dir = exe.parent()?.to_path_buf();
        Some(Self {
            claude: vec!["claude".to_owned()],
            plugin_dir: find_plugin_dir(&bin_dir)?,
            bin_dir,
            path: std::env::var_os("PATH").unwrap_or_default(),
            claude_config_dir: crate::tools::CheckEnv::from_process().claude_config_dir,
        })
    }
}

/// The plugin folder: `<bin>/plugin`, `<bin>/../share/otaq/plugin`, or the repository's `plugin/` (dev
/// builds run from `target/`). A folder counts when it has `.claude-plugin/plugin.json`.
pub fn find_plugin_dir(bin_dir: &Path) -> Option<PathBuf> {
    let repo_plugin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("plugin"));
    [
        Some(bin_dir.join("plugin")),
        bin_dir.parent().map(|prefix| {
            prefix
                .join("share")
                .join(naming::COMMAND_NAME)
                .join("plugin")
        }),
        repo_plugin,
    ]
    .into_iter()
    .flatten()
    .find(|dir| dir.join(".claude-plugin").join("plugin.json").is_file())
}

/// A command ready for [`super::Sessions::spawn`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// The Claude session ID the conversation uses.
    pub claude_session_id: String,
}

/// A new agent session for `ticket` in `worktree`, with a fresh Claude session ID. `settings` is Otaq's
/// status-line settings file.
pub fn launch_agent(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
    settings: Option<&Path>,
) -> AgentCommand {
    let session_id = uuid::Uuid::now_v7().to_string();
    let mut args = common_flags(ctx, settings);
    args.extend([
        "--session-id".to_owned(),
        session_id.clone(),
        naming::skill(WORK_TICKET_SKILL),
    ]);
    command(ctx, ticket, worktree, agent_port, args, session_id)
}

/// A new conversation for `ticket` in `cwd` with `prompt` as its first prompt: the epic's delivery session runs the
/// Open PR or Push skill this way (EPC-08), an answer session's fallback too.
pub fn launch_with_prompt(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    cwd: &Path,
    port: u16,
    prompt: String,
    settings: Option<&Path>,
) -> AgentCommand {
    let session_id = uuid::Uuid::now_v7().to_string();
    let mut args = common_flags(ctx, settings);
    args.extend(["--session-id".to_owned(), session_id.clone(), prompt]);
    command(ctx, ticket, cwd, port, args, session_id)
}

/// A ticket's validator (VAL-01, d0063): a new conversation in the ticket's worktree with the project's Validate
/// skill as its first prompt, the ticket and the validator role in its environment, and no `PORT` (it runs no
/// dev server). The same plugin, pre-approval, status line and no permission flags as the agent (d0011).
pub fn launch_validator(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    skill: &str,
    settings: Option<&Path>,
) -> AgentCommand {
    let session_id = uuid::Uuid::now_v7().to_string();
    let mut args = common_flags(ctx, settings);
    args.extend([
        "--session-id".to_owned(),
        session_id.clone(),
        skill.to_owned(),
    ]);
    let mut command = command(ctx, ticket, worktree, 0, args, session_id);
    command.env.retain(|(name, _)| name != "PORT");
    command
        .env
        .push((naming::role_env_var(), naming::ROLE_VALIDATOR.to_owned()));
    command
}

/// A new Project agent (PAG-01, d0068): a conversation in the project's own folder (the user's checkout) with
/// `/otaq:project-agent` as its first prompt and the Project agent role in its environment. No ticket, no
/// `PORT`. The same plugin, pre-approval, status line and no permission flags as the agent (d0011).
pub fn launch_project_agent(
    ctx: &LaunchContext,
    repo: &Path,
    settings: Option<&Path>,
) -> AgentCommand {
    let session_id = uuid::Uuid::now_v7().to_string();
    let mut args = common_flags(ctx, settings);
    args.extend([
        "--session-id".to_owned(),
        session_id.clone(),
        naming::skill(PROJECT_AGENT_SKILL),
    ]);
    project_agent_command(ctx, repo, args, session_id)
}

/// Continues a Project agent's conversation `session_id` (Resume after it ended, d0068) with the same
/// environment and flags.
pub fn resume_project_agent(
    ctx: &LaunchContext,
    repo: &Path,
    session_id: &str,
    settings: Option<&Path>,
) -> AgentCommand {
    let mut args = common_flags(ctx, settings);
    args.extend(["--resume".to_owned(), session_id.to_owned()]);
    project_agent_command(ctx, repo, args, session_id.to_owned())
}

fn project_agent_command(
    ctx: &LaunchContext,
    repo: &Path,
    flags: Vec<String>,
    claude_session_id: String,
) -> AgentCommand {
    let (program, leading, path) = program_and_path(ctx);
    AgentCommand {
        program,
        args: leading.into_iter().chain(flags).collect(),
        cwd: repo.to_path_buf(),
        env: vec![
            ("PATH".to_owned(), path),
            (
                naming::role_env_var(),
                naming::ROLE_PROJECT_AGENT.to_owned(),
            ),
        ],
        claude_session_id,
    }
}

/// Continues the stored conversation `session_id` (Resume, Park, d0045, d0052) with the same environment and
/// flags.
pub fn resume_agent(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
    session_id: &str,
) -> AgentCommand {
    resume_agent_with(ctx, ticket, worktree, agent_port, session_id, None, None)
}

/// Like [`resume_agent`], with a first prompt for the resumed conversation: the skill a Send back or Send to
/// agent to fix asked for (`claude --resume <id> "/otaq:address-review"`), so it runs as soon as the
/// conversation is loaded instead of being typed into a terminal that may not be ready yet.
pub fn resume_agent_with(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
    session_id: &str,
    prompt: Option<String>,
    settings: Option<&Path>,
) -> AgentCommand {
    let mut args = common_flags(ctx, settings);
    args.extend(["--resume".to_owned(), session_id.to_owned()]);
    args.extend(prompt);
    command(
        ctx,
        ticket,
        worktree,
        agent_port,
        args,
        session_id.to_owned(),
    )
}

/// `--plugin-dir <plugin> --allowedTools "Bash(otaq:*)" [--settings <file>]`: the plugin, the one
/// pre-approval (d0011) and Otaq's status line (USG-04).
fn common_flags(ctx: &LaunchContext, settings: Option<&Path>) -> Vec<String> {
    let mut flags = vec![
        "--plugin-dir".to_owned(),
        ctx.plugin_dir.display().to_string(),
        "--allowedTools".to_owned(),
        format!("Bash({}:*)", naming::COMMAND_NAME),
    ];
    if let Some(settings) = settings {
        flags.extend(["--settings".to_owned(), settings.display().to_string()]);
    }
    flags
}

fn command(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
    flags: Vec<String>,
    claude_session_id: String,
) -> AgentCommand {
    let (program, leading, path) = program_and_path(ctx);
    AgentCommand {
        program,
        args: leading.into_iter().chain(flags).collect(),
        cwd: worktree.to_path_buf(),
        env: vec![
            (naming::ticket_env_var(), ticket.to_string()),
            ("PORT".to_owned(), agent_port.to_string()),
            ("PATH".to_owned(), path),
        ],
        claude_session_id,
    }
}

/// The program standing for `claude`, its leading arguments, and the PATH with the `otaq` folder first.
fn program_and_path(ctx: &LaunchContext) -> (String, Vec<String>, String) {
    let (program, leading) = ctx
        .claude
        .split_first()
        .map(|(p, rest)| (p.clone(), rest.to_vec()))
        .unwrap_or_else(|| ("claude".to_owned(), Vec::new()));
    let path = crate::shell::prepend_path(Some(&ctx.bin_dir), &ctx.path);
    (program, leading, path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> LaunchContext {
        LaunchContext {
            claude: vec!["claude".into()],
            bin_dir: "/opt/cb/bin".into(),
            plugin_dir: "/opt/cb/plugin".into(),
            path: platform_path(&["/usr/bin", "/bin"]),
            claude_config_dir: None,
        }
    }

    /// A PATH value with the platform's separator.
    fn platform_path(dirs: &[&str]) -> OsString {
        std::env::join_paths(dirs).unwrap()
    }

    fn key() -> TicketKey {
        "SHOP-18".parse().unwrap()
    }

    /// The validator (T-0183, VAL-01): the Validate skill as its prompt, the role, the ticket, no `PORT`, no
    /// permission flags.
    #[test]
    fn validator_launch() {
        let cmd = launch_validator(
            &ctx(),
            &key(),
            Path::new("/wt/shop-web/SHOP-18"),
            "/my-validate",
            Some(Path::new("/home/me/.data/claude/SHOP-18.settings.json")),
        );
        let name = naming::COMMAND_NAME;
        assert_eq!(
            cmd.args,
            [
                "--plugin-dir".to_owned(),
                "/opt/cb/plugin".to_owned(),
                "--allowedTools".to_owned(),
                format!("Bash({name}:*)"),
                "--settings".to_owned(),
                "/home/me/.data/claude/SHOP-18.settings.json".to_owned(),
                "--session-id".to_owned(),
                cmd.claude_session_id.clone(),
                "/my-validate".to_owned(),
            ]
        );
        assert!(
            !cmd.args.iter().any(|a| a.contains("permission")),
            "no permission flags (d0011)"
        );
        assert_eq!(cmd.cwd, Path::new("/wt/shop-web/SHOP-18"));
        assert_eq!(
            cmd.env,
            [
                (naming::ticket_env_var(), "SHOP-18".to_owned()),
                (
                    "PATH".to_owned(),
                    platform_path(&["/opt/cb/bin", "/usr/bin", "/bin"])
                        .to_string_lossy()
                        .into_owned()
                ),
                (naming::role_env_var(), naming::ROLE_VALIDATOR.to_owned()),
            ]
        );
    }

    /// A Project agent (T-0199, PAG-01): in the project's folder, the role, the skill, no ticket and no `PORT`,
    /// no permission flags; Resume continues the same conversation with the same environment.
    #[test]
    fn project_agent_launch() {
        let settings = Path::new("/home/me/.data/claude/project-agent-a1.settings.json");
        let cmd = launch_project_agent(&ctx(), Path::new("/src/shop-web"), Some(settings));
        let name = naming::COMMAND_NAME;
        assert_eq!(cmd.program, "claude");
        assert_eq!(
            cmd.args,
            [
                "--plugin-dir".to_owned(),
                "/opt/cb/plugin".to_owned(),
                "--allowedTools".to_owned(),
                format!("Bash({name}:*)"),
                "--settings".to_owned(),
                settings.display().to_string(),
                "--session-id".to_owned(),
                cmd.claude_session_id.clone(),
                format!("/{name}:project-agent"),
            ]
        );
        assert!(!cmd.args.iter().any(|a| a.contains("permission")));
        assert_eq!(cmd.cwd, Path::new("/src/shop-web"));
        let path = platform_path(&["/opt/cb/bin", "/usr/bin", "/bin"])
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            cmd.env,
            [
                ("PATH".to_owned(), path),
                (
                    naming::role_env_var(),
                    naming::ROLE_PROJECT_AGENT.to_owned()
                ),
            ]
        );
        assert!(
            !cmd.env
                .iter()
                .any(|(k, _)| *k == naming::ticket_env_var() || k == "PORT"),
            "no ticket, no port"
        );

        let resume =
            resume_project_agent(&ctx(), Path::new("/src/shop-web"), "c-1", Some(settings));
        assert_eq!(resume.env, cmd.env);
        assert_eq!(resume.args[..6], cmd.args[..6]);
        assert_eq!(resume.args[6..], ["--resume".to_owned(), "c-1".to_owned()]);
        assert_eq!(resume.claude_session_id, "c-1");
    }

    #[test]
    fn launch_is_exactly_the_documented_command() {
        let cmd = launch_agent(
            &ctx(),
            &key(),
            Path::new("/wt/shop-web/SHOP-18"),
            4019,
            Some(Path::new("/home/me/.data/claude/SHOP-18.settings.json")),
        );
        assert_eq!(cmd.program, "claude");
        let session = cmd.claude_session_id.clone();
        assert!(uuid::Uuid::parse_str(&session).is_ok());
        let name = naming::COMMAND_NAME;
        assert_eq!(
            cmd.args,
            [
                "--plugin-dir".to_owned(),
                "/opt/cb/plugin".to_owned(),
                "--allowedTools".to_owned(),
                format!("Bash({name}:*)"),
                "--settings".to_owned(),
                "/home/me/.data/claude/SHOP-18.settings.json".to_owned(),
                "--session-id".to_owned(),
                session,
                format!("/{name}:work-ticket"),
            ]
        );
        assert_eq!(cmd.cwd, Path::new("/wt/shop-web/SHOP-18"));
        assert_eq!(
            cmd.env,
            [
                (naming::ticket_env_var(), "SHOP-18".to_owned()),
                ("PORT".to_owned(), "4019".to_owned()),
                (
                    "PATH".to_owned(),
                    platform_path(&["/opt/cb/bin", "/usr/bin", "/bin"])
                        .to_string_lossy()
                        .into_owned()
                ),
            ]
        );
    }

    #[test]
    fn each_launch_gets_its_own_session_id() {
        let a = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019, None);
        let b = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019, None);
        assert_ne!(a.claude_session_id, b.claude_session_id);
    }

    #[test]
    fn resume_uses_the_stored_session_with_the_same_env_and_flags() {
        let id = "0199d2a4-3b1c-7c3e-9f00-123456789abc";
        let launch = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019, None);
        let resume = resume_agent(&ctx(), &key(), Path::new("/wt"), 4019, id);
        assert_eq!(resume.env, launch.env);
        assert_eq!(resume.args[..4], launch.args[..4]);
        assert_eq!(resume.args[4..], ["--resume".to_owned(), id.to_owned()]);
        assert_eq!(resume.claude_session_id, id);
    }

    #[test]
    fn no_permission_flags_ever() {
        let forbidden = [
            "--permission-mode",
            "--dangerously-skip-permissions",
            "--allow-dangerously-skip-permissions",
            "--permission-prompt-tool",
            "--disallowedTools",
        ];
        let settings = Path::new("/data/claude/SHOP-18.settings.json");
        let launch = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019, Some(settings));
        let resume = resume_agent_with(
            &ctx(),
            &key(),
            Path::new("/wt"),
            4019,
            "id",
            None,
            Some(settings),
        );
        for cmd in [launch, resume] {
            // `--settings` only ever carries Otaq's status-line file (its content is tested in
            // `statusline`: one key, `statusLine`).
            let passed: Vec<_> = cmd
                .args
                .windows(2)
                .filter(|w| w[0] == "--settings")
                .map(|w| w[1].clone())
                .collect();
            assert_eq!(passed, [settings.display().to_string()]);
            for arg in &cmd.args {
                assert!(
                    !forbidden.iter().any(|f| arg.starts_with(f)),
                    "{arg} must never be passed (d0011)"
                );
            }
            // The only pre-approval is Otaq's own command.
            let allowed: Vec<_> = cmd
                .args
                .windows(2)
                .filter(|w| w[0] == "--allowedTools")
                .map(|w| w[1].clone())
                .collect();
            assert_eq!(allowed, [format!("Bash({}:*)", naming::COMMAND_NAME)]);
        }
    }

    #[test]
    fn a_substituted_program_keeps_its_leading_arguments() {
        let mut ctx = ctx();
        ctx.claude = vec!["sh".into(), "-c".into(), "echo".into(), "sh".into()];
        ctx.path = OsString::new();
        let cmd = launch_agent(&ctx, &key(), Path::new("/wt"), 1, None);
        assert_eq!(cmd.program, "sh");
        assert_eq!(cmd.args[..3], ["-c", "echo", "sh"]);
        assert_eq!(cmd.args[3], "--plugin-dir");
        assert_eq!(cmd.env[2].1, "/opt/cb/bin", "no trailing colon");
    }

    #[test]
    fn the_repository_plugin_is_found_in_dev_builds() {
        let dir = find_plugin_dir(Path::new("/nonexistent/bin")).expect("repo plugin");
        assert!(dir.ends_with("plugin"));
        assert!(dir.join(".claude-plugin/plugin.json").is_file());
    }
}
