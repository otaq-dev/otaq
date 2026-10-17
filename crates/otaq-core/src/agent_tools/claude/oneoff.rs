//! Claude Code's one-off read-only question (RUN-02 "Ask the agent"; d0086, d0088): `claude -p --output-format
//! stream-json --verbose --tools Read,Glob,Grep [--model <m>] <prompt>`. `-p` answers once and exits; `--tools`
//! leaves only the reading tools in that conversation; stream-json reports each tool it uses and the final result.
//! The model goes unchanged, before the prompt, which is always last.

use serde_json::Value;

use crate::agent_tools::{OneOffCommand, OneOffLine, OneOffRequest, OneOffTool};

/// The reading tools the question may use.
const READ_ONLY_TOOLS: &str = "Read,Glob,Grep";

/// The question for `program` (`["claude"]`, or what stands for it).
pub fn command(program: &[String], request: &OneOffRequest) -> OneOffCommand {
    let (program, leading) = program.split_first().map_or_else(
        || (super::command_name().to_owned(), &[][..]),
        |(p, rest)| (p.clone(), rest),
    );
    let mut args = leading.to_vec();
    args.extend(
        [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--tools",
            READ_ONLY_TOOLS,
        ]
        .map(str::to_owned),
    );
    if let Some(model) = &request.model {
        args.extend(["--model".to_owned(), model.clone()]);
    }
    args.push(request.prompt.clone());
    OneOffCommand {
        program,
        args,
        cwd: request.cwd.clone(),
        env: Vec::new(),
    }
}

/// One line of the stream-json output: an `assistant` message's `tool_use` blocks (the file, pattern or path each
/// looked at), or the final `result`.
pub fn read_line(line: &str) -> Option<OneOffLine> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    match value.get("type")?.as_str()? {
        "assistant" => {
            let content = value.get("message")?.get("content")?.as_array()?;
            let tools: Vec<OneOffTool> = content
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
                .filter_map(|block| {
                    let name = block.get("name")?.as_str()?.to_owned();
                    let input = block.get("input");
                    let field = |f: &str| input.and_then(|i| i.get(f)).and_then(Value::as_str);
                    let target = field("file_path")
                        .or_else(|| field("pattern"))
                        .or_else(|| field("path"))
                        .map(str::to_owned);
                    Some(OneOffTool { name, target })
                })
                .collect();
            (!tools.is_empty()).then_some(OneOffLine::Tools(tools))
        }
        "result" => Some(OneOffLine::Answer {
            text: value
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            is_error: value
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(model: Option<&str>) -> OneOffRequest {
        OneOffRequest {
            cwd: "/r/shop-web".into(),
            prompt: "Suggest a run command".into(),
            model: model.map(str::to_owned),
        }
    }

    #[test]
    fn the_question_reads_only_and_passes_the_model_before_the_prompt() {
        let plain = command(&["claude".into()], &request(None));
        assert_eq!(plain.program, "claude");
        assert_eq!(
            plain.args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--tools",
                "Read,Glob,Grep",
                "Suggest a run command"
            ],
            "the tool default passes no model option"
        );
        assert_eq!(plain.cwd, std::path::Path::new("/r/shop-web"));
        let strong = command(&["claude".into()], &request(Some("opus")));
        assert_eq!(
            strong.args,
            [
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--tools",
                "Read,Glob,Grep",
                "--model",
                "opus",
                "Suggest a run command"
            ]
        );
    }

    #[test]
    fn tools_and_the_result_are_read_from_stream_json() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/r/shop/package.json"}},{"type":"text","text":"x"}]}}"#;
        assert_eq!(
            read_line(line),
            Some(OneOffLine::Tools(vec![OneOffTool {
                name: "Read".into(),
                target: Some("/r/shop/package.json".into())
            }]))
        );
        let text =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Looking"}]}}"#;
        assert_eq!(read_line(text), None);
        assert_eq!(read_line("not json"), None);
        assert_eq!(read_line(r#"{"type":"system"}"#), None);
        assert_eq!(
            read_line(r#"{"type":"result","is_error":false,"result":"{}"}"#),
            Some(OneOffLine::Answer {
                text: "{}".into(),
                is_error: false
            })
        );
        assert_eq!(
            read_line(r#"{"type":"result","is_error":true,"result":"Credit balance is too low"}"#),
            Some(OneOffLine::Answer {
                text: "Credit balance is too low".into(),
                is_error: true
            })
        );
    }

    #[test]
    fn a_stand_in_keeps_its_leading_arguments() {
        let c = command(
            &["sh".into(), "-c".into(), "exit 0".into(), "sh".into()],
            &request(None),
        );
        assert_eq!(c.program, "sh");
        assert_eq!(&c.args[..4], ["-c", "exit 0", "sh", "-p"]);
        assert_eq!(command(&[], &request(None)).program, "claude");
    }
}
