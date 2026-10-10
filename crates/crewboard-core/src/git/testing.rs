//! Temporary git repositories for tests (d0056): repos with commits and branches, a bare "origin" whose
//! default branch is `develop`, and repos with no remote. Everything lives in a temporary folder that is removed
//! when the value is dropped.
//!
//! Setup commands ignore the user's global and system git config (so signing or hooks there can't break a
//! test) and use a fixed author. The [`Git`](super::Git) runner under test still runs with the user's config.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A folder of temporary repositories.
pub struct TempRepos {
    dir: TempDir,
}

impl Default for TempRepos {
    fn default() -> Self {
        Self::new()
    }
}

impl TempRepos {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().expect("temporary folder"),
        }
    }

    /// The folder holding the repositories (canonical, so it compares equal to what `git` reports).
    pub fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().expect("canonical temp path")
    }

    /// A repository with no remote, on branch `main`, with `commits` commits.
    pub fn repo(&self, name: &str, commits: usize) -> PathBuf {
        let path = self.root().join(name);
        std::fs::create_dir_all(&path).expect("repo folder");
        git(&path, &["init", "--quiet", "--initial-branch=main"]);
        local_identity(&path);
        for n in 1..=commits {
            commit(&path, &format!("file{n}.txt"), &format!("{n}\n"));
        }
        path
    }

    /// A bare `origin` whose default branch is `default_branch`, holding `commits` commits, and a clone of it
    /// named `name` (so `origin/HEAD` points to `default_branch`). Returns the clone.
    pub fn clone_with_origin(&self, name: &str, default_branch: &str, commits: usize) -> PathBuf {
        let origin = self.root().join(format!("{name}-origin.git"));
        let initial = format!("--initial-branch={default_branch}");
        git(
            &self.root(),
            &["init", "--quiet", "--bare", &initial, path_str(&origin)],
        );
        let seed = self.repo(&format!("{name}-seed"), 0);
        git(&seed, &["checkout", "--quiet", "-b", default_branch]);
        for n in 1..=commits.max(1) {
            commit(&seed, &format!("file{n}.txt"), &format!("{n}\n"));
        }
        git(&seed, &["remote", "add", "origin", path_str(&origin)]);
        git(&seed, &["push", "--quiet", "origin", default_branch]);
        let clone = self.root().join(name);
        git(
            &self.root(),
            &["clone", "--quiet", path_str(&origin), path_str(&clone)],
        );
        local_identity(&clone);
        clone
    }

    /// A plain folder that is not a git repository.
    pub fn plain_folder(&self, name: &str) -> PathBuf {
        let path = self.root().join(name);
        std::fs::create_dir_all(&path).expect("plain folder");
        path
    }
}

/// A fixed author and no signing in the repository's own config, so commits that the code under test makes
/// with the user's `git` (a checkpoint, a squash merge) work on any machine.
fn local_identity(repo: &Path) {
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
}

/// Writes `file` with `content` and commits it.
pub fn commit(repo: &Path, file: &str, content: &str) {
    std::fs::write(repo.join(file), content).expect("write file");
    git(repo, &["add", "--", file]);
    git(repo, &["commit", "--quiet", "-m", &format!("edit {file}")]);
}

/// Runs a setup `git` command and panics if it fails. Returns its trimmed standard output.
pub fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Every file and folder under `path` (relative, sorted), `.git` included, for "nothing was written" checks.
pub fn snapshot(path: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("readable folder") {
            let path = entry.expect("entry").path();
            let relative = path.strip_prefix(root).expect("under root");
            out.push(relative.display().to_string());
            if path.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(path, path, &mut out);
    out.sort();
    out
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temp path")
}
