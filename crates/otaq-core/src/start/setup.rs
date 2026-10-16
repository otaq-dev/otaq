//! Env files and the setup command (SES-01 steps 4 and 5; d0010, d0023, d0007).
//!
//! The project's **env files** (e.g. `.env.local`, which git doesn't carry) are copied from the user's repo into
//! the new worktree at the same relative path, then the project's **setup command** (e.g. `pnpm install`) runs
//! in the worktree. Both come from the app's settings, never from a file in the repo (d0023). Re-running
//! overwrites the copies and runs setup again. With no env files and no setup command both steps pass straight
//! through.
//!
//! The setup output (stdout and stderr together) goes to a per-ticket log in the data folder
//! (`logs/setup/<KEY>.log`, replaced on every run), so a failed start can show it later (T-0063).

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
            return Err(StartError::Other(format!(
                "`{}` not found in {}",
                relative.display(),
                home_relative(repo)
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

/// Whether the setup command ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setup {
    /// No setup command is set.
    Skipped,
    Succeeded,
}

/// Runs `command` with `sh -c` (`cmd /c` on Windows, [`crate::shell::one_line`]) in `worktree`, with the service's (the user's) environment, writing its output to
/// `log`. A non-zero exit fails the step with the exit code and the last lines of output.
///
/// `project_env` is the project's applied variables ([`crate::session::env::applied`]), set on top of the
/// service's environment with the same order rule as sessions (ENV-02, d0082).
pub async fn run_setup(
    command: &str,
    worktree: &Path,
    log: &Path,
    project_env: &[(String, String)],
) -> Result<Setup, StartError> {
    let command = command.trim();
    if command.is_empty() {
        return Ok(Setup::Skipped);
    }
    let cannot = |what: &str, e: std::io::Error| StartError::Other(format!("can't {what}: {e}"));
    if let Some(parent) = log.parent() {
        crate::data_dir::DataDir::ensure_dir(parent)
            .map_err(|e| cannot("create the setup log folder", e))?;
    }
    let mut file = std::fs::File::create(log).map_err(|e| cannot("write the setup log", e))?;
    {
        use std::io::Write;
        writeln!(file, "$ {command}").map_err(|e| cannot("write the setup log", e))?;
    }
    let stderr = file
        .try_clone()
        .map_err(|e| cannot("write the setup log", e))?;
    let shell = crate::shell::one_line(command);
    let status = tokio::process::Command::new(&shell.program)
        .args(&shell.args)
        .envs(crate::session::env::layered(
            project_env,
            &shell.env,
            None,
            crate::tools::Os::current(),
        ))
        .current_dir(worktree)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| cannot("run the setup command", e))?;
    if status.success() {
        return Ok(Setup::Succeeded);
    }
    let exit = status
        .code()
        .map_or_else(|| "killed by a signal".to_owned(), |c| format!("exit {c}"));
    let output = std::fs::read_to_string(log).unwrap_or_default();
    let tail = last_lines(&output, FAILURE_TAIL_LINES);
    Err(StartError::Other(if tail.is_empty() {
        format!("setup command failed ({exit})")
    } else {
        format!("setup command failed ({exit}):\n{tail}")
    }))
}

/// The last `n` non-empty lines of `text`, without the `$ command` header.
fn last_lines(text: &str, n: usize) -> String {
    let mut tail = VecDeque::with_capacity(n);
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
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
    }
}
