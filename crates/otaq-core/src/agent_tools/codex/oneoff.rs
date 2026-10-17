//! "Ask the agent" (RUN-02; d0086, d0088): a one-off `codex exec --sandbox read-only --ephemeral
//! --skip-git-repo-check --json [-m <model>] "<prompt>"` in the project's folder (checked with `codex exec --help`,
//! 0.157.1). `exec` answers once and exits; the read-only sandbox lets its commands read and nothing else;
//! `--ephemeral` keeps no session file (nothing to resume); `--skip-git-repo-check` lets it run in a folder that isn't
//! a git repository; `--json` prints its events as JSON lines. Never an approval or bypass flag (d0011).
//!
//! Its output, one JSON object per line: `thread.started`, `turn.started`, `item.started` / `item.completed` with
//! an `item` (`command_execution` with its `command`, `agent_message` with its `text`, `reasoning`, `error`),
//! `error` (a retry), then `turn.completed` or `turn.failed` with `error.message`. The thread, turn, `error` lines and
//! `turn.failed` were captured with `CODEX_HOME` in a temporary folder (signed out, so no command ran); the
//! `command_execution` and `agent_message` items are written from Codex's `exec` event types (T-0369 replaces them
//! with a capture).

use serde_json::Value;

use super::{COMMAND_NAME, CodexContext};
use crate::agent_tools::{LaunchError, OneOffCommand, OneOffLine, OneOffRequest, OneOffTool};

/// The options before the prompt.
const FLAGS: [&str; 6] = [
    "exec",
    "--sandbox",
    "read-only",
    "--ephemeral",
    "--skip-git-repo-check",
    "--json",
];

/// The tool name a command shows under in the progress line.
const COMMAND_TOOL: &str = "shell";

pub(super) fn command(
    _context: &CodexContext,
    request: &OneOffRequest,
) -> Result<OneOffCommand, LaunchError> {
    let mut args: Vec<String> = FLAGS.map(str::to_owned).to_vec();
    if let Some(model) = &request.model {
        args.extend(["-m".to_owned(), model.clone()]);
    }
    args.push(request.prompt.clone());
    Ok(OneOffCommand {
        program: COMMAND_NAME.to_owned(),
        args,
        cwd: request.cwd.clone(),
        env: Vec::new(),
    })
}

/// A command it starts becomes progress; each agent message is an answer (the last one counts); a failed turn is
/// the error.
pub(super) fn read_line(line: &str) -> Option<OneOffLine> {
    tracing::debug!(line, "a line of the one-off question's output");
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let item = value.get("item");
    let item_type = item.and_then(|i| i.get("type")).and_then(Value::as_str);
    match (value.get("type")?.as_str()?, item_type) {
        ("item.started", Some("command_execution")) => {
            let command = item?.get("command")?.as_str()?;
            Some(OneOffLine::Tools(vec![OneOffTool {
                name: COMMAND_TOOL.to_owned(),
                target: Some(script_of(command).to_owned()),
            }]))
        }
        ("item.completed", Some("agent_message")) => Some(OneOffLine::Answer {
            text: item?.get("text")?.as_str()?.to_owned(),
            is_error: false,
        }),
        ("turn.failed", _) => Some(OneOffLine::Answer {
            text: value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            is_error: true,
        }),
        _ => None,
    }
}

