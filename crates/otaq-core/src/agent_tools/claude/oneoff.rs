//! Claude Code's one-off read-only question (RUN-02 "Ask the agent"; d0086, d0088): `claude -p --output-format
//! stream-json --verbose --tools Read,Glob,Grep [--model <m>] <prompt>`. `-p` answers once and exits; `--tools`
//! leaves only the reading tools in that conversation; stream-json reports each tool it uses and the final result.
//! The model goes unchanged, before the prompt, which is always last.

use crate::agent_tools::{OneOffCommand, OneOffRequest};

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
