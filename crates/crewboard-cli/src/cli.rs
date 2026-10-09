//! Argument parsing. The command set and the global flags follow beads (`bd`) conventions (d0032); the CLI is a
//! thin client of the background service (d0031). Commands appear here only once they are built.

use std::ffi::OsString;

use clap::{Args, Parser, Subcommand};
use crewboard_core::naming;

use crate::output::Mode;

#[derive(Debug, Parser)]
#[command(
    name = naming::COMMAND_NAME,
    version,
    about = "Kanban board that drives Claude Code agents. Without a command, opens the app.",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Flags accepted by every command (d0032).
#[derive(Debug, Default, Args)]
pub struct GlobalArgs {
    /// Print machine-readable JSON; errors become {"error": {...}}
    #[arg(long, global = true)]
    pub json: bool,

    /// Print errors only
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Print only the ID of what was created or changed
    #[arg(long, global = true)]
    pub silent: bool,

    /// The project to work in, by key (SHOP)
    #[arg(short = 'P', long, global = true, value_name = "KEY")]
    pub project: Option<String>,

    /// Who is acting, recorded on changes (for example agent:SHOP-18)
    #[arg(long, global = true, value_name = "ACTOR")]
    pub actor: Option<String>,
}

impl GlobalArgs {
    /// The output mode. When several flags are given, `--json` wins, then `--silent`, then `--quiet`.
    pub fn mode(&self) -> Mode {
        if self.json {
            Mode::Json
        } else if self.silent {
            Mode::Silent
        } else if self.quiet {
            Mode::Quiet
        } else {
            Mode::Human
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start, stop or check the background service
    #[command(subcommand)]
    Daemon(DaemonCommand),
    /// Open a ticket's live agent session in this terminal (detach with Ctrl+])
    Attach {
        /// The ticket: SHOP-18, or 18 in the current project. Default: $CREWBOARD_TICKET
        key: Option<String>,
    },
    /// Internal: what the plugin's hooks run (`crewboard hook Stop`, payload on stdin)
    #[command(hide = true)]
    Hook {
        /// The Claude Code hook event: Notification, Stop, StopFailure or UserPromptSubmit
        event: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum DaemonCommand {
    /// Start the background service if it isn't running
    Start,
    /// Stop the background service (working agents are interrupted)
    Stop,
    /// Show whether the background service runs, with its PID, build ID and uptime
    Status,
    /// Run the service in the foreground (what `daemon start` launches in the background)
    #[command(hide = true)]
    Run,
}

/// Whether the raw arguments ask for JSON output. Used to format errors clap reports before parsing succeeds.
pub fn wants_json(args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .take_while(|a| *a != "--")
        .any(|a| a == "--json")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once(naming::COMMAND_NAME).chain(args.iter().copied()))
    }

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn no_command_means_open_the_app() {
        let cli = parse(&[]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.global.mode(), Mode::Human);
    }

    #[test]
    fn daemon_subcommands() {
        for (arg, expected) in [
            ("start", DaemonCommand::Start),
            ("stop", DaemonCommand::Stop),
            ("status", DaemonCommand::Status),
        ] {
            let cli = parse(&["daemon", arg]).unwrap();
            assert!(matches!(cli.command, Some(Command::Daemon(d)) if d == expected));
        }
        assert!(parse(&["daemon"]).is_err());
        assert!(parse(&["daemon", "restart"]).is_err());
    }

    #[test]
    fn attach_takes_an_optional_key() {
        let cli = parse(&["attach", "SHOP-18"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Attach { key: Some(k) }) if k == "SHOP-18"));
        let cli = parse(&["attach"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Attach { key: None })));
        let cli = parse(&["-P", "SHOP", "attach", "18"]).unwrap();
        assert_eq!(cli.global.project.as_deref(), Some("SHOP"));
    }

    #[test]
    fn global_flags_parse_before_and_after_the_subcommand() {
        let cli = parse(&["--json", "daemon", "status"]).unwrap();
        assert_eq!(cli.global.mode(), Mode::Json);
        let cli = parse(&[
            "daemon",
            "status",
            "--json",
            "-P",
            "SHOP",
            "--actor",
            "agent:SHOP-18",
        ])
        .unwrap();
        assert_eq!(cli.global.mode(), Mode::Json);
        assert_eq!(cli.global.project.as_deref(), Some("SHOP"));
        assert_eq!(cli.global.actor.as_deref(), Some("agent:SHOP-18"));
        let cli = parse(&["daemon", "-q", "start"]).unwrap();
        assert_eq!(cli.global.mode(), Mode::Quiet);
        let cli = parse(&["daemon", "start", "--project", "BILL", "--silent"]).unwrap();
        assert_eq!(cli.global.mode(), Mode::Silent);
        assert_eq!(cli.global.project.as_deref(), Some("BILL"));
    }

    #[test]
    fn mode_precedence() {
        let all = GlobalArgs {
            json: true,
            quiet: true,
            silent: true,
            ..Default::default()
        };
        assert_eq!(all.mode(), Mode::Json);
        let silent_quiet = GlobalArgs {
            quiet: true,
            silent: true,
            ..Default::default()
        };
        assert_eq!(silent_quiet.mode(), Mode::Silent);
    }

    #[test]
    fn help_lists_only_built_commands() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("daemon"), "{help}");
        assert!(help.contains("attach"), "{help}");
        for later in ["hook", "prime", "create"] {
            assert!(
                !help.contains(&format!("\n  {later}")),
                "{later} shown:\n{help}"
            );
        }
    }

    #[test]
    fn json_detection() {
        let args = |a: &[&str]| -> Vec<OsString> { a.iter().map(OsString::from).collect() };
        assert!(wants_json(&args(&["x", "daemon", "--json"])));
        assert!(!wants_json(&args(&["x", "daemon"])));
        assert!(!wants_json(&args(&["x", "--", "--json"])));
    }
}
