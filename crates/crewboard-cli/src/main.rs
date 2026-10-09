//! The `crewboard` command: launches the app and provides the `daemon`, `attach`, `hook` and agent-facing
//! subcommands (d0002, d0031).

mod attach;
mod cli;
mod daemon;
mod launch;
mod output;

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
        Some(Command::Attach { key }) => attach::run(key, &cli.global),
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
