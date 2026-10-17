//! The Codex skeleton (T-0347): names, input quirks, skills, detect on a stand-in program. Each lane adds its tests
//! beside its own file.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::agent_tools::SkillRef;
use crate::agent_tools::test_support::{context, stand_in, write_skill};
use crate::board::settings::WorkflowStep;
use crate::naming::PLUGIN_NAMESPACE as NS;

fn codex(bin: &Path, home: Option<&Path>) -> Codex {
    Codex::new(context(bin, home))
}

#[test]
fn names() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = codex(bin.path(), None);
    assert_eq!(adapter.tool(), Tool::Codex);
    assert_eq!(adapter.display_name(), "Codex");
    assert_eq!(adapter.short_name(), "Codex");
    assert_eq!(adapter.command_name(), "codex");
    assert_eq!(adapter.resume_hint("019a1b2c"), "codex resume 019a1b2c");
    assert_eq!(
        adapter.login_command(),
        Some(vec!["codex".to_owned(), "login".to_owned()])
    );
    assert_eq!(
        adapter.install_command(Os::Mac),
        "npm install -g @openai/codex"
    );
}

/// ADP-06: Enter 100 ms after the text (its paste detection); only the current line of a draft clears.
#[test]
fn input_quirks() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = codex(bin.path(), None);
    let quirks = adapter.input();
    assert_eq!(quirks.clear_line, b"\x05\x15");
    assert_eq!(quirks.enter_delay, Duration::from_millis(100));
    assert!(!quirks.multi_line_clearable);
    assert!(!adapter.typed_key_answers_prompt());
    assert!(!adapter.turn_end_reports_background());
    assert!(!adapter.swaps_refused_model());
}

#[test]
fn skills() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = codex(bin.path(), None);
    assert_eq!(adapter.skill_reference("commit"), format!("${NS}:commit"));
    assert_eq!(adapter.skill_delivery(), SkillDelivery::OneTimeInstall);
    assert!(!adapter.runs_skills_by_name());
}

/// d0086: a stored skill Codex can't find as a file becomes the step's built-in, with what it replaces; one it finds
/// is followed by its path; Otaq's own is named in Codex's syntax.
#[test]
fn workflow_skills_fall_back_to_the_built_in() {
    let bin = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let adapter = codex(bin.path(), Some(home.path()));
    let cwd = project.path();
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/their-plugin:commit", cwd),
        SkillRef::BuiltIn {
            skill: format!("${NS}:commit"),
            instead_of: "/their-plugin:commit".into()
        }
    );
    assert_eq!(
        adapter
            .workflow_skill(WorkflowStep::OpenPr, "/my-pr", cwd)
            .prompt(),
        format!("${NS}:open-pr")
    );
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Push, &format!("/{NS}:push"), cwd),
        SkillRef::Named(format!("${NS}:push"))
    );
    let user = write_skill(&home.path().join(".agents"), "my-commit");
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::File { file: user }
    );
    let own = write_skill(&cwd.join(".agents"), "my-commit");
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::File { file: own },
        "the project's wins"
    );
}

/// Detect runs the program on the context's PATH (here a stand-in that prints what `codex --version` prints).
#[tokio::test]
async fn detect_reads_the_version_from_the_path() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = codex(bin.path(), None);
    assert_eq!(adapter.detect().await, Detection::default());

    stand_in(bin.path(), "codex", "codex-cli 0.157.1\n");
    let found = adapter.detect().await;
    assert!(found.installed);
    assert_eq!(found.version.as_deref(), Some("0.157.1"));
    assert!(!found.update_needed);
    assert_eq!(found.signed_in, None, "T-0351 says");

    stand_in(bin.path(), "codex", "something else\n");
    assert_eq!(adapter.detect().await, Detection::default());
}

#[test]
fn hooks_are_unrecognised_until_mapped() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = codex(bin.path(), None);
    assert_eq!(
        adapter.map_hook(
            "Stop",
            &json!({"session_id": "s"}),
            SignalContext::default()
        ),
        [ToolEvent::Unrecognised {
            event: "Stop".into()
        }]
    );
}
