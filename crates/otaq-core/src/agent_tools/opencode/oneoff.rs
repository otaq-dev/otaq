//! "Ask the agent" (RUN-02; d0011, d0086, d0088): a one-off `opencode run --standalone --format json --title
//! "Otaq · run command question" [-m <provider/id>] "<prompt>"` in the project's folder that may only read.
//!
//! The rules come in `OPENCODE_CONFIG_CONTENT`: every action denied, then `read`, `glob` and `grep` allowed (`.env`
//! files still denied). OpenCode takes the last rule that matches, and the variable's rules come after the user's
//! own config, so they win over the user's `allow` rules; a tool whose every use is denied isn't even offered to the
//! model. Checked with OpenCode 2.0.17, its folders in a scratch folder and a local stand-in model server (no real
//! model call): the model was offered only `glob`, `grep` and `read`, and a `shell` call it made anyway failed with
//! "No tool named "shell" is currently available", also with a user config allowing `shell` and `edit`. Never
//! `--auto` (d0011). `--standalone` so no background service is left running; the question's session stays in the
//! user's OpenCode session list under its title.
//!
//! Its output, one JSON object per line with `type`, `timestamp`, `sessionID`: `step_start`, `tool_use` (`part.tool`,
//! `part.state.input`, `status` `completed` or `error`), `text` (`part.text`, a whole text part), `step_finish`, and
//! `error` (`error.message`, e.g. "Model unavailable: opencode/foo-bar-9", exit 1). Captured with the stand-in model
//! (`tests/fixtures/opencode/oneoff/`).

use serde_json::{Value, json};

use super::{COMMAND_NAME, OpenCodeContext};
use crate::agent_tools::{LaunchError, OneOffCommand, OneOffLine, OneOffRequest, OneOffTool};
use crate::naming;

/// The variable OpenCode reads its extra config from.
const CONFIG_CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

/// The question's rules: nothing but reading (OpenCode's actions; `edit` covers writing files, `shell` commands).
pub(super) fn read_only_config() -> Value {
    let rule = |action: &str, resource: &str, effect: &str| json!({ "action": action, "resource": resource, "effect": effect });
    json!({
        "permissions": [
            rule("*", "*", "deny"),
            rule("read", "*", "allow"),
            rule("glob", "*", "allow"),
            rule("grep", "*", "allow"),
            rule("read", "*.env", "deny"),
            rule("read", "*.env.*", "deny"),
        ]
    })
}

pub(super) fn command(
    _context: &OpenCodeContext,
    request: &OneOffRequest,
) -> Result<OneOffCommand, LaunchError> {
    let mut args: Vec<String> = ["run", "--standalone", "--format", "json", "--title"]
        .map(str::to_owned)
        .to_vec();
    args.push(format!("{} · run command question", naming::PRODUCT_NAME));
    if let Some(model) = &request.model {
        args.extend(["-m".to_owned(), model.clone()]);
    }
    args.push(request.prompt.clone());
    Ok(OneOffCommand {
        program: COMMAND_NAME.to_owned(),
        args,
        cwd: request.cwd.clone(),
        env: vec![(CONFIG_CONTENT.to_owned(), read_only_config().to_string())],
    })
}

