//! The `crewboard` command: launches the app and provides the `daemon`, `attach`, `hook` and agent-facing
//! subcommands (d0002, d0031).

mod agent;
mod attach;
mod cli;
mod daemon;
mod deps;
mod hook;
mod launch;
mod output;
mod pr;
mod prime;
mod read;
mod service;
mod statusline;
mod target;
mod write;

use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind as ClapErrorKind;
use crewboard_core::protocol::{ErrorBody, ErrorKind};

use cli::{Cli, Command};
use output::{EXIT_ERROR, EXIT_USAGE, Mode, Output};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => return usage_error(e, cli::wants_json(&args)),
    };
    let out = Output::new(cli.global.mode());
    let result = match cli.command {
        None => launch::launch_app(),
        Some(Command::Daemon(command)) => daemon::run(command, &out),
        Some(Command::Attach { key, validator }) => attach::run(key, validator, &cli.global),
        Some(Command::Prime { key, validate }) => prime::run(key, validate, &cli.global, &out),
        Some(Command::Show {
            key,
            long,
            children,
        }) => read::show(
            read::ShowArgs {
                key,
                long,
                children,
            },
            &cli.global,
            &out,
        ),
        Some(Command::List {
            status,
            tree,
            parent,
            priority,
        }) => read::list(
            read::ListArgs {
                status,
                tree,
                parent,
                priority,
            },
            &cli.global,
            &out,
        ),
        Some(Command::Ready) => read::readiness(false, &cli.global, &out),
        Some(Command::Blocked) => read::readiness(true, &cli.global, &out),
        Some(Command::Create {
            title,
            description,
            priority,
            deps,
            stdin,
            file,
            kind,
            parent,
        }) => write::create(
            write::CreateArgs {
                title,
                description,
                priority,
                deps,
                stdin,
                file,
                epic: kind.as_deref() == Some("epic"),
                parent,
            },
            &cli.global,
            &out,
        ),
        Some(Command::Update {
            key,
            title,
            description,
            priority,
            stdin,
        }) => write::update(
            write::UpdateArgs {
                key,
                title,
                description,
                priority,
                stdin,
            },
            &cli.global,
            &out,
        ),
        Some(Command::Ask {
            target,
            stdin,
            options,
            recommended,
        }) => agent::ask(
            agent::AskArgs {
                target,
                stdin,
                options,
                recommended,
            },
            &cli.global,
            &out,
        ),
        Some(Command::Review(cli::ReviewCommand::Submit { key, stdin })) => {
            agent::submit(key, stdin, &cli.global, &out)
        }
        Some(Command::Validate(command)) => agent::validate(command, &cli.global, &out),
        Some(Command::Agent(cli::AgentCommand::Name { name, rename })) => {
            agent::name(&name, rename, &cli.global, &out)
        }
        Some(Command::Review(cli::ReviewCommand::Feedback { key })) => {
            agent::feedback(key, &cli.global, &out)
        }
        Some(Command::Pr(command)) => pr::pr(command, &cli.global, &out),
        Some(Command::Branch(command)) => pr::branch(command, &cli.global, &out),
        Some(Command::Answer { id, stdin }) => agent::answer(id, stdin, &cli.global, &out),
        Some(Command::Questions { key }) => agent::questions(key, &cli.global, &out),
        Some(Command::Dep(command)) => deps::run(command, &cli.global, &out),
        Some(Command::Project(command)) => read::project(command, &cli.global, &out),
        Some(Command::Hook { event }) => {
            hook::run(&event);
            Ok(())
        }
        Some(Command::Statusline) => std::process::exit(statusline::run()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => out.error(&error, EXIT_ERROR),
    }
}

/// Help and version go to stdout as usual. Other parse errors follow the output mode: with `--json` they are
/// `{"error": {"kind": "usage", ...}}`.
fn usage_error(error: clap::Error, json: bool) -> ExitCode {
    if matches!(
        error.kind(),
        ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
    ) || !json
    {
        error.exit();
    }
    let rendered = error.render().to_string();
    let first_line = rendered.lines().next().unwrap_or_default();
    let message = first_line.strip_prefix("error: ").unwrap_or(first_line);
    Output::new(Mode::Json).error(&ErrorBody::new(ErrorKind::Usage, message), EXIT_USAGE)
}

#[cfg(test)]
mod tests {
    #[test]
    fn binary_is_named_after_the_command() {
        assert_eq!(env!("CARGO_BIN_NAME"), crewboard_core::naming::COMMAND_NAME);
    }
}
