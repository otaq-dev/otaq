//! First run and Add project as the Tauri page draws them (T-0233; UI-08, UI-11, SET-02, PRJ-01…PRJ-03; *14 · First
//! run & onboarding*, *02 → No projects yet*): the title bar's step indicator (Welcome · Theme · Tools · Project ·
//! Done), the Welcome and Theme steps' words, the tools checklist from [`OnboardingModel`] (rows, the permission-mode
//! warning, Continue and why it is off), the Add project form from [`AddProjectModel`] (the onboarding card and the
//! later dialog), and the "No projects yet" board. Every text the GPUI app wrote in its drawing code lives here now.

use crewboard_core::board::snapshot::ProjectInfo;
use crewboard_core::naming;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::keymap::{KeyAction, Platform};
use crate::shell::tilde;
use crate::view_models::add_project::AddProjectModel;
use crate::view_models::onboarding::{OnboardingModel, RowIcon, STEPS, Step};

/// An onboarding step as the page names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "OnboardingStepId")]
pub enum StepId {
    Welcome,
    Theme,
    Tools,
    Project,
}

impl StepId {
    pub fn step(self) -> Step {
        match self {
            Self::Welcome => Step::Welcome,
            Self::Theme => Step::Theme,
            Self::Tools => Step::Tools,
            Self::Project => Step::Project,
        }
    }

    pub const ALL: [Self; 4] = [Self::Welcome, Self::Theme, Self::Tools, Self::Project];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "OnboardingDotState")]
pub enum DotState {
    /// A step behind: a check.
    Done,
    Current,
    Next,
}

/// One step of the indicator: "1 Welcome", or a check once it is behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingDot")]
pub struct Dot {
    pub number: String,
    pub label: String,
    pub state: DotState,
}

/// The window's title bar during onboarding: the product name and the five steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingTitleBar")]
pub struct TitleBar {
    pub product: String,
    pub steps: Vec<Dot>,
}