/// A tool it used is progress; each text part is an answer (the last one counts); an error is the error.
pub(super) fn read_line(line: &str) -> Option<OneOffLine> {
    tracing::debug!(line, "a line of the one-off question's output");
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let part = value.get("part");
    let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
    match value.get("type")?.as_str()? {
        "tool_use" => {
            let part = part?;
            let input = part.pointer("/state/input");
            let target = ["path", "filePath", "pattern", "command", "url"]
                .iter()
                .find_map(|field| text(input?.get(*field)));
            Some(OneOffLine::Tools(vec![OneOffTool {
                name: text(part.get("tool"))?,
                target,
            }]))
        }
        "text" => Some(OneOffLine::Answer {
            text: text(part?.get("text"))?,
            is_error: false,
        }),
        "error" => Some(OneOffLine::Answer {
            text: text(value.pointer("/error/message")).unwrap_or_default(),
            is_error: true,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::AgentTool;
    use crate::agent_tools::opencode::OpenCode;
    use crate::agent_tools::test_support::context;

    fn request(model: Option<&str>) -> OneOffRequest {
        OneOffRequest {
            cwd: "/r/shop-web".into(),
            prompt: "Suggest a run command".into(),
            model: model.map(str::to_owned),
        }
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/opencode/oneoff/{name}.jsonl",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    /// `opencode run` with its own private server, JSON lines out, the model before the prompt, only reading
    /// allowed through the config variable; never `--auto` or a config folder.
    #[test]
    fn the_question_may_only_read() {
        let bin = tempfile::tempdir().unwrap();
        let adapter = OpenCode::new(context(bin.path(), None));
        let plain = adapter.one_off(&request(None)).unwrap();
        assert_eq!(plain.program, "opencode");
        assert_eq!(plain.cwd, std::path::Path::new("/r/shop-web"));
        let title = format!("{} · run command question", naming::PRODUCT_NAME);
        assert_eq!(
            plain.args,
            [
                "run",
                "--standalone",
                "--format",
                "json",
                "--title",
                &title,
                "Suggest a run command"
            ]
        );
        let picked = adapter
            .one_off(&request(Some("anthropic/claude-sonnet-5-5")))
            .unwrap();
        assert_eq!(
            picked.args[6..],
            ["-m", "anthropic/claude-sonnet-5-5", "Suggest a run command"]
        );
        let [(name, config)] = picked.env.as_slice() else {
            panic!("{:?}", picked.env)
        };
        assert_eq!(name, "OPENCODE_CONFIG_CONTENT");
        let config: Value = serde_json::from_str(config).unwrap();
        assert_eq!(
            config["permissions"][0],
            json!({"action": "*", "resource": "*", "effect": "deny"}),
            "everything denied first"
        );
        let allowed: Vec<&str> = config["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["effect"] == "allow")
            .map(|r| r["action"].as_str().unwrap())
            .collect();
        assert_eq!(allowed, ["read", "glob", "grep"]);
        assert!(!picked.args.iter().any(|a| a == "--auto"));
        assert!(!picked.env.iter().any(|(k, _)| k == "OPENCODE_CONFIG_DIR"));
    }

    /// The captured lines of a question that read `package.json` and answered: the read is progress, the text the
    /// answer; the step lines nothing.
    #[test]
    fn tools_are_progress_the_text_is_the_answer() {
        let bin = tempfile::tempdir().unwrap();
        let adapter = OpenCode::new(context(bin.path(), None));
        let lines: Vec<Option<OneOffLine>> = fixture("read")
            .lines()
            .map(|l| adapter.one_off_line(l))
            .collect();
        assert_eq!(
            lines,
            [
                None,
                Some(OneOffLine::Tools(vec![OneOffTool {
                    name: "read".into(),
                    target: Some("package.json".into())
                }])),
                None,
                None,
                Some(OneOffLine::Answer {
                    text: r#"{"command": "pnpm dev --port $PORT"}"#.into(),
                    is_error: false
                }),
            ]
        );
        // A shell call the rules refused is still only progress; the answer follows.
        let denied: Vec<OneOffLine> = fixture("shell-denied")
            .lines()
            .filter_map(|l| adapter.one_off_line(l))
            .collect();
        assert_eq!(
            denied[0],
            OneOffLine::Tools(vec![OneOffTool {
                name: "shell".into(),
                target: Some("ls".into())
            }])
        );
        assert!(matches!(
            denied[1],
            OneOffLine::Answer {
                is_error: false,
                ..
            }
        ));
        assert_eq!(adapter.one_off_line("not json"), None);
    }

    #[test]
    fn an_error_is_the_answer() {
        let bin = tempfile::tempdir().unwrap();
        let adapter = OpenCode::new(context(bin.path(), None));
        assert_eq!(
            adapter.one_off_line(fixture("model-unavailable").trim()),
            Some(OneOffLine::Answer {
                text: "Model unavailable: opencode/foo-bar-9".into(),
                is_error: true
            })
        );
    }
}
