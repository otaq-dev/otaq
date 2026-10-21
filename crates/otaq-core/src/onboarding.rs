//! What there is for Flow onboarding to look at (ONB-03, d0108 smaller rule 6): Claude Code history has at least one
//! entry, the project's repo has a commit, `gh` or `glab` is signed in. Any one offers **Look and propose a flow**;
//! none shows the presets. **No content is read**: the history file is read only until its first line that parses
//! as an entry, and nothing of that line is kept, logged or returned. The agent's name for the go-ahead's line
//! comes with it: the tool of the agent with the Project agent role (d0086).

use std::io::BufRead;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agent_tools::Tool as AgentTool;
use crate::agents::Role;
use crate::agents::gate::LaunchGate;
use crate::agents::resolve;
use crate::board::{Board, BoardError};
use crate::tools::{self, CheckEnv, Tool};

/// Claude Code's prompt history, in its config folder (ONB-01).
pub const HISTORY_FILE: &str = "history.jsonl";

/// The answer to `onboarding.sources`: facts only, no counts and no content (d0108 changelog 2026-10-21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sources {
    /// The project's name, for "proposes a flow for shop-web".
    pub project_name: String,
    /// Claude Code history has at least one entry.
    pub history: bool,
    /// The project's repo has at least one commit.
    pub commits: bool,
    /// `gh auth status` succeeds.
    pub gh: bool,
    /// `glab auth status` succeeds.
    pub glab: bool,
    /// The Project agent's tool: it is the one that looks ("Claude looks at…").
    pub tool: AgentTool,
}

impl Sources {
    /// Whether there is anything to look at (else the presets, "Nothing to look at yet").
    pub fn any(&self) -> bool {
        self.history || self.commits || self.gh || self.glab
    }
}

/// What there is for `project` (its key). Runs `git rev-parse`, `gh auth status` and `glab auth status`; reads
/// the history file only as far as its first entry.
pub async fn sources(
    board: &Board,
    gate: &LaunchGate,
    env: &CheckEnv,
    project: &str,
) -> Result<Sources, BoardError> {
    let project = board.project(project).await?;
    let agent = resolve::role_agent(board.store(), Role::ProjectAgent).await?;
    let tool = gate.model_of(&agent).await?.tool;
    let commits = async {
        match board.git() {
            Ok(git) => matches!(
                git.resolve(Path::new(&project.repo_path), "HEAD").await,
                Ok(Some(_))
            ),
            Err(_) => false,
        }
    };
    let (commits, gh, glab) = tokio::join!(
        commits,
        signed_in(Tool::Gh, env),
        signed_in(Tool::Glab, env)
    );
    let history = env
        .claude_config_dir
        .as_deref()
        .is_some_and(|dir| has_history(&dir.join(HISTORY_FILE)));
    Ok(Sources {
        project_name: project.name,
        history,
        commits,
        gh,
        glab,
        tool,
    })
}

async fn signed_in(tool: Tool, env: &CheckEnv) -> bool {
    tools::check_one(tool, env).await.logged_in == Some(true)
}

/// Whether the history file has at least one entry: a line that is a JSON object with a `display` string. Reads
/// line by line and stops at the first one; a missing or unreadable file has none. Nothing is kept.
pub fn has_history(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = std::io::BufReader::new(file);
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return false,
            Ok(_) => {
                if serde_json::from_slice::<Entry>(&line).is_ok() {
                    return true;
                }
            }
        }
    }
}

/// An entry's shape, holding none of it: `display` only has to be a string.
#[derive(Deserialize)]
struct Entry {
    #[allow(dead_code)]
    display: AString,
}

/// Any string, not kept.
struct AString;

impl<'de> Deserialize<'de> for AString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = AString;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string")
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<AString, E> {
                Ok(AString)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(text: &str) -> bool {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(HISTORY_FILE);
        std::fs::write(&path, text).unwrap();
        has_history(&path)
    }

    #[test]
    fn one_entry_is_enough() {
        // Made up, shaped like the real file.
        assert!(history(
            r#"{"display":"/commit","pastedContents":{},"project":"/Users/sam/pocket-notes","sessionId":"s1","timestamp":1760000000000}"#
        ));
        // Lines Otaq can't read come first: the reader goes on to the entry.
        assert!(history(
            "not json\n{\"display\":3}\n[]\n{\"display\":\"fix the tests\"}\n"
        ));
    }

    #[test]
    fn no_entry_means_no_history() {
        assert!(!history(""));
        assert!(!history(
            "\n\nnot json\n{\"display\":null}\n{\"project\":\"/x\"}\n"
        ));
        assert!(!has_history(Path::new("/nonexistent/folder/history.jsonl")));
    }
}