/// `/bin/zsh -lc 'cat package.json'` → `cat package.json`: the script a shell wrapper runs, else the command.
fn script_of(command: &str) -> &str {
    let command = command.trim();
    command
        .split_once(" -lc ")
        .or_else(|| command.split_once(" -c "))
        .map(|(_, script)| script.trim())
        .map_or(command, |script| {
            script
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .or_else(|| script.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
                .unwrap_or(script)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::AgentTool;
    use crate::agent_tools::codex::Codex;
    use crate::agent_tools::test_support::context;

    fn request(model: Option<&str>) -> OneOffRequest {
        OneOffRequest {
            cwd: "/r/shop-web".into(),
            prompt: "Suggest a run command".into(),
            model: model.map(str::to_owned),
        }
    }

    /// `codex exec` read-only, with no session file, JSON lines out; the model before the prompt; never an
    /// approval or bypass flag.
    #[test]
    fn the_question_reads_only_and_passes_the_model_before_the_prompt() {
        let bin = tempfile::tempdir().unwrap();
        let adapter = Codex::new(context(bin.path(), None));
        let plain = adapter.one_off(&request(None)).unwrap();
        assert_eq!(plain.program, "codex");
        assert_eq!(plain.cwd, std::path::Path::new("/r/shop-web"));
        assert_eq!(
            plain.args,
            [
                "exec",
                "--sandbox",
                "read-only",
                "--ephemeral",
                "--skip-git-repo-check",
                "--json",
                "Suggest a run command"
            ]
        );
        let strong = adapter.one_off(&request(Some("gpt-6-sol"))).unwrap();
        assert_eq!(
            strong.args,
            [
                "exec",
                "--sandbox",
                "read-only",
                "--ephemeral",
                "--skip-git-repo-check",
                "--json",
                "-m",
                "gpt-6-sol",
                "Suggest a run command"
            ]
        );
        for flag in [
            "--dangerously-bypass-approvals-and-sandbox",
            "--dangerously-bypass-hook-trust",
            "--full-auto",
            "--approve-for-me",
            "workspace-write",
            "danger-full-access",
        ] {
            assert!(!strong.args.iter().any(|a| a == flag), "{flag}");
        }
    }

    /// The lines of `codex exec --json`: the thread and turn lines, a retry and `turn.failed` as captured; the
    /// command and the agent's message written from Codex's `exec` event types (T-0369 replaces them with a capture).
    #[test]
    fn commands_are_progress_the_message_is_the_answer() {
        let adapter = Codex::new(context(std::path::Path::new("/nonexistent"), None));
        for ignored in [
            r#"{"type":"thread.started","thread_id":"01a14bae-dd63-7c03-b96e-8f7557586707"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"error","message":"Reconnecting... 2/5 (unexpected status 401 Unauthorized)"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Falling back from WebSockets to HTTPS transport."}}"#,
            r#"{"type":"item.completed","item":{"id":"item_1","type":"reasoning","text":"Looking at package.json"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":5}}"#,
            "not json",
        ] {
            assert_eq!(adapter.one_off_line(ignored), None, "{ignored}");
        }
        let command = r#"{"type":"item.started","item":{"id":"item_2","type":"command_execution","command":"/bin/zsh -lc 'cat package.json'","aggregated_output":"","status":"in_progress"}}"#;
        assert_eq!(
            adapter.one_off_line(command),
            Some(OneOffLine::Tools(vec![OneOffTool {
                name: "shell".into(),
                target: Some("cat package.json".into())
            }]))
        );
        let message = r#"{"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"{\"command\": \"pnpm dev --port $PORT\"}"}}"#;
        assert_eq!(
            adapter.one_off_line(message),
            Some(OneOffLine::Answer {
                text: r#"{"command": "pnpm dev --port $PORT"}"#.into(),
                is_error: false
            })
        );
        let failed = r#"{"type":"turn.failed","error":{"message":"unexpected status 401 Unauthorized: Missing bearer or basic authentication in header"}}"#;
        assert_eq!(
            adapter.one_off_line(failed),
            Some(OneOffLine::Answer {
                text: "unexpected status 401 Unauthorized: Missing bearer or basic authentication in header"
                    .into(),
                is_error: true
            })
        );
    }

    #[test]
    fn the_script_of_a_shell_wrapper() {
        assert_eq!(
            script_of("/bin/zsh -lc 'cat package.json'"),
            "cat package.json"
        );
        assert_eq!(script_of("bash -c \"ls -a\""), "ls -a");
        assert_eq!(script_of("rg --files"), "rg --files");
    }
}
