//! The Claude Code plugin in `plugin/` (T-0073, PLG-01…PLG-05, GIT-08): valid JSON, every hook calls a
//! `crewboard hook <event>` the CLI accepts, and the skills exist with the steps the decisions require. Real
//! `claude` behaviour is checked by hand.

use std::path::{Path, PathBuf};

use crewboard_core::board::settings::WorkflowSkills;
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

/// The front matter of a SKILL.md, as `key: value` lines.
fn front_matter(name: &str) -> Vec<(String, String)> {
    let path = plugin().join("skills").join(name).join("SKILL.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rest = text.strip_prefix("---\n").expect("front matter");
    let (front, _) = rest.split_once("\n---\n").expect("front matter end");
    front
        .lines()
        .filter_map(|l| l.split_once(": "))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Every skill folder in the plugin.
fn skill_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(plugin().join("skills"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
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

/// T-0164, T-0087 (PLG-06, d0061): a skill teaches both forms of a question to the user, with the rules for
/// options, and keeps the question text on stdin (PLG-05).
fn assert_teaches_answer_options(skill_name: &str, options_example: &str, open_example: &str) {
    let (_, body) = skill(skill_name);
    let cmd = naming::COMMAND_NAME;
    let with_options = format!("{cmd} ask human {options_example} --stdin");
    assert!(
        body.contains(&with_options),
        "{skill_name}: the options example"
    );
    assert!(
        body.contains(&format!(
            "{cmd} ask human --stdin <<'QUESTION'\n{open_example}"
        )),
        "{skill_name}: the open-question example"
    );
    for needle in [
        "**2–4 options**",
        "short label",
        "recommend one when there is a sensible default",
        "Never invent options for an open question",
        "The question text always goes through stdin",
        "Agent instructions take precedence",
    ] {
        assert!(body.contains(needle), "{skill_name} must say {needle:?}");
    }
    // Every `ask human` example keeps the question on stdin.
    for line in body.lines().filter(|l| l.contains("ask human")) {
        assert!(
            !line.contains(&format!("{cmd} ask human")) || line.contains("--stdin"),
            "{line}"
        );
    }
}

#[test]
fn work_ticket_teaches_answer_options() {
    assert_teaches_answer_options(
        WORK_TICKET_SKILL,
        "--option \"Postmark\" --option \"SES\" --option \"Resend\" --recommended 1",
        "What should",
    );
}

/// T-0087 (REV-05, PLG-02, PLG-03): address-review reads the feedback after prime, then commits and submits.
#[test]
fn address_review_reads_the_feedback_fixes_commits_and_submits() {
    let (name, body) = skill("address-review");
    assert_eq!(name, "address-review");
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime`"),
        "Follow the Agent instructions".to_owned(),
        format!("`{cmd} review feedback`"),
        "Fix every comment".to_owned(),
        naming::skill("commit"),
        format!("{cmd} review submit --stdin"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("address-review must say {step:?} after the step before"));
        at += found;
    }
    assert!(body.contains("Never push"));
    assert_teaches_answer_options(
        "address-review",
        "--option \"Add a setting\" --option \"Keep 20\" --recommended 1",
        "What should",
    );
}

/// T-0090 (GIT-03, d0038): restack rebases onto the target prime names, resolves, commits and submits.
#[test]
fn restack_rebases_resolves_commits_and_submits() {
    let (name, body) = skill("restack");
    assert_eq!(name, "restack");
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime`"),
        "Follow the Agent instructions".to_owned(),
        "Rebase onto the target".to_owned(),
        "Resolve every conflict".to_owned(),
        naming::skill("commit"),
        format!("{cmd} review submit --stdin"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("restack must say {step:?} after the step before"));
        at += found;
    }
    for needle in [
        "Never push",
        "never stash",
        "Long text always goes through stdin",
    ] {
        assert!(body.contains(needle), "restack must say {needle:?}");
    }
}

/// T-0140 (REV-10, PLG-05, d0050): open-pr proposes after prime, waits for the user's confirmation, and only then
/// pushes and reports the PR; provider detection follows the remote URL.
#[test]
fn open_pr_proposes_first_and_pushes_only_after_the_confirmation() {
    let (name, body) = skill("open-pr");
    assert_eq!(name, "open-pr");
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime`"),
        "Follow the Agent instructions".to_owned(),
        "github.com".to_owned(),
        "dev.azure.com".to_owned(),
        "your own Open PR skill".to_owned(),
        format!("{cmd} pr propose --stdin"),
        "End your turn".to_owned(),
        "The user confirmed".to_owned(),
        "git push".to_owned(),
        "gh pr create".to_owned(),
        "az repos pr create".to_owned(),
        format!("{cmd} pr opened"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("open-pr must say {step:?} after the step before"));
        at += found;
    }
    for needle in [
        "Never push before Crewboard tells you the user confirmed",
        "visualstudio.com",
        &format!("{}/SHOP-18-", naming::PLUGIN_NAMESPACE),
        "Never store, print or ask for a token",
        "Long text always goes through stdin",
    ] {
        assert!(body.contains(needle), "open-pr must say {needle:?}");
    }
}

/// T-0140 (REV-14, d0041, d0050): push proposes the new commits' messages, then pushes after the confirmation.
#[test]
fn push_proposes_the_new_commits_and_pushes_after_the_confirmation() {
    let (name, body) = skill("push");
    assert_eq!(name, "push");
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime`"),
        "Follow the Agent instructions".to_owned(),
        format!("{cmd} pr propose --stdin"),
        "End your turn".to_owned(),
        "The user confirmed".to_owned(),
        "git push origin".to_owned(),
        format!("{cmd} pr opened"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("push must say {step:?} after the step before"));
        at += found;
    }
    assert!(body.contains("never with `--force`"));
}

/// T-0140 (MSG-03, d0030): answer-question answers only and replies with `answer <ID> --stdin`.
#[test]
fn answer_question_changes_nothing_and_ends_with_answer() {
    let (name, body) = skill("answer-question");
    assert_eq!(name, "answer-question");
    let cmd = naming::COMMAND_NAME;
    let prime = body.find(&format!("`{cmd} prime`")).expect("prime first");
    let answer = body
        .find(&format!("{cmd} answer "))
        .expect("answer with the ID");
    assert!(prime < answer);
    for needle in [
        "Follow the Agent instructions",
        "You change nothing",
        "don't commit",
        "don't push",
        "--stdin <<'ANSWER'",
    ] {
        assert!(body.contains(needle), "answer-question must say {needle:?}");
    }
}

/// T-0191 (PLG-08, d0063): the validate skill is the default Validate skill, user-invoked only, a read-only judge
/// that reads `prime --validate`, checks the description, the checks and leftovers, writes notes for gaps another
/// ticket covers or this one doesn't ask for, and ends with `validate pass|fail --stdin`.
#[test]
fn validate_is_a_read_only_judge_that_ends_with_a_verdict() {
    let (name, body) = skill("validate");
    assert_eq!(name, "validate");
    assert_eq!(WorkflowSkills::default().validate, naming::skill(&name));
    assert!(
        front_matter("validate")
            .contains(&("disable-model-invocation".to_owned(), "true".to_owned()))
    );
    let cmd = naming::COMMAND_NAME;
    let order = [
        "read-only judge".to_owned(),
        format!("`{cmd} prime --validate`"),
        "Follow the Agent instructions".to_owned(),
        "against the description".to_owned(),
        "infer reasonable ones".to_owned(),
        "**each review comment**".to_owned(),
        "Run the checks".to_owned(),
        "obvious ones".to_owned(),
        "leftovers and out-of-scope edits".to_owned(),
        "one line per finding, with its file and line".to_owned(),
        "**covered by a linked ticket** is a **note**, never a failure".to_owned(),
        "Note: UI for adding items: covered by SHOP-40.2".to_owned(),
        "**not asked for by this ticket**".to_owned(),
        "Note: Not covered by any ticket: removing items".to_owned(),
        format!("{cmd} validate fail --stdin"),
        format!("{cmd} validate pass --stdin"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("validate must say {step:?} after the step before"));
        at += found;
    }
    for needle in [
        "Never edit, create or delete files, never commit, and never fix anything",
        "Long text always goes through stdin",
        &format!("don't run `{cmd} review submit`"),
    ] {
        assert!(body.contains(needle), "validate must say {needle:?}");
    }
    // The example notes are what the service reads as notes (the rest are findings).
    let notes: Vec<&str> = body
        .lines()
        .filter_map(crewboard_core::board::validation::note_of)
        .collect();
    assert!(
        notes.contains(&"Not covered by any ticket: removing items"),
        "{notes:?}"
    );
}

/// T-0191 (PLG-07, d0067): no skill mentions "Done when"; the agent's skills say to end the turn after submitting
/// and to fix and submit again after a failed validation.
#[test]
fn no_skill_mentions_done_when_and_the_agent_waits_for_validation() {
    let names = skill_names();
    assert!(names.contains(&"validate".to_owned()), "{names:?}");
    for name in &names {
        let (_, body) = skill(name);
        assert!(
            !body.to_lowercase().contains("done when"),
            "{name} mentions Done when"
        );
    }
    let cmd = naming::COMMAND_NAME;
    for name in [WORK_TICKET_SKILL, "address-review"] {
        let (_, body) = skill(name);
        let submit = body
            .find(&format!("{cmd} review submit --stdin"))
            .expect("submit");
        let rest = &body[submit..];
        for needle in [
            "**end your turn** and wait",
            "arrives in this session as a\n   message",
            "If validation failed",
            "fix them, commit everything and submit again",
        ] {
            assert!(
                rest.contains(needle),
                "{name} must say {needle:?} after submitting"
            );
        }
    }
    let (_, work) = skill(WORK_TICKET_SKILL);
    assert!(work.contains("until the ticket is done, as its description describes"));
}
