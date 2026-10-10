//! The Claude Code plugin in `plugin/` (T-0073, PLG-01…PLG-05, GIT-08): valid JSON, every hook calls a
//! `crewboard hook <event>` the CLI accepts, and the skills exist with the steps the decisions require. Real
//! `claude` behaviour is checked by hand.

use std::path::{Path, PathBuf};

use crewboard_core::naming;
use crewboard_core::session::launch::{WORK_TICKET_SKILL, find_plugin_dir};
use crewboard_core::signals::HookEvent;
use serde_json::Value;

fn plugin() -> PathBuf {
    find_plugin_dir(Path::new("/nonexistent")).expect("the repository's plugin folder")
}

fn json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The front matter's `name:` and the body of a SKILL.md.
fn skill(name: &str) -> (String, String) {
    let path = plugin().join("skills").join(name).join("SKILL.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rest = text.strip_prefix("---\n").expect("front matter");
    let (front, body) = rest.split_once("\n---\n").expect("front matter end");
    let field = |key: &str| {
        front
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{key}: ")))
            .unwrap_or_else(|| panic!("{name}: no {key}"))
            .to_owned()
    };
    assert!(!field("description").is_empty());
    (field("name"), body.to_owned())
}

#[test]
fn the_manifest_names_the_plugin_namespace() {
    let manifest = json(&plugin().join(".claude-plugin").join("plugin.json"));
    assert_eq!(manifest["name"], naming::PLUGIN_NAMESPACE);
    assert_eq!(manifest["license"], "MIT");
}

#[test]
fn every_hook_runs_an_accepted_crewboard_hook_event() {
    let hooks = json(&plugin().join("hooks").join("hooks.json"));
    let events = hooks["hooks"].as_object().expect("hooks object");
    let mut wired: Vec<HookEvent> = Vec::new();
    for (event, groups) in events {
        let parsed: HookEvent = event
            .parse()
            .unwrap_or_else(|_| panic!("{event} isn't an event `hook` accepts"));
        wired.push(parsed);
        for group in groups.as_array().unwrap() {
            for hook in group["hooks"].as_array().unwrap() {
                assert_eq!(hook["type"], "command");
                let command = hook["command"].as_str().unwrap();
                let call = format!("{} hook {event}", naming::COMMAND_NAME);
                assert!(command.ends_with(&call), "{command}");
                // Without the command installed, the hook is a no-op rather than an error.
                assert!(
                    command.starts_with(&format!("command -v {} ", naming::COMMAND_NAME)),
                    "{command}"
                );
                assert!(hook["timeout"].as_u64().unwrap() <= 10);
            }
        }
    }
    wired.sort();
    assert_eq!(
        wired,
        HookEvent::ALL,
        "Notification, Stop, StopFailure, UserPromptSubmit"
    );
}

#[test]
fn work_ticket_teaches_the_whole_path_to_review() {
    let (name, body) = skill(WORK_TICKET_SKILL);
    assert_eq!(name, WORK_TICKET_SKILL);
    assert_eq!(
        naming::skill(&name),
        format!("/{}:work-ticket", naming::PLUGIN_NAMESPACE)
    );
    let cmd = naming::COMMAND_NAME;
    let prime = body.find(&format!("`{cmd} prime`")).expect("prime first");
    let commit = body
        .find(&naming::skill("commit"))
        .expect("the Commit skill");
    let submit = body
        .find(&format!("{cmd} review submit --stdin"))
        .expect("submit with stdin");
    assert!(prime < commit && commit < submit, "prime → commit → submit");
    for needle in [
        "Follow the Agent instructions",
        "never remove",
        &format!("{cmd} ask human --stdin"),
        "Never push",
        "Long text always goes through stdin",
    ] {
        assert!(body.contains(needle), "work-ticket must say {needle:?}");
    }
}

#[test]
fn the_commit_skill_commits_everything_and_never_pushes() {
    let (name, body) = skill("commit");
    assert_eq!(name, "commit");
    for needle in [
        "git add -A",
        "SHOP-19: ",
        "Never push",
        "never use\n   `--no-verify`",
    ] {
        assert!(body.contains(needle), "commit must say {needle:?}");
    }
}

/// T-0164 (PLG-06, d0061): work-ticket teaches both forms of a question to the user, with the rules for options,
/// and keeps the question text on stdin (PLG-05).
#[test]
fn work_ticket_teaches_answer_options() {
    let (_, body) = skill(WORK_TICKET_SKILL);
    let cmd = naming::COMMAND_NAME;
    let with_options = format!(
        "{cmd} ask human --option \"Postmark\" --option \"SES\" --option \"Resend\" --recommended 1 --stdin"
    );
    assert!(body.contains(&with_options), "the options example");
    assert!(
        body.contains(&format!(
            "{cmd} ask human --stdin <<'QUESTION'\nWhat should"
        )),
        "the open-question example"
    );
    for needle in [
        "**2–4 options**",
        "short label",
        "recommend one when there is a sensible default",
        "Never invent options for an open question",
        "The question text always goes through stdin",
        "Agent instructions take precedence",
    ] {
        assert!(body.contains(needle), "work-ticket must say {needle:?}");
    }
    // Every `ask human` example keeps the question on stdin.
    for line in body.lines().filter(|l| l.contains("ask human")) {
        assert!(
            !line.contains(&format!("{cmd} ask human")) || line.contains("--stdin"),
            "{line}"
        );
    }
}
