//! Argument parsing. The command set and the global flags follow beads (`bd`) conventions (d0032); the CLI is a
//! thin client of the background service (d0031). Commands appear here only once they are built.

use std::ffi::OsString;
use std::path::PathBuf;

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
    /// The current ticket's context for its agent: details, parents, rules and your Agent instructions
    Prime {
        /// SHOP-18, or 18 in the current project. Default: $CREWBOARD_TICKET
        key: Option<String>,
    },
    /// Show a ticket: details, Done when, dependencies (default: $CREWBOARD_TICKET)
    #[command(visible_alias = "view")]
    Show {
        /// SHOP-18, or 18 in the current project
        key: Option<String>,
        /// Also show the ticket's history
        #[arg(long)]
        long: bool,
        /// For an epic: list its tickets with their statuses
        #[arg(long)]
        children: bool,
    },
    /// List the current project's tickets (-P to name the project)
    List {
        /// Only this status (working, asks_you, …) or column (backlog, dev, ready_for_review, approved, done)
        #[arg(long)]
        status: Option<String>,
        /// Show epics with their tickets indented under them
        #[arg(long)]
        tree: bool,
        /// Only the tickets of this epic
        #[arg(long, value_name = "EPIC")]
        parent: Option<String>,
    },
    /// Tickets whose dependencies are all merged
    Ready,
    /// Tickets that wait for a dependency to merge
    Blocked,
    /// Create a ticket in Backlog and print its key (long text: --stdin)
    #[command(visible_alias = "new")]
    Create {
        /// The title
        title: Option<String>,
        /// A short description; use --stdin for anything longer
        #[arg(short = 'd', long = "description")]
        description: Option<String>,
        /// A "Done when" item (repeatable)
        #[arg(long = "done-when", value_name = "ITEM")]
        done_when: Vec<String>,
        /// Tickets it depends on: SHOP-18,SHOP-19
        #[arg(long, value_delimiter = ',', value_name = "KEYS")]
        deps: Vec<String>,
        /// Read the description (Markdown) from stdin
        #[arg(long)]
        stdin: bool,
        /// Create every ticket of a Markdown file (### headings), all or nothing
        #[arg(short = 'f', long = "file", value_name = "FILE")]
        file: Option<PathBuf>,
        /// ticket (default) or epic
        #[arg(long = "type", value_name = "TYPE", value_parser = ["ticket", "epic"])]
        kind: Option<String>,
        /// Create it inside this epic (SHOP-40 → SHOP-40.4)
        #[arg(long, value_name = "EPIC")]
        parent: Option<String>,
    },
    /// Change a ticket's title, description or Done when (default: $CREWBOARD_TICKET)
    Update {
        /// SHOP-18, or 18 in the current project
        key: Option<String>,
        /// The new title
        #[arg(long)]
        title: Option<String>,
        /// The new description (short); use --stdin for anything longer
        #[arg(short = 'd', long = "description")]
        description: Option<String>,
        /// Add a "Done when" item (repeatable)
        #[arg(long = "add-done-when", value_name = "ITEM")]
        add_done_when: Vec<String>,
        /// Read the new description (Markdown) from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Ask the user a question (`ask human --stdin`); the ticket shows "Asks you"
    Ask {
        /// `human`
        #[arg(value_name = "human")]
        target: String,
        /// Read the question from stdin
        #[arg(long)]
        stdin: bool,
        /// An answer option (repeatable, 2–4 short labels); without options the question is free text
        #[arg(long = "option", value_name = "LABEL")]
        options: Vec<String>,
        /// The recommended option, 1-based
        #[arg(long, value_name = "N")]
        recommended: Option<u32>,
    },
    /// Answer a question another ticket's agent asked this one (`answer <ID> --stdin`)
    #[command(hide = true)]
    Answer {
        /// The question's ID, as `prime` or `questions` shows it
        id: String,
        /// Read the answer from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// The questions other tickets' agents asked this one that wait for an answer
    #[command(hide = true)]
    Questions {
        /// SHOP-18, or 18 in the current project. Default: $CREWBOARD_TICKET
        key: Option<String>,
    },
    /// Submit the ticket for review (`review submit --stdin`) or read the review feedback (`review feedback`)
    #[command(subcommand)]
    Review(ReviewCommand),
    /// The Open PR and Push skills' commands: `pr propose --stdin`, `pr opened <url>`
    #[command(subcommand)]
    Pr(PrCommand),
    /// Rename the ticket's branch (`branch rename <name>`); refused once it was pushed
    #[command(subcommand)]
    Branch(BranchCommand),
    /// Add, remove or show dependencies
    #[command(subcommand)]
    Dep(DepCommand),
    /// List or show projects
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Internal: what the plugin's hooks run (`crewboard hook Stop`, payload on stdin)
    #[command(hide = true)]
    Hook {
        /// The Claude Code hook event: Notification, Stop, StopFailure or UserPromptSubmit
        event: String,
    },
    /// Internal: the status line agents run with (status-line JSON on stdin; runs your own status line)
    #[command(hide = true)]
    Statusline,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ReviewCommand {
    /// Move the ticket to Ready for Review with a summary on stdin; refused while files are uncommitted
    Submit {
        /// SHOP-18, or 18 in the current project. Default: $CREWBOARD_TICKET
        key: Option<String>,
        /// Read the summary from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// The latest Send back's comments, by file, and the reviewer's note (reading it changes nothing)
    Feedback {
        /// SHOP-18, or 18 in the current project. Default: $CREWBOARD_TICKET
        key: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum PrCommand {
    /// Propose the branch name, commit messages, PR title and description (JSON on stdin); nothing is pushed
    Propose {
        /// Read the proposal from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Report the pull request the skill opened (or pushed to)
    Opened {
        /// The PR's URL, e.g. https://github.com/acme/shop-web/pull/481
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum BranchCommand {
    /// Rename the ticket's temporary branch; refused after the first push
    Rename {
        /// The new branch name, e.g. bugfix/SHOP-18-login-redirect
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum DepCommand {
    /// Make KEY depend on DEPENDS-ON (KEY waits until DEPENDS-ON is merged)
    Add {
        key: String,
        #[arg(value_name = "DEPENDS-ON")]
        depends_on: String,
    },
    /// Remove the link "KEY depends on DEPENDS-ON"
    Remove {
        key: String,
        #[arg(value_name = "DEPENDS-ON")]
        depends_on: String,
    },
    /// The ticket's Stack list: its chain, indented, with statuses (default: $CREWBOARD_TICKET)
    Tree { key: Option<String> },
    /// List dependency loops in the current project (none on a healthy project)
    Cycles,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ProjectCommand {
    /// Every project: key, name, base branch, repository
    List,
    /// One project (default: the current one)
    Show {
        /// The project's key (SHOP)
        key: Option<String>,
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
        for shown in [
            "prime", "show", "list", "ready", "blocked", "create", "update", "ask", "review", "pr",
            "branch", "dep", "project",
        ] {
            assert!(help.contains(&format!("\n  {shown}")), "{shown}:\n{help}");
        }
        // `hook` and `statusline` are internal (hidden); `answer` and `questions` (T-0151) stay hidden until
        // they are built.
        for later in ["hook", "statusline", "answer", "questions"] {
            assert!(
                !help.contains(&format!("\n  {later} ")),
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
