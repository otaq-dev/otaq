//! A project's environment variables in what Otaq starts (ENV-01…03; d0082).
//!
//! A project lists `NAME = value` rows ([`EnvVar`]). The rows with no problem ([`applied`]) are set in everything
//! Otaq starts in the project: every session ([`super::Sessions::spawn`] asks its [`EnvProvider`] at each spawn),
//! the setup command ([`crate::start::run_setup`]) and the git commands Otaq runs in the project's repository and
//! worktrees ([`crate::board::Board::project_git`]), so the user's hooks see them. `gh` and `az` don't get them.
//!
//! **The order rule.** A program gets the service's environment, then the project variables on top, then Otaq's
//! own variables on top of those: **Otaq's own win** over a row of the same name, because Otaq needs them to
//! work (the data folder variable, `OTAQ_TICKET`, the role and epic variables, `PORT`, `TERM`, `COLORTERM`, and
//! for setup on Windows the variable carrying the command). The one exception is `PATH`: a `PATH` row replaces
//! the service's `PATH`, and where Otaq puts its bin folder in front of `PATH` (sessions), it puts it in front
//! of the row's value instead, so `otaq` still resolves. Names compare as the OS does: ignoring case on Windows.
//!
//! **When a change applies.** The rows are read at every spawn, setup and git command, never cached: a change
//! reaches what starts (or resumes) after it. A running session keeps the values it started with; nothing
//! restarts and nothing is offered.

use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::board::settings::{EnvVar, env_var_problems, same_env_name};
use crate::tools::Os;

/// The applied rows of the project with this id, read when asked: set once at service start
/// ([`super::Sessions::set_env_provider`]).
pub type EnvProvider = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Vec<(String, String)>> + Send>> + Send + Sync,
>;

/// Otaq's own variables for the ticket with this id (the ticket resource variables, d0087), read when asked: set
/// once at service start ([`super::Sessions::set_ticket_env_provider`]). Only sessions of a ticket ask.
pub type TicketEnvProvider = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Vec<(String, String)>> + Send>> + Send + Sync,
>;

/// The rows that apply, in row order: every row with no problem (ENV-03). The first of two rows with the same
/// name applies; on Windows names that differ only in case are the same.
pub fn applied(env_vars: &[EnvVar], os: Os) -> Vec<(String, String)> {
    env_vars
        .iter()
        .zip(env_var_problems(env_vars, os))
        .filter(|(_, problem)| problem.is_none())
        .map(|(row, _)| (row.name.clone(), row.value.clone()))
        .collect()
}

/// What to set on top of the inherited environment, by the order rule (module doc): the project's `rows`, then
/// Otaq's `own` variables, which win. When both set `PATH`, Otaq's `PATH` keeps what it put in front of
/// `inherited_path` (the service's `PATH`, which `own`'s extends) and continues with the row's value.
pub fn layered(
    rows: &[(String, String)],
    own: &[(String, String)],
    inherited_path: Option<&OsStr>,
    os: Os,
) -> Vec<(String, String)> {
    let is_path = |name: &str| same_env_name(name, "PATH", os);
    let row_path = rows.iter().find(|(name, _)| is_path(name));
    let mut env: Vec<(String, String)> = rows
        .iter()
        .filter(|(name, _)| !own.iter().any(|(o, _)| same_env_name(o, name, os)))
        .cloned()
        .collect();
    for (name, value) in own {
        let value = match row_path {
            Some((_, row_value)) if is_path(name) => {
                path_with_row(value, inherited_path.unwrap_or_default(), row_value)
            }
            _ => value.clone(),
        };
        env.push((name.clone(), value));
    }
    env
}

/// Otaq's `PATH` (`own`, which is what Otaq put in front, then `inherited`) with the row's value in place of
/// `inherited`. When `own` doesn't end with `inherited`, only its first folder counts as Otaq's.
fn path_with_row(own: &str, inherited: &OsStr, row: &str) -> String {
    let own: Vec<PathBuf> = split(OsStr::new(own));
    let inherited: Vec<PathBuf> = split(inherited);
    let front = if own.ends_with(&inherited) {
        &own[..own.len() - inherited.len()]
    } else {
        &own[..own.len().min(1)]
    };
    let parts = front.iter().cloned().chain(split(OsStr::new(row)));
    std::env::join_paths(parts)
        .unwrap_or_else(|_| OsString::from(row))
        .to_string_lossy()
        .into_owned()
}