pub fn title_bar(step: StepId) -> TitleBar {
    let current = step.step().index();
    TitleBar {
        product: naming::PRODUCT_NAME.to_owned(),
        steps: STEPS
            .iter()
            .enumerate()
            .map(|(i, label)| Dot {
                number: (i + 1).to_string(),
                label: (*label).to_owned(),
                state: match i.cmp(&current) {
                    std::cmp::Ordering::Less => DotState::Done,
                    std::cmp::Ordering::Equal => DotState::Current,
                    std::cmp::Ordering::Greater => DotState::Next,
                },
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingPoint")]
pub struct Point {
    pub icon: IconName,
    pub title: String,
    pub text: String,
}

/// *1 · Welcome*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingWelcome")]
pub struct Welcome {
    pub title: String,
    pub text: String,
    pub points: Vec<Point>,
    pub foot: String,
    pub button: String,
}

pub fn welcome(platform: Platform) -> Welcome {
    let inbox_key = KeyAction::NextInboxItem.keys(platform).unwrap_or_default();
    Welcome {
        title: "Run Claude Code agents from a board".to_owned(),
        text: format!(
            "{} turns tickets into real claude sessions \u{2014} one per ticket, each in its own git worktree. You watch and type into them from an embedded terminal, review the diff, and merge or open a PR.",
            naming::PRODUCT_NAME
        ),
        points: vec![
            Point {
                icon: IconName::SquareTerminal,
                title: "Real sessions".to_owned(),
                text: format!(
                    "Each agent is the normal claude CLI. Attach from any terminal with {} attach SHOP-18.",
                    naming::COMMAND_NAME
                ),
            },
            Point {
                icon: IconName::Inbox,
                title: "One inbox".to_owned(),
                text: format!(
                    "Permission prompts, questions and reviews from every project in one list. {inbox_key} opens the next."
                ),
            },
            Point {
                icon: IconName::ShieldCheck,
                title: "You stay in control".to_owned(),
                text: "Nothing starts, retries or deletes without a click. Your repos are never written to by settings."
                    .to_owned(),
            },
        ],
        foot: "Takes about a minute".to_owned(),
        button: "Choose a theme".to_owned(),
    }
}

/// *2 · Theme* around the picker (d0070).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingTheme")]
pub struct ThemeStep {
    pub title: String,
    pub text: String,
    pub foot: String,
    pub button: String,
}

pub fn theme_step() -> ThemeStep {
    ThemeStep {
        title: "Pick a look".to_owned(),
        text: format!(
            "Pick one theme. {} uses it all the time, and the board, panels and terminal recolor as soon as you click.",
            naming::PRODUCT_NAME
        ),
        foot: "Change it anytime in Settings \u{2192} Appearance.".to_owned(),
        button: "Check my tools".to_owned(),
    }
}

/// The permission-mode warning on the tools step (d0011): a link to the docs in the middle of its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PermissionWarningView")]
pub struct PermissionWarning {
    pub title: String,
    pub before: String,
    pub link: String,
    pub after: String,
    /// The close button's name.
    pub close: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "ToolRowLook")]
pub enum RowLook {
    Found,
    MissingRequired,
    MissingOptional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ToolRowView")]
pub struct ToolRowView {
    pub name: String,
    /// "required" or "optional".
    pub badge: String,
    pub look: RowLook,
    pub icon: IconName,
    pub status: String,
    pub explanation: Option<String>,
    /// The install or login command, copyable.
    pub command: Option<String>,
}

/// *2 · Tool checklist*, *claude missing · blocking*, *gh and az missing · non-blocking*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingTools")]
pub struct Tools {
    pub title: String,
    pub text: String,
    pub warning: Option<PermissionWarning>,
    pub rows: Vec<ToolRowView>,
    /// While no report is in: "Checking your tools…".
    pub waiting: Option<String>,
    pub error: Option<String>,
    pub recheck: String,
    pub continue_label: String,
    pub continue_enabled: bool,
    /// "Install claude to continue".
    pub continue_reason: Option<String>,
    /// The user has projects already (Check tools from the board): Continue ends here, no Project step.
    pub has_projects: bool,
    pub copy: String,
}

/// The checklist's place while the check runs.
pub const CHECKING: &str = "Checking your tools\u{2026}";

pub fn tools(model: &OnboardingModel, has_projects: bool) -> Tools {
    let state = model.continue_state();
    Tools {
        title: "Tools".to_owned(),
        text: format!(
            "{} uses the tools already on your machine. It never installs or updates them.",
            naming::PRODUCT_NAME
        ),
        warning: model.show_permission_warning().then(|| PermissionWarning {
            title: "Your Claude Code settings ask for permission prompts".to_owned(),
            before: "With 5\u{2013}10 agents, prompts will stop agents often. Consider allowlists or auto mode ("
                .to_owned(),
            link: "Claude Code docs \u{2197}".to_owned(),
            after: format!(
                "). {} won\u{2019}t change your settings.",
                naming::PRODUCT_NAME
            ),
            close: "Close".to_owned(),
        }),
        rows: model
            .rows()
            .into_iter()
            .map(|r| {
                let (look, icon) = match r.icon {
                    RowIcon::Found => (RowLook::Found, IconName::CircleCheck),
                    RowIcon::MissingRequired => (RowLook::MissingRequired, IconName::CircleX),
                    RowIcon::MissingOptional => (RowLook::MissingOptional, IconName::CircleMinus),
                };
                ToolRowView {
                    name: r.name.to_owned(),
                    badge: if r.required { "required" } else { "optional" }.to_owned(),
                    look,
                    icon,
                    status: r.status,
                    explanation: r.explanation,
                    command: r.command,
                }
            })
            .collect(),
        waiting: model.report.is_none().then(|| {
            if model.checking {
                CHECKING
            } else {
                "No result yet"
            }
            .to_owned()
        }),
        error: model.error.as_ref().map(|e| format!("Check failed: {e}")),
        recheck: "Re-check".to_owned(),
        continue_label: "Continue".to_owned(),
        continue_enabled: state.enabled,
        continue_reason: state.reason,
        has_projects,
        copy: "Copy".to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FolderLook")]
pub enum FolderLook {
    /// No folder yet.
    Empty,
    /// Being checked.
    Checking,
    /// A git repository, not added yet: the green outline.
    Valid,
    /// Refused: the red outline and the error box.
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AddProjectError")]
pub struct FolderErrorView {
    pub title: String,
    pub detail: String,
}

/// A detected value: "Name shop-web".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AddProjectDetected")]
pub struct Detected {
    pub icon: IconName,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AddProjectKey")]
pub struct KeyField {
    pub label: String,
    pub note: String,
    pub value: String,
    pub placeholder: String,
    /// "Used in ticket IDs like SHOP-18. Can’t be changed later." while the key is fine.
    pub hint: Option<String>,
    pub error: Option<String>,
}

/// The base-branch field: the detected branch, editable before adding (PRJ-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AddProjectBase")]
pub struct BaseField {
    pub icon: IconName,
    pub label: String,
    pub value: String,
    pub placeholder: String,
}

/// The Add project form: the onboarding card (*3 · Add project · valid*, *Not a git repo*, *Already added*) and the
/// dialog of a later project (*Adding a second project later*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AddProjectView")]
pub struct AddProjectPage {
    pub first_run: bool,
    pub title: String,
    /// The onboarding card's line under its title.
    pub text: Option<String>,
    /// The folder as shown ("~/code/shop-web"), or "No folder chosen yet".
    pub folder: String,
    pub folder_look: FolderLook,
    /// "Choose folder…", or "Change…" in the dialog once a folder is picked.
    pub choose: String,
    pub choose_quiet: bool,
    /// The picker's title.
    pub picker_title: String,
    pub error: Option<FolderErrorView>,
    /// The repository's top folder, sent with Add.
    pub repo_path: Option<String>,
    /// The onboarding card's detected name.
    pub name: Option<Detected>,
    pub base: Option<BaseField>,
    pub key: Option<KeyField>,
    /// The onboarding card's last line.
    pub footnote: Option<String>,
    pub add: String,
    pub adding: String,
    pub can_add: bool,
    pub add_error: Option<String>,
    pub cancel: String,
}

pub const PICKER_TITLE: &str = "Add project";

pub fn add_project(
    model: &AddProjectModel,
    projects: &[ProjectInfo],
    first_run: bool,
) -> AddProjectPage {
    let folder_look = if model.probing {
        FolderLook::Checking
    } else if model.folder_error.is_some() {
        FolderLook::Refused
    } else if model.probe.is_some() {
        FolderLook::Valid
    } else {
        FolderLook::Empty
    };
    let picked = model.folder.is_some();
    let probe = model.probe.as_ref();
    let key_error = probe.and_then(|_| model.key_error(projects));
    AddProjectPage {
        first_run,
        title: if first_run {
            "Add your first project"
        } else {
            "Add project"
        }
        .to_owned(),
        text: first_run.then(|| {
            format!(
                "Pick a folder that is a git repository. {} keeps everything in its own database; nothing is written into the repo.",
                naming::PRODUCT_NAME
            )
        }),
        folder: model
            .folder
            .as_deref()
            .map_or_else(|| "No folder chosen yet".to_owned(), tilde),
        folder_look,
        choose: if !first_run && picked {
            "Change\u{2026}"
        } else {
            "Choose folder\u{2026}"
        }
        .to_owned(),
        choose_quiet: !first_run && picked,
        picker_title: PICKER_TITLE.to_owned(),
        error: model.folder_error.as_ref().map(|e| FolderErrorView {
            title: e.title.clone(),
            detail: e.detail.clone(),
        }),
        repo_path: probe.map(|p| p.repo_path.clone()),
        name: probe.filter(|_| first_run).map(|p| Detected {
            icon: IconName::Tag,
            label: "Name".to_owned(),
            value: p.name.clone(),
        }),
        base: probe.map(|_| BaseField {
            icon: IconName::GitBranch,
            label: "Base branch".to_owned(),
            value: model.base_branch.clone(),
            placeholder: "main".to_owned(),
        }),
        key: probe.map(|_| KeyField {
            label: "Project key".to_owned(),
            note: "prefilled from the repo name".to_owned(),
            value: model.key.clone(),
            placeholder: "KEY".to_owned(),
            hint: key_error.is_none().then(|| model.key_hint()),
            error: key_error,
        }),
        footnote: first_run.then(|| {
            "Setup command, run command and env files can be added later in project settings."
                .to_owned()
        }),
        add: "Add project".to_owned(),
        adding: "Adding\u{2026}".to_owned(),
        can_add: model.can_add(projects),
        add_error: model.add_error.clone(),
        cancel: "Cancel".to_owned(),
    }
}

/// *02 → No projects yet*: the board of an app with no project (onboarding skipped, or every project removed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "NoProjectsView")]
pub struct NoProjects {
    pub title: String,
    pub text: String,
    pub add: String,
    pub check: String,
    pub drop: String,
}

pub fn no_projects() -> NoProjects {
    NoProjects {
        title: "No projects yet".to_owned(),
        text: format!(
            "A project is any folder that is a git repository. {} never writes into it \u{2014} all settings stay in the app.",
            naming::PRODUCT_NAME
        ),
        add: "Add project folder\u{2026}".to_owned(),
        check: "Check tools".to_owned(),
        drop: "or drop a folder anywhere in this window".to_owned(),
    }
}

/// Everything onboarding says that doesn't depend on the machine, asked once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OnboardingTexts")]
pub struct Texts {
    pub welcome: Welcome,
    pub theme: ThemeStep,
    /// The title bar at each step, in [`StepId::ALL`]'s order.
    pub title_bars: Vec<TitleBar>,
    pub no_projects: NoProjects,
    /// The tools list while the check runs.
    pub checking: String,
}

pub fn texts(platform: Platform) -> Texts {
    Texts {
        welcome: welcome(platform),
        theme: theme_step(),
        title_bars: StepId::ALL.iter().map(|s| title_bar(*s)).collect(),
        no_projects: no_projects(),
        checking: CHECKING.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::board::projects::FolderProbe;
    use crewboard_core::tools::{
        PermissionCheck, PermissionVerdict, Tool, ToolStatus, ToolsReport,
    };

    use super::*;

    fn status(tool: Tool, found: bool) -> ToolStatus {
        ToolStatus {
            tool,
            required: tool.required(),
            found,
            version: found.then(|| "1.0".to_owned()),
            logged_in: None,
            extension_installed: None,
            install_command: (!found).then(|| tool.install_command().to_owned()),
            login_command: None,
        }
    }

    fn checked(claude: bool, verdict: PermissionVerdict) -> OnboardingModel {
        let mut m = OnboardingModel::new(false);
        m.start_check();
        m.finish_check(Ok(ToolsReport {
            tools: vec![
                status(Tool::Git, true),
                status(Tool::Claude, claude),
                status(Tool::Gh, false),
                status(Tool::Az, false),
            ],
            permission: PermissionCheck {
                verdict,
                mode: None,
                source: None,
            },
        }));
        m
    }

    #[test]
    fn the_indicator_checks_the_steps_behind() {
        let bar = title_bar(StepId::Tools);
        assert_eq!(bar.product, naming::PRODUCT_NAME);
        let states: Vec<DotState> = bar.steps.iter().map(|d| d.state).collect();
        assert_eq!(
            states,
            [
                DotState::Done,
                DotState::Done,
                DotState::Current,
                DotState::Next,
                DotState::Next
            ]
        );
        assert_eq!(bar.steps[4].label, "Done");
        assert_eq!(texts(Platform::Mac).title_bars.len(), 4);
    }

    #[test]
    fn welcome_and_theme_words_match_the_frames() {
        let w = welcome(Platform::Mac);
        assert_eq!(w.title, "Run Claude Code agents from a board");
        assert_eq!(w.points.len(), 3);
        assert!(w.points[1].text.ends_with("⌘J opens the next."));
        assert_eq!(w.button, "Choose a theme");
        let t = theme_step();
        assert_eq!(t.title, "Pick a look");
        assert_eq!(t.button, "Check my tools");
    }

    #[test]
    fn a_missing_claude_blocks_continue_and_optional_tools_dont() {
        let blocked = tools(&checked(false, PermissionVerdict::NotPrompting), false);
        assert!(!blocked.continue_enabled);
        assert_eq!(
            blocked.continue_reason.as_deref(),
            Some("Install claude to continue")
        );
        let claude = &blocked.rows[1];
        assert_eq!(
            (claude.look, claude.badge.as_str()),
            (RowLook::MissingRequired, "required")
        );
        assert!(claude.command.is_some(), "the install command copies");
        assert!(claude.explanation.is_some());
        let fine = tools(&checked(true, PermissionVerdict::NotPrompting), false);
        assert!(fine.continue_enabled);
        assert_eq!(fine.rows[2].look, RowLook::MissingOptional);
        assert_eq!(fine.rows[3].name, "az + azure-devops");
        assert!(fine.rows[2].command.is_some());
        assert_eq!(fine.warning, None);
        assert_eq!(fine.waiting, None);
    }

    #[test]
    fn the_permission_warning_shows_while_prompting() {
        let t = tools(&checked(true, PermissionVerdict::Prompting), false);
        let w = t.warning.unwrap();
        assert_eq!(
            w.title,
            "Your Claude Code settings ask for permission prompts"
        );
        assert_eq!(w.link, "Claude Code docs ↗");
        let mut waiting = OnboardingModel::new(false);
        waiting.start_check();
        assert_eq!(
            tools(&waiting, false).waiting.as_deref(),
            Some("Checking your tools…")
        );
    }

    fn probe() -> FolderProbe {
        FolderProbe {
            repo_path: "/code/shop-web".into(),
            name: "shop-web".into(),
            suggested_key: "SHOP".into(),
            base_branch: "main".into(),
        }
    }

    #[test]
    fn the_form_shows_the_detected_values_and_the_key_hint() {
        let mut m = AddProjectModel::default();
        let empty = add_project(&m, &[], true);
        assert_eq!(empty.title, "Add your first project");
        assert_eq!(empty.folder, "No folder chosen yet");
        assert_eq!(empty.folder_look, FolderLook::Empty);
        assert!(!empty.can_add && empty.key.is_none());
        m.pick("/code/shop-web".into());
        assert_eq!(add_project(&m, &[], true).folder_look, FolderLook::Checking);
        m.probed(Ok(probe()), &[], Platform::Mac);
        let valid = add_project(&m, &[], true);
        assert_eq!(valid.folder_look, FolderLook::Valid);
        assert_eq!(valid.name.unwrap().value, "shop-web");
        assert_eq!(valid.base.unwrap().value, "main");
        let key = valid.key.unwrap();
        assert_eq!(key.value, "SHOP");
        assert_eq!(
            key.hint.as_deref(),
            Some("Used in ticket IDs like SHOP-18. Can’t be changed later.")
        );
        assert!(valid.can_add);
        assert_eq!(valid.repo_path.as_deref(), Some("/code/shop-web"));
        let dialog = add_project(&m, &[], false);
        assert_eq!(
            (dialog.title.as_str(), dialog.choose.as_str()),
            ("Add project", "Change…")
        );
        assert!(dialog.name.is_none() && dialog.footnote.is_none());
    }

    #[test]
    fn refused_folders_say_why() {
        let mut m = AddProjectModel::default();
        m.pick("/tmp/designs".into());
        m.probed(
            Err((
                Some(
                    crewboard_core::board::Refusal::NotAGitRepo
                        .as_str()
                        .to_owned(),
                ),
                "not a git repository".into(),
            )),
            &[],
            Platform::Mac,
        );
        let v = add_project(&m, &[], true);
        assert_eq!(v.folder_look, FolderLook::Refused);
        assert_eq!(v.error.unwrap().title, "Not a git repository");
        assert!(!v.can_add);
    }

    #[test]
    fn no_projects_yet_matches_the_frame() {
        let v = no_projects();
        assert_eq!(v.title, "No projects yet");
        assert_eq!(v.add, "Add project folder…");
        assert_eq!(v.check, "Check tools");
    }
}
