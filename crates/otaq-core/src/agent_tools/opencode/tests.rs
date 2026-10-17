//! The OpenCode skeleton (T-0347): names, input quirks, skills, detect on a stand-in program. Each lane adds its tests
//! beside its own file.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::agent_tools::SkillRef;
use crate::agent_tools::test_support::{context, stand_in, write_skill};
use crate::board::settings::WorkflowStep;
use crate::naming::PLUGIN_NAMESPACE as NS;

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
    // The skill's name until T-0354 verifies OpenCode's form.
    assert_eq!(adapter.skill_reference("commit"), "commit");
    assert_eq!(adapter.skill_delivery(), SkillDelivery::LaunchConfig);
    assert!(!adapter.runs_skills_by_name());
}

/// d0086: a stored skill OpenCode can't find as a file becomes the step's built-in, with what it replaces; one it finds
/// is followed by its path; Otaq's own is named in its syntax.
#[test]
fn workflow_skills_fall_back_to_the_built_in() {
    let bin = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), Some(home.path()));
    let cwd = project.path();
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/their-plugin:commit", cwd),
        SkillRef::BuiltIn {
            skill: "commit".into(),
            instead_of: "/their-plugin:commit".into()
        }
    );
    assert_eq!(
        adapter
            .workflow_skill(WorkflowStep::OpenPr, "/my-pr", cwd)
            .prompt(),
        "open-pr"
    );
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Push, &format!("/{NS}:push"), cwd),
        SkillRef::Named("push".into())
    );
    let user = write_skill(&home.path().join(".claude"), "my-commit");
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::File { file: user }
    );
    let own = write_skill(&cwd.join(".opencode"), "my-commit");
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::File { file: own },
        "the project's wins"
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
    assert_eq!(found.signed_in, None, "T-0355 says");

    stand_in(bin.path(), "opencode", "something else\n");
    assert_eq!(adapter.detect().await, Detection::default());
}

#[test]
fn hooks_are_unrecognised_until_mapped() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = opencode(bin.path(), None);
    assert_eq!(
        adapter.map_hook(
            "session.execution.succeeded",
            &json!({"id": "e1"}),
            SignalContext::default()
        ),
        [ToolEvent::Unrecognised {
            event: "session.execution.succeeded".into()
        }]
    );
}
