//! Env files and the setup command (SES-01 steps 4 and 5; d0010, d0023, d0007).
//!
//! The project's **env files** (e.g. `.env.local`, which git doesn't carry) are copied from the user's repo into
//! the new worktree at the same relative path, then the project's **setup command** (e.g. `pnpm install`) runs
//! in the worktree. Both come from the app's settings, never from a file in the repo (d0023). Re-running
//! overwrites the copies and runs setup again. With no env files and no setup command both steps pass straight
//! through.
//!
//! The setup output (stdout and stderr together) goes to a per-ticket log in the data folder
//! (`logs/setup/<KEY>.log`, begun anew by every start that sets up a worktree, [`begin_setup_log`]), so a failed
//! start can show it later (T-0063). A seeded worktree's log starts with where it was seeded from (WRM-04); the
//! setup command's output is appended after it.

use std::collections::VecDeque;
use std::path::{Component, Path};
use std::process::Stdio;

use super::StartError;

/// How many output lines a failed setup quotes in its message.
const FAILURE_TAIL_LINES: usize = 10;

/// Copies each env file from `repo` into `worktree` at the same relative path, replacing an older copy.
pub fn copy_env_files(repo: &Path, worktree: &Path, files: &[String]) -> Result<(), StartError> {
    for file in files {
        let relative = Path::new(file.trim());
        let escapes = relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|c| matches!(c, Component::ParentDir));
        if escapes {
            return Err(StartError::Other(format!(
                "env file `{file}` must be a path inside the repository, like .env.local"
            )));
        }
        let source = repo.join(relative);
        if !source.is_file() {
            return Err(StartError::Other(crate::board::settings::missing_env_file(
                &relative.display().to_string(),
                repo,
            )));
        }
        let target = worktree.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StartError::Other(format!("can't create {}: {e}", parent.display()))
            })?;
        }
        std::fs::copy(&source, &target).map_err(|e| {
            StartError::Other(format!(
                "can't copy `{}` into the worktree: {e}",
                relative.display()
            ))
        })?;
    }
    Ok(())
}

/// Begins the setup log anew: with `first_line` (where a seeded worktree came from), or empty (removed). The setup
/// command's output is appended after it ([`run_setup`]).
pub fn begin_setup_log(log: &Path, first_line: Option<&str>) -> std::io::Result<()> {
    let Some(line) = first_line else {
        return match std::fs::remove_file(log) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    };
    if let Some(parent) = log.parent() {
        crate::data_dir::DataDir::ensure_dir(parent)?;
    }
    std::fs::write(log, format!("{line}\n"))
}

/// Whether the setup command ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setup {
    /// No setup command is set.
    Skipped,
    Succeeded,
}

/// Runs `command` with `sh -c` (`cmd /c` on Windows, [`crate::shell::one_line`]) in `worktree`, with the service's (the user's) environment, appending its output to
/// `log` after a `$ <command>` line. A non-zero exit fails the step with the exit code and the last lines of output.
///
/// `project_env` is the project's applied variables ([`crate::session::env::applied`]), set on top of the
/// service's environment with the same order rule as sessions (ENV-02, d0082); `own` is Otaq's own variables for
/// the ticket (`OTAQ_TICKET`, and the ticket resource variables where the project has them, d0087), which win.
pub async fn run_setup(
    command: &str,
    worktree: &Path,
    log: &Path,
    project_env: &[(String, String)],
    own: &[(String, String)],
) -> Result<Setup, StartError> {
    if command.trim().is_empty() {
        return Ok(Setup::Skipped);
    }
    run_command("setup command", command, worktree, log, project_env, own).await?;
    Ok(Setup::Succeeded)
}

/// Runs one project command (setup, or a ticket resource command) as [`run_setup`] describes, in `cwd`, appending
/// to `log`. `what` names it in the failure message: "setup command failed (exit 1):" and its last lines, which
/// are this command's output only, never an earlier command's in the same log.
pub(crate) async fn run_command(
    what: &str,
    command: &str,
    cwd: &Path,
    log: &Path,
    project_env: &[(String, String)],
    own: &[(String, String)],
) -> Result<(), StartError> {
    let command = command.trim();
    let cannot =
        |action: &str, e: std::io::Error| StartError::Other(format!("can't {action}: {e}"));
    if let Some(parent) = log.parent() {
        crate::data_dir::DataDir::ensure_dir(parent)
            .map_err(|e| cannot("create the setup log folder", e))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| cannot("write the setup log", e))?;
    let from = file.metadata().map_or(0, |m| m.len());
    {
        use std::io::Write;
        writeln!(file, "$ {command}").map_err(|e| cannot("write the setup log", e))?;
    }
    let stderr = file
        .try_clone()
        .map_err(|e| cannot("write the setup log", e))?;
    let shell = crate::shell::one_line(command);
    let own: Vec<(String, String)> = own.iter().cloned().chain(shell.env).collect();
    let status = tokio::process::Command::new(&shell.program)
        .args(&shell.args)
        .envs(crate::session::env::layered(
            project_env,
            &own,
            None,
            crate::tools::Os::current(),
        ))
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| cannot(&format!("run the {what}"), e))?;
    if status.success() {
        return Ok(());
    }
    let exit = status
        .code()
        .map_or_else(|| "killed by a signal".to_owned(), |c| format!("exit {c}"));
    let output = std::fs::read(log).unwrap_or_default();
    let output = String::from_utf8_lossy(output.get(from as usize..).unwrap_or_default());
    let tail = last_lines(&output, FAILURE_TAIL_LINES);
    Err(StartError::Other(if tail.is_empty() {
        format!("{what} failed ({exit})")
    } else {
        format!("{what} failed ({exit}):\n{tail}")
    }))
}

/// The last `n` non-empty lines of `text` after the `$ command` header (and so without a seed line before it).
fn last_lines(text: &str, n: usize) -> String {
    let mut tail = VecDeque::with_capacity(n);
    let output = text.lines().skip_while(|l| !l.starts_with("$ ")).skip(1);
    for line in output.filter(|l| !l.trim().is_empty()) {
        if tail.len() == n {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    tail.into_iter().collect::<Vec<_>>().join("\n")
}

/// `~/code/shop-web` for a folder in the user's home, the full path otherwise.
pub fn home_relative(path: &Path) -> String {
    match std::env::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned)) {
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_lines_skip_the_header_and_blanks() {
        assert_eq!(last_lines("$ cmd\na\n\nb\nc\n", 2), "b\nc");
        assert_eq!(last_lines("$ cmd\n", 3), "");
        assert_eq!(
            last_lines("Started from your checkout of shop-web.\n$ cmd\na\n", 3),
            "a"
        );
    }
}
