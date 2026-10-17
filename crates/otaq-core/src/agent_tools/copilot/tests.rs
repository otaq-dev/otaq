//! The Copilot skeleton (T-0347): names, input quirks, skills, detect on a stand-in program. Each lane adds its tests
//! beside its own file.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::agent_tools::SkillRef;
use crate::agent_tools::test_support::{context, stand_in, write_skill};
use crate::board::settings::WorkflowStep;
use crate::naming::PLUGIN_NAMESPACE as NS;

fn copilot(bin: &Path, home: Option<&Path>) -> Copilot {
    Copilot::new(context(bin, home))
}

#[test]
fn names() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = copilot(bin.path(), None);
    assert_eq!(adapter.tool(), Tool::Copilot);
    assert_eq!(adapter.display_name(), "GitHub Copilot");
    assert_eq!(adapter.short_name(), "Copilot");
    assert_eq!(adapter.command_name(), "copilot");
    assert_eq!(adapter.resume_hint("5f1c"), "copilot --resume 5f1c");
    assert_eq!(
        adapter.install_command(Os::Mac),
        "npm install -g @github/copilot"
    );
}

/// ADP-06: Enter at once; a multi-line draft clears. Esc at its prompt ends the turn quietly (d0086).
#[test]
fn input_quirks() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = copilot(bin.path(), None);
    let quirks = adapter.input();
    assert_eq!(quirks.clear_line, b"\x05\x15");
    assert_eq!(quirks.enter_delay, Duration::ZERO);
    assert!(quirks.multi_line_clearable);
    assert!(adapter.typed_key_answers_prompt());
    assert!(!adapter.turn_end_reports_background());
    assert!(adapter.swaps_refused_model());
}

#[test]
fn skills() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = copilot(bin.path(), None);
    assert_eq!(adapter.skill_reference("commit"), format!("/{NS}:commit"));
    assert_eq!(
        adapter.skill_delivery(),
        SkillDelivery::PluginFolderAtLaunch
    );
    assert!(!adapter.runs_skills_by_name());
}

/// d0086: a stored skill Copilot can't find as a file becomes the step's built-in, with what it replaces; one it finds
/// is followed by its path; Otaq's own is named in its syntax.
#[test]
fn workflow_skills_fall_back_to_the_built_in() {
    let bin = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let adapter = copilot(bin.path(), Some(home.path()));
    let cwd = project.path();
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/their-plugin:commit", cwd),
        SkillRef::BuiltIn {
            skill: format!("/{NS}:commit"),
            instead_of: "/their-plugin:commit".into()
        }
    );
    assert_eq!(
        adapter
            .workflow_skill(WorkflowStep::OpenPr, "/my-pr", cwd)
            .prompt(),
        format!("/{NS}:open-pr")
    );
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Push, &format!("/{NS}:push"), cwd),
        SkillRef::Named(format!("/{NS}:push"))
    );
    // Not the user's Claude folder.
    write_skill(&home.path().join(".claude"), "my-commit");
    assert!(matches!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::BuiltIn { .. }
    ));
    let user = write_skill(&home.path().join(".agents"), "my-commit");
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::File { file: user }
    );
    let own = write_skill(&cwd.join(".github"), "my-commit");
    assert_eq!(
        adapter.workflow_skill(WorkflowStep::Commit, "/my-commit", cwd),
        SkillRef::File { file: own },
        "the project's wins"
    );
}

/// Detect runs the program on the context's PATH (here a stand-in that prints what `copilot --version` prints).
/// The Homebrew 0.0.404 needs an update (Q-G); 1.0.98 runs.
#[tokio::test]
async fn detect_reads_the_version_and_an_old_one_needs_an_update() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = copilot(bin.path(), None);
    assert_eq!(adapter.detect().await, Detection::default());

    stand_in(
        bin.path(),
        "copilot",
        "GitHub Copilot CLI 0.0.404.\nRun copilot --banner to see the banner.\n",
    );
    assert_eq!(
        adapter.detect().await,
        Detection {
            installed: true,
            signed_in: None,
            version: Some("0.0.404".into()),
            update_needed: true,
            account: None,
        }
    );

    stand_in(bin.path(), "copilot", "GitHub Copilot CLI 1.0.98.\n");
    let found = adapter.detect().await;
    assert!(found.installed);
    assert_eq!(found.version.as_deref(), Some("1.0.98"));
    assert!(!found.update_needed);
}

#[test]
fn versions_before_1_0_98_need_an_update() {
    for (version, needed) in [
        ("0.0.404", true),
        ("1.0.97", true),
        ("0.9.200", true),
        ("1.0.98", false),
        ("1.0.98-1", false),
        ("1.1.0", false),
        ("2.0.0", false),
        ("garbage", true),
    ] {
        assert_eq!(detect::update_needed(version), needed, "{version}");
    }
    assert_eq!(
        detect::parse_version("GitHub Copilot CLI 0.0.404.\nmore").as_deref(),
        Some("0.0.404")
    );
    assert_eq!(detect::parse_version("nothing here"), None);
}

/// The hooks file's events reach the adapter (T-0352); a name it doesn't know is unrecognised.
#[test]
fn hooks_are_mapped() {
    let bin = tempfile::tempdir().unwrap();
    let adapter = copilot(bin.path(), None);
    assert_eq!(
        adapter.map_hook(
            "UserPromptSubmit",
            &json!({"session_id": "s", "prompt": "hi"}),
            SignalContext::default()
        ),
        [ToolEvent::NewTurn {
            prompt: Some("hi".into())
        }]
    );
    assert_eq!(
        adapter.map_hook("subagentStop", &json!({}), SignalContext::default()),
        [ToolEvent::Unrecognised {
            event: "subagentStop".into()
        }]
    );
}
