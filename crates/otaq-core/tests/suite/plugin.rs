//! The Claude Code plugin in `plugin/` (T-0073, PLG-01…PLG-05, GIT-08): valid JSON, every hook calls a
//! `otaq hook --tool claude <event>` the Claude Code adapter knows, and the skills exist with the steps the decisions require. Real
//! `claude` behaviour is checked by hand.

use std::path::{Path, PathBuf};

use otaq_core::agent_tools::Tool;
use otaq_core::agent_tools::WORK_TICKET_SKILL;
use otaq_core::agent_tools::claude::find_plugin_dir;
use otaq_core::agent_tools::claude::hooks::HookEvent;
use otaq_core::board::settings::WorkflowSkills;
use otaq_core::naming;
use serde_json::Value;

/// How the skills name the Commit skill: the one `prime` names, never one tool's syntax (ADP-05).
const COMMIT_STEP: &str = "with the Commit skill that `prime` names";

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
    assert_eq!(manifest["license"], "FSL-1.1-MIT");
}

#[test]
fn every_hook_runs_an_accepted_otaq_hook_event() {
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
                let call = format!(
                    "{} hook --tool {} {event}",
                    naming::COMMAND_NAME,
                    Tool::Claude.as_str()
                );
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
    let commit = body.find(COMMIT_STEP).expect("the Commit skill");
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
        COMMIT_STEP.to_owned(),
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
        COMMIT_STEP.to_owned(),
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
        "Never push before Otaq tells you the user confirmed",
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
        .filter_map(otaq_core::board::validation::note_of)
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

/// T-0202 (PLG-09, PLG-01, PLG-02, PLG-05; d0068, d0040): the project-agent skill is user-invoked only (the session
/// starts with it), reads `prime --project` first, follows the Project agent instructions, names itself after the
/// first request, investigates without editing or committing, files Backlog tickets with the CLI (long text on
/// stdin, batches with `create -f`), uses the user's skills for imports, asks with `ask <KEY>` and `ask human`, and
/// tells the user what it would need when the CLI refuses.
#[test]
fn project_agent_investigates_and_files_backlog_tickets() {
    let (name, body) = skill(otaq_core::agent_tools::PROJECT_AGENT_SKILL);
    assert_eq!(name, "project-agent");
    assert!(
        front_matter("project-agent")
            .contains(&("disable-model-invocation".to_owned(), "true".to_owned()))
    );
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime --project`"),
        "Follow the Project agent instructions".to_owned(),
        "Imports from trackers use the user's own\n   skills**".to_owned(),
        "Right after you read the first request".to_owned(),
        format!("{cmd} agent name \"Checkout slowness\""),
        "Rename yourself only when the user asks".to_owned(),
        "ticket worktrees".to_owned(),
        "**Never edit files and never commit**".to_owned(),
        format!("{cmd} create \"Slow checkout query\" -p 2 --deps SHOP-19 --stdin"),
        format!("`{cmd} create -f tickets.md`"),
        "--parent".to_owned(),
        "**still in Backlog**".to_owned(),
        format!("`{cmd} dep add SHOP-35 SHOP-19`"),
        format!("`{cmd} ask SHOP-18 --stdin`"),
        format!("`{cmd} ask human --stdin`"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("project-agent must say {step:?} after the step before"));
        at += found;
    }
    for needle in [
        "**Never edit files, never commit, never push.**",
        "You can't move, start, approve, merge, close or delete tickets",
        "tell the user what you would need",
        otaq_core::policy::BACKLOG_ONLY,
        "**Long text always goes through stdin**",
        "**2–4 options**",
        "Never invent options for an open question",
        "The question text always goes through stdin",
    ] {
        assert!(body.contains(needle), "project-agent must say {needle:?}");
    }
    for line in body.lines().filter(|l| l.contains("ask human")) {
        assert!(
            !line.contains(&format!("{cmd} ask human")) || line.contains("--stdin"),
            "{line}"
        );
    }
}

/// The text of the first `<<'TAG'` heredoc after `from` in a skill, with each line's indent removed.
fn heredoc(body: &str, from: &str, tag: &str) -> String {
    let start = body.find(from).unwrap_or_else(|| panic!("no {from:?}"));
    let open = format!("<<'{tag}'\n");
    let rest = &body[start..];
    let rest = &rest[rest.find(&open).expect("heredoc") + open.len()..];
    rest.lines()
        .map(str::trim)
        .take_while(|l| *l != tag)
        .collect::<Vec<_>>()
        .join("\n")
}

/// T-0286 (DEC-01, LEAD-10; d0084, d0083, d0040): work-ticket reads its notes at each new step, records a product
/// question it can go on without (the example parses as a complete decision), asks only when it can't go on, lets
/// the Agent instructions add to the rule, and ends its submit summary with notes for later tickets.
#[test]
fn work_ticket_records_product_choices_and_reads_its_notes() {
    let (_, body) = skill(WORK_TICKET_SKILL);
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime`"),
        format!("Run `{cmd} notes` each time you start a new step"),
        COMMIT_STEP.to_owned(),
        format!("{cmd} review submit --stdin"),
        "Notes for later tickets:".to_owned(),
        "If you can't go on without the answer".to_owned(),
        "Otherwise decide and record it".to_owned(),
        format!("{cmd} decision record --stdin <<'DECISION'"),
        "Agent instructions may add to this rule".to_owned(),
        format!("{cmd} ask human --stdin"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("work-ticket must say {step:?} after the step before"));
        at += found;
    }
    let example = heredoc(&body, &format!("{cmd} decision record"), "DECISION");
    let decision = otaq_core::decisions::NewDecision::parse(&example).expect("the example parses");
    assert!(decision.missing().is_empty(), "{example}");
    assert_eq!(decision.built, "only in the profile form");
}

