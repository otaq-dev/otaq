//! The plugin's skills only teach commands the CLI has (T-0140): every `crewboard <command> [<subcommand>]` written in
//! a skill is accepted by the built CLI (`--help` on it succeeds).

use std::path::Path;
use std::process::Command;

use crewboard_core::naming;
use crewboard_core::session::launch::find_plugin_dir;

const CB: &str = env!("CARGO_BIN_EXE_crewboard");

/// The command words after each `crewboard ` in a text: lowercase words, up to two.
fn commands(text: &str) -> Vec<Vec<String>> {
    let lead = format!("{} ", naming::COMMAND_NAME);
    let mut found = Vec::new();
    for (at, _) in text.match_indices(&lead) {
        // Skip `CREWBOARD_…`, `/crewboard:…` and paths: the name must start a word.
        let before = text[..at].chars().last();
        if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '/' | ':' | '-')) {
            continue;
        }
        let words: Vec<String> = text[at + lead.len()..]
            .split([' ', '\n', '`', '"'])
            .take_while(|w| {
                w.starts_with(|c: char| c.is_ascii_lowercase())
                    && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
            })
            .take(2)
            .map(str::to_owned)
            .collect();
        if !words.is_empty() {
            found.push(words);
        }
    }
    found
}

#[test]
fn every_command_a_skill_names_exists() {
    let skills = find_plugin_dir(Path::new("/nonexistent"))
        .expect("the repository's plugin folder")
        .join("skills");
    let mut checked = 0;
    for entry in std::fs::read_dir(&skills).unwrap() {
        let file = entry.unwrap().path().join("SKILL.md");
        let text = std::fs::read_to_string(&file).unwrap();
        for words in commands(&text) {
            let ok = Command::new(CB)
                .args(&words)
                .arg("--help")
                .output()
                .unwrap()
                .status
                .success();
            assert!(
                ok,
                "{}: `{} {}` isn't a command",
                file.display(),
                naming::COMMAND_NAME,
                words.join(" ")
            );
            checked += 1;
        }
    }
    assert!(checked > 20, "only {checked} commands found");
}

#[test]
fn command_words_are_found() {
    let cmd = naming::COMMAND_NAME;
    let text = format!(
        "Run `{cmd} prime`, then `{cmd} pr propose --stdin` and {cmd} answer <ID>. Not /{cmd}:commit or $CREWBOARD_X"
    );
    assert_eq!(
        commands(&text),
        vec![
            vec!["prime".to_owned()],
            vec!["pr".to_owned(), "propose".to_owned()],
            vec!["answer".to_owned()]
        ]
    );
}
