//! The agent's command line (SES-03, SES-04; d0009, d0011, d0031).
//!
//! A ticket's agent is the real interactive `claude` CLI, started in the ticket's worktree:
//!
//! ```text
//! CREWBOARD_TICKET=SHOP-18  PORT=<agent port>  PATH=<crewboard bin dir>:$PATH
//! claude --plugin-dir <crewboard plugin> --allowedTools "Bash(crewboard:*)" --session-id <uuid> "/crewboard:work-ticket"
//! ```
//!
//! **No permission mode or other permission flag**: the user's own Claude Code settings apply (d0011). The only
//! pre-approval is `Bash(crewboard:*)`. The status-line `--settings` comes with T-0127 (M8).
//!
//! The Claude session ID is chosen by Crewboard and passed with `--session-id` (the installed CLI supports it,
//! checked with `claude --help`, 2.1.296), so it is known and stored before the session starts; Resume, Park and
//! "Before the restart" use it (d0045). [`resume_agent`] builds `claude --resume <id>` with the same environment
//! and flags.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::domain::TicketKey;
use crate::naming;

/// The skill every agent session starts with.
pub const WORK_TICKET_SKILL: &str = "work-ticket";

/// Where things are on this machine. [`LaunchContext::from_process`] finds them for the running service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContext {
    /// The program and any leading arguments that stand for `claude` (`["claude"]`; tests substitute an
    /// ordinary command).
    pub claude: Vec<String>,
    /// The folder of the `crewboard` command, put first on the agent's PATH.
    pub bin_dir: PathBuf,
    /// The Crewboard plugin folder for `--plugin-dir`.
    pub plugin_dir: PathBuf,
    /// The PATH the agent extends (the service's).
    pub path: OsString,
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
        })
    }
}

/// The plugin folder: `<bin>/plugin`, `<bin>/../share/crewboard/plugin`, or the repository's `plugin/` (dev
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

/// A new agent session for `ticket` in `worktree`, with a fresh Claude session ID.
pub fn launch_agent(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
) -> AgentCommand {
    let session_id = uuid::Uuid::now_v7().to_string();
    let mut args = common_flags(ctx);
    args.extend([
        "--session-id".to_owned(),
        session_id.clone(),
        naming::skill(WORK_TICKET_SKILL),
    ]);
    command(ctx, ticket, worktree, agent_port, args, session_id)
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
    resume_agent_with(ctx, ticket, worktree, agent_port, session_id, None)
}

/// Like [`resume_agent`], with a first prompt for the resumed conversation: the skill a Send back or Send to
/// agent to fix asked for (`claude --resume <id> "/crewboard:address-review"`), so it runs as soon as the
/// conversation is loaded instead of being typed into a terminal that may not be ready yet.
pub fn resume_agent_with(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
    session_id: &str,
    prompt: Option<String>,
) -> AgentCommand {
    let mut args = common_flags(ctx);
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

/// `--plugin-dir <plugin> --allowedTools "Bash(crewboard:*)"`: the plugin, and the one pre-approval (d0011).
fn common_flags(ctx: &LaunchContext) -> Vec<String> {
    vec![
        "--plugin-dir".to_owned(),
        ctx.plugin_dir.display().to_string(),
        "--allowedTools".to_owned(),
        format!("Bash({}:*)", naming::COMMAND_NAME),
    ]
}

fn command(
    ctx: &LaunchContext,
    ticket: &TicketKey,
    worktree: &Path,
    agent_port: u16,
    flags: Vec<String>,
    claude_session_id: String,
) -> AgentCommand {
    let (program, leading) = ctx
        .claude
        .split_first()
        .map(|(p, rest)| (p.clone(), rest.to_vec()))
        .unwrap_or_else(|| ("claude".to_owned(), Vec::new()));
    let mut path = OsString::from(ctx.bin_dir.as_os_str());
    if !ctx.path.is_empty() {
        path.push(":");
        path.push(&ctx.path);
    }
    AgentCommand {
        program,
        args: leading.into_iter().chain(flags).collect(),
        cwd: worktree.to_path_buf(),
        env: vec![
            (naming::ticket_env_var(), ticket.to_string()),
            ("PORT".to_owned(), agent_port.to_string()),
            ("PATH".to_owned(), path.to_string_lossy().into_owned()),
        ],
        claude_session_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> LaunchContext {
        LaunchContext {
            claude: vec!["claude".into()],
            bin_dir: "/opt/cb/bin".into(),
            plugin_dir: "/opt/cb/plugin".into(),
            path: "/usr/bin:/bin".into(),
        }
    }

    fn key() -> TicketKey {
        "SHOP-18".parse().unwrap()
    }

    #[test]
    fn launch_is_exactly_the_documented_command() {
        let cmd = launch_agent(&ctx(), &key(), Path::new("/wt/shop-web/SHOP-18"), 4019);
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
                ("PATH".to_owned(), "/opt/cb/bin:/usr/bin:/bin".to_owned()),
            ]
        );
    }

    #[test]
    fn each_launch_gets_its_own_session_id() {
        let a = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019);
        let b = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019);
        assert_ne!(a.claude_session_id, b.claude_session_id);
    }

    #[test]
    fn resume_uses_the_stored_session_with_the_same_env_and_flags() {
        let id = "0199d2a4-3b1c-7c3e-9f00-123456789abc";
        let launch = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019);
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
            "--settings",
        ];
        let launch = launch_agent(&ctx(), &key(), Path::new("/wt"), 4019);
        let resume = resume_agent(&ctx(), &key(), Path::new("/wt"), 4019, "id");
        for cmd in [launch, resume] {
            for arg in &cmd.args {
                assert!(
                    !forbidden.iter().any(|f| arg.starts_with(f)),
                    "{arg} must never be passed (d0011)"
                );
            }
            // The only pre-approval is Crewboard's own command.
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
        let cmd = launch_agent(&ctx, &key(), Path::new("/wt"), 1);
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