/// T-0286 (LEAD-10, d0083): the Commit skill reads the lead's notes before it stages anything.
#[test]
fn the_commit_skill_reads_notes_first() {
    let (_, body) = skill("commit");
    let cmd = naming::COMMAND_NAME;
    let notes = body
        .find(&format!("Run `{cmd} notes` first"))
        .expect("notes");
    let status = body.find("`git status`").expect("status");
    let add = body.find("git add -A").expect("add");
    assert!(notes < status && status < add, "notes → status → add");
    assert!(body.contains("apply it to your work before you commit"));
}

/// T-0286 (LEAD-01, LEAD-05…LEAD-10, DEC-04; d0083): the epic-lead skill is user-invoked only, reads `prime --epic`
/// first, stays read-only and CLI-only, plans with `create --parent` and dependencies, passes on what merged, marks
/// contradicted decisions with a source, answers or passes on questions from written sources, sends one note with
/// `--view` to a stuck child, suggests closes, never does what is the user's, and ends every turn idle. It contains
/// no instruction that edits files: every command in its examples is an `otaq` command.
#[test]
fn epic_lead_directs_through_the_cli_and_never_edits_files() {
    let (name, body) = skill(otaq_core::agent_tools::EPIC_LEAD_SKILL);
    assert_eq!(name, "epic-lead");
    assert!(
        front_matter("epic-lead")
            .contains(&("disable-model-invocation".to_owned(), "true".to_owned()))
    );
    let cmd = naming::COMMAND_NAME;
    let order = [
        format!("`{cmd} prime --epic`"),
        "**Never edit, create or delete files, never commit.**".to_owned(),
        "Never use subagents or agent teams".to_owned(),
        "name the source".to_owned(),
        "never answer a permission prompt".to_owned(),
        "don't look for a way around it".to_owned(),
        "**End every turn idle.**".to_owned(),
        format!("{cmd} create \"Wishlist API\" --parent SHOP-40 -p 2 --stdin"),
        format!("`{cmd} dep add SHOP-40.3 SHOP-40.2`"),
        format!("`{cmd} update SHOP-40.3 --stdin`"),
        "## Checking the plan".to_owned(),
        "## The merge turn".to_owned(),
        "Notes for later tickets".to_owned(),
        format!("{cmd} update SHOP-40.3 --stdin <<'BODY'"),
        "Never\n   overrule it".to_owned(),
        format!("{cmd} decision mark SHOP-40.1#1 --source \"PAY-123\" --stdin"),
        "only when it could\n   otherwise start before the other merges".to_owned(),
        "at most one note to a running child".to_owned(),
        format!("{cmd} note SHOP-40.4 --stdin"),
        format!("`{cmd} questions`"),
        format!("{cmd} answer 7 --stdin"),
        format!("{cmd} ask human --pass-on 7"),
        "--recommended 1 --stdin <<'REASON'".to_owned(),
        "Never guess".to_owned(),
        format!("{cmd} note SHOP-40.3 --view \""),
        "**Don't retry:**".to_owned(),
        format!("{cmd} suggest close SHOP-40.6 --stdin"),
        format!("`{cmd} suggest delete <KEY> --stdin`"),
        "dismissed by the user".to_owned(),
        format!("{cmd} ask human --option"),
    ];
    let mut at = 0;
    for step in &order {
        let found = body[at..]
            .find(step.as_str())
            .unwrap_or_else(|| panic!("epic-lead must say {step:?} after the step before"));
        at += found;
    }
    for needle in [
        "never move,\n  queue, take back, park, approve or merge",
        "never edit a started child",
        "**Long text always goes through stdin**",
    ] {
        assert!(body.contains(needle), "epic-lead must say {needle:?}");
    }
    // No file-editing instruction anywhere in the text.
    let lower = body.to_lowercase();
    for banned in [
        "git add",
        "git commit",
        "git push",
        "git merge",
        "git rebase",
        "git stash",
        "git reset",
        "git checkout",
        "sed -i",
        "rm -",
        "mkdir",
        "edit tool",
        "write tool",
        "/commit",
    ] {
        assert!(!lower.contains(banned), "epic-lead mentions {banned:?}");
    }
    // Every command line in the examples (outside heredoc bodies) is an `otaq` command, with no redirection into a
    // file.
    let mut in_block = false;
    let mut heredoc_end: Option<String> = None;
    let mut commands = 0;
    for line in body.lines().map(str::trim) {
        if line.starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if !in_block {
            continue;
        }
        if let Some(end) = &heredoc_end {
            if line == end {
                heredoc_end = None;
            }
            continue;
        }
        assert!(
            line.starts_with(&format!("{cmd} ")),
            "not a CLI command: {line}"
        );
        let open = line.find("<<'");
        let without_heredoc = open.map_or(line, |o| &line[..o]);
        assert!(
            !without_heredoc.contains('>'),
            "redirects into a file: {line}"
        );
        if let Some(o) = open {
            let tag = &line[o + 3..];
            heredoc_end = Some(tag.trim_end_matches('\'').to_owned());
        }
        commands += 1;
    }
    assert!(heredoc_end.is_none() && !in_block);
    assert!(commands >= 8, "only {commands} example commands");
    for line in body.lines().filter(|l| l.contains("ask human")) {
        assert!(
            !line.contains(&format!("{cmd} ask human")) || line.contains("--stdin"),
            "{line}"
        );
    }
}
