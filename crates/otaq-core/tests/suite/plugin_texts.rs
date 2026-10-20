//! Otaq's skill texts work on every tool (ADP-05, d0088): each `plugin/skills/*/SKILL.md` has front matter every
//! tool's YAML parser reads (Copilot rejects an unquoted `: ` in a value), and no text names a skill in one tool's
//! syntax: the agent uses the skill that `prime` names.

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::claude::find_plugin_dir;
use otaq_core::naming;

/// Every SKILL.md of the plugin, with its folder name.
fn skill_files() -> Vec<(String, PathBuf)> {
    let skills = find_plugin_dir(Path::new("/nonexistent"))
        .expect("the repository's plugin folder")
        .join("skills");
    let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(&skills)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name, e.path().join("SKILL.md"))
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no skills in {}", skills.display());
    files
}

/// The front matter's `key: value` lines and the body.
fn split(name: &str, text: &str) -> (Vec<(String, String)>, String) {
    let rest = text
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("{name}: no front matter"));
    let (front, body) = rest
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("{name}: front matter not closed"));
    let fields = front
        .lines()
        .map(|line| {
            let (key, value) = line
                .split_once(": ")
                .unwrap_or_else(|| panic!("{name}: {line:?} isn't `key: value`"));
            (key.to_owned(), value.to_owned())
        })
        .collect();
    (fields, body.to_owned())
}

/// A YAML quoted scalar on one line: `'…'` with inner quotes doubled, or `"…"` with inner quotes escaped.
fn quoted(value: &str) -> bool {
    let single = value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'');
    let double = value.len() >= 2 && value.starts_with('"') && value.ends_with('"');
    if single {
        let inner = &value[1..value.len() - 1];
        return inner.replace("''", "").find('\'').is_none();
    }
    if double {
        let inner = &value[1..value.len() - 1];
        return inner
            .replace("\\\\", "")
            .replace("\\\"", "")
            .find('"')
            .is_none();
    }
    false
}

#[test]
fn every_skill_has_front_matter_with_name_and_description() {
    for (name, path) in skill_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        let (fields, _) = split(&name, &text);
        let field = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        assert_eq!(field("name").map(String::as_str), Some(name.as_str()));
        assert!(
            field("description").is_some_and(|d| !d.trim().is_empty()),
            "{name}: no description"
        );
    }
}

#[test]
fn a_value_with_a_colon_and_space_is_quoted() {
    for (name, path) in skill_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        for (key, value) in split(&name, &text).0 {
            if value.contains(": ") {
                assert!(quoted(&value), "{name}: {key} must be quoted: {value}");
            }
        }
    }
}

#[test]
fn no_skill_text_names_a_skill_in_one_tools_syntax() {
    let ns = naming::PLUGIN_NAMESPACE;
    // Claude Code and Copilot `/otaq:commit`, Codex `$otaq:commit`.
    let syntaxes = [format!("/{ns}:"), format!("${ns}:")];
    for (name, path) in skill_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        for syntax in &syntaxes {
            assert!(!text.contains(syntax), "{name} contains {syntax:?}");
        }
    }
}

/// T-0409 (AGM-12, AGM-10, AGM-11; d0108): no shipped skill names a removed command or flag. Stages end with
/// `stage next` / `stage done`, checks with `check pass|fail`, and tickets set stage choices with `--stage`. The
/// only `--validator` left is `show --terminal --validator` (a running check's terminal), which isn't an agent pick.
#[test]
fn no_skill_names_a_removed_command_or_flag() {
    let ns = naming::PLUGIN_NAMESPACE;
    let gone = [
        "review submit".to_owned(),
        "validate pass".to_owned(),
        "validate fail".to_owned(),
        "--developer".to_owned(),
        "--validator".to_owned(),
        format!("/{ns}:"),
    ];
    for (name, path) in skill_files() {
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("--terminal --validator", "--terminal");
        for word in &gone {
            assert!(!text.contains(word.as_str()), "{name} names {word:?}");
        }
    }
}

/// T-0409 (AGM-12, AGM-07): the skills that end an Agent stage name `stage next`, the check ends with `check pass|fail`,
/// and a skill another skill names is named by its copy, `otaq-<name>`.
#[test]
fn skills_end_stages_with_the_stage_commands() {
    let cmd = naming::COMMAND_NAME;
    let read = |name: &str| {
        let (_, path) = skill_files()
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no skill {name}"));
        std::fs::read_to_string(path).unwrap()
    };
    for name in ["work-ticket", "address-review", "restack", "commit"] {
        assert!(
            read(name).contains(&format!("{cmd} stage next")),
            "{name} must name `stage next`"
        );
    }
    for name in ["work-ticket", "address-review", "restack"] {
        assert!(
            read(name).contains(&format!("(`{}`", naming::copy_name("commit"))),
            "{name} must name the commit skill by its copy"
        );
    }
    let validate = read("validate");
    for verdict in ["pass", "fail"] {
        assert!(validate.contains(&format!("{cmd} check {verdict} --stdin")));
    }
    for name in ["validate", "answer-question"] {
        assert!(
            read(name).contains(&format!("don't run `{cmd} stage next`")),
            "{name} must say not to run `stage next`"
        );
    }
}

#[test]
fn the_quoting_check_matches_yaml() {
    assert!(quoted(r#"'Otaq''s skill with "<KEY>: <what>" messages'"#));
    assert!(quoted(r#""a \"<KEY>: x\" message""#));
    assert!(!quoted(r#"Otaq's skill with "<KEY>: <what>" messages"#));
    assert!(!quoted("'it's'"));
}