fn split(path: &OsStr) -> Vec<PathBuf> {
    if path.is_empty() {
        Vec::new()
    } else {
        std::env::split_paths(path).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::settings::{
        ENV_VAR_EMPTY_NAME, env_var_duplicate_message, env_var_equals_message, env_var_null_message,
    };

    fn rows(pairs: &[(&str, &str)]) -> Vec<EnvVar> {
        pairs.iter().map(|(n, v)| EnvVar::new(n, v)).collect()
    }

    fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn joined(dirs: &[&str]) -> String {
        std::env::join_paths(dirs)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn each_problem_has_its_wording() {
        let env = rows(&[
            ("", "x"),
            ("MAX=JOBS", "2"),
            ("A\0B", "1"),
            ("CI", "true"),
            ("CI", "false"),
        ]);
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            assert_eq!(
                env_var_problems(&env, os),
                [
                    Some(ENV_VAR_EMPTY_NAME.to_owned()),
                    Some(env_var_equals_message("MAX=JOBS")),
                    Some(env_var_null_message("A\0B")),
                    None,
                    Some(env_var_duplicate_message("CI")),
                ]
            );
        }
        assert_eq!(ENV_VAR_EMPTY_NAME, "Add a name, or remove the row");
        assert_eq!(
            env_var_equals_message("MAX=JOBS"),
            "“MAX=JOBS” can’t be a name: a name can’t contain “=”. This row isn’t set until it’s fixed."
        );
        assert_eq!(
            env_var_duplicate_message("NODE_OPTIONS"),
            "NODE_OPTIONS is already set above. Only the first row applies; remove one of them."
        );
        assert!(env_var_null_message("A\0B").starts_with("“A\\0B” can’t be a name"));
    }

    #[test]
    fn spaces_digits_and_any_value_are_allowed() {
        let env = rows(&[("MY VAR", ""), ("1ST", "a=b"), ("X", " spaced \0? ")]);
        assert_eq!(env_var_problems(&env, Os::current()), [None, None, None]);
        assert_eq!(applied(&env, Os::current()).len(), 3);
    }

    #[test]
    fn names_differing_in_case_clash_on_windows_only() {
        let env = rows(&[("NODE_OPTIONS", "--a"), ("node_options", "--b")]);
        assert_eq!(
            env_var_problems(&env, Os::Windows)[1],
            Some(env_var_duplicate_message("node_options"))
        );
        assert_eq!(
            applied(&env, Os::Windows),
            pairs(&[("NODE_OPTIONS", "--a")])
        );
        for os in [Os::Mac, Os::Linux] {
            assert_eq!(env_var_problems(&env, os), [None, None]);
            assert_eq!(
                applied(&env, os),
                pairs(&[("NODE_OPTIONS", "--a"), ("node_options", "--b")])
            );
        }
    }

    #[test]
    fn applied_keeps_row_order_and_the_first_of_a_name() {
        let env = rows(&[
            ("B", "1"),
            ("", "skipped"),
            ("A", "2"),
            ("B", "3"),
            ("C", "4"),
        ]);
        assert_eq!(
            applied(&env, Os::Linux),
            pairs(&[("B", "1"), ("A", "2"), ("C", "4")])
        );
        assert!(applied(&[], Os::current()).is_empty());
    }

    #[test]
    fn own_variables_win() {
        let rows = pairs(&[("PORT", "1"), ("CI", "true"), ("port", "2")]);
        let own = pairs(&[("PORT", "4019")]);
        assert_eq!(
            layered(&rows, &own, None, Os::Linux),
            pairs(&[("CI", "true"), ("port", "2"), ("PORT", "4019")])
        );
        // On Windows `port` is `PORT`.
        assert_eq!(
            layered(&rows, &own, None, Os::Windows),
            pairs(&[("CI", "true"), ("PORT", "4019")])
        );
    }

    #[test]
    fn a_path_row_keeps_the_bin_folder_first() {
        let service = joined(&["/usr/bin", "/bin"]);
        let own = pairs(&[("PATH", &joined(&["/opt/cb/bin", "/usr/bin", "/bin"]))]);
        let row = joined(&["/custom/bin", "/usr/bin"]);
        let env = layered(
            &pairs(&[("PATH", &row)]),
            &own,
            Some(OsStr::new(&service)),
            Os::Linux,
        );
        assert_eq!(
            env,
            pairs(&[("PATH", &joined(&["/opt/cb/bin", "/custom/bin", "/usr/bin"]))])
        );
        // A `Path` row is `PATH` on Windows.
        let env = layered(
            &pairs(&[("Path", &row)]),
            &own,
            Some(OsStr::new(&service)),
            Os::Windows,
        );
        assert_eq!(
            env,
            pairs(&[("PATH", &joined(&["/opt/cb/bin", "/custom/bin", "/usr/bin"]))])
        );
        // With nothing of Otaq's in PATH (setup, git), the row's PATH is set as written.
        assert_eq!(
            layered(&pairs(&[("PATH", &row)]), &[], None, Os::Linux),
            pairs(&[("PATH", &row)])
        );
        // No PATH row: Otaq's PATH as it is.
        assert_eq!(
            layered(&[], &own, Some(OsStr::new(&service)), Os::Linux),
            own
        );
    }
}
