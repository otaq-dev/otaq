//! The OpenCode skeleton (T-0347): names, input quirks, skills, detect on a stand-in program. Each lane adds its tests
//! beside its own file.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::agent_tools::SkillRef;
use crate::agent_tools::test_support::{context, follow_validate, stand_in, validate_folder};
use crate::board::settings::WorkflowStep;

fn opencode(bin: &Path, home: Option<&Path>) -> OpenCode {
    OpenCode::new(context(bin, home))
}

#[test]
fn names() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    assert_eq!(adapter.tool(), Tool::OpenCode);
    assert_eq!(adapter.display_name(), "OpenCode");
    assert_eq!(adapter.short_name(), "OpenCode");
    assert_eq!(adapter.command_name(), "opencode");
    assert_eq!(adapter.resume_hint("ses_1"), "opencode --session ses_1");
    assert_eq!(
        adapter.install_command(Os::Linux),
        "npm install -g opencode-ai"
    );
    // No one-time setup: no folder trust, nothing written.
    assert_eq!(adapter.record_project(Path::new("/repo")), None);
    assert!(adapter.setup_entries().is_empty());
    assert_eq!(adapter.remove_setup(), SetupReport::default());
}

/// ADP-06: Enter at once; only the current line of a draft clears.
#[test]
fn input_quirks() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    let quirks = adapter.input();
    assert_eq!(quirks.clear_line, b"\x05\x15");
    assert_eq!(quirks.enter_delay, Duration::ZERO);
    assert!(!quirks.multi_line_clearable);
    assert!(!adapter.typed_key_answers_prompt());
    assert!(!adapter.turn_end_reports_background());
    assert!(adapter.swaps_refused_model());
}

#[test]
fn skills() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    // The skill's name: OpenCode has no syntax of its own for running a skill from text (T-0354).
    assert_eq!(adapter.skill_reference("commit"), "commit");
    assert_eq!(adapter.skill_delivery(), SkillDelivery::LaunchConfig);
}

/// ADP-05, WSK-06 (d0092): Otaq's own workflow skills in this tool's syntax; an imported one by its copy in the
/// worktree, worded the same on every tool.
#[test]
fn workflow_skills_by_syntax_or_worktree_folder() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    for step in WorkflowStep::ALL {
        assert_eq!(
            adapter.workflow_skill(step, false),
            SkillRef::Own(adapter.skill_reference(step.own_skill()))
        );
    }
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Push, false).prompt(),
        "push"
    );
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Validate, true),
        SkillRef::Folder {
            folder: validate_folder()
        }
    );
    assert_eq!(
        adapter
            .workflow_skill(WorkflowStep::Validate, true)
            .prompt(),
        follow_validate()
    );
}

/// Detect runs the program on the context's PATH (here a stand-in that prints what `opencode --version` prints).
#[tokio::test]
async fn detect_reads_the_version_from_the_path() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    assert_eq!(adapter.detect().await, Detection::default());

    stand_in(bin.path(), "opencode", "v2.0.17\n");
    let found = adapter.detect().await;
    assert!(found.installed);
    assert_eq!(found.version.as_deref(), Some("2.0.17"));
    assert!(!found.update_needed);
    assert_eq!(found.signed_in, None, "its auth list answer isn't a list");

    stand_in(bin.path(), "opencode", "something else\n");
    assert_eq!(adapter.detect().await, Detection::default());
}

/// OpenCode sends many event types; one the adapter doesn't map gives nothing (not *unrecognised*).
#[test]
fn unmapped_events_give_nothing() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    assert_eq!(
        adapter.map_hook(
            "session.text.delta",
            &json!({"id": "e1", "data": {"sessionID": "ses_1"}}),
            SignalContext::default()
        ),
        []
    );
}
