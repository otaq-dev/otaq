//! Settings as the Tauri page draws it (T-0234; SET-01, PRJ-03, PRJ-04, SCH-03, USG-02, ATT-11, REV-09, REV-11, MSG-05,
//! UI-11; *15 · Settings*): the nav, the global page and each project page as sections of rows, every row with its
//! label, description and control already worded, and the rules for applying an edit ([`edit_global`],
//! [`edit_project`]). The page only lays the rows out and sends back `field` + value; the service still flags invalid
//! values (a missing env file, a URL that isn't one), shown in red under their field.
//!
//! Section ids are the page registry's (`ui/src/app/registry/settings.ts`): the Settings screen draws the sections
//! it has registered, so a later ticket adds a section here and registers its id there. T-0240 added Validation &
//! Auto-merge and the Project agent instructions (VAL-13, AMG-01, PAG-03; d0063, d0064, d0068): a project's empty
//! value uses the global one and says so ("Use global (3)"), and a set one offers "Use global" to empty it again.

use otaq_core::attention::notify::{Kind, PermissionState};
use otaq_core::board::projects::ProjectUpdate;
use otaq_core::board::settings::{
    AutoMerge, EnvVar, GlobalSettings, ProjectSettings, SettingProblem, SkillSetting,
    TERMINAL_FONT_SIZES, TerminalRenderer, UsageData, VALIDATION_ROUNDS, WorkflowStep,
    env_var_field, env_var_problems,
};
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::named_agents::{NamedAgent, RoleDefaults};
use otaq_core::naming::{self, PRODUCT_NAME};
use otaq_core::pr::provider::PrProvider;
use otaq_core::scheduler::SchedulerStatus;
use otaq_core::tools::ToolsReport;
use otaq_core::warm::clone::CloneSupport;
use otaq_core::warm::store::{WarmBuildInfo, WarmList};
use otaq_core::workflow_skills::{SkillKind, SkillStatus, copy_missing};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::keymap::{Platform, TerminalKey};

/// The OS the app runs on: the file manager's name and the terminal's font keys differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    fn keys(self) -> Platform {
        match self {
            Self::Mac => Platform::Mac,
            Self::Windows | Self::Linux => Platform::WinLinux,
        }
    }
}
use crate::shell::tilde;
use crate::view_models::cleanup::{ConfirmLine, WarmConfirmPage};
use crate::view_models::limits::{
    BACK_PRESSURE_ITEMS, MAX_AGENTS, right_now, settings_rows, toggled,
};
use crate::view_models::menu::MenuSearch;
use crate::view_models::named_agents;
use crate::view_models::onboarding::{OnboardingModel, RowIcon};
use crate::view_models::onboarding_page::{
    ToolButton, ToolTerminalTexts, tool_button, tool_terminal_texts, tools_intro,
};
use crate::view_models::settings::{
    GLOBAL_SECTIONS, PROJECT_ICON, ProviderTone, env_file_problem, instructions_placeholder,
    problem_for, provider_row, remove_text,
};
use crate::view_models::theme_credits;
use crate::view_models::usage_data;
use crate::view_models::warm_build;

/// A section of the global page; its id is the page registry's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "GlobalSectionId")]
pub enum GlobalSection {
    /// M23, T-0360: the agent tools, drawn from `agent_tools.view` ([`super::agent_tools_page`]).
    AgentTools,
    /// M22, T-0340: named agents and the role defaults, drawn from `settings.agents` ([`named_agents::section`]).
    Agents,
    AgentLimits,
    AgentInstructions,
    WorkflowSkills,
    /// M12, T-0240.
    Validation,
    /// M12, T-0240.
    ProjectAgentInstructions,
    Usage,
    Notifications,
    Data,
    Appearance,
    /// M29, T-0395: custom shortcuts and the single-letter switch, drawn from `keyboard.view` ([`super::keyboard_page`]).
    KeyboardShortcuts,
    Tools,
    /// The last one: the mark, version, licenses and credits (d0071).
    About,
}

/// A section of a project page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "ProjectSectionId")]
pub enum ProjectSection {
    /// Name, key, repository, base branch.
    General,
    /// M22, T-0340: the role default overrides, drawn from `settings.project_agents` ([`project_agents_block`]).
    Agents,
    AgentInstructions,
    /// M12, T-0240.
    ProjectAgentInstructions,
    WorkflowSkills,
    /// M12, T-0240.
    Validation,
    /// "Let finished tickets answer questions".
    Answers,
    /// Setup and run commands, Open URL, env files.
    Commands,
    /// Create, Remove and Template refresh commands, schema paths (M19-d, M19-e; d0087).
    TicketResources,
    PrProvider,
    RemoveProject,
}

/// Where a nav row goes: one topic of the global page, or a project's page. A global topic is its own page: the
/// window shows only its `sections` (the first is `section`), never the whole global page scrolling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "page", rename_all = "snake_case")]
#[ts(rename = "SettingsTarget")]
pub enum Target {
    Global {
        section: GlobalSection,
        sections: Vec<GlobalSection>,
    },
    Project {
        key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsNavRow")]
pub struct NavRow {
    pub label: String,
    pub icon: IconName,
    pub target: Target,
    /// A flagged value on that project's page: the red alert icon after the label.
    pub alert: bool,
}

/// One choice of a segmented control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsChoice")]
pub struct Choice {
    pub id: String,
    pub label: String,
}

/// A choice of a radio list, with its explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsRadioChoice")]
pub struct RadioChoice {
    pub id: String,
    pub label: String,
    pub detail: String,
}

/// One workflow step's row on Settings → Workflow skills (WSK-01, WSK-02, WSK-04, WSK-07; d0092; *15 → update AC*):
/// the step, the skill's name with its tag, where it came from and when, the lines under it, the button and the ⋯
/// menu. The page keeps only the menu's open state, the dialogs and the folder picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillRow")]
pub struct SkillRow {
    /// `workflow_skills.open_pr`: the row's button carries its DOM id ("use your own Open PR skill" focuses it).
    pub field: String,
    #[ts(type = "\"commit\" | \"push\" | \"open_pr\" | \"validate\"")]
    pub step: WorkflowStep,
    pub label: String,
    /// `my-commit`, `otaq:commit`, a typed `/my-commit`.
    pub name: String,
    /// Drawn in the secondary colour: a step on "Use global", or a typed name.
    pub name_muted: bool,
    pub tag: SkillTag,
    /// The original folder, in mono after the folder icon (an imported skill).
    pub folder: Option<String>,
    /// "· imported today, 10:42", "Otaq's default", "Global: imported from ~/team-skills/my-commit".
    pub source: String,
    pub lines: Vec<SkillLine>,
    /// Import skill folder… or Re-import; none while the copy is missing (Import again is in its line).
    pub button: Option<SkillButton>,
    /// The ⋯ menu; empty: no ⋯.
    pub menu: Vec<SkillMenuItem>,
    /// "More for Commit".
    pub menu_label: String,
    /// The folder picker's title.
    pub picker_title: String,
    /// Asked before Re-import.
    pub reimport: Option<SkillDialog>,
    /// Asked before Use built-in / Use global, only when the service answers `needs_confirmation`.
    pub reset: Option<SkillDialog>,
}

/// How a tag or a line under a skill looks: quiet, amber (`st-paused`) or red (`st-error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SettingsSkillLook")]
pub enum SkillLook {
    Quiet,
    Warning,
    Error,
}

/// Built-in, Imported, Use global, Not imported (amber), Skill missing (red).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillTag")]
pub struct SkillTag {
    pub label: String,
    pub look: SkillLook,
}

/// A line under the skill: a refused import (red), a missing file (amber), the original gone, the copy note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillLine")]
pub struct SkillLine {
    pub icon: IconName,
    pub text: String,
    pub look: SkillLook,
    /// "Import again" after "Skill missing ·".
    pub link: Option<SkillButton>,
}

/// What a skill row's button, link or menu item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SettingsSkillAction")]
pub enum SkillAction {
    /// The folder picker, then `settings.skill_import` (Import skill folder…, Import a different folder…, Import
    /// again when the original is gone).
    Import,
    /// Asks [`SkillRow::reimport`], then `settings.skill_reimport`.
    Reimport,
    /// Import again with the original there: `settings.skill_reimport` at once (the copy is gone or broken).
    ImportAgain,
    /// Otaq's copy in the file manager (`settings.reveal`).
    Reveal,
    /// Use built-in / Use global: `settings.skill_reset`, asking [`SkillRow::reset`] when the service says so.
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillButton")]
pub struct SkillButton {
    pub label: String,
    pub icon: Option<IconName>,
    pub action: SkillAction,
    /// Re-import with the original gone (the reason is a line under the skill).
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillMenuItem")]
pub struct SkillMenuItem {
    pub action: SkillAction,
    pub label: String,
    pub icon: IconName,
    /// A separator above it.
    pub separated: bool,
}

/// Re-import's question (accent button) or going back with an edited copy (red button).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillDialog")]
pub struct SkillDialog {
    pub title: String,
    pub text: String,
    /// The grey note with the history icon.
    pub note: String,
    pub confirm: String,
    pub destructive: bool,
}

/// One env file, its field `env_files.<index>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsEnvFile")]
pub struct EnvFileRow {
    pub value: String,
    pub problem: Option<String>,
}

/// One project environment variable, `NAME = value` (ENV-01; *15 → Row · Environment variables (0082)*). Its fields
/// are `env_vars.<index>.name` and `env_vars.<index>.value`; `problem` (ENV-03, from the service) marks the name red.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsEnvVar")]
pub struct EnvVarRow {
    pub name: String,
    pub value: String,
    pub problem: Option<String>,
    /// The × button's name and tooltip: "Remove NODE_OPTIONS".
    pub remove_label: String,
}

/// What a row edits or shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "SettingsControl")]
pub enum Control {
    /// Saved on blur and on ↵ (a multiline field: on blur and ⌘↵).
    Text {
        value: String,
        placeholder: String,
        mono: bool,
        multiline: bool,
        problem: Option<String>,
    },
    Toggle {
        on: bool,
    },
    /// The main notifications switch with macOS's answer under the row's text (ATT-14, d0090): sent like a toggle.
    /// `status` is `None` where the OS state isn't known (no line).
    NotificationsSwitch {
        on: bool,
        status: Option<OsStatus>,
    },
    /// A checkbox row: a notification kind or Play a sound (ATT-16), indented under the main switch and disabled
    /// (not hidden) while it is off.
    Checkbox {
        on: bool,
        disabled: bool,
        indented: bool,
    },
    Stepper {
        value: u32,
        min: u32,
        max: u32,
        /// " px" after the number.
        suffix: String,
        disabled: bool,
        /// Shown in place of the value while a project uses the global one: "Use global (3)". − and + start from
        /// `value` (the global one).
        empty: Option<String>,
    },
    /// One of a few choices, each with its one-line explanation (Auto-merge). `value` is `None` while a project
    /// uses the global one (`empty` says which).
    Radio {
        choices: Vec<RadioChoice>,
        value: Option<String>,
        disabled: bool,
        empty: Option<String>,
        /// Under the choices, while they do nothing: "Validation is off for shop-web, so Auto-merge does nothing…".
        note: Option<String>,
    },
    Segmented {
        choices: Vec<Choice>,
        value: String,
    },
    /// A quiet text in place of a control ("Always on").
    Note {
        text: String,
    },
    /// A read-only value; `boxed` draws it in a field-like box (the project key).
    Value {
        text: String,
        mono: bool,
        boxed: bool,
    },
    /// A path with the button that shows it in the file manager (`settings.reveal`).
    Path {
        text: String,
        reveal: Reveal,
        reveal_label: String,
    },
    /// A button; `action` is what the Settings screen does with a click.
    Button {
        label: String,
        icon: IconName,
        destructive: bool,
        action: ButtonAction,
    },
    /// The workflow skills (WSK-01; d0092), one row per step across the page's width under `heading`; `hint` is
    /// the dot-folder line under them (macOS only).
    Skills {
        heading: String,
        rows: Vec<SkillRow>,
        hint: Option<String>,
    },
    /// ↵ in the last field adds a file (`env_files.add`); × removes one (`env_files.remove`).
    EnvFiles {
        rows: Vec<EnvFileRow>,
        add_placeholder: String,
        remove_label: String,
    },
    /// The project environment variables (ENV-01, d0082), drawn across the row's width under its text. "Add
    /// variable" opens a new row (`env_vars.add` with a `{ name, value }` once it has a name); × removes one
    /// (`env_vars.remove`).
    EnvVars {
        rows: Vec<EnvVarRow>,
        /// The always-visible example: `CI = true`, then its reason.
        hint_example: String,
        hint: String,
        /// The box shown while there are no rows.
        empty: String,
        add_label: String,
        /// The × of a new line that isn't saved yet.
        drop_label: String,
        /// The line under the list: changes reach only what starts after them.
        applies: String,
        name_placeholder: String,
        value_placeholder: String,
    },
    /// "Start new worktrees from a warm build" (WRM-05, WRM-06, d0082). The switch sends `warm_builds` like a
    /// toggle; turning it off first asks `confirm` when there is one (only when it would delete a warm build).
    WarmBuild {
        on: bool,
        /// Drawn off and disabled, `reason` under it and in its tooltip. Never set since M19-b: the switch works on
        /// every disk (where the disk can't clone, the build is handed over).
        disabled: bool,
        reason: Option<String>,
        /// "Clearing… (waiting for SHOP-21’s copy)" while the cleared warm build waits for a seed.
        clearing: Option<String>,
        /// What turning it off asks (*Warm build · turning off · confirm (0082)*).
        confirm: Option<WarmConfirmPage>,
    },
    /// The base branch: a dropdown of the repository's branches (`settings.branches`).
    Branch {
        value: String,
        /// The dropdown's search field.
        search: MenuSearch,
    },
    /// The 15 themes (`theme.picker`, `theme.pick`).
    Themes,
    /// The PR provider, detected when the page opens (`settings.provider`).
    Provider,
    /// The tools checklist (`settings.tools`) and Re-check.
    Tools {
        recheck_label: String,
    },
    /// The About section (`settings.about`; d0071): the page draws it whole.
    About,
}

/// What the OS allows Otaq's notifications, as one line with the one action that fits (ATT-14, d0090).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsOsStatus")]
pub struct OsStatus {
    pub text: String,
    /// "Allow notifications" or "Open System Settings".
    pub action: Option<OsStatusAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsOsStatusAction")]
pub struct OsStatusAction {
    pub label: String,
    pub action: ButtonAction,
}

/// The status line under the main switch, by macOS's permission (ATT-14, d0090; *CHANGES-2026-10-18-aa* §4). Shown
/// whatever the switch's state; none where the state isn't read (Windows and Linux for now).
pub fn os_status(permission: PermissionState) -> Option<OsStatus> {
    let action = |label: &str, action| {
        Some(OsStatusAction {
            label: label.to_owned(),
            action,
        })
    };
    let (text, action) = match permission {
        PermissionState::NotAsked => (
            "macOS hasn\u{2019}t been asked yet.".to_owned(),
            action("Allow notifications", ButtonAction::AllowNotifications),
        ),
        PermissionState::Allowed => ("Allowed in macOS.".to_owned(), None),
        PermissionState::TurnedOff => (
            format!("Turned off in macOS, so {PRODUCT_NAME} can\u{2019}t notify you."),
            action(
                crate::view_models::banners::OPEN_SYSTEM_SETTINGS,
                ButtonAction::OpenSystemSettings,
            ),
        ),
        PermissionState::OutsideBundle => (
            format!(
                "Running outside {}: notifications appear as Script Editor, and {PRODUCT_NAME} can\u{2019}t check \
                 whether macOS allows them.",
                naming::bundle_folder_name()
            ),
            None,
        ),
        PermissionState::Unknown => return None,
    };
    Some(OsStatus { text, action })
}

/// The checkbox of each notification kind (ATT-16, d0090), in d0090's order: its field, label and who it covers
/// (d0090's parentheses; none where d0090 gives none).
pub fn notification_kind_row(kind: Kind) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Permission => (
            "notification_kinds.permission",
            "Permission prompts",
            "Agents, validators, Project agents and epic leads.",
        ),
        Kind::Question => (
            "notification_kinds.question",
            "Questions",
            "Agents, Project agents and epic leads.",
        ),
        Kind::TurnEnded => ("notification_kinds.turn_ended", "Agent ended its turn", ""),
        Kind::Interrupted => ("notification_kinds.interrupted", "Session interrupted", ""),
        Kind::ValidationDecision => (
            "notification_kinds.validation_decision",
            "Validation needs a decision",
            "Failed N times, or no result.",
        ),
        Kind::AutoMergeStopped => (
            "notification_kinds.auto_merge_stopped",
            "Auto-merge stopped",
            "",
        ),
        Kind::ToolUnavailable => (
            "notification_kinds.tool_unavailable",
            "Agent tool not available",
            "",
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SettingsReveal")]
pub enum Reveal {
    Database,
    Logs,
    /// Otaq's copy of an imported workflow skill (WSK-04): the global setting's, or `project`'s.
    Skill {
        project: Option<String>,
        #[ts(type = "\"commit\" | \"push\" | \"open_pr\" | \"validate\"")]
        step: WorkflowStep,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SettingsButtonAction")]
pub enum ButtonAction {
    /// Opens Recovery and Cleanup.
    Cleanup,
    /// Opens the Theme credits dialog.
    Credits,
    /// Asks before removing the project.
    RemoveProject,
    /// Brings up macOS's notifications question (in the app's process) and answers Otaq's ask (ATT-13, ATT-14).
    AllowNotifications,
    /// Opens Otaq's entry in System Settings → Notifications (ATT-14).
    OpenSystemSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsRow")]
pub struct Row {
    /// What `settings.edit` names; empty for a row that edits nothing.
    pub field: String,
    pub label: String,
    pub description: String,
    pub control: Control,
    /// The limits are off: the row dims with its control.
    pub dimmed: bool,
    /// A project's own value: the link that empties it so the global one applies again (`settings.edit` with
    /// `""`).
    pub reset: Option<String>,
    /// A button after the control: Run command's **Ask the agent** (T-0232, T-0342), which opens the run-command
    /// dialog.
    pub button: Option<RowButton>,
}

/// A row's button and what it opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsRowButton")]
pub struct RowButton {
    pub label: String,
    pub icon: IconName,
    /// "Asks Strong (the Project agent's) to suggest a run command".
    pub tooltip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSection")]
pub struct Section<Id> {
    pub id: Id,
    /// The small title above the rows (the global page); none on a project page.
    pub title: Option<String>,
    pub intro: Option<String>,
    pub rows: Vec<Row>,
    /// A line under the rows ("Right now: 3 running · …").
    pub note: Option<String>,
}

/// The page on the right: "General" (the global sections; the window shows one nav topic's at a time) or a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "page", rename_all = "snake_case")]
#[ts(rename = "SettingsPage")]
pub enum Page {
    Global {
        title: String,
        sections: Vec<Section<GlobalSection>>,
    },
    Project {
        key: String,
        title: String,
        intro: String,
        sections: Vec<Section<ProjectSection>>,
    },
}

/// The whole Settings view: the nav and the open page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsView")]
pub struct View {
    pub title: String,
    pub global_heading: String,
    pub projects_heading: String,
    pub close_label: String,
    pub nav: Vec<NavRow>,
    pub page: Page,
}

/// What the global page is built from.
pub struct GlobalInputs<'a> {
    pub settings: &'a GlobalSettings,
    pub problems: &'a [SettingProblem],
    /// The scheduler's view, for the "Right now" line while the limits are on.
    pub scheduler: Option<&'a SchedulerStatus>,
    /// The database file and the logs folder (`settings::data_paths`).
    pub data: Option<(String, String)>,
    pub cleanup_count: usize,
    pub os: Os,
    /// What the OS allows Otaq's notifications (`notifications.state`); `Unknown` when it couldn't be read.
    pub notification_permission: PermissionState,
    /// The Workflow skills rows' statuses (`workflow_skills.status`).
    pub skills: SkillInputs<'a>,
}

/// What a project page is built from.
pub struct ProjectInputs<'a> {
    pub info: &'a ProjectInfo,
    pub settings: &'a ProjectSettings,
    pub problems: &'a [SettingProblem],
    pub global: &'a GlobalSettings,
    /// `warm.list` and the time now (for the warm build's age); `None` when it couldn't be read.
    pub warm: Option<WarmInputs<'a>>,
    /// The name of the project's Project agent (its role's named agent), which Ask the agent asks.
    pub project_agent: &'a str,
    /// An agent tool is connected: Run command's Ask the agent is hidden only without one (RUN-02, d0086).
    pub tool_connected: bool,
    /// The Workflow skills rows' statuses, the project's and the global page's (`workflow_skills.status`).
    pub skills: SkillInputs<'a>,
}

/// What the warm build switch is drawn from.
#[derive(Clone, Copy)]
pub struct WarmInputs<'a> {
    pub list: &'a WarmList,
    pub now: i64,
}

/// The global sections, in nav order.
const DRAWN: [GlobalSection; 14] = [
    GlobalSection::AgentTools,
    GlobalSection::Agents,
    GlobalSection::AgentLimits,
    GlobalSection::AgentInstructions,
    GlobalSection::WorkflowSkills,
    GlobalSection::Validation,
    GlobalSection::ProjectAgentInstructions,
    GlobalSection::Usage,
    GlobalSection::Notifications,
    GlobalSection::Data,
    GlobalSection::Appearance,
    GlobalSection::KeyboardShortcuts,
    GlobalSection::Tools,
    GlobalSection::About,
];

/// The nav row a global section is drawn under: Agent limits and Agent instructions sit on the Agents page, every
/// other section is a topic of its own.
pub fn topic_of(id: GlobalSection) -> GlobalSection {
    match id {
        GlobalSection::AgentLimits | GlobalSection::AgentInstructions => GlobalSection::Agents,
        other => other,
    }
}

/// The nav: one row per global topic the page has, then every project.
pub fn nav(sections: &[GlobalSection], projects: &[ProjectInfo], alerts: &[String]) -> Vec<NavRow> {
    let globals = sections
        .iter()
        .filter(|id| topic_of(**id) == **id)
        .map(|id| {
            let (label, icon) = nav_label(*id);
            NavRow {
                label: label.to_owned(),
                icon,
                target: Target::Global {
                    section: *id,
                    sections: sections
                        .iter()
                        .copied()
                        .filter(|s| topic_of(*s) == *id)
                        .collect(),
                },
                alert: false,
            }
        });
    let projects = projects.iter().map(|p| NavRow {
        label: p.name.clone(),
        icon: PROJECT_ICON,
        target: Target::Project { key: p.key.clone() },
        alert: alerts.contains(&p.key),
    });
    globals.chain(projects).collect()
}

/// A global section's nav label and icon (*15 → Global · Usage, notifications, data, appearance*).
fn nav_label(id: GlobalSection) -> (&'static str, IconName) {
    let shared = |i: usize| GLOBAL_SECTIONS[i];
    match id {
        GlobalSection::AgentTools => ("Agent tools", IconName::Plug),
        GlobalSection::Agents => ("Agents", IconName::Bot),
        GlobalSection::AgentLimits => shared(0),
        GlobalSection::AgentInstructions => shared(1),
        GlobalSection::WorkflowSkills => shared(2),
        GlobalSection::Validation => ("Validation & Auto-merge", IconName::ShieldCheck),
        GlobalSection::ProjectAgentInstructions => ("Project agents", IconName::Bot),
        GlobalSection::Usage => shared(3),
        GlobalSection::Notifications => shared(4),
        GlobalSection::Data => shared(5),
        GlobalSection::Appearance => shared(6),
        GlobalSection::KeyboardShortcuts => ("Keyboard shortcuts", IconName::Keyboard),
        GlobalSection::Tools => shared(7),
        GlobalSection::About => ("About", IconName::Info),
    }
}

/// The view around a page.
pub fn view(page: Page, projects: &[ProjectInfo], alerts: &[String]) -> View {
    let sections: Vec<GlobalSection> = DRAWN.to_vec();
    View {
        title: "Settings".into(),
        global_heading: "Global".into(),
        projects_heading: "Projects".into(),
        close_label: "Close settings".into(),
        nav: nav(&sections, projects, alerts),
        page,
    }
}

fn row(field: &str, label: &str, description: impl Into<String>, control: Control) -> Row {
    Row {
        field: field.to_owned(),
        label: label.to_owned(),
        description: description.into(),
        control,
        dimmed: false,
        reset: None,
        button: None,
    }
}

fn text(value: &str, placeholder: &str, mono: bool, problem: Option<String>) -> Control {
    Control::Text {
        value: value.to_owned(),
        placeholder: placeholder.to_owned(),
        mono,
        multiline: false,
        problem,
    }
}

fn multiline(value: &str, placeholder: &str) -> Control {
    Control::Text {
        value: value.to_owned(),
        placeholder: placeholder.to_owned(),
        mono: false,
        multiline: true,
        problem: None,
    }
}

fn section<Id>(id: Id, title: Option<&str>, rows: Vec<Row>) -> Section<Id> {
    Section {
        id,
        title: title.map(str::to_owned),
        intro: None,
        rows,
        note: None,
    }
}

/// The label of a workflow step's field.
pub fn step_label(step: WorkflowStep) -> &'static str {
    match step {
        WorkflowStep::Commit => "Commit",
        WorkflowStep::Push => "Push",
        WorkflowStep::OpenPr => "Open PR",
        WorkflowStep::Validate => "Validate",
    }
}

/// "Reveal in Finder" as each OS calls it.
pub fn reveal_label(os: Os) -> &'static str {
    match os {
        Os::Mac => "Reveal in Finder",
        Os::Windows => "Show in Explorer",
        Os::Linux => "Show in Files",
    }
}

/// The terminal renderer's words (Q38, built as option 1).
pub fn renderer_choices() -> Vec<Choice> {
    [
        (TerminalRenderer::Webgl, "WebGL (faster)"),
        (TerminalRenderer::Dom, "DOM (safer)"),
    ]
    .into_iter()
    .map(|(r, label)| Choice {
        id: renderer_id(r).to_owned(),
        label: label.to_owned(),
    })
    .collect()
}

fn renderer_id(renderer: TerminalRenderer) -> &'static str {
    match renderer {
        TerminalRenderer::Webgl => "webgl",
        TerminalRenderer::Dom => "dom",
    }
}

pub fn global_page(input: &GlobalInputs) -> Page {
    let s = input.settings;
    let limits = s.agent_limits;
    let [toggle, max, pause] = settings_rows(&limits);
    let stepper = |value, range: &std::ops::RangeInclusive<u32>| Control::Stepper {
        value,
        min: *range.start(),
        max: *range.end(),
        suffix: String::new(),
        disabled: !limits.enabled,
        empty: None,
    };
    let limit_rows = vec![
        row(
            "agent_limits.enabled",
            toggle.label,
            toggle.description,
            Control::Toggle { on: limits.enabled },
        ),
        Row {
            dimmed: !max.enabled,
            ..row(
                "agent_limits.max_concurrency",
                max.label,
                max.description,
                stepper(limits.max_concurrency, &MAX_AGENTS),
            )
        },
        Row {
            dimmed: !pause.enabled,
            ..row(
                "agent_limits.back_pressure",
                pause.label,
                pause.description,
                stepper(limits.back_pressure, &BACK_PRESSURE_ITEMS),
            )
        },
    ];
    let agent_limits = Section {
        intro: Some("Optional caps on how many agents run at once.".into()),
        note: input.scheduler.filter(|_| limits.enabled).map(right_now),
        ..section(GlobalSection::AgentLimits, Some("Agent limits"), limit_rows)
    };

    let instructions = section(
        GlobalSection::AgentInstructions,
        Some("Agent instructions"),
        vec![row(
            "agent_instructions",
            "Default instructions",
            "Given to every agent in every project. A project can override it.",
            multiline(
                &s.agent_instructions,
                "Your rules for every agent, e.g. “Ask before coding.”",
            ),
        )],
    );

    let skill_statuses = statuses_of(input.skills.statuses, |step| {
        status_of(step, s.workflow_skills.get(step))
    });
    let skills = Section {
        intro: Some(
            "Commit, Push, Open PR and Validate follow these skills. Import a skill folder (its SKILL.md and the files \
             it uses) to replace one with your own. Projects can override each step."
                .into(),
        ),
        ..section(
            GlobalSection::WorkflowSkills,
            None,
            vec![row(
                "",
                "Workflow skills",
                "",
                skills_control("Steps", &skill_statuses, None, &input.skills),
            )],
        )
    };

    let validation = section(
        GlobalSection::Validation,
        Some("Validation & Auto-merge"),
        vec![
            row(
                "validation",
                "Validation",
                "A separate read-only claude session (the validator) checks every submit before Review. Failures go back to the agent. Projects can turn it off.",
                Control::Toggle { on: s.validation },
            ),
            row(
                "validation_rounds",
                "Validation rounds",
                ROUNDS_DESCRIPTION,
                rounds_stepper(s.validation_rounds, None),
            ),
            row(
                "auto_merge",
                "Auto-merge",
                AUTO_MERGE_DESCRIPTION,
                auto_merge_radio(
                    Some(s.auto_merge),
                    s.auto_merge,
                    (!s.validation).then(|| {
                        "Validation is off, so Auto-merge does nothing. Turn Validation on to use it.".to_owned()
                    }),
                ),
            ),
        ],
    );

    let project_agents = section(
        GlobalSection::ProjectAgentInstructions,
        Some("Project agents"),
        vec![row(
            "project_agent_instructions",
            "Project agent instructions",
            "Followed by every Project agent (the sessions that investigate and create tickets), e.g. which import skill to use and how big tickets should be.",
            multiline(
                &s.project_agent_instructions,
                "Your rules for Project agents, e.g. “Split into tickets of about one day.”",
            ),
        )],
    );

    let usage = section(
        GlobalSection::Usage,
        Some("Usage"),
        vec![row(
            "",
            "Token count",
            "Always shown in the header (today, all projects) and per ticket.",
            Control::Note {
                text: "Always on".into(),
            },
        )],
    );

    // The main switch with macOS's answer under it, then a checkbox per kind and Play a sound, disabled (not hidden)
    // while the switch is off (ATT-14, ATT-16; d0090).
    let mut notification_rows = vec![row(
        "notifications",
        "OS notifications for blocking items",
        "Permission prompts, agent questions and interrupted sessions. Nothing else ever notifies.",
        Control::NotificationsSwitch {
            on: s.notifications,
            status: os_status(input.notification_permission),
        },
    )];
    let checkbox = |field: &str, label: &str, description: &str, on: bool| Row {
        dimmed: !s.notifications,
        ..row(
            field,
            label,
            description,
            Control::Checkbox {
                on,
                disabled: !s.notifications,
                indented: true,
            },
        )
    };
    for kind in Kind::ALL {
        let (field, label, description) = notification_kind_row(kind);
        notification_rows.push(checkbox(
            field,
            label,
            description,
            s.notification_kinds.allows(kind),
        ));
    }
    notification_rows.push(checkbox(
        "notification_sound",
        "Play a sound",
        "The OS\u{2019}s default notification sound.",
        s.notification_sound,
    ));
    let notifications = section(
        GlobalSection::Notifications,
        Some("Notifications"),
        notification_rows,
    );

    let reveal = reveal_label(input.os);
    let mut data_rows = Vec::new();
    if let Some((db, logs)) = &input.data {
        data_rows.push(row(
            "",
            "Database file",
            format!(
                "Tickets, runs and settings. Your normal machine backup covers it — {PRODUCT_NAME} has no separate backup."
            ),
            Control::Path {
                text: db.clone(),
                reveal: Reveal::Database,
                reveal_label: reveal.to_owned(),
            },
        ));
        let folder = logs.trim_end_matches(['/', '\\']).trim_end_matches("logs");
        data_rows.push(row(
            "",
            "Logs folder",
            format!(
                "Service and session logs. The same {folder} folder on macOS, Windows and Linux."
            ),
            Control::Path {
                text: logs.clone(),
                reveal: Reveal::Logs,
                reveal_label: reveal.to_owned(),
            },
        ));
    }
    let count = input.cleanup_count;
    data_rows.push(row(
        "",
        "Cleanup",
        if count == 0 {
            "Leftover sessions, worktrees, branches and ports that no ticket owns. Nothing is deleted without your click.".to_owned()
        } else {
            format!(
                "{count} {} waiting. Nothing is deleted without your click.",
                if count == 1 { "leftover" } else { "leftovers" }
            )
        },
        Control::Button {
            label: "Cleanup…".into(),
            icon: IconName::BrushCleaning,
            destructive: false,
            action: ButtonAction::Cleanup,
        },
    ));
    // d0085: off until the user said yes, in the first-run dialog or here.
    data_rows.push(row(
        "usage_data",
        usage_data::ROW_LABEL,
        usage_data::row_description(),
        Control::Toggle {
            on: s.usage_data.sends(),
        },
    ));
    let data = section(GlobalSection::Data, Some("Data"), data_rows);

    let bigger = TerminalKey::FontBigger.keys(input.os.keys())[0];
    let smaller = TerminalKey::FontSmaller.keys(input.os.keys())[0];
    let appearance = section(
        GlobalSection::Appearance,
        Some("Appearance"),
        vec![
            row(
                "theme",
                "Theme",
                "One theme, used all the time. Click a card to recolor the window; it saves at once.",
                Control::Themes,
            ),
            row(
                "",
                "Theme credits",
                theme_credits::ROW_DESCRIPTION,
                Control::Button {
                    label: "View credits".into(),
                    icon: IconName::ScrollText,
                    destructive: false,
                    action: ButtonAction::Credits,
                },
            ),
            row(
                "terminal_font_size",
                "Terminal font size",
                format!("Geist Mono. Also {bigger} / {smaller} inside a focused terminal."),
                Control::Stepper {
                    value: s.terminal_font_size,
                    min: *TERMINAL_FONT_SIZES.start(),
                    max: *TERMINAL_FONT_SIZES.end(),
                    suffix: " px".into(),
                    disabled: false,
                    empty: None,
                },
            ),
            // Q38, built as option 1: one global row, read by every terminal (T-0216).
            row(
                "terminal_renderer",
                "Terminal renderer",
                "WebGL draws on the GPU. Pick DOM if a terminal stays blank or garbled; every terminal redraws at once.",
                Control::Segmented {
                    choices: renderer_choices(),
                    value: renderer_id(s.terminal_renderer).to_owned(),
                },
            ),
        ],
    );

    let tools = Section {
        intro: Some(tools_intro()),
        ..section(
            GlobalSection::Tools,
            Some("Tools"),
            vec![row(
                "",
                "",
                "",
                Control::Tools {
                    recheck_label: "Re-check".into(),
                },
            )],
        )
    };

    Page::Global {
        title: "General".into(),
        sections: vec![
            // Its own view: the four tools' rows ([`super::agent_tools_page`]).
            section(GlobalSection::AgentTools, Some("Agent tools"), Vec::new()),
            // Its own view: the table, the dialogs and the defaults ([`named_agents::section`]).
            section(GlobalSection::Agents, Some("Agents"), Vec::new()),
            agent_limits,
            instructions,
            skills,
            validation,
            project_agents,
            usage,
            notifications,
            data,
            appearance,
            // Its own view: the switch and every action's keys ([`super::keyboard_page`]).
            section(
                GlobalSection::KeyboardShortcuts,
                Some("Keyboard shortcuts"),
                Vec::new(),
            ),
            tools,
            section(
                GlobalSection::About,
                Some("About"),
                vec![row("", "", "", Control::About)],
            ),
        ],
    }
}

const ROUNDS_DESCRIPTION: &str = "How many times the validator may send work back before the ticket comes to you (\u{201c}Validation failed N times\u{201d}). 1\u{2013}5.";
const AUTO_MERGE_DESCRIPTION: &str = "Lets validated tickets land without your click. Only ever a Local merge, never pushed; epics and tickets that skipped or failed validation never auto-merge.";
const USE_GLOBAL: &str = "Use global";

/// Validation rounds, 1–5; `empty` while a project uses the global value.
fn rounds_stepper(value: u32, empty: Option<String>) -> Control {
    Control::Stepper {
        value,
        min: *VALIDATION_ROUNDS.start(),
        max: *VALIDATION_ROUNDS.end(),
        suffix: String::new(),
        disabled: false,
        empty,
    }
}

/// Auto-merge's three choices with the frame's explanations (d0064).
pub fn auto_merge_choices() -> Vec<RadioChoice> {
    [
        (
            AutoMerge::Off,
            "Off",
            "Validated tickets wait in Ready for Review for you. (default)",
        ),
        (
            AutoMerge::EpicChildren,
            "Epic children only",
            "Validated epic children are merged into their epic branch; you still review the epic.",
        ),
        (
            AutoMerge::All,
            "All tickets",
            "Validated tickets are merged into the base branch, locally, never pushed.",
        ),
    ]
    .into_iter()
    .map(|(id, label, detail)| RadioChoice {
        id: id.as_str().to_owned(),
        label: label.to_owned(),
        detail: detail.to_owned(),
    })
    .collect()
}

fn auto_merge_label(value: AutoMerge) -> &'static str {
    match value {
        AutoMerge::Off => "Off",
        AutoMerge::EpicChildren => "Epic children only",
        AutoMerge::All => "All tickets",
    }
}

/// `own` is the project's value (`None`: the global one, `global`, applies); `note` disables the choices.
fn auto_merge_radio(own: Option<AutoMerge>, global: AutoMerge, note: Option<String>) -> Control {
    Control::Radio {
        choices: auto_merge_choices(),
        value: own.map(|v| v.as_str().to_owned()),
        disabled: note.is_some(),
        empty: own
            .is_none()
            .then(|| format!("{USE_GLOBAL} ({})", auto_merge_label(global))),
        note,
    }
}

/// What the Workflow skills rows are drawn from besides the settings (`workflow_skills.status`, WSK-01).
#[derive(Clone, Copy)]
pub struct SkillInputs<'a> {
    /// The page's statuses in step order; empty when they couldn't be read (the rows then follow the settings).
    pub statuses: &'a [SkillStatus],
    /// On a project page, the global page's statuses: what "Use global" follows (empty: from the settings).
    pub global: &'a [SkillStatus],
    /// The last import or Re-import the service refused, written under its step (WSK-02).
    pub refused: Option<&'a SkillRefusal>,
    pub os: Os,
    /// For "imported today, 10:42".
    pub now: i64,
}

impl SkillInputs<'_> {
    /// No statuses read: the rows follow the settings.
    pub fn none(os: Os) -> SkillInputs<'static> {
        SkillInputs {
            statuses: &[],
            global: &[],
            refused: None,
            os,
            now: 0,
        }
    }
}

/// An import or Re-import the service refused (`bad_skill_folder`, `original_not_found`…): nothing changed, and
/// the reason is the red line under the step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRefusal {
    pub step: WorkflowStep,
    /// The folder that was checked (the picked one, or the original on Re-import).
    pub folder: Option<String>,
    /// The service's text ("The folder has no SKILL.md").
    pub message: String,
}

/// The built-in as the rows name it: `otaq:commit`.
fn built_in_name(step: WorkflowStep) -> String {
    step.own_default().trim_start_matches('/').to_owned()
}

/// A setting as a status, when the statuses couldn't be read: an imported skill counts as present and its
/// original as there.
fn status_of(step: WorkflowStep, setting: &SkillSetting) -> SkillStatus {
    let mut status = SkillStatus {
        step,
        kind: SkillKind::BuiltIn,
        name: None,
        original: None,
        original_exists: false,
        imported_at: None,
        copy: None,
        copy_ok: false,
        copy_problem: None,
        edited: false,
        warnings: Vec::new(),
    };
    match setting {
        SkillSetting::BuiltIn => {}
        SkillSetting::UseGlobal => status.kind = SkillKind::UseGlobal,
        SkillSetting::Legacy(name) => {
            status.kind = SkillKind::Legacy;
            status.name = Some(name.clone());
        }
        SkillSetting::Imported(skill) => {
            status.kind = SkillKind::Imported;
            status.name = Some(skill.name.clone());
            status.original = Some(skill.original.clone());
            status.original_exists = true;
            status.imported_at = Some(skill.imported_at);
            status.copy_ok = true;
            status.warnings.clone_from(&skill.warnings);
        }
    }
    status
}

/// Each step's status: the one read, else the setting's.
fn statuses_of(
    read: &[SkillStatus],
    settings: impl Fn(WorkflowStep) -> SkillStatus,
) -> Vec<SkillStatus> {
    WorkflowStep::ALL
        .into_iter()
        .map(|step| {
            read.iter()
                .find(|s| s.step == step)
                .cloned()
                .unwrap_or_else(|| settings(step))
        })
        .collect()
}

/// "today, 10:42" on the day it was imported, else "Oct 12".
fn imported_when(at: i64, now: i64) -> String {
    use crate::view_models::clock::{local_hhmm, month_day};
    use otaq_core::local_time::local;
    match (local(at), local(now)) {
        (Some(t), Some(n)) if (t.year, t.month, t.day) == (n.year, n.month, n.day) => {
            format!("today, {}", local_hhmm(at))
        }
        (Some(t), _) => month_day(&t),
        (None, _) => "at an unknown time".to_owned(),
    }
}

/// The red line of a refused import, in the frames' words (*Checks when importing (§2)*): "Not imported: ~/x has
/// no SKILL.md.", "Not imported: the front matter at the top of ~/x/SKILL.md can't be read.", else the service's
/// reason after "Not imported: ".
pub fn refusal_line(refusal: &SkillRefusal) -> String {
    let folder = refusal.folder.as_deref().map(tilde);
    let message = refusal.message.trim_end_matches('.');
    match folder {
        Some(folder) if message == "The folder has no SKILL.md" => {
            format!("Not imported: {folder} has no SKILL.md.")
        }
        Some(folder) if message.starts_with("SKILL.md's front matter can't be read") => {
            format!("Not imported: the front matter at the top of {folder}/SKILL.md can't be read.")
        }
        _ if message.ends_with(':') => format!("Not imported: {message}"),
        _ => format!("Not imported: {message}."),
    }
}

/// "Show Otaq's copy in Finder", as each OS calls its file manager (*The ⋯ menu on an imported skill*).
fn show_copy_label(os: Os) -> String {
    let manager = match os {
        Os::Mac => "Finder",
        Os::Windows => "Explorer",
        Os::Linux => "the file manager",
    };
    format!("Show {PRODUCT_NAME}\u{2019}s copy in {manager}")
}

/// The dot-folder hint under the list, macOS only (the macOS picker hides dot folders).
pub fn dot_folder_hint(os: Os) -> Option<String> {
    (os == Os::Mac).then(|| {
        "Skill in a dot folder such as .claude? The macOS folder picker hides them: press \u{2318}\u{21e7}. in the \
         picker to show them."
            .to_owned()
    })
}

/// The project page's Workflow skills intro (*Project · shop-web overrides Open PR*).
const PROJECT_SKILLS_INTRO: &str = "Settings for this project only. A step on \u{201c}Use global\u{201d} follows Settings \u{2192} Workflow skills.";

const IMPORT_FOLDER: &str = "Import skill folder\u{2026}";
const IMPORT_DIFFERENT: &str = "Import a different folder\u{2026}";

/// The Workflow skills control: one row per step. `global` is `Some` on a project page: the global page's
/// statuses, which a step on "Use global" follows.
fn skills_control(
    heading: &str,
    statuses: &[SkillStatus],
    global: Option<&[SkillStatus]>,
    input: &SkillInputs,
) -> Control {
    Control::Skills {
        heading: heading.to_owned(),
        rows: statuses
            .iter()
            .map(|status| {
                let follows = global.and_then(|g| g.iter().find(|s| s.step == status.step));
                skill_row(status, global.is_some(), follows, input)
            })
            .collect(),
        hint: dot_folder_hint(input.os),
    }
}

/// The skill a step on "Use global" runs, as the project's row names it: the global's imported name, else the
/// built-in (a typed global name uses the built-in, WSK-07).
fn global_name(step: WorkflowStep, global: Option<&SkillStatus>) -> (String, Option<String>) {
    match global {
        Some(g) if g.kind == SkillKind::Imported => (
            g.name.clone().unwrap_or_default(),
            g.original.as_deref().map(tilde),
        ),
        _ => (built_in_name(step), None),
    }
}

/// One step's row (*15 → update AC §1–§3*).
fn skill_row(
    status: &SkillStatus,
    project: bool,
    global: Option<&SkillStatus>,
    input: &SkillInputs,
) -> SkillRow {
    let step = status.step;
    let label = step_label(step);
    let built_in = built_in_name(step);
    let tag = |label: &str, look| SkillTag {
        label: label.to_owned(),
        look,
    };
    let button = |label: &str, icon, action, disabled| SkillButton {
        label: label.to_owned(),
        icon: Some(icon),
        action,
        disabled,
    };
    let item = |action, label: String, icon, separated| SkillMenuItem {
        action,
        label,
        icon,
        separated,
    };
    let line = |icon, text: String, look| SkillLine {
        icon,
        text,
        look,
        link: None,
    };
    let back = if project {
        "Use global".to_owned()
    } else {
        format!("Use built-in ({built_in})")
    };
    let mut lines = Vec::new();
    if let Some(refused) = input.refused.filter(|r| r.step == step) {
        lines.push(line(
            IconName::CircleAlert,
            refusal_line(refused),
            SkillLook::Error,
        ));
    }
    let import = button(
        IMPORT_FOLDER,
        IconName::FolderInput,
        SkillAction::Import,
        false,
    );
    let mut row = SkillRow {
        field: format!("workflow_skills.{}", step.field()),
        step,
        label: label.to_owned(),
        name: built_in.clone(),
        name_muted: false,
        tag: tag("Built-in", SkillLook::Quiet),
        folder: None,
        source: format!("{PRODUCT_NAME}\u{2019}s default"),
        lines: Vec::new(),
        button: Some(import.clone()),
        menu: Vec::new(),
        menu_label: format!("More for {label}"),
        picker_title: IMPORT_FOLDER.trim_end_matches('\u{2026}').to_owned(),
        reimport: None,
        reset: None,
    };
    match status.kind {
        // Q211: a project step stored as Otaq's own reads as the built-in; Use global takes it back to the global.
        SkillKind::BuiltIn => {
            if project {
                row.menu = vec![item(SkillAction::Reset, back, IconName::RotateCcw, false)];
            }
        }
        SkillKind::UseGlobal => {
            let (name, original) = global_name(step, global);
            row.name = name;
            row.name_muted = true;
            row.tag = tag("Use global", SkillLook::Quiet);
            row.source = match original {
                Some(original) => format!("Global: imported from {original}"),
                None => format!("Global: {PRODUCT_NAME}\u{2019}s default"),
            };
        }
        SkillKind::Legacy => {
            row.name = status.name.clone().unwrap_or_default();
            row.name_muted = true;
            row.tag = tag("Not imported", SkillLook::Warning);
            row.source = "Typed name from an earlier version".into();
            let until = if project {
                let (name, _) = global_name(step, global);
                format!(
                    "Import its folder, or pick Use global. Until then {label} uses the global skill ({name})."
                )
            } else {
                format!(
                    "Import its folder to use it. Until then {label} uses the built-in {built_in}."
                )
            };
            lines.push(line(IconName::Info, until, SkillLook::Quiet));
            if project {
                row.menu = vec![item(SkillAction::Reset, back, IconName::RotateCcw, false)];
            }
        }
        SkillKind::Imported => {
            let name = status.name.clone().unwrap_or_default();
            let original = status.original.as_deref().map(tilde).unwrap_or_default();
            let copy = status
                .copy
                .as_deref()
                .map(|c| tilde(&c.display().to_string()))
                .unwrap_or_default();
            row.name.clone_from(&name);
            row.folder = Some(original.clone());
            row.source = match status.imported_at {
                Some(at) => format!("\u{b7} imported {}", imported_when(at, input.now)),
                None => String::new(),
            };
            row.tag = tag("Imported", SkillLook::Quiet);
            let copy_note = line(
                IconName::Info,
                format!(
                    "{PRODUCT_NAME} uses its own copy. Changes to the original apply after Re-import."
                ),
                SkillLook::Quiet,
            );
            if status.copy_ok {
                lines.extend(status.warnings.iter().map(|w| {
                    line(
                        IconName::TriangleAlert,
                        format!("Imported, but {}.", w.trim_end_matches('.')),
                        SkillLook::Warning,
                    )
                }));
                if !status.original_exists {
                    lines.push(line(
                        IconName::FolderX,
                        format!(
                            "Original not found at {original}, so Re-import is unavailable. {PRODUCT_NAME}\u{2019}s \
                             copy keeps working; {IMPORT_DIFFERENT} in the \u{22ef} menu points it at the new place."
                        ),
                        SkillLook::Quiet,
                    ));
                }
                lines.push(copy_note);
                row.button = Some(button(
                    "Re-import",
                    IconName::RefreshCw,
                    SkillAction::Reimport,
                    !status.original_exists,
                ));
            } else {
                // WSK-04: the copy is missing or broken, so the step runs the built-in until Import again works.
                row.tag = tag("Skill missing", SkillLook::Error);
                let again = if status.original_exists {
                    SkillAction::ImportAgain
                } else {
                    SkillAction::Import
                };
                lines.push(SkillLine {
                    link: Some(SkillButton {
                        label: "Import again".into(),
                        icon: None,
                        action: again,
                        disabled: false,
                    }),
                    ..line(
                        IconName::CircleAlert,
                        "Skill missing \u{b7}".into(),
                        SkillLook::Error,
                    )
                });
                lines.push(line(
                    IconName::Info,
                    format!(
                        "{PRODUCT_NAME}\u{2019}s copy in {copy} is missing or broken. Agents use the built-in \
                         {built_in} meanwhile."
                    ),
                    SkillLook::Quiet,
                ));
                row.button = None;
            }
            // Show Otaq's copy only while there is a folder to show (a broken copy, not a missing one).
            let copy_there = status.copy.is_some()
                && status.copy_problem.as_deref() != Some(copy_missing().as_str());
            row.menu = [
                copy_there.then(|| {
                    item(
                        SkillAction::Reveal,
                        show_copy_label(input.os),
                        IconName::FolderOpen,
                        false,
                    )
                }),
                Some(item(
                    SkillAction::Import,
                    IMPORT_DIFFERENT.to_owned(),
                    IconName::FolderInput,
                    false,
                )),
                Some(item(SkillAction::Reset, back, IconName::RotateCcw, true)),
            ]
            .into_iter()
            .flatten()
            .collect();
            row.reimport = Some(SkillDialog {
                title: format!("Re-import {name}?"),
                text: format!(
                    "{PRODUCT_NAME} replaces its copy with the folder at {original}. Sessions already running keep \
                     the copy they started with; the next launch uses the new one."
                ),
                note: format!("Edits made to {PRODUCT_NAME}\u{2019}s copy in {copy} are lost."),
                confirm: "Re-import".into(),
                destructive: false,
            });
            row.reset = Some(if project {
                let (global, _) = global_name(step, global);
                SkillDialog {
                    title: format!("Use the global {global}?"),
                    text: format!(
                        "{label} goes back to the global skill and {PRODUCT_NAME}\u{2019}s copy of {name} is \
                         deleted. The original at {original} isn\u{2019}t touched."
                    ),
                    note: format!(
                        "{PRODUCT_NAME}\u{2019}s copy has edits that aren\u{2019}t in the original. Going back to \
                         the global skill deletes them."
                    ),
                    confirm: "Use global".into(),
                    destructive: true,
                }
            } else {
                SkillDialog {
                    title: format!("Use the built-in {built_in}?"),
                    text: format!(
                        "{label} goes back to {PRODUCT_NAME}\u{2019}s built-in skill and {PRODUCT_NAME}\u{2019}s \
                         copy of {name} is deleted. The original at {original} isn\u{2019}t touched."
                    ),
                    note: format!(
                        "{PRODUCT_NAME}\u{2019}s copy has edits that aren\u{2019}t in the original. Going back to \
                         the built-in deletes them."
                    ),
                    confirm: "Use built-in".into(),
                    destructive: true,
                }
            });
        }
    }
    // A refused import on a row with an imported skill: its red line first, the skill's own lines after.
    row.lines = lines;
    row
}

pub fn project_page(input: &ProjectInputs) -> Page {
    let (info, s, p) = (input.info, input.settings, input.problems);
    let key = &info.key;
    let general = section(
        ProjectSection::General,
        None,
        vec![
            row(
                "name",
                "Name",
                "",
                text(&info.name, "Project name", false, None),
            ),
            row(
                "",
                "Project key",
                format!(
                    "Used in ticket IDs like {key}-18. Set when the project was added; it can’t be changed."
                ),
                Control::Value {
                    text: key.clone(),
                    mono: true,
                    boxed: true,
                },
            ),
            row(
                "",
                "Repository",
                "",
                Control::Value {
                    text: tilde(&info.repo_path),
                    mono: true,
                    boxed: false,
                },
            ),
            row(
                "base_branch",
                "Base branch",
                "Prefilled from the remote’s default branch (origin/HEAD); editable. Tickets and epics branch from here, and Local merge squashes into it.",
                Control::Branch {
                    value: info.base_branch.clone(),
                    search: MenuSearch::branches(),
                },
            ),
        ],
    );
    let instructions = section(
        ProjectSection::AgentInstructions,
        None,
        vec![row(
            "agent_instructions",
            "Agent instructions",
            "Per-project override, given to every agent in this project. Empty = the global default is used.",
            multiline(
                &s.agent_instructions,
                &instructions_placeholder(input.global),
            ),
        )],
    );
    let skill_statuses = statuses_of(input.skills.statuses, |step| {
        status_of(step, s.workflow_skills.get(step))
    });
    let global_statuses = statuses_of(input.skills.global, |step| {
        status_of(step, input.global.workflow_skills.get(step))
    });
    let skills = Section {
        intro: Some(PROJECT_SKILLS_INTRO.to_owned()),
        ..section(
            ProjectSection::WorkflowSkills,
            None,
            vec![row(
                "",
                "Workflow skills",
                "",
                skills_control(
                    "Workflow skills",
                    &skill_statuses,
                    Some(&global_statuses),
                    &input.skills,
                ),
            )],
        )
    };
    let project_agents = section(
        ProjectSection::ProjectAgentInstructions,
        None,
        vec![row(
            "project_agent_instructions",
            "Project agent instructions",
            "Per-project override for Project agents. Empty = the global default is used.",
            multiline(&s.project_agent_instructions, USE_GLOBAL),
        )],
    );
    let g = input.global;
    let validating = s.validation.unwrap_or(g.validation);
    let reset = |set: bool| set.then(|| USE_GLOBAL.to_owned());
    let validation = section(
        ProjectSection::Validation,
        None,
        vec![
            Row {
                reset: reset(s.validation.is_some()),
                ..row(
                    "validation",
                    "Validation",
                    if validating {
                        "A separate read-only validator checks the agent\u{2019}s work before Review; failures go back to the agent."
                    } else {
                        "Off for this project: review submit moves the ticket straight to Ready for Review, as before validation existed."
                    },
                    Control::Toggle { on: validating },
                )
            },
            Row {
                reset: reset(s.validation_rounds.is_some()),
                ..row(
                    "validation_rounds",
                    "Validation rounds",
                    format!("{ROUNDS_DESCRIPTION} Empty = the global default."),
                    rounds_stepper(
                        s.validation_rounds.unwrap_or(g.validation_rounds),
                        s.validation_rounds
                            .is_none()
                            .then(|| format!("{USE_GLOBAL} ({})", g.validation_rounds)),
                    ),
                )
            },
            Row {
                reset: reset(s.auto_merge.is_some()),
                ..row(
                    "auto_merge",
                    "Auto-merge",
                    format!("{AUTO_MERGE_DESCRIPTION} Empty = the global default."),
                    auto_merge_radio(
                        s.auto_merge,
                        g.auto_merge,
                        (!validating).then(|| {
                            format!(
                                "Validation is off for {}, so Auto-merge does nothing. Turn Validation on to use it.",
                                info.name
                            )
                        }),
                    ),
                )
            },
        ],
    );
    let answers = section(
        ProjectSection::Answers,
        None,
        vec![row(
            "let_finished_tickets_answer",
            "Let finished tickets answer questions",
            format!(
                "When a linked agent asks a Done ticket something, {PRODUCT_NAME} resumes that ticket\u{2019}s session answer-only (it changes nothing). Off: those questions go to your inbox."
            ),
            Control::Toggle {
                on: s.let_finished_tickets_answer,
            },
        )],
    );
    let commands = section(
        ProjectSection::Commands,
        None,
        vec![
            row(
                "setup_command",
                "Setup command",
                "Runs once in every new worktree, before the agent starts.",
                text(
                    &s.setup_command,
                    "none — e.g. pnpm install",
                    true,
                    problem_for(p, "setup_command"),
                ),
            ),
            Row {
                button: input.tool_connected.then(|| RowButton {
                    label: crate::view_models::run_page::ASK_LABEL.into(),
                    icon: IconName::Sparkles,
                    tooltip: Some(crate::view_models::run_page::ask_tooltip(
                        input.project_agent,
                    )),
                }),
                ..row(
                    "run_command",
                    "Run command",
                    "Runs in the ticket’s worktree when you press Run. Use $PORT if it listens on a port.",
                    text(
                        &s.run_command,
                        "not set",
                        true,
                        problem_for(p, "run_command"),
                    ),
                )
            },
            row(
                "open_url",
                "Open URL",
                "Optional. Shows an Open button while it runs. Leave empty for backends, CLIs and libraries.",
                text(&s.open_url, "none", true, problem_for(p, "open_url")),
            ),
            row(
                "env_files",
                "Env files to copy into worktrees",
                "Copied from the repo root into each new worktree. Never committed.",
                Control::EnvFiles {
                    rows: s
                        .env_files
                        .iter()
                        .map(|f| EnvFileRow {
                            value: f.clone(),
                            problem: env_file_problem(p, f),
                        })
                        .collect(),
                    add_placeholder: "Add file".into(),
                    remove_label: "Remove file".into(),
                },
            ),
            env_vars_row(s, p),
            warm_builds_row(info, s, input.warm),
        ],
    );
    let resources = ticket_resources_section(s, p);
    let provider = section(
        ProjectSection::PrProvider,
        None,
        vec![row("", "PR provider", "", Control::Provider)],
    );
    let remove = section(
        ProjectSection::RemoveProject,
        None,
        vec![row(
            "",
            "Remove project",
            format!(
                "Deletes this project’s tickets and history from {PRODUCT_NAME}. Never touches the repository."
            ),
            Control::Button {
                label: "Remove project…".into(),
                icon: IconName::Trash2,
                destructive: true,
                action: ButtonAction::RemoveProject,
            },
        )],
    );
    Page::Project {
        key: key.clone(),
        title: info.name.clone(),
        intro: "Zero config works: agents can start with just the repo. Fill these in when you need them."
            .into(),
        sections: vec![
            general,
            // Its own view, collapsed or expanded ([`project_agents_block`]).
            section(ProjectSection::Agents, None, Vec::new()),
            instructions,
            project_agents,
            skills,
            validation,
            answers,
            commands,
            resources,
            provider,
            remove,
        ],
    }
}

/// The project's Agents block (AGT-03, d0086; *15 → Project settings → Agents* `mQijC` / `w6fVf`): collapsed "Uses
/// app defaults · Change" while it overrides nothing; expanded, the four roles, each a picker with "Use app default
/// (<name>)" first, then the named agents with their model as Settings shows it, and with several tools connected
/// their tool too ("Strong · Claude Code · Opus · always the newest", with its avatar). A project with an override
/// opens expanded; Change expands it (the page keeps that).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ProjectAgentsBlock")]
pub struct ProjectAgentsBlock {
    pub project: String,
    /// "Agents for this project", beside the collapsed line.
    pub collapsed_title: String,
    /// "Uses app defaults".
    pub collapsed_text: String,
    pub change_label: String,
    /// The heading over the expanded rows: "Agents".
    pub title: String,
    /// Opens expanded: the project overrides at least one role.
    pub expanded: bool,
    pub rows: Vec<ProjectAgentRow>,
}

/// One role of the block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ProjectAgentRow")]
pub struct ProjectAgentRow {
    pub role: named_agents::RoleId,
    pub label: String,
    pub picker: named_agents::AgentPicker,
}

/// The block for `project` from `agents.list`, the project's overrides (`project.agent_defaults`), the agent tools
/// and their cached model lists.
pub fn project_agents_block(
    project: &str,
    agents: &[NamedAgent],
    app: &RoleDefaults,
    own: &RoleDefaults,
    tools: &named_agents::AgentTools,
) -> ProjectAgentsBlock {
    let rows: Vec<ProjectAgentRow> = named_agents::ROLES
        .iter()
        .map(|&role| ProjectAgentRow {
            role: role.into(),
            label: role.label().to_owned(),
            picker: named_agents::project_picker(agents, app, own, role, tools),
        })
        .collect();
    ProjectAgentsBlock {
        project: project.to_owned(),
        collapsed_title: "Agents for this project".to_owned(),
        collapsed_text: "Uses app defaults".to_owned(),
        change_label: "Change".to_owned(),
        title: "Agents".to_owned(),
        expanded: rows.iter().any(|r| r.picker.value.agent.is_some()),
        rows,
    }
}

/// The Ticket resources section (M19-d, M19-e; d0087): the commands that make, remove and refresh a ticket's own
/// resources, and the schema paths. All empty by default, and then nothing changes.
pub fn ticket_resources_section(
    s: &ProjectSettings,
    p: &[SettingProblem],
) -> Section<ProjectSection> {
    let (db_name, db_template, resource_dir) = (
        naming::db_name_env_var(),
        naming::db_template_env_var(),
        naming::resource_dir_env_var(),
    );
    let schema_problems: Vec<&str> = p
        .iter()
        .filter(|problem| problem.field == "schema_paths")
        .map(|problem| problem.message.as_str())
        .collect();
    Section {
        intro: Some(format!(
            "For what each ticket needs of its own, like a database. With a Create command, {PRODUCT_NAME} sets \
             {db_name}, {db_template} and {resource_dir} in the ticket’s commands and sessions."
        )),
        ..section(
            ProjectSection::TicketResources,
            Some("Ticket resources"),
            vec![
                row(
                    "resource_create",
                    "Create command",
                    format!(
                        "Runs in each new worktree after the env files and before setup. Creates the ticket’s \
                         resources, e.g. a database named ${db_name} from ${db_template}."
                    ),
                    text(
                        &s.resource_create,
                        "none",
                        true,
                        problem_for(p, "resource_create"),
                    ),
                ),
                row(
                    "resource_remove",
                    "Remove command",
                    "Runs at Done and Close in the repository, for a resource Create made.",
                    text(
                        &s.resource_remove,
                        "none",
                        true,
                        problem_for(p, "resource_remove"),
                    ),
                ),
                row(
                    "resource_template_refresh",
                    "Template refresh command",
                    format!(
                        "Runs in the repository after Done, so new tickets start from the merged schema. \
                         Creates wait while it runs. ${db_template} is set."
                    ),
                    text(
                        &s.resource_template_refresh,
                        "none",
                        true,
                        problem_for(p, "resource_template_refresh"),
                    ),
                ),
                row(
                    "schema_paths",
                    "Schema paths",
                    "One path per line, like db/migrations/**. Two tickets in Review adding files under the same \
                     path get a warning.",
                    Control::Text {
                        value: s.schema_paths.join("\n"),
                        placeholder: "none".into(),
                        mono: true,
                        multiline: true,
                        problem: (!schema_problems.is_empty()).then(|| schema_problems.join("; ")),
                    },
                ),
            ],
        )
    }
}

/// The × of an environment variable row without a name.
const REMOVE_VARIABLE: &str = "Remove variable";

/// The Environment variables row's description: where the rows are set, and that a value can name Otaq's own
/// variables as `${…}` (d0087; [`otaq_core::session::env::substitute`]).
fn env_vars_description() -> String {
    format!(
        "Set in every agent session, Run and Shell of this project. You can use ${{{}}} and {PRODUCT_NAME}’s other variables inside a value.",
        naming::db_name_env_var()
    )
}

/// The Environment variables row (ENV-01, ENV-03; d0082, d0087; *15 → Row · Environment variables (0082)*): every row as
/// written, in full and in plain text, each with the problem the service found (`env_vars.<index>`). There are no
/// built-in, read-only or computed rows; the hint is only an example.
pub fn env_vars_row(s: &ProjectSettings, problems: &[SettingProblem]) -> Row {
    row(
        "env_vars",
        "Environment variables",
        env_vars_description(),
        Control::EnvVars {
            rows: s
                .env_vars
                .iter()
                .enumerate()
                .map(|(i, v)| EnvVarRow {
                    name: v.name.clone(),
                    value: v.value.clone(),
                    problem: problem_for(problems, &env_var_field(i)),
                    remove_label: if v.name.is_empty() {
                        REMOVE_VARIABLE.into()
                    } else {
                        format!("Remove {}", v.name)
                    },
                })
                .collect(),
            hint_example: "CI = true".into(),
            hint: "e.g. so tools never wait for input or start watch mode".into(),
            empty: "No variables. Sessions get your normal environment.".into(),
            add_label: "Add variable".into(),
            drop_label: REMOVE_VARIABLE.into(),
            applies: "Changes apply to sessions, Runs and Shells started after this.".into(),
            name_placeholder: "NAME".into(),
            value_placeholder: "value".into(),
        },
    )
}

/// The warm build switch (WRM-05, WRM-06; d0082; *15 → Row · Start new worktrees from a warm build (0082)*), on by
/// default, on every disk (M19-b, d0085). Where the data folder's disk can't clone, the description says the build is
/// handed over (renamed) to the next ticket instead. Turning it off asks only when it would delete the project's warm
/// build; while that Clear waits for a seed, the row says so. It names no build tool.
pub fn warm_builds_row(info: &ProjectInfo, s: &ProjectSettings, warm: Option<WarmInputs>) -> Row {
    let build = warm.and_then(|w| {
        w.list
            .builds
            .iter()
            .find(|b| b.project.as_deref() == Some(info.key.as_str()))
            .map(|b| (b, w.now))
    });
    let clearing = build
        .filter(|(b, _)| b.status.clearing)
        .map(|(b, _)| warm_build::waiting(&b.status.copying));
    let control = Control::WarmBuild {
        on: s.warm_builds,
        disabled: false,
        reason: None,
        confirm: build
            .filter(|_| s.warm_builds && clearing.is_none())
            .map(|(b, now)| turn_off_confirm(&info.name, b, now)),
        clearing,
    };
    let description = match warm {
        Some(w) if !w.list.support.available => HANDED_OVER,
        _ => {
            "New tickets start from the last finished ticket’s build, so their first build only redoes what changed."
        }
    };
    row(
        "warm_builds",
        "Start new worktrees from a warm build",
        description,
        control,
    )
}

/// The warm build switch's description where the disk can't clone (plan §4.3): builds are renamed, not cloned.
pub const HANDED_OVER: &str =
    "This disk can’t copy-on-write; the build is handed over to the next ticket.";

/// *Warm build · turning off · confirm (0082)*: the same shape as Cleanup's Clear… confirmation.
fn turn_off_confirm(name: &str, b: &WarmBuildInfo, now: i64) -> WarmConfirmPage {
    let built = match (&b.built_at, b.kept_at) {
        (Some(sha), Some(at)) => format!(", built at {sha}, {}", warm_build::ago(now - at)),
        (Some(sha), None) => format!(", built at {sha}"),
        _ => String::new(),
    };
    let up_to = warm_build::up_to(b.size_bytes);
    WarmConfirmPage {
        title: warm_build::confirm_title(name, b.size_bytes),
        text: format!(
            "Turning this off deletes {name}’s warm build{built}. New tickets will run setup and build from \
             nothing. Ticket worktrees and your repo aren’t touched."
        ),
        shared: warm_build::SHARED_SPACE,
        line: ConfirmLine {
            icon: IconName::Flame,
            text: format!("{} · {up_to} (delete)", tilde(&b.path)),
        },
        confirm: warm_build::turn_off_label(b.size_bytes),
    }
}

/// One state of the warm build switch in the gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "WarmSwitchGalleryCell")]
pub struct WarmSwitchCell {
    pub name: String,
    pub project: String,
    pub row: Row,
}

/// The notification banners and Settings → Notifications in the component gallery (T-0372; ATT-13, ATT-14, ATT-16;
/// no frames, d0090): the ask, the blocked banner and their stacking with the folder trust banners, the permission
/// warning, the usage limit and Reconnecting; the section in every status-line state, and with the switch off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "NotificationsGallery")]
pub struct NotificationsGallery {
    pub banners: Vec<NotificationsBannerCell>,
    pub pages: Vec<NotificationsPageCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "NotificationsBannerCell")]
pub struct NotificationsBannerCell {
    pub name: String,
    pub banners: Vec<crate::view_models::banners::BannerView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "NotificationsPageCell")]
pub struct NotificationsPageCell {
    pub name: String,
    pub section: Section<GlobalSection>,
}

pub fn notifications_gallery() -> NotificationsGallery {
    use crate::view_models::banners::{BannerModel, Situation, SituationKind, TrustFailure};
    fn limit() -> Situation {
        Situation::UsageLimit {
            title: "Session limit reached".into(),
            text: "Resets at 3:00 pm \u{b7} 8 agents stopped".into(),
            resume: None,
            lines: Vec::new(),
        }
    }
    fn failed() -> Vec<TrustFailure> {
        vec![TrustFailure {
            project: "NOTE".into(),
            reason: "Claude Code\u{2019}s config file couldn\u{2019}t be written".into(),
        }]
    }
    type Raise = fn(&mut BannerModel);
    let banner_cells: [(&str, Raise); 6] = [
        ("the ask · the first agent started, macOS not asked", |m| {
            m.set_notifications(true, false)
        }),
        ("macOS blocked a notification", |m| {
            m.set_notifications(false, true)
        }),
        (
            "stacking · blocked below the folder trust warning, above the permission warning",
            |m| {
                m.raise(Situation::PermissionWarning);
                m.set_trust(false, Vec::new(), failed());
                m.set_notifications(false, true);
            },
        ),
        ("stacking · blocked above the permission warning", |m| {
            m.raise(Situation::PermissionWarning);
            m.set_notifications(true, true);
        }),
        ("stacking · the usage limit on top, reconnecting", |m| {
            m.raise(limit());
            m.raise(Situation::Reconnecting);
            m.set_notifications(true, true);
        }),
        (
            "stacking · the ask below the folder trust note, reconnecting",
            |m| {
                m.set_trust(true, Vec::new(), Vec::new());
                m.set_notifications(true, false);
                m.raise(Situation::Reconnecting);
            },
        ),
    ];
    let banners = banner_cells
        .into_iter()
        .map(|(name, set)| {
            let mut model = BannerModel::default();
            set(&mut model);
            debug_assert!(
                model.is_active(SituationKind::NotificationsAsk)
                    || model.is_active(SituationKind::NotificationsBlocked)
            );
            NotificationsBannerCell {
                name: name.to_owned(),
                banners: model.rows().iter().map(|r| r.view(false)).collect(),
            }
        })
        .collect();
    let mut off = GlobalSettings {
        notifications: false,
        ..GlobalSettings::default()
    };
    off.notification_kinds.set(Kind::TurnEnded, false);
    let mut some = GlobalSettings::default();
    some.notification_kinds.set(Kind::Question, false);
    some.notification_sound = false;
    let on = GlobalSettings::default();
    let page_cells = [
        ("macOS not asked yet", &on, PermissionState::NotAsked),
        (
            "allowed · Questions and Play a sound unticked",
            &some,
            PermissionState::Allowed,
        ),
        ("turned off in macOS", &on, PermissionState::TurnedOff),
        (
            "running outside the app bundle",
            &on,
            PermissionState::OutsideBundle,
        ),
        ("state unknown · no line", &on, PermissionState::Unknown),
        (
            "the switch off · turned off in macOS",
            &off,
            PermissionState::TurnedOff,
        ),
    ];
    let pages = page_cells
        .into_iter()
        .map(|(name, settings, permission)| {
            let input = GlobalInputs {
                settings,
                problems: &[],
                scheduler: None,
                data: None,
                cleanup_count: 0,
                os: Os::Mac,
                notification_permission: permission,
                skills: SkillInputs::none(Os::Mac),
            };
            let Page::Global { sections, .. } = global_page(&input) else {
                unreachable!("the global page")
            };
            NotificationsPageCell {
                name: name.to_owned(),
                section: sections
                    .into_iter()
                    .find(|s| s.id == GlobalSection::Notifications)
                    .expect("the Notifications section"),
            }
        })
        .collect();
    NotificationsGallery { banners, pages }
}

/// *15 → update AC* for the gallery (T-0380): Settings → Workflow skills, global and per project, in every state
/// the frames draw, with the frames' samples (`~/team-skills/my-commit`, shop-web's `.claude/skills/my-pr`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "WorkflowSkillsGallery")]
pub struct WorkflowSkillsGallery {
    pub cells: Vec<WorkflowSkillsCell>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "WorkflowSkillsCell")]
pub struct WorkflowSkillsCell {
    pub name: String,
    /// The section as its page draws it: the global page's, or a project's (`project`).
    pub section: Section<String>,
    pub project: Option<String>,
}

pub fn workflow_skills_gallery() -> WorkflowSkillsGallery {
    use otaq_core::naming::data_dir_name;
    let home = std::env::home_dir().unwrap_or_default();
    let at = |rest: &str| home.join(rest).display().to_string();
    let now = 1_760_000_000_000;
    let blank = |step| status_of(step, &SkillSetting::BuiltIn);
    let imported = |step, name: &str, original: &str, ago: i64| SkillStatus {
        kind: SkillKind::Imported,
        name: Some(name.to_owned()),
        original: Some(at(original)),
        original_exists: true,
        imported_at: Some(now - ago),
        copy: Some(home.join(format!("{}/skills/{name}", data_dir_name()))),
        copy_ok: true,
        ..blank(step)
    };
    let commit = || {
        imported(
            WorkflowStep::Commit,
            "my-commit",
            "team-skills/my-commit",
            0,
        )
    };
    let page = |statuses: Vec<SkillStatus>| -> Vec<SkillStatus> { statuses_of(&statuses, blank) };
    let global_section = |statuses: Vec<SkillStatus>, refused: Option<SkillRefusal>| {
        let statuses = page(statuses);
        let input = SkillInputs {
            statuses: &statuses,
            global: &[],
            refused: refused.as_ref(),
            os: Os::Mac,
            now,
        };
        let Page::Global { sections, .. } = global_page(&GlobalInputs {
            settings: &GlobalSettings::default(),
            problems: &[],
            scheduler: None,
            data: None,
            cleanup_count: 0,
            os: Os::Mac,
            notification_permission: PermissionState::Unknown,
            skills: input,
        }) else {
            unreachable!()
        };
        sections
            .into_iter()
            .find(|s| s.id == GlobalSection::WorkflowSkills)
            .map(|s| Section {
                id: "workflow_skills".to_owned(),
                title: s.title,
                intro: s.intro,
                rows: s.rows,
                note: s.note,
            })
            .expect("the Workflow skills section")
    };
    let project_section = |statuses: Vec<SkillStatus>, global: Vec<SkillStatus>| {
        let (statuses, global) = (page(statuses), page(global));
        let all_global = |step| status_of(step, &SkillSetting::UseGlobal);
        let statuses = statuses
            .into_iter()
            .map(|s| {
                if s.kind == SkillKind::BuiltIn {
                    all_global(s.step)
                } else {
                    s
                }
            })
            .collect::<Vec<_>>();
        let input = SkillInputs {
            statuses: &statuses,
            global: &global,
            refused: None,
            os: Os::Mac,
            now,
        };
        Section {
            id: "workflow_skills".to_owned(),
            title: None,
            intro: Some(PROJECT_SKILLS_INTRO.to_owned()),
            rows: vec![row(
                "",
                "Workflow skills",
                "",
                skills_control("Workflow skills", &statuses, Some(&global), &input),
            )],
            note: None,
        }
    };
    let refused = |message: &str, folder: &str| SkillRefusal {
        step: WorkflowStep::Commit,
        folder: Some(at(folder)),
        message: message.to_owned(),
    };
    let cell = |name: &str, section, project: Option<&str>| WorkflowSkillsCell {
        name: name.to_owned(),
        section,
        project: project.map(str::to_owned),
    };
    WorkflowSkillsGallery {
        cells: vec![
            cell(
                "Global · built-in (fresh install)",
                global_section(Vec::new(), None),
                None,
            ),
            cell(
                "Global · Commit imported",
                global_section(vec![commit()], None),
                None,
            ),
            cell(
                "Project · shop-web overrides Open PR",
                project_section(
                    vec![imported(
                        WorkflowStep::OpenPr,
                        "my-pr",
                        "code/shop-web/.claude/skills/my-pr",
                        9 * 60_000,
                    )],
                    vec![commit()],
                ),
                Some("SHOP"),
            ),
            cell(
                "§2 A · the folder has no SKILL.md",
                global_section(
                    Vec::new(),
                    Some(refused(
                        "The folder has no SKILL.md",
                        "team-skills/commit-helpers",
                    )),
                ),
                None,
            ),
            cell(
                "§2 B · the front matter can't be read",
                global_section(
                    Vec::new(),
                    Some(refused(
                        "SKILL.md's front matter can't be read: line 3 isn't key: value",
                        "team-skills/my-commit",
                    )),
                ),
                None,
            ),
            cell(
                "§2 C · files it mentions are missing (warning, imported)",
                global_section(
                    vec![SkillStatus {
                        warnings: vec![
                            "SKILL.md mentions scripts/check-staged.sh, which isn't in the folder"
                                .into(),
                        ],
                        ..commit()
                    }],
                    None,
                ),
                None,
            ),
            cell(
                "§3 A · original moved or deleted",
                global_section(
                    vec![SkillStatus {
                        original_exists: false,
                        ..commit()
                    }],
                    None,
                ),
                None,
            ),
            cell(
                &format!("§3 B · {PRODUCT_NAME}'s copy missing or broken"),
                global_section(
                    vec![SkillStatus {
                        copy_ok: false,
                        copy_problem: Some(copy_missing()),
                        ..imported(
                            WorkflowStep::Commit,
                            "my-commit",
                            "team-skills/my-commit",
                            6 * 24 * 3_600_000,
                        )
                    }],
                    None,
                ),
                None,
            ),
            cell(
                "§3 C · a setting from before 0092 (a typed name)",
                global_section(
                    vec![status_of(
                        WorkflowStep::Commit,
                        &SkillSetting::Legacy("/my-commit".into()),
                    )],
                    None,
                ),
                None,
            ),
            cell(
                "§3 D · project: a typed override from before 0092",
                project_section(
                    vec![status_of(
                        WorkflowStep::OpenPr,
                        &SkillSetting::Legacy("/my-pr".into()),
                    )],
                    Vec::new(),
                ),
                Some("SHOP"),
            ),
        ],
    }
}

/// *15 → Row · Start new worktrees from a warm build (0082)* and the dark frame's Linux state for the gallery
/// (T-0301): the frames' samples (shop-web 4.2 GB at 3e1f9a2, 2 h; SHOP-21 copying).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "WarmSwitchGallery")]
pub struct WarmSwitchGallery {
    pub cells: Vec<WarmSwitchCell>,
}

pub fn warm_switch_gallery() -> WarmSwitchGallery {
    use otaq_core::naming::{COMMAND_NAME, data_dir_name};
    use otaq_core::warm::store::WarmStatus;
    let now = 1_760_000_000_000;
    let support = |available: bool, file_system: &str, volume: &str| CloneSupport {
        available,
        file_system: file_system.to_owned(),
        volume: volume.to_owned(),
    };
    let build = |status: WarmStatus| WarmBuildInfo {
        repo: "shop-web".into(),
        path: format!("~/{}/warm/shop-web", data_dir_name()),
        project: Some("SHOP".into()),
        size_bytes: 42 * (1u64 << 30) / 10,
        built_at: Some("3e1f9a2".into()),
        from: Some("SHOP-20".into()),
        kept_at: Some(now - 2 * 3_600_000),
        status,
        entries: vec![],
        limit: None,
    };
    let copying = WarmStatus {
        copying: vec!["SHOP-21".into()],
        clearing: true,
    };
    let apfs = || support(true, "apfs", "/");
    let cells = [
        (
            "on (the default) · no warm build yet",
            "shop-web",
            true,
            apfs(),
            None,
        ),
        (
            "on · with a warm build (turning it off asks)",
            "shop-web",
            true,
            apfs(),
            Some(build(WarmStatus::default())),
        ),
        (
            "off, right after the confirmation",
            "shop-web",
            false,
            apfs(),
            None,
        ),
        (
            "turning off while SHOP-21 is seeded",
            "shop-web",
            false,
            apfs(),
            Some(build(copying)),
        ),
        (
            "handed over · Windows NTFS",
            "shop-web",
            true,
            support(false, "NTFS", "C:"),
            None,
        ),
        (
            "handed over · Linux ext4",
            COMMAND_NAME,
            true,
            support(false, "ext4", "/home"),
            None,
        ),
        (
            "handed over, off · macOS exFAT disk",
            "shop-web",
            false,
            support(false, "exfat", "/Volumes/Untitled"),
            None,
        ),
    ];
    WarmSwitchGallery {
        cells: cells
            .into_iter()
            .map(|(name, project, on, support, build)| {
                let info = ProjectInfo {
                    key: "SHOP".into(),
                    name: project.to_owned(),
                    repo_path: String::new(),
                    base_branch: "main".into(),
                    agents_working: 0,
                    needs_you: 0,
                };
                let settings = ProjectSettings {
                    warm_builds: on,
                    ..ProjectSettings::default()
                };
                let list = WarmList {
                    mode: otaq_core::ticket_env::handoff::HandoffMode::of(&support),
                    support,
                    builds: build.into_iter().collect(),
                };
                WarmSwitchCell {
                    name: name.to_owned(),
                    project: project.to_owned(),
                    row: warm_builds_row(&info, &settings, Some(WarmInputs { list: &list, now })),
                }
            })
            .collect(),
    }
}

/// The problems the service would find in `rows` on `os` (ENV-03), as it sends them: one per marked row.
pub fn env_var_setting_problems(rows: &[EnvVar], os: otaq_core::tools::Os) -> Vec<SettingProblem> {
    env_var_problems(rows, os)
        .into_iter()
        .enumerate()
        .filter_map(|(i, message)| {
            message.map(|message| SettingProblem {
                field: env_var_field(i),
                message,
            })
        })
        .collect()
}

/// One state of the Environment variables row in the gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EnvVarsGalleryCell")]
pub struct EnvVarsCell {
    pub name: String,
    pub row: Row,
}

/// *15 → Row · Environment variables (0082)* and *… · edit, remove, errors (0082)* for the gallery (T-0300): the
/// frames' sample rows, with the problems as the service finds them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EnvVarsGallery")]
pub struct EnvVarsGallery {
    pub cells: Vec<EnvVarsCell>,
}

pub fn env_vars_gallery() -> EnvVarsGallery {
    use otaq_core::tools::Os as Platform;
    let shop = [
        EnvVar::new("NEXT_TELEMETRY_DISABLED", "1"),
        EnvVar::new("NODE_OPTIONS", "--max-old-space-size=4096"),
        EnvVar::new("DATABASE_URL", "postgres://localhost:5432/shop_dev"),
    ];
    let with = |extra: &[EnvVar]| [&shop[..], extra].concat();
    let cells = [
        ("empty (the default)", Vec::new(), Platform::Mac),
        ("a few rows", shop.to_vec(), Platform::Mac),
        (
            "invalid name",
            with(&[EnvVar::new("MAX=JOBS", "2")]),
            Platform::Mac,
        ),
        ("empty name", with(&[EnvVar::new("", "2")]), Platform::Mac),
        (
            "duplicate name",
            with(&[EnvVar::new("NODE_OPTIONS", "--inspect")]),
            Platform::Mac,
        ),
        (
            "differing case · Windows",
            with(&[EnvVar::new("node_options", "--inspect")]),
            Platform::Windows,
        ),
        (
            "differing case · macOS and Linux",
            with(&[EnvVar::new("node_options", "--inspect")]),
            Platform::Linux,
        ),
    ];
    EnvVarsGallery {
        cells: cells
            .into_iter()
            .map(|(name, rows, os)| {
                let settings = ProjectSettings {
                    env_vars: rows,
                    ..ProjectSettings::default()
                };
                let problems = env_var_setting_problems(&settings.env_vars, os);
                EnvVarsCell {
                    name: name.to_owned(),
                    row: env_vars_row(&settings, &problems),
                }
            })
            .collect(),
    }
}

/// An edit of the Environment variables row: `env_vars.add` (a new row; one without a name isn't saved),
/// `env_vars.remove` (no confirmation: nothing on disk is deleted), `env_vars.<index>.name` / `.value`. Names and
/// values are kept as typed: a space is allowed, and the service marks only what would break the start (ENV-03).
fn edit_env_var(
    s: &mut ProjectSettings,
    field: &str,
    value: &Value,
) -> Option<Result<ProjectChange, String>> {
    let rest = field.strip_prefix("env_vars.")?;
    let rows = &mut s.env_vars;
    let result = match rest {
        "add" => match value {
            Value::Var(v) if v.name.is_empty() => Ok(ProjectChange::Nothing),
            Value::Var(v) => {
                rows.push(EnvVar::new(&v.name, &v.value));
                Ok(ProjectChange::Settings)
            }
            other => Err(format!("expected a name and a value, got {other:?}")),
        },
        "remove" => value.number().map(|i| {
            if (i as usize) < rows.len() {
                rows.remove(i as usize);
            }
            ProjectChange::Settings
        }),
        _ => {
            let (index, part) = rest.split_once('.')?;
            let i: usize = index.parse().ok()?;
            let text = match value.text() {
                Ok(t) => t.to_owned(),
                Err(e) => return Some(Err(e)),
            };
            match (rows.get_mut(i), part) {
                (None, _) => Err(format!("no environment variable {i}")),
                (Some(row), "name") => {
                    row.name = text;
                    Ok(ProjectChange::Settings)
                }
                (Some(row), "value") => {
                    row.value = text;
                    Ok(ProjectChange::Settings)
                }
                _ => return None,
            }
        }
    };
    Some(result)
}

/// How the PR provider row's status looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "ProviderLook")]
pub enum ProviderLook {
    Ready,
    Attention,
    NotDetected,
}

/// The PR provider row (`settings.provider`): read-only, detected from origin (REV-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsProvider")]
pub struct ProviderView {
    pub description: String,
    pub status: String,
    pub icon: IconName,
    pub look: ProviderLook,
    /// "use your own Open PR skill": focuses the Open PR field.
    pub link: Option<String>,
}

pub fn provider(detected: Option<&PrProvider>) -> ProviderView {
    let row = provider_row(detected);
    let (icon, look) = match row.tone {
        ProviderTone::Ready => (IconName::CircleCheck, ProviderLook::Ready),
        ProviderTone::Attention => (IconName::TriangleAlert, ProviderLook::Attention),
        ProviderTone::NotDetected => (IconName::CircleQuestionMark, ProviderLook::NotDetected),
    };
    ProviderView {
        description: row.description,
        status: row.status,
        icon,
        look,
        link: row.link.map(str::to_owned),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "ToolLook")]
pub enum ToolLook {
    Found,
    MissingRequired,
    MissingOptional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsToolRow")]
pub struct ToolView {
    /// "git", "gh", "az".
    pub tool: String,
    pub name: String,
    pub icon: IconName,
    pub look: ToolLook,
    /// "required" / "optional".
    pub need: String,
    pub status: String,
    pub explanation: Option<String>,
    pub command: Option<String>,
    pub button: Option<ToolButton>,
    pub note: Option<String>,
}

/// The tools checklist (`settings.tools`), or why the check failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsTools")]
pub struct ToolsView {
    pub rows: Vec<ToolView>,
    pub error: Option<String>,
    pub copy: String,
    pub terminal: ToolTerminalTexts,
}

pub fn tools(result: Result<ToolsReport, String>) -> ToolsView {
    let mut model = OnboardingModel::new(true);
    model.start_check();
    let error = result.as_ref().err().cloned();
    model.finish_check(result);
    ToolsView {
        rows: model
            .rows()
            .into_iter()
            .map(|r| {
                let (icon, look) = match r.icon {
                    RowIcon::Found => (IconName::CircleCheck, ToolLook::Found),
                    RowIcon::MissingRequired => (IconName::CircleX, ToolLook::MissingRequired),
                    RowIcon::MissingOptional => (IconName::CircleMinus, ToolLook::MissingOptional),
                };
                ToolView {
                    tool: r.tool.command().to_owned(),
                    button: tool_button(&r),
                    note: r.note.clone(),
                    name: r.name.to_owned(),
                    icon,
                    look,
                    need: if r.required { "required" } else { "optional" }.to_owned(),
                    status: r.status,
                    explanation: r.explanation,
                    command: r.command,
                }
            })
            .collect(),
        error,
        copy: "Copy".to_owned(),
        terminal: tool_terminal_texts(),
    }
}

/// The Remove project confirmation (PRJ-04; *15 → Remove project · confirm*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "RemoveProjectDialog")]
pub struct RemoveView {
    pub title: String,
    pub deleted_heading: String,
    pub deleted: Vec<String>,
    pub kept_heading: String,
    pub kept: Vec<String>,
    pub agents: Option<String>,
    pub confirm: String,
    pub cancel: String,
}

pub fn remove_dialog(info: &ProjectInfo, tickets: usize) -> RemoveView {
    let t = remove_text(info, tickets);
    RemoveView {
        title: t.title,
        deleted_heading: format!("Deleted from {PRODUCT_NAME}"),
        deleted: t.deleted,
        kept_heading: "Never touched".into(),
        kept: t.kept,
        agents: t.agents,
        confirm: "Remove project".into(),
        cancel: "Cancel".into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ThemeCredit")]
pub struct CreditView {
    pub project: String,
    /// "→ Catppuccin Latte, Frappé, …".
    pub themes: String,
    pub copyright: String,
    pub license: String,
    pub source: String,
}

/// *15 → Theme credits · dialog* (UI-11, d0069).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ThemeCreditsDialog")]
pub struct CreditsView {
    pub title: String,
    pub description: String,
    pub credits: Vec<CreditView>,
    pub source_label: String,
    pub mit_title: String,
    pub mit_note: String,
    pub mit_text: Vec<String>,
    pub notices: String,
    pub copy_label: String,
    pub copied_label: String,
    pub done_label: String,
}

pub fn credits() -> CreditsView {
    CreditsView {
        title: "Theme credits".into(),
        description: theme_credits::description(),
        credits: theme_credits::CREDITS
            .iter()
            .map(|c| CreditView {
                project: c.project.to_owned(),
                themes: format!("→ {}", c.themes),
                copyright: c.copyright.to_owned(),
                license: c.license.to_owned(),
                source: c.source.to_owned(),
            })
            .collect(),
        source_label: "Source".into(),
        mit_title: "MIT License — full text".into(),
        mit_note: "Applies to each project above, with its own copyright line".into(),
        mit_text: theme_credits::MIT_TEXT
            .iter()
            .map(|p| (*p).to_owned())
            .collect(),
        notices: theme_credits::notices(),
        copy_label: "Copy notices".into(),
        copied_label: "Copied".into(),
        done_label: "Done".into(),
    }
}

/// Whether `url` is one of the credits' sources (`settings.open_source` opens nothing else).
pub fn is_credit_source(url: &str) -> bool {
    theme_credits::CREDITS.iter().any(|c| c.source == url)
}

/// An edited value as the page sends it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(untagged)]
#[ts(rename = "SettingsValue")]
pub enum Value {
    Flag(bool),
    Number(u32),
    Text(String),
    /// A new environment variable row (`env_vars.add`).
    Var(EnvVarValue),
}

/// A new environment variable as the page sends it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(rename = "SettingsEnvVarValue")]
pub struct EnvVarValue {
    pub name: String,
    pub value: String,
}

impl Value {
    fn text(&self) -> Result<&str, String> {
        match self {
            Self::Text(t) => Ok(t),
            other => Err(format!("expected a text, got {other:?}")),
        }
    }
    fn flag(&self) -> Result<bool, String> {
        match self {
            Self::Flag(b) => Ok(*b),
            other => Err(format!("expected on or off, got {other:?}")),
        }
    }
    fn number(&self) -> Result<u32, String> {
        match self {
            Self::Number(n) => Ok(*n),
            other => Err(format!("expected a number, got {other:?}")),
        }
    }
}

fn auto_merge_of(id: &str) -> Result<AutoMerge, String> {
    id.parse()
        .map_err(|_| format!("unknown Auto-merge choice: {id}"))
}

/// A workflow skill field can't be typed into (WSK-01): skills are imported folders.
fn typed_skill() -> String {
    "A workflow skill is imported as a folder; it can't be typed".to_owned()
}

fn step_of(field: &str) -> Option<WorkflowStep> {
    let name = field.strip_prefix("workflow_skills.")?;
    WorkflowStep::ALL.into_iter().find(|s| s.field() == name)
}

/// Applies one edit to the global settings. Steppers clamp to their range; the service flags the rest.
pub fn edit_global(s: &mut GlobalSettings, field: &str, value: &Value) -> Result<(), String> {
    let clamp = |n: u32, r: std::ops::RangeInclusive<u32>| n.clamp(*r.start(), *r.end());
    match field {
        "agent_limits.enabled" => s.agent_limits = toggled(&s.agent_limits, value.flag()?),
        "agent_limits.max_concurrency" => {
            s.agent_limits.max_concurrency = clamp(value.number()?, MAX_AGENTS)
        }
        "agent_limits.back_pressure" => {
            s.agent_limits.back_pressure = clamp(value.number()?, BACK_PRESSURE_ITEMS)
        }
        "agent_instructions" => s.agent_instructions = value.text()?.to_owned(),
        "validation" => s.validation = value.flag()?,
        "validation_rounds" => s.validation_rounds = clamp(value.number()?, VALIDATION_ROUNDS),
        "auto_merge" => s.auto_merge = auto_merge_of(value.text()?)?,
        "project_agent_instructions" => s.project_agent_instructions = value.text()?.to_owned(),
        "notifications" => s.notifications = value.flag()?,
        "notification_sound" => s.notification_sound = value.flag()?,
        kind if kind.starts_with("notification_kinds.") => {
            let kind = Kind::ALL
                .into_iter()
                .find(|k| notification_kind_row(*k).0 == kind)
                .ok_or_else(|| format!("unknown setting: {kind}"))?;
            s.notification_kinds.set(kind, value.flag()?);
        }
        "usage_data" => {
            s.usage_data = if value.flag()? {
                UsageData::On
            } else {
                UsageData::Off
            }
        }
        "terminal_font_size" => s.terminal_font_size = clamp(value.number()?, TERMINAL_FONT_SIZES),
        "terminal_renderer" => {
            s.terminal_renderer = match value.text()? {
                "webgl" => TerminalRenderer::Webgl,
                "dom" => TerminalRenderer::Dom,
                other => return Err(format!("unknown terminal renderer: {other}")),
            }
        }
        f => match step_of(f) {
            Some(_) => return Err(typed_skill()),
            None => return Err(format!("unknown setting: {f}")),
        },
    }
    Ok(())
}

/// What a project edit changes: its settings, or the project itself (name, base branch: `project.update`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectChange {
    Settings,
    Project(ProjectUpdate),
    /// An empty name: nothing to save.
    Nothing,
}

/// Applies one edit to a project's settings, or says which project update it is.
pub fn edit_project(
    s: &mut ProjectSettings,
    field: &str,
    value: &Value,
) -> Result<ProjectChange, String> {
    match field {
        "name" => {
            let name = value.text()?.trim();
            return Ok(if name.is_empty() {
                ProjectChange::Nothing
            } else {
                ProjectChange::Project(ProjectUpdate {
                    name: Some(name.to_owned()),
                    ..ProjectUpdate::default()
                })
            });
        }
        "base_branch" => {
            return Ok(ProjectChange::Project(ProjectUpdate {
                base_branch: Some(value.text()?.to_owned()),
                ..ProjectUpdate::default()
            }));
        }
        "agent_instructions" => s.agent_instructions = value.text()?.to_owned(),
        "project_agent_instructions" => s.project_agent_instructions = value.text()?.to_owned(),
        // `""` (Use global) empties a project's own value.
        "validation" => {
            s.validation = match value {
                Value::Text(t) if t.is_empty() => None,
                v => Some(v.flag()?),
            }
        }
        "validation_rounds" => {
            s.validation_rounds = match value {
                Value::Text(t) if t.is_empty() => None,
                v => Some(
                    v.number()?
                        .clamp(*VALIDATION_ROUNDS.start(), *VALIDATION_ROUNDS.end()),
                ),
            }
        }
        "auto_merge" => {
            s.auto_merge = match value.text()? {
                "" => None,
                id => Some(auto_merge_of(id)?),
            }
        }
        "setup_command" => s.setup_command = value.text()?.to_owned(),
        "run_command" => s.run_command = value.text()?.to_owned(),
        "open_url" => s.open_url = value.text()?.trim().to_owned(),
        "let_finished_tickets_answer" => s.let_finished_tickets_answer = value.flag()?,
        // Off clears the warm build after saving (the service; the page asked first when there was one).
        "warm_builds" => s.warm_builds = value.flag()?,
        "resource_create" => s.resource_create = value.text()?.to_owned(),
        "resource_remove" => s.resource_remove = value.text()?.to_owned(),
        "resource_template_refresh" => s.resource_template_refresh = value.text()?.to_owned(),
        // One glob per line; empty lines are dropped.
        "schema_paths" => {
            s.schema_paths = value
                .text()?
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        }
        "env_files.add" => {
            let file = value.text()?.trim();
            if file.is_empty() {
                return Ok(ProjectChange::Nothing);
            }
            s.env_files.push(file.to_owned());
        }
        "env_files.remove" => {
            let i = value.number()? as usize;
            if i < s.env_files.len() {
                s.env_files.remove(i);
            }
        }
        f => {
            if let Some(change) = edit_env_var(s, f, value) {
                return change;
            }
            if let Some(i) = f
                .strip_prefix("env_files.")
                .and_then(|i| i.parse::<usize>().ok())
            {
                let file = value.text()?.trim().to_owned();
                if i >= s.env_files.len() {
                    return Err(format!("no env file {i}"));
                }
                // An emptied field removes the file.
                if file.is_empty() {
                    s.env_files.remove(i);
                } else {
                    s.env_files[i] = file;
                }
            } else if step_of(f).is_some() {
                return Err(typed_skill());
            } else {
                return Err(format!("unknown setting: {f}"));
            }
        }
    }
    Ok(ProjectChange::Settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use otaq_core::agent_tools::Tool;

    fn info() -> ProjectInfo {
        ProjectInfo {
            key: "BILL".into(),
            name: "billing-api".into(),
            repo_path: "/code/billing-api".into(),
            base_branch: "develop".into(),
            agents_working: 1,
            needs_you: 0,
        }
    }

    fn global(settings: &GlobalSettings) -> Vec<Section<GlobalSection>> {
        let input = GlobalInputs {
            settings,
            problems: &[],
            scheduler: None,
            data: Some(("~/x/data.db".into(), "~/x/logs/".into())),
            cleanup_count: 0,
            os: Os::Mac,
            notification_permission: PermissionState::Unknown,
            skills: SkillInputs::none(Os::Mac),
        };
        match global_page(&input) {
            Page::Global { sections, .. } => sections,
            Page::Project { .. } => unreachable!(),
        }
    }

    /// The Notifications section with macOS's permission `permission`.
    fn notifications_rows(settings: &GlobalSettings, permission: PermissionState) -> Vec<Row> {
        let input = GlobalInputs {
            settings,
            problems: &[],
            scheduler: None,
            data: None,
            cleanup_count: 0,
            os: Os::Mac,
            notification_permission: permission,
            skills: SkillInputs::none(Os::Mac),
        };
        let Page::Global { sections, .. } = global_page(&input) else {
            unreachable!()
        };
        sections
            .into_iter()
            .find(|s| s.id == GlobalSection::Notifications)
            .expect("the Notifications section")
            .rows
    }

    /// T-0372 (ATT-14, d0090): the status line under the main switch in every macOS state, with its one action;
    /// none where the state isn't read; the same whatever the switch's state.
    #[test]
    fn the_notifications_status_line_follows_macos() {
        let product = PRODUCT_NAME;
        type Line = Option<(String, Option<(&'static str, ButtonAction)>)>;
        let cases: [(PermissionState, Line); 5] = [
            (
                PermissionState::NotAsked,
                Some((
                    "macOS hasn’t been asked yet.".into(),
                    Some(("Allow notifications", ButtonAction::AllowNotifications)),
                )),
            ),
            (
                PermissionState::Allowed,
                Some(("Allowed in macOS.".into(), None)),
            ),
            (
                PermissionState::TurnedOff,
                Some((
                    format!("Turned off in macOS, so {product} can’t notify you."),
                    Some(("Open System Settings", ButtonAction::OpenSystemSettings)),
                )),
            ),
            (
                PermissionState::OutsideBundle,
                Some((
                    format!(
                        "Running outside {product}.app: notifications appear as Script Editor, and {product} can’t \
                         check whether macOS allows them."
                    ),
                    None,
                )),
            ),
            (PermissionState::Unknown, None),
        ];
        for (permission, wanted) in cases {
            for on in [true, false] {
                let settings = GlobalSettings {
                    notifications: on,
                    ..GlobalSettings::default()
                };
                let rows = notifications_rows(&settings, permission);
                let Control::NotificationsSwitch { on: shown, status } = &rows[0].control else {
                    panic!("{:?}", rows[0].control)
                };
                assert_eq!(*shown, on);
                assert_eq!(rows[0].field, "notifications");
                let status = status.as_ref().map(|s| {
                    (
                        s.text.clone(),
                        s.action.as_ref().map(|a| (a.label.as_str(), a.action)),
                    )
                });
                assert_eq!(status, wanted, "{permission:?}, switch on = {on}");
            }
        }
    }

    /// T-0372 (ATT-16, d0090): the seven kinds then Play a sound, indented under the main switch, disabled and dimmed
    /// (not hidden) while it is off; ticking one edits its kind.
    #[test]
    fn the_notification_checkboxes_follow_the_switch() {
        let mut s = GlobalSettings::default();
        let rows = notifications_rows(&s, PermissionState::Allowed);
        let labels: Vec<(&str, &str)> = rows[1..]
            .iter()
            .map(|r| (r.label.as_str(), r.description.as_str()))
            .collect();
        assert_eq!(
            labels,
            [
                (
                    "Permission prompts",
                    "Agents, validators, Project agents and epic leads."
                ),
                ("Questions", "Agents, Project agents and epic leads."),
                ("Agent ended its turn", ""),
                ("Session interrupted", ""),
                (
                    "Validation needs a decision",
                    "Failed N times, or no result."
                ),
                ("Auto-merge stopped", ""),
                ("Agent tool not available", ""),
                ("Play a sound", "The OS’s default notification sound."),
            ]
        );
        for row in &rows[1..] {
            assert_eq!(
                row.control,
                Control::Checkbox {
                    on: true,
                    disabled: false,
                    indented: true
                },
                "{}",
                row.label
            );
            assert!(!row.dimmed);
        }

        edit_global(&mut s, "notification_kinds.question", &Value::Flag(false)).unwrap();
        edit_global(&mut s, "notification_sound", &Value::Flag(false)).unwrap();
        assert!(!s.notification_kinds.allows(Kind::Question));
        assert!(s.notification_kinds.allows(Kind::Permission));
        assert!(!s.notification_sound);
        assert!(edit_global(&mut s, "notification_kinds.nothing", &Value::Flag(false)).is_err());
        assert!(edit_global(&mut s, "notification_sound", &Value::Text("no".into())).is_err());
        for kind in Kind::ALL {
            let field = notification_kind_row(kind).0;
            edit_global(&mut s, field, &Value::Flag(false)).unwrap();
            assert!(!s.notification_kinds.allows(kind), "{field}");
        }

        s.notifications = false;
        let rows = notifications_rows(&s, PermissionState::Allowed);
        assert_eq!(rows.len(), 9, "not hidden");
        for row in &rows[1..] {
            let Control::Checkbox { disabled, .. } = row.control else {
                panic!()
            };
            assert!(disabled && row.dimmed, "{}", row.label);
        }
    }

    fn find<'a>(sections: &'a [Section<GlobalSection>], field: &str) -> &'a Row {
        sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.field == field)
            .unwrap_or_else(|| panic!("no row {field}"))
    }

    #[test]
    fn the_global_page_has_every_drawn_section_in_nav_order() {
        let sections = global(&GlobalSettings::default());
        let ids: Vec<GlobalSection> = sections.iter().map(|s| s.id).collect();
        assert_eq!(ids, DRAWN);
        let nav = view(
            Page::Global {
                title: String::new(),
                sections: Vec::new(),
            },
            &[info()],
            &["BILL".into()],
        )
        .nav;
        assert_eq!(nav.len(), 13);
        // Agent tools first, then Agents (M22, `bVdGw`; M23, `bGHVl`).
        assert_eq!(
            (nav[0].label.as_str(), nav[0].icon),
            ("Agent tools", IconName::Plug)
        );
        assert_eq!(
            (nav[1].label.as_str(), nav[1].icon),
            ("Agents", IconName::Bot)
        );
        // The Agents page holds Agent limits and Agent instructions; every other topic is its own page.
        assert_eq!(
            nav[1].target,
            Target::Global {
                section: GlobalSection::Agents,
                sections: vec![
                    GlobalSection::Agents,
                    GlobalSection::AgentLimits,
                    GlobalSection::AgentInstructions
                ],
            }
        );
        assert_eq!(
            nav[0].target,
            Target::Global {
                section: GlobalSection::AgentTools,
                sections: vec![GlobalSection::AgentTools],
            }
        );
        assert_eq!(nav[2].label, "Workflow skills");
        assert_eq!(
            (nav[3].label.as_str(), nav[3].icon),
            ("Validation & Auto-merge", IconName::ShieldCheck)
        );
        assert_eq!(
            (nav[4].label.as_str(), nav[4].icon),
            ("Project agents", IconName::Bot)
        );
        assert_eq!(nav[5].label, "Usage");
        // Keyboard shortcuts after Appearance (M29, T-0395; design `s15-46`).
        assert_eq!(nav[8].label, "Appearance");
        assert_eq!(
            (nav[9].label.as_str(), nav[9].icon),
            ("Keyboard shortcuts", IconName::Keyboard)
        );
        // About is the last global row (d0071).
        assert_eq!(
            (nav[11].label.as_str(), nav[11].icon),
            ("About", IconName::Info)
        );
        assert_eq!(nav[12].target, Target::Project { key: "BILL".into() });
        assert!(nav[12].alert);
    }

    // ---- Settings → Workflow skills (T-0380; WSK-01, WSK-02, WSK-04, WSK-07; *15 → update AC*) ----

    const SKILL_NOW: i64 = 1_760_000_000_000;

    /// A text with Otaq's names written as `<P>` (the product), `<N>` (the plugin namespace) and `<D>` (the data
    /// folder), so the expected texts below name nothing outside `naming` (d0028).
    fn unfill(text: &str) -> String {
        text.replace(&naming::data_dir_name(), "<D>")
            .replace(PRODUCT_NAME, "<P>")
            .replace(&format!("{}:", naming::PLUGIN_NAMESPACE), "<N>:")
    }

    /// Every text of a row, through [`unfill`].
    fn unfill_row(mut row: SkillRow) -> SkillRow {
        let dialog = |d: &mut SkillDialog| {
            for t in [&mut d.title, &mut d.text, &mut d.note, &mut d.confirm] {
                *t = unfill(t);
            }
        };
        row.name = unfill(&row.name);
        row.source = unfill(&row.source);
        row.folder = row.folder.as_deref().map(unfill);
        for line in &mut row.lines {
            line.text = unfill(&line.text);
        }
        for item in &mut row.menu {
            item.label = unfill(&item.label);
        }
        row.reimport.as_mut().map(dialog);
        row.reset.as_mut().map(dialog);
        row
    }

    fn home(rest: &str) -> String {
        std::env::home_dir()
            .unwrap_or_default()
            .join(rest)
            .display()
            .to_string()
    }

    fn built(step: WorkflowStep) -> SkillStatus {
        status_of(step, &SkillSetting::BuiltIn)
    }

    /// `my-commit` imported from `~/team-skills/my-commit` at SKILL_NOW, its copy in `~/.otaq/skills/c1`.
    fn my_commit() -> SkillStatus {
        SkillStatus {
            kind: SkillKind::Imported,
            name: Some("my-commit".into()),
            original: Some(home("team-skills/my-commit")),
            original_exists: true,
            imported_at: Some(SKILL_NOW),
            copy: Some(home(&format!("{}/skills/c1", naming::data_dir_name())).into()),
            copy_ok: true,
            ..built(WorkflowStep::Commit)
        }
    }

    fn skill_inputs<'a>(
        statuses: &'a [SkillStatus],
        global: &'a [SkillStatus],
        refused: Option<&'a SkillRefusal>,
        os: Os,
    ) -> SkillInputs<'a> {
        SkillInputs {
            statuses,
            global,
            refused,
            os,
            now: SKILL_NOW,
        }
    }

    /// The global page's Workflow skills section with `statuses` (the other steps built-in).
    fn skills_section(
        statuses: Vec<SkillStatus>,
        refused: Option<&SkillRefusal>,
        os: Os,
    ) -> Section<GlobalSection> {
        let statuses = statuses_of(&statuses, built);
        let Page::Global { sections, .. } = global_page(&GlobalInputs {
            settings: &GlobalSettings::default(),
            problems: &[],
            scheduler: None,
            data: None,
            cleanup_count: 0,
            os,
            notification_permission: PermissionState::Unknown,
            skills: skill_inputs(&statuses, &[], refused, os),
        }) else {
            unreachable!()
        };
        sections
            .into_iter()
            .find(|s| s.id == GlobalSection::WorkflowSkills)
            .expect("the Workflow skills section")
    }

    fn skills_of(control: &Control) -> (&str, &[SkillRow], Option<&str>) {
        let Control::Skills {
            heading,
            rows,
            hint,
        } = control
        else {
            panic!("the skills control")
        };
        (heading, rows, hint.as_deref())
    }

    fn global_rows(statuses: Vec<SkillStatus>, refused: Option<&SkillRefusal>) -> Vec<SkillRow> {
        skills_of(&skills_section(statuses, refused, Os::Mac).rows[0].control)
            .1
            .iter()
            .cloned()
            .map(unfill_row)
            .collect()
    }

    /// shop-web's page with `own` (the other steps on Use global) and the global page's `global`.
    fn skill_project_rows(own: Vec<SkillStatus>, global: Vec<SkillStatus>) -> Vec<SkillRow> {
        let own = statuses_of(&own, |step| status_of(step, &SkillSetting::UseGlobal));
        let global = statuses_of(&global, built);
        let Page::Project { sections, .. } = project_page(&ProjectInputs {
            info: &info(),
            settings: &ProjectSettings::default(),
            problems: &[],
            global: &GlobalSettings::default(),
            warm: None,
            project_agent: "Strong",
            tool_connected: true,
            skills: skill_inputs(&own, &global, None, Os::Mac),
        }) else {
            unreachable!()
        };
        let section = sections
            .into_iter()
            .find(|s| s.id == ProjectSection::WorkflowSkills)
            .unwrap();
        assert_eq!(section.intro.as_deref(), Some(PROJECT_SKILLS_INTRO));
        let (heading, rows, _) = skills_of(&section.rows[0].control);
        assert_eq!(heading, "Workflow skills");
        rows.iter().cloned().map(unfill_row).collect()
    }

    fn texts(row: &SkillRow) -> Vec<(SkillLook, &str)> {
        row.lines
            .iter()
            .map(|l| (l.look, l.text.as_str()))
            .collect()
    }

    fn menu_of(row: &SkillRow) -> Vec<(SkillAction, &str, bool)> {
        row.menu
            .iter()
            .map(|m| (m.action, m.label.as_str(), m.separated))
            .collect()
    }

    const COPY_NOTE: &str = "<P> uses its own copy. Changes to the original apply after Re-import.";

    /// *Global · built-in (fresh install)*: the intro, "Steps", a row per step on the built-in with Import skill
    /// folder… and no ⋯, and the dot-folder hint on macOS only; the rows follow the settings when no status was read.
    #[test]
    fn workflow_skills_fresh_install_rows_are_built_in() {
        let section = skills_section(Vec::new(), None, Os::Mac);
        assert_eq!(section.title, None);
        assert_eq!(
            section.intro.as_deref(),
            Some(
                "Commit, Push, Open PR and Validate follow these skills. Import a skill folder (its SKILL.md and the \
                 files it uses) to replace one with your own. Projects can override each step."
            )
        );
        let (heading, rows, hint) = skills_of(&section.rows[0].control);
        assert_eq!(heading, "Steps");
        assert_eq!(
            hint,
            Some(
                "Skill in a dot folder such as .claude? The macOS folder picker hides them: press \u{2318}\u{21e7}. \
                 in the picker to show them."
            )
        );
        let unfilled: Vec<SkillRow> = rows.iter().cloned().map(unfill_row).collect();
        let shown: Vec<_> = unfilled
            .iter()
            .map(|r| {
                (
                    r.label.as_str(),
                    r.name.as_str(),
                    r.tag.label.as_str(),
                    r.source.as_str(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("Commit", "<N>:commit", "Built-in", "<P>\u{2019}s default"),
                ("Push", "<N>:push", "Built-in", "<P>\u{2019}s default"),
                ("Open PR", "<N>:open-pr", "Built-in", "<P>\u{2019}s default"),
                (
                    "Validate",
                    "<N>:validate",
                    "Built-in",
                    "<P>\u{2019}s default"
                ),
            ]
        );
        for row in rows {
            let button = row.button.as_ref().unwrap();
            assert_eq!(
                (
                    button.label.as_str(),
                    button.icon,
                    button.action,
                    button.disabled
                ),
                (
                    "Import skill folder\u{2026}",
                    Some(IconName::FolderInput),
                    SkillAction::Import,
                    false
                )
            );
            assert!(row.menu.is_empty() && row.lines.is_empty() && row.folder.is_none());
            assert!(row.reimport.is_none() && row.reset.is_none());
        }
        assert_eq!(rows[2].field, "workflow_skills.open_pr");
        // The same page from the settings alone (no status read).
        let from_settings = global(&GlobalSettings::default());
        let ws = from_settings
            .iter()
            .find(|s| s.id == GlobalSection::WorkflowSkills)
            .unwrap();
        assert_eq!(skills_of(&ws.rows[0].control).1, rows);
    }

    /// The dot-folder hint is macOS only.
    #[test]
    fn workflow_skills_dot_folder_hint_is_macos_only() {
        for (os, shown) in [(Os::Mac, true), (Os::Windows, false), (Os::Linux, false)] {
            let section = skills_section(Vec::new(), None, os);
            assert_eq!(
                skills_of(&section.rows[0].control).2.is_some(),
                shown,
                "{os:?}"
            );
        }
    }

    /// *Global · Commit imported*: the name with Imported, the original and when, the copy note, Re-import and
    /// the ⋯ menu (Show Otaq's copy in Finder, Import a different folder…, a separator, Use built-in (otaq:commit)).
    #[test]
    fn workflow_skills_imported_row() {
        let rows = global_rows(vec![my_commit()], None);
        let r = &rows[0];
        assert_eq!((r.name.as_str(), r.name_muted), ("my-commit", false));
        assert_eq!(
            r.tag,
            SkillTag {
                label: "Imported".into(),
                look: SkillLook::Quiet
            }
        );
        assert_eq!(
            r.folder.as_deref(),
            Some(tilde(&home("team-skills/my-commit")).as_str())
        );
        assert_eq!(
            r.source,
            format!(
                "\u{b7} imported today, {}",
                crate::view_models::clock::local_hhmm(SKILL_NOW)
            )
        );
        assert_eq!(texts(r), [(SkillLook::Quiet, COPY_NOTE)]);
        assert_eq!(r.lines[0].icon, IconName::Info);
        let button = r.button.as_ref().unwrap();
        assert_eq!(
            (
                button.label.as_str(),
                button.icon,
                button.action,
                button.disabled
            ),
            (
                "Re-import",
                Some(IconName::RefreshCw),
                SkillAction::Reimport,
                false
            )
        );
        assert_eq!(r.menu_label, "More for Commit");
        assert_eq!(
            menu_of(r),
            [
                (
                    SkillAction::Reveal,
                    "Show <P>\u{2019}s copy in Finder",
                    false
                ),
                (
                    SkillAction::Import,
                    "Import a different folder\u{2026}",
                    false
                ),
                (SkillAction::Reset, "Use built-in (<N>:commit)", true),
            ]
        );
        assert_eq!(
            r.menu.iter().map(|m| m.icon).collect::<Vec<_>>(),
            [
                IconName::FolderOpen,
                IconName::FolderInput,
                IconName::RotateCcw
            ]
        );
        // The other steps stay on the built-in.
        assert_eq!(rows[1].tag.label, "Built-in");
    }

    /// Both dialogs' texts (*Re-import asks first*, *Going back to the built-in*): the accent Re-import, the red Use
    /// built-in; a project's reset goes back to the global skill.
    #[test]
    fn workflow_skills_dialogs() {
        let rows = global_rows(vec![my_commit()], None);
        let (original, copy) = (
            tilde(&home("team-skills/my-commit")),
            tilde(&home("<D>/skills/c1")),
        );
        assert_eq!(
            rows[0].reimport,
            Some(SkillDialog {
                title: "Re-import my-commit?".into(),
                text: format!(
                    "<P> replaces its copy with the folder at {original}. Sessions already running keep the copy \
                     they started with; the next launch uses the new one."
                ),
                note: format!("Edits made to <P>\u{2019}s copy in {copy} are lost."),
                confirm: "Re-import".into(),
                destructive: false,
            })
        );
        assert_eq!(
            rows[0].reset,
            Some(SkillDialog {
                title: "Use the built-in <N>:commit?".into(),
                text: format!(
                    "Commit goes back to <P>\u{2019}s built-in skill and <P>\u{2019}s copy of my-commit is deleted. \
                     The original at {original} isn\u{2019}t touched."
                ),
                note: "<P>\u{2019}s copy has edits that aren\u{2019}t in the original. Going back to the built-in \
                       deletes them."
                    .into(),
                confirm: "Use built-in".into(),
                destructive: true,
            })
        );
        let project = skill_project_rows(vec![my_commit()], Vec::new());
        let reset = project[0].reset.as_ref().unwrap();
        assert_eq!(reset.title, "Use the global <N>:commit?");
        assert_eq!(reset.confirm, "Use global");
        assert!(reset.destructive);
        assert!(
            reset
                .note
                .ends_with("Going back to the global skill deletes them.")
        );
    }

    /// §2 C: files the SKILL.md mentions that aren't in the folder are an amber line on the imported row, above the
    /// copy note.
    #[test]
    fn workflow_skills_imported_with_a_warning() {
        let rows = global_rows(
            vec![SkillStatus {
                warnings: vec![
                    "SKILL.md mentions scripts/check-staged.sh, which isn't in the folder".into(),
                ],
                ..my_commit()
            }],
            None,
        );
        assert_eq!(
            texts(&rows[0]),
            [
                (
                    SkillLook::Warning,
                    "Imported, but SKILL.md mentions scripts/check-staged.sh, which isn't in the folder."
                ),
                (SkillLook::Quiet, COPY_NOTE),
            ]
        );
        assert_eq!(rows[0].lines[0].icon, IconName::TriangleAlert);
        assert_eq!(rows[0].tag.label, "Imported");
    }

    /// §3 A: the original moved or deleted: Re-import disabled with the reason under the skill, pointing to Import a
    /// different folder…; the copy keeps working.
    #[test]
    fn workflow_skills_original_gone() {
        let rows = global_rows(
            vec![SkillStatus {
                original_exists: false,
                ..my_commit()
            }],
            None,
        );
        let r = &rows[0];
        let original = tilde(&home("team-skills/my-commit"));
        assert_eq!(
            texts(r),
            [
                (
                    SkillLook::Quiet,
                    format!(
                        "Original not found at {original}, so Re-import is unavailable. <P>\u{2019}s copy keeps \
                         working; Import a different folder\u{2026} in the \u{22ef} menu points it at the new place."
                    )
                    .as_str()
                ),
                (SkillLook::Quiet, COPY_NOTE),
            ]
        );
        assert_eq!(r.lines[0].icon, IconName::FolderX);
        let button = r.button.as_ref().unwrap();
        assert_eq!(
            (button.action, button.disabled),
            (SkillAction::Reimport, true)
        );
        assert_eq!(r.tag.label, "Imported");
        assert_eq!(menu_of(r)[1].0, SkillAction::Import);
    }

    /// §3 B: Otaq's copy missing or broken: the red tag and line with Import again (Re-import from the original, or
    /// the picker when the original is gone too), no button; Show Otaq's copy only while its folder is there.
    #[test]
    fn workflow_skills_skill_missing() {
        let missing = SkillStatus {
            copy_ok: false,
            copy_problem: Some(copy_missing()),
            ..my_commit()
        };
        let rows = global_rows(vec![missing.clone()], None);
        let r = &rows[0];
        assert_eq!(
            r.tag,
            SkillTag {
                label: "Skill missing".into(),
                look: SkillLook::Error
            }
        );
        assert!(r.button.is_none());
        let copy = tilde(&home("<D>/skills/c1"));
        assert_eq!(
            texts(r),
            [
                (SkillLook::Error, "Skill missing \u{b7}"),
                (
                    SkillLook::Quiet,
                    format!(
                        "<P>\u{2019}s copy in {copy} is missing or broken. Agents use the built-in <N>:commit \
                         meanwhile."
                    )
                    .as_str()
                ),
            ]
        );
        let link = r.lines[0].link.as_ref().unwrap();
        assert_eq!(
            (link.label.as_str(), link.action),
            ("Import again", SkillAction::ImportAgain)
        );
        assert_eq!(
            menu_of(r).iter().map(|m| m.0).collect::<Vec<_>>(),
            [SkillAction::Import, SkillAction::Reset]
        );
        // The original gone too: Import again opens the picker.
        let rows = global_rows(
            vec![SkillStatus {
                original_exists: false,
                ..missing
            }],
            None,
        );
        assert_eq!(
            rows[0].lines[0].link.as_ref().unwrap().action,
            SkillAction::Import
        );
        // A broken copy (its folder is there): Show Otaq's copy stays.
        let rows = global_rows(
            vec![SkillStatus {
                copy_ok: false,
                copy_problem: Some("SKILL.md's front matter has no name:".into()),
                ..my_commit()
            }],
            None,
        );
        assert_eq!(rows[0].tag.label, "Skill missing");
        assert_eq!(rows[0].menu[0].action, SkillAction::Reveal);
    }

    /// §3 C: a global typed name from before 0092: "Not imported" in amber, Import skill folder…, and the step uses
    /// the built-in until then.
    #[test]
    fn workflow_skills_legacy_global() {
        let rows = global_rows(
            vec![status_of(
                WorkflowStep::Commit,
                &SkillSetting::Legacy("/my-commit".into()),
            )],
            None,
        );
        let r = &rows[0];
        assert_eq!((r.name.as_str(), r.name_muted), ("/my-commit", true));
        assert_eq!(
            r.tag,
            SkillTag {
                label: "Not imported".into(),
                look: SkillLook::Warning
            }
        );
        assert_eq!(r.source, "Typed name from an earlier version");
        assert_eq!(
            texts(r),
            [(
                SkillLook::Quiet,
                "Import its folder to use it. Until then Commit uses the built-in <N>:commit."
            )]
        );
        assert_eq!(r.button.as_ref().unwrap().action, SkillAction::Import);
        assert!(r.menu.is_empty());
    }

    /// §3 D: a project's typed name follows the global skill, named (the built-in, or the global's import); its ⋯
    /// offers Use global.
    #[test]
    fn workflow_skills_legacy_project() {
        let legacy = || status_of(WorkflowStep::OpenPr, &SkillSetting::Legacy("/my-pr".into()));
        let rows = skill_project_rows(vec![legacy()], Vec::new());
        let r = &rows[2];
        assert_eq!(
            (r.name.as_str(), r.tag.label.as_str()),
            ("/my-pr", "Not imported")
        );
        assert_eq!(
            texts(r),
            [(
                SkillLook::Quiet,
                "Import its folder, or pick Use global. Until then Open PR uses the global skill (<N>:open-pr)."
            )]
        );
        assert_eq!(menu_of(r), [(SkillAction::Reset, "Use global", false)]);
        let team_pr = SkillStatus {
            step: WorkflowStep::OpenPr,
            name: Some("team-pr".into()),
            ..my_commit()
        };
        let rows = skill_project_rows(vec![legacy()], vec![team_pr]);
        assert!(
            rows[2].lines[0]
                .text
                .ends_with("Until then Open PR uses the global skill (team-pr).")
        );
    }

    /// *Project · shop-web overrides Open PR*: steps on Use global name what the global runs (its import, or Otaq's
    /// default); the project's own import offers Use global last in its ⋯.
    #[test]
    fn workflow_skills_project_page() {
        let my_pr = SkillStatus {
            step: WorkflowStep::OpenPr,
            name: Some("my-pr".into()),
            original: Some(home("code/shop-web/.claude/skills/my-pr")),
            ..my_commit()
        };
        let rows = skill_project_rows(vec![my_pr], vec![my_commit()]);
        let shown: Vec<_> = rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.name_muted,
                    r.tag.label.as_str(),
                    r.source.as_str(),
                )
            })
            .collect();
        let global_source = format!(
            "Global: imported from {}",
            tilde(&home("team-skills/my-commit"))
        );
        assert_eq!(
            shown[0],
            ("my-commit", true, "Use global", global_source.as_str())
        );
        assert_eq!(
            shown[1],
            (
                "<N>:push",
                true,
                "Use global",
                "Global: <P>\u{2019}s default"
            )
        );
        assert_eq!(shown[2].0, "my-pr");
        assert_eq!(shown[2].2, "Imported");
        assert_eq!(rows[0].button.as_ref().unwrap().action, SkillAction::Import);
        assert!(rows[0].menu.is_empty());
        assert_eq!(
            menu_of(&rows[2]).last(),
            Some(&(SkillAction::Reset, "Use global", true))
        );
        // A global typed name: Use global follows the built-in (WSK-07).
        let rows = skill_project_rows(
            Vec::new(),
            vec![status_of(
                WorkflowStep::Commit,
                &SkillSetting::Legacy("/my-commit".into()),
            )],
        );
        assert_eq!(rows[0].name, "<N>:commit");
        // Q211: a project step stored as Otaq's own reads as the built-in; its ⋯ goes back to Use global.
        let rows = skill_project_rows(vec![built(WorkflowStep::Push)], Vec::new());
        assert_eq!(rows[1].tag.label, "Built-in");
        assert_eq!(
            menu_of(&rows[1]),
            [(SkillAction::Reset, "Use global", false)]
        );
    }

    /// §2 A/B: a refused import is the red line under its step in the frames' words; the step keeps what it had.
    #[test]
    fn workflow_skills_refusals() {
        let refused = |message: &str| SkillRefusal {
            step: WorkflowStep::Commit,
            folder: Some(home("team-skills/commit-helpers")),
            message: message.into(),
        };
        let helpers = tilde(&home("team-skills/commit-helpers"));
        assert_eq!(
            refusal_line(&refused("The folder has no SKILL.md")),
            format!("Not imported: {helpers} has no SKILL.md.")
        );
        assert_eq!(
            refusal_line(&refused(
                "SKILL.md's front matter can't be read: line 3 isn't key: value"
            )),
            format!(
                "Not imported: the front matter at the top of {helpers}/SKILL.md can't be read."
            )
        );
        assert_eq!(
            refusal_line(&refused("SKILL.md's front matter has no name:")),
            "Not imported: SKILL.md's front matter has no name:"
        );
        assert_eq!(
            refusal_line(&refused("Original not found: /x")),
            "Not imported: Original not found: /x."
        );
        let no_skill = refused("The folder has no SKILL.md");
        let rows = global_rows(Vec::new(), Some(&no_skill));
        assert_eq!(rows[0].tag.label, "Built-in");
        assert_eq!(
            texts(&rows[0]),
            [(
                SkillLook::Error,
                format!("Not imported: {helpers} has no SKILL.md.").as_str()
            )]
        );
        assert_eq!(rows[0].lines[0].icon, IconName::CircleAlert);
        assert!(rows[1].lines.is_empty());
        // On an imported skill (Import a different folder… refused): the red line first, the skill unchanged.
        let rows = global_rows(vec![my_commit()], Some(&no_skill));
        assert_eq!(rows[0].name, "my-commit");
        assert_eq!(rows[0].lines[0].look, SkillLook::Error);
        assert_eq!(rows[0].lines[1].text, COPY_NOTE);
    }

    /// The file manager's name per OS in the ⋯ menu.
    #[test]
    fn workflow_skills_show_copy_per_os() {
        assert_eq!(
            unfill(&show_copy_label(Os::Windows)),
            "Show <P>\u{2019}s copy in Explorer"
        );
        assert_eq!(
            unfill(&show_copy_label(Os::Linux)),
            "Show <P>\u{2019}s copy in the file manager"
        );
    }

    /// "today, 10:42" on the import's day, else the day: "Oct 12".
    #[test]
    fn workflow_skills_import_time() {
        let day = 24 * 3_600_000;
        assert_eq!(
            imported_when(SKILL_NOW, SKILL_NOW),
            format!(
                "today, {}",
                crate::view_models::clock::local_hhmm(SKILL_NOW)
            )
        );
        let earlier = otaq_core::local_time::local(SKILL_NOW - 6 * day).unwrap();
        assert_eq!(
            imported_when(SKILL_NOW - 6 * day, SKILL_NOW),
            crate::view_models::clock::month_day(&earlier)
        );
    }

    /// The gallery has every state of update AC.
    #[test]
    fn workflow_skills_gallery_has_every_state() {
        let gallery = workflow_skills_gallery();
        assert_eq!(gallery.cells.len(), 10);
        let tags: Vec<String> = gallery
            .cells
            .iter()
            .flat_map(|c| match &c.section.rows[0].control {
                Control::Skills { rows, .. } => rows.iter().map(|r| r.tag.label.clone()).collect(),
                _ => Vec::new(),
            })
            .collect();
        for tag in [
            "Built-in",
            "Imported",
            "Use global",
            "Not imported",
            "Skill missing",
        ] {
            assert!(tags.iter().any(|t| t == tag), "{tag}");
        }
    }

    /// The project's Agents block (AGT-03, `mQijC` in its one-tool form).
    #[test]
    fn the_project_agents_block_starts_collapsed_and_offers_the_app_default_first() {
        use crate::view_models::named_agents::{RoleId, fixtures};
        use otaq_core::named_agents::Role;

        let list = named_agents::AgentTools::one(Some(fixtures::claude_list()));
        let (agents, app) = fixtures::made();
        let b = project_agents_block("BILL", &agents, &app, &RoleDefaults::default(), &list);
        assert!(!b.expanded, "overrides nothing: collapsed");
        assert_eq!(
            (
                b.collapsed_title.as_str(),
                b.collapsed_text.as_str(),
                b.change_label.as_str()
            ),
            ("Agents for this project", "Uses app defaults", "Change")
        );
        assert_eq!(b.title, "Agents");
        let labels: Vec<&str> = b.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Developer", "Validator", "Epic lead", "Project agent"]
        );
        let developer = &b.rows[0].picker;
        assert_eq!(developer.value.name, "Use app default (Claude Code)");
        assert_eq!(developer.value.agent, None);
        let options: Vec<(Option<&str>, &str, Option<&str>)> = developer
            .options
            .iter()
            .map(|o| (o.agent.as_deref(), o.name.as_str(), o.model.as_deref()))
            .collect();
        assert_eq!(
            options,
            [
                (None, "Use app default (Claude Code)", None),
                (Some("cc"), "Claude Code", Some("tool default")),
                (Some("strong"), "Strong", Some("Opus · always the newest")),
                (Some("light"), "Light", Some("Haiku · always the newest")),
                (Some("sonnet"), "Sonnet", Some("Sonnet · always the newest")),
                (
                    Some("fable"),
                    "Fable test",
                    Some("Fable · always the newest")
                ),
            ]
        );
        assert!(!developer.read_only);

        // shop-web overrides Developer → Strong: opens expanded, the rest on the app default.
        let mut own = RoleDefaults::default();
        own.set(Role::Developer, Some("strong".into()));
        let b = project_agents_block("SHOP", &agents, &app, &own, &list);
        assert!(b.expanded);
        assert_eq!(b.rows[0].role, RoleId::Developer);
        let value = &b.rows[0].picker.value;
        assert_eq!(
            (value.name.as_str(), value.model.as_deref()),
            ("Strong", Some("Opus · always the newest"))
        );
        assert_eq!(b.rows[1].picker.value.name, "Use app default (Claude Code)");
        assert_eq!(b.rows[2].role, RoleId::EpicLead);

        // An override on a deleted entry counts as none.
        own.set(Role::Developer, Some("gone".into()));
        let b = project_agents_block("SHOP", &agents, &app, &own, &list);
        assert!(!b.expanded);

        // One named agent: nothing offers a choice.
        let (fresh, app) = fixtures::fresh();
        let b = project_agents_block("BILL", &fresh, &app, &RoleDefaults::default(), &list);
        assert!(b.rows.iter().all(|r| r.picker.read_only));
        assert!(
            b.rows
                .iter()
                .all(|r| r.picker.options.iter().all(|o| o.tool.is_none())),
            "no avatars with one tool"
        );
    }

    /// The project's Agents block with several tools (`mQijC`): every picker names the agent's tool, with its
    /// avatar; "Use app default (Verifier)" carries the app default's.
    #[test]
    fn the_project_agents_block_names_the_tool_with_several_tools() {
        use crate::view_models::named_agents::fixtures;
        use otaq_core::named_agents::Role;

        let (agents, app) = fixtures::several();
        let mut own = RoleDefaults::default();
        own.set(Role::Developer, Some("strong".into()));
        let b = project_agents_block("SHOP", &agents, &app, &own, &fixtures::tools(Tool::ALL));
        let developer = &b.rows[0].picker;
        assert_eq!(
            (
                developer.value.name.as_str(),
                developer.value.model.as_deref(),
                developer.value.tool.as_ref().map(|t| t.letters.as_str())
            ),
            (
                "Strong",
                Some("Claude Code · Opus · always the newest"),
                Some("CC")
            )
        );
        let validator = &b.rows[1].picker.value;
        assert_eq!(validator.name, "Use app default (Verifier)");
        assert_eq!(
            validator.tool.as_ref().map(|t| t.letters.as_str()),
            Some("CP")
        );
        let models: Vec<Option<&str>> = developer
            .options
            .iter()
            .map(|o| o.model.as_deref())
            .collect();
        assert_eq!(
            models,
            [
                None,
                Some("Claude Code · Opus · always the newest"),
                Some("Codex · gpt-5.5"),
                Some("Claude Code · Haiku · always the newest"),
                Some("GitHub Copilot · gpt-5.5"),
                Some("Codex · gpt-5.5-pro"),
                Some("Claude Code · Sonnet · always the newest"),
                Some("OpenCode · google/gemini-3-pro"),
            ]
        );
    }

    /// The Usage section has only the token count: the dollar estimate is gone (d0086).
    #[test]
    fn usage_shows_the_token_count_and_no_dollar_estimate() {
        let sections = global(&GlobalSettings::default());
        let usage = sections
            .iter()
            .find(|s| s.id == GlobalSection::Usage)
            .unwrap();
        let labels: Vec<&str> = usage.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Token count"]);
        let mut s = GlobalSettings::default();
        assert!(edit_global(&mut s, "dollar_estimate", &Value::Flag(true)).is_err());
    }

    /// *15 → Global · Usage, notifications, data, appearance* (VALIDATION & AUTO-MERGE, PROJECT AGENTS).
    #[test]
    fn the_global_validation_and_project_agent_sections() {
        let mut s = GlobalSettings::default();
        let sections = global(&s);
        assert!(matches!(
            find(&sections, "validation").control,
            Control::Toggle { on: true }
        ));
        assert!(matches!(
            find(&sections, "validation_rounds").control,
            Control::Stepper {
                value: 3,
                min: 1,
                max: 5,
                empty: None,
                ..
            }
        ));
        let Control::Radio {
            choices,
            value,
            disabled,
            empty,
            note,
        } = &find(&sections, "auto_merge").control
        else {
            panic!()
        };
        let ids: Vec<&str> = choices.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["off", "epic_children", "all"]);
        assert_eq!(choices[1].label, "Epic children only");
        assert_eq!(
            choices[0].detail,
            "Validated tickets wait in Ready for Review for you. (default)"
        );
        assert_eq!(
            (value.as_deref(), *disabled, empty, note),
            (Some("off"), false, &None, &None)
        );
        let pa = find(&sections, "project_agent_instructions");
        assert!(matches!(
            &pa.control,
            Control::Text {
                multiline: true,
                ..
            }
        ));

        edit_global(&mut s, "validation", &Value::Flag(false)).unwrap();
        let sections = global(&s);
        let Control::Radio { disabled, note, .. } = &find(&sections, "auto_merge").control else {
            panic!()
        };
        assert!(disabled, "Auto-merge does nothing while validation is off");
        assert_eq!(
            note.as_deref(),
            Some("Validation is off, so Auto-merge does nothing. Turn Validation on to use it.")
        );
        edit_global(&mut s, "validation_rounds", &Value::Number(9)).unwrap();
        assert_eq!(s.validation_rounds, 5, "rounds accept only 1–5");
        edit_global(&mut s, "validation_rounds", &Value::Number(0)).unwrap();
        assert_eq!(s.validation_rounds, 1);
        edit_global(&mut s, "auto_merge", &Value::Text("epic_children".into())).unwrap();
        assert_eq!(s.auto_merge, AutoMerge::EpicChildren);
        assert!(edit_global(&mut s, "auto_merge", &Value::Text("some".into())).is_err());
        edit_global(
            &mut s,
            "project_agent_instructions",
            &Value::Text("Use /import-github.".into()),
        )
        .unwrap();
        assert_eq!(s.project_agent_instructions, "Use /import-github.");
    }

    fn project_rows(settings: &ProjectSettings, global: &GlobalSettings) -> Vec<Row> {
        let info = ProjectInfo {
            name: "web-shop".into(),
            ..info()
        };
        let Page::Project { sections, .. } = project_page(&ProjectInputs {
            info: &info,
            settings,
            problems: &[],
            global,
            warm: None,
            project_agent: "Claude Code",
            tool_connected: true,
            skills: SkillInputs::none(Os::Mac),
        }) else {
            panic!()
        };
        let ids: Vec<ProjectSection> = sections.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                ProjectSection::General,
                ProjectSection::Agents,
                ProjectSection::AgentInstructions,
                ProjectSection::ProjectAgentInstructions,
                ProjectSection::WorkflowSkills,
                ProjectSection::Validation,
                ProjectSection::Answers,
                ProjectSection::Commands,
                ProjectSection::TicketResources,
                ProjectSection::PrProvider,
                ProjectSection::RemoveProject,
            ]
        );
        sections.into_iter().flat_map(|s| s.rows).collect()
    }

    /// Ticket resources (M19-d, d0087): empty by default, each field edits its setting, and the schema paths are one
    /// glob per line with empty lines dropped.
    #[test]
    fn ticket_resources_section_edits_each_field() {
        let mut s = ProjectSettings::default();
        let section = ticket_resources_section(&s, &[]);
        assert_eq!(section.id, ProjectSection::TicketResources);
        assert_eq!(section.title.as_deref(), Some("Ticket resources"));
        let intro = section.intro.clone().unwrap();
        for var in [
            naming::db_name_env_var(),
            naming::db_template_env_var(),
            naming::resource_dir_env_var(),
        ] {
            assert!(intro.contains(&var), "{intro}");
        }
        let fields: Vec<&str> = section.rows.iter().map(|r| r.field.as_str()).collect();
        assert_eq!(
            fields,
            [
                "resource_create",
                "resource_remove",
                "resource_template_refresh",
                "schema_paths"
            ]
        );
        for row in &section.rows {
            let Control::Text {
                value,
                placeholder,
                mono,
                problem,
                ..
            } = &row.control
            else {
                panic!("{}", row.field)
            };
            assert_eq!((value.as_str(), placeholder.as_str()), ("", "none"));
            assert!(mono, "{}", row.field);
            assert_eq!(*problem, None);
        }

        for (field, text) in [
            ("resource_create", "createdb -T $T $N"),
            ("resource_remove", "dropdb $N"),
            ("resource_template_refresh", "make template"),
        ] {
            assert_eq!(
                edit_project(&mut s, field, &Value::Text(text.into())),
                Ok(ProjectChange::Settings)
            );
        }
        assert_eq!(s.resource_create, "createdb -T $T $N");
        assert_eq!(s.resource_remove, "dropdb $N");
        assert_eq!(s.resource_template_refresh, "make template");
        edit_project(
            &mut s,
            "schema_paths",
            &Value::Text(" db/migrations/** \n\n prisma/schema.prisma\n".into()),
        )
        .unwrap();
        assert_eq!(s.schema_paths, ["db/migrations/**", "prisma/schema.prisma"]);
        assert!(edit_project(&mut s, "schema_paths", &Value::Flag(true)).is_err());

        let problems = [
            SettingProblem {
                field: "schema_paths".into(),
                message: "a".into(),
            },
            SettingProblem {
                field: "schema_paths".into(),
                message: "b".into(),
            },
            SettingProblem {
                field: "resource_remove".into(),
                message: "c".into(),
            },
        ];
        let section = ticket_resources_section(&s, &problems);
        let control = |field: &str| {
            section
                .rows
                .iter()
                .find(|r| r.field == field)
                .unwrap()
                .control
                .clone()
        };
        let Control::Text {
            value,
            multiline,
            problem,
            ..
        } = control("schema_paths")
        else {
            panic!()
        };
        assert_eq!(value, "db/migrations/**\nprisma/schema.prisma");
        assert!(multiline);
        assert_eq!(problem.as_deref(), Some("a; b"));
        let Control::Text { problem, .. } = control("resource_remove") else {
            panic!()
        };
        assert_eq!(problem.as_deref(), Some("c"));
    }

    /// *Project · fresh* (Validation off for otaq) and *Project · shop-web · Validation & Auto-merge*.
    #[test]
    fn project_values_say_use_global_and_auto_merge_waits_for_validation() {
        let global = GlobalSettings::default();
        let fresh = ProjectSettings {
            validation: Some(false),
            ..ProjectSettings::default()
        };
        let rows = project_rows(&fresh, &global);
        let by = |field: &str| rows.iter().find(|r| r.field == field).unwrap().clone();
        let validation = by("validation");
        assert!(matches!(validation.control, Control::Toggle { on: false }));
        assert!(validation.description.starts_with("Off for this project:"));
        assert_eq!(validation.reset.as_deref(), Some("Use global"));
        let rounds = by("validation_rounds");
        assert!(
            matches!(&rounds.control, Control::Stepper { value: 3, empty: Some(e), .. } if e == "Use global (3)")
        );
        assert_eq!(rounds.reset, None);
        let auto = by("auto_merge");
        let Control::Radio {
            value,
            disabled,
            empty,
            note,
            ..
        } = &auto.control
        else {
            panic!()
        };
        assert_eq!(value, &None);
        assert!(disabled);
        assert_eq!(empty.as_deref(), Some("Use global (Off)"));
        assert_eq!(
            note.as_deref(),
            Some(
                "Validation is off for web-shop, so Auto-merge does nothing. Turn Validation on to use it."
            )
        );
        let pa = by("project_agent_instructions");
        assert!(
            matches!(&pa.control, Control::Text { placeholder, .. } if placeholder == "Use global")
        );

        let shop = ProjectSettings {
            validation: Some(true),
            validation_rounds: Some(3),
            auto_merge: Some(AutoMerge::EpicChildren),
            ..ProjectSettings::default()
        };
        let rows = project_rows(&shop, &global);
        let auto = rows.iter().find(|r| r.field == "auto_merge").unwrap();
        assert!(
            matches!(&auto.control, Control::Radio { value: Some(v), disabled: false, empty: None, note: None, .. } if v == "epic_children")
        );
        assert_eq!(auto.reset.as_deref(), Some("Use global"));

        let mut s = shop;
        edit_project(&mut s, "validation_rounds", &Value::Number(7)).unwrap();
        assert_eq!(s.validation_rounds, Some(5));
        edit_project(&mut s, "validation_rounds", &Value::Text(String::new())).unwrap();
        assert_eq!(s.validation_rounds, None, "Use global empties it");
        edit_project(&mut s, "auto_merge", &Value::Text(String::new())).unwrap();
        assert_eq!(s.auto_merge, None);
        edit_project(&mut s, "auto_merge", &Value::Text("all".into())).unwrap();
        assert_eq!(s.auto_merge, Some(AutoMerge::All));
        edit_project(&mut s, "validation", &Value::Text(String::new())).unwrap();
        assert_eq!(s.validation, None);
        edit_project(&mut s, "validation", &Value::Flag(false)).unwrap();
        assert_eq!(s.validation, Some(false));
        edit_project(
            &mut s,
            "project_agent_instructions",
            &Value::Text("x".into()),
        )
        .unwrap();
        assert_eq!(s.project_agent_instructions, "x");
    }

    #[test]
    fn limits_dim_while_off_and_say_right_now_while_on() {
        let mut s = GlobalSettings::default();
        let sections = global(&s);
        let max = find(&sections, "agent_limits.max_concurrency");
        assert!(max.dimmed);
        assert!(matches!(
            max.control,
            Control::Stepper {
                disabled: true,
                min: 1,
                max: 50,
                ..
            }
        ));
        edit_global(&mut s, "agent_limits.enabled", &Value::Flag(true)).unwrap();
        assert!(s.agent_limits.enabled);
        assert!(!find(&global(&s), "agent_limits.max_concurrency").dimmed);
        edit_global(&mut s, "agent_limits.max_concurrency", &Value::Number(99)).unwrap();
        assert_eq!(s.agent_limits.max_concurrency, 50, "a stepper clamps");
        let status = SchedulerStatus::default();
        let input = GlobalInputs {
            settings: &s,
            problems: &[],
            scheduler: Some(&status),
            data: None,
            cleanup_count: 2,
            os: Os::Linux,
            notification_permission: PermissionState::Unknown,
            skills: SkillInputs::none(Os::Mac),
        };
        let Page::Global { sections, .. } = global_page(&input) else {
            unreachable!()
        };
        assert!(
            sections[2]
                .note
                .as_deref()
                .is_some_and(|n| n.starts_with("Right now:"))
        );
        let cleanup = sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.label == "Cleanup")
            .unwrap();
        assert_eq!(
            cleanup.description,
            "2 leftovers waiting. Nothing is deleted without your click."
        );
        let size = sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.field == "terminal_font_size")
            .unwrap();
        assert_eq!(
            size.description,
            "Geist Mono. Also Ctrl+Shift+= / Ctrl+Shift+- inside a focused terminal."
        );
    }

    #[test]
    fn appearance_has_the_renderer_row_and_edits_apply() {
        let mut s = GlobalSettings::default();
        let sections = global(&s);
        let renderer = find(&sections, "terminal_renderer");
        let Control::Segmented { choices, .. } = &renderer.control else {
            panic!()
        };
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[1].label, "DOM (safer)");
        edit_global(&mut s, "terminal_renderer", &Value::Text("dom".into())).unwrap();
        assert_eq!(s.terminal_renderer, TerminalRenderer::Dom);
        assert!(edit_global(&mut s, "terminal_renderer", &Value::Text("x".into())).is_err());
        edit_global(&mut s, "terminal_font_size", &Value::Number(3)).unwrap();
        assert_eq!(s.terminal_font_size, 8);
        // A typed name can't be entered any more (WSK-01).
        assert!(
            edit_global(
                &mut s,
                "workflow_skills.open_pr",
                &Value::Text(" /my-pr ".into()),
            )
            .is_err()
        );
        assert_eq!(s.workflow_skills.open_pr, SkillSetting::BuiltIn);
        edit_global(&mut s, "usage_data", &Value::Flag(true)).unwrap();
        assert_eq!(s.usage_data, UsageData::On);
        edit_global(&mut s, "usage_data", &Value::Flag(false)).unwrap();
        assert_eq!(s.usage_data, UsageData::Off);
        assert!(edit_global(&mut s, "usage_data", &Value::Text("yes".into())).is_err());
        edit_global(&mut s, "notifications", &Value::Flag(false)).unwrap();
        assert!(!s.notifications);
        assert!(edit_global(&mut s, "notifications", &Value::Text("no".into())).is_err());
        assert!(edit_global(&mut s, "nothing", &Value::Flag(true)).is_err());
        let data = sections
            .iter()
            .find(|s| s.id == GlobalSection::Data)
            .unwrap();
        assert_eq!(
            data.rows[1].description,
            "Service and session logs. The same ~/x/ folder on macOS, Windows and Linux."
        );
        assert!(
            matches!(&data.rows[0].control, Control::Path { reveal_label, .. } if reveal_label == "Reveal in Finder")
        );
    }

    #[test]
    fn a_project_page_shows_flagged_values_and_routes_name_and_branch() {
        let mut s = ProjectSettings {
            env_files: vec!["appsettings.json".into(), ".env.local".into()],
            open_url: "localhost".into(),
            ..ProjectSettings::default()
        };
        let problems = vec![
            SettingProblem {
                field: "env_files".into(),
                message: "Can\u{2019}t find .env.local in ~/code/shop-web. Add it to the repo root, or remove it from this list."
                    .into(),
            },
            SettingProblem {
                field: "open_url".into(),
                message: "localhost isn't a URL".into(),
            },
        ];
        let page = project_page(&ProjectInputs {
            info: &info(),
            settings: &s,
            problems: &problems,
            global: &GlobalSettings::default(),
            warm: None,
            project_agent: "Strong",
            tool_connected: true,
            skills: SkillInputs::none(Os::Mac),
        });
        let Page::Project {
            title, sections, ..
        } = page
        else {
            panic!()
        };
        assert_eq!(title, "billing-api");
        let rows: Vec<&Row> = sections.iter().flat_map(|s| &s.rows).collect();
        let run = rows.iter().find(|r| r.field == "run_command").unwrap();
        assert_eq!(
            run.button,
            Some(RowButton {
                label: "Ask the agent".into(),
                icon: IconName::Sparkles,
                tooltip: Some(
                    "Asks Strong (the Project agent\u{2019}s) to suggest a run command".into()
                ),
            }),
            "the Project agent's named agent is asked (d0086)"
        );
        // With no agent tool connected there is nobody to ask (RUN-02, d0086).
        let page = project_page(&ProjectInputs {
            info: &info(),
            settings: &s,
            problems: &problems,
            global: &GlobalSettings::default(),
            warm: None,
            project_agent: "Strong",
            tool_connected: false,
            skills: SkillInputs::none(Os::Mac),
        });
        let Page::Project { sections, .. } = page else {
            panic!()
        };
        let run = sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.field == "run_command")
            .unwrap();
        assert_eq!(run.button, None);
        let url = rows.iter().find(|r| r.field == "open_url").unwrap();
        assert!(
            matches!(&url.control, Control::Text { problem: Some(p), .. } if p == "localhost isn't a URL")
        );
        let env = rows.iter().find(|r| r.field == "env_files").unwrap();
        let Control::EnvFiles { rows: files, .. } = &env.control else {
            panic!()
        };
        assert_eq!(files[0].problem, None);
        assert!(files[1].problem.is_some());
        let key = rows.iter().find(|r| r.label == "Project key").unwrap();
        assert!(key.description.contains("BILL-18"));

        assert_eq!(
            edit_project(&mut s, "name", &Value::Text("  ".into())),
            Ok(ProjectChange::Nothing)
        );
        assert!(
            matches!(edit_project(&mut s, "base_branch", &Value::Text("main".into())), Ok(ProjectChange::Project(u)) if u.base_branch.as_deref() == Some("main"))
        );
        edit_project(&mut s, "env_files.add", &Value::Text(" .env ".into())).unwrap();
        assert_eq!(s.env_files.last().map(String::as_str), Some(".env"));
        edit_project(&mut s, "env_files.remove", &Value::Number(0)).unwrap();
        assert_eq!(s.env_files[0], ".env.local");
        edit_project(&mut s, "env_files.0", &Value::Text("".into())).unwrap();
        assert_eq!(s.env_files, [".env"], "an emptied file is removed");
        edit_project(&mut s, "let_finished_tickets_answer", &Value::Flag(false)).unwrap();
        assert!(!s.let_finished_tickets_answer);
        assert!(edit_project(&mut s, "workflow_skills.commit", &Value::Text("/c".into())).is_err());
        assert_eq!(s.workflow_skills.commit, SkillSetting::UseGlobal);
        assert!(edit_project(&mut s, "env_files.9", &Value::Text("x".into())).is_err());
    }

    #[test]
    fn dialogs_and_side_rows_are_worded_here() {
        let r = remove_dialog(&info(), 2);
        assert_eq!(r.deleted_heading, format!("Deleted from {PRODUCT_NAME}"));
        assert_eq!(
            r.agents.as_deref(),
            Some("1 agent is running. Removing stops it first.")
        );
        let c = credits();
        assert_eq!(c.credits.len(), theme_credits::CREDITS.len());
        assert!(c.credits[0].themes.starts_with("→ "));
        assert!(is_credit_source(&c.credits[0].source));
        assert!(!is_credit_source("https://example.com"));
        assert_eq!(provider(None).status, "Checking…");
        let t = tools(Err("boom".into()));
        assert_eq!(t.error.as_deref(), Some("boom"));
        assert!(t.rows.is_empty());
        let json = serde_json::to_value(view(
            Page::Global {
                title: "General".into(),
                sections: global(&GlobalSettings::default()),
            },
            &[],
            &[],
        ))
        .unwrap();
        assert_eq!(json["page"]["page"], "global");
        assert_eq!(json["page"]["sections"][0]["id"], "agent_tools");
        assert_eq!(json["page"]["sections"][1]["id"], "agents");
        assert_eq!(json["page"]["sections"][2]["id"], "agent_limits");
        assert_eq!(
            json["page"]["sections"][2]["rows"][0]["control"]["kind"],
            "toggle"
        );
        assert_eq!(
            json["nav"][0]["target"],
            serde_json::json!({ "page": "global", "section": "agent_tools", "sections": ["agent_tools"] })
        );
        let v: Value = serde_json::from_value(serde_json::json!(true)).unwrap();
        assert_eq!(v, Value::Flag(true));
        let v: Value = serde_json::from_value(serde_json::json!(4)).unwrap();
        assert_eq!(v, Value::Number(4));
    }

    fn env_control(settings: &ProjectSettings, problems: &[SettingProblem]) -> Control {
        let Page::Project { sections, .. } = project_page(&ProjectInputs {
            info: &info(),
            settings,
            problems,
            global: &GlobalSettings::default(),
            warm: None,
            project_agent: "Claude Code",
            tool_connected: true,
            skills: SkillInputs::none(Os::Mac),
        }) else {
            panic!()
        };
        let rows: Vec<&Row> = sections.iter().flat_map(|s| &s.rows).collect();
        let at = rows.iter().position(|r| r.field == "env_vars").unwrap();
        assert_eq!(rows[at - 1].field, "env_files", "after Env files");
        assert_eq!(
            rows[at + 1].field,
            "warm_builds",
            "before the warm build switch"
        );
        let row = rows[at];
        assert_eq!(row.label, "Environment variables");
        assert_eq!(
            row.description,
            format!(
                "Set in every agent session, Run and Shell of this project. You can use ${{{}}} and {PRODUCT_NAME}’s other variables inside a value.",
                naming::db_name_env_var()
            )
        );
        row.control.clone()
    }

    fn env_rows(control: &Control) -> &[EnvVarRow] {
        let Control::EnvVars { rows, .. } = control else {
            panic!("{control:?}")
        };
        rows
    }

    fn vars(rows: &[(&str, &str)]) -> ProjectSettings {
        ProjectSettings {
            env_vars: rows.iter().map(|(n, v)| EnvVar::new(n, v)).collect(),
            ..ProjectSettings::default()
        }
    }

    #[test]
    fn env_vars_empty_shows_the_box_the_hint_and_the_applies_line() {
        let control = env_control(&ProjectSettings::default(), &[]);
        let Control::EnvVars {
            rows,
            hint_example,
            hint,
            empty,
            add_label,
            drop_label,
            applies,
            name_placeholder,
            value_placeholder,
        } = control
        else {
            panic!()
        };
        assert!(rows.is_empty(), "no built-in, read-only or computed rows");
        assert_eq!(hint_example, "CI = true");
        assert_eq!(
            hint,
            "e.g. so tools never wait for input or start watch mode"
        );
        assert_eq!(empty, "No variables. Sessions get your normal environment.");
        assert_eq!(add_label, "Add variable");
        assert_eq!(drop_label, "Remove variable");
        assert_eq!(
            applies,
            "Changes apply to sessions, Runs and Shells started after this."
        );
        assert_eq!(
            (name_placeholder.as_str(), value_placeholder.as_str()),
            ("NAME", "value")
        );
    }

    #[test]
    fn env_vars_rows_are_shown_in_order_in_full_with_their_remove_label() {
        let s = vars(&[
            ("NEXT_TELEMETRY_DISABLED", "1"),
            ("DATABASE_URL", "postgres://localhost:5432/shop_dev"),
        ]);
        let control = env_control(&s, &[]);
        let rows = env_rows(&control);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "NEXT_TELEMETRY_DISABLED");
        assert_eq!(
            rows[1].value, "postgres://localhost:5432/shop_dev",
            "plain, in full"
        );
        assert_eq!(rows[1].remove_label, "Remove DATABASE_URL");
        assert!(rows.iter().all(|r| r.problem.is_none()));
    }

    #[test]
    fn env_vars_each_problem_is_worded_as_the_frames_and_marks_only_its_row() {
        use otaq_core::tools::Os as Platform;
        let s = vars(&[
            ("NODE_OPTIONS", "--max-old-space-size=4096"),
            ("MAX=JOBS", "2"),
            ("", "3"),
            ("BAD\0NAME", "x"),
            ("NODE_OPTIONS", "--inspect"),
            ("1 SPACE OK", "y"),
        ]);
        let problems = env_var_setting_problems(&s.env_vars, Platform::Mac);
        let control = env_control(&s, &problems);
        let rows = env_rows(&control);
        assert_eq!(rows[0].problem, None, "the first NODE_OPTIONS applies");
        assert_eq!(
            rows[1].problem.as_deref(),
            Some("Remove “=” from the name. This row isn’t set until then.")
        );
        assert_eq!(
            rows[2].problem.as_deref(),
            Some("Add a name, or remove the row")
        );
        assert_eq!(rows[2].remove_label, "Remove variable");
        assert_eq!(
            rows[3].problem.as_deref(),
            Some("Remove the null character from the name. This row isn’t set until then.")
        );
        assert_eq!(
            rows[4].problem.as_deref(),
            Some(
                "NODE_OPTIONS is already set above. Only the first row applies; remove one of them."
            )
        );
        assert_eq!(
            rows[5].problem, None,
            "a space or a leading digit is allowed"
        );
    }

    #[test]
    fn env_vars_duplicates_ignore_case_on_windows_only() {
        use otaq_core::tools::Os as Platform;
        let s = vars(&[("NODE_OPTIONS", "a"), ("node_options", "b")]);
        for (os, marked) in [
            (Platform::Windows, true),
            (Platform::Mac, false),
            (Platform::Linux, false),
        ] {
            let problems = env_var_setting_problems(&s.env_vars, os);
            let control = env_control(&s, &problems);
            let rows = env_rows(&control);
            assert_eq!(rows[0].problem, None, "{os:?}");
            assert_eq!(
                rows[1].problem.as_deref(),
                marked.then_some(
                    "node_options is already set above. Only the first row applies; remove one of them."
                ),
                "{os:?}"
            );
        }
    }

    #[test]
    fn env_vars_edits_add_change_and_remove_rows() {
        let mut s = ProjectSettings::default();
        let var = |name: &str, value: &str| {
            Value::Var(EnvVarValue {
                name: name.into(),
                value: value.into(),
            })
        };
        assert_eq!(
            edit_project(&mut s, "env_vars.add", &var("", "1")).unwrap(),
            ProjectChange::Nothing,
            "a row with an empty name isn't saved"
        );
        assert!(s.env_vars.is_empty());
        edit_project(&mut s, "env_vars.add", &var("CI", "true")).unwrap();
        edit_project(&mut s, "env_vars.add", &var("MY LOG ", " a b ")).unwrap();
        assert_eq!(
            s.env_vars[1],
            EnvVar::new("MY LOG ", " a b "),
            "kept as typed"
        );
        edit_project(&mut s, "env_vars.0.value", &Value::Text("false".into())).unwrap();
        edit_project(&mut s, "env_vars.1.name", &Value::Text("".into())).unwrap();
        assert_eq!(
            s.env_vars,
            [EnvVar::new("CI", "false"), EnvVar::new("", " a b ")],
            "an emptied name stays, marked"
        );
        edit_project(&mut s, "env_vars.remove", &Value::Number(0)).unwrap();
        assert_eq!(s.env_vars, [EnvVar::new("", " a b ")]);
        edit_project(&mut s, "env_vars.remove", &Value::Number(7)).unwrap();
        assert_eq!(s.env_vars.len(), 1);
        assert!(edit_project(&mut s, "env_vars.4.name", &Value::Text("X".into())).is_err());
        assert!(edit_project(&mut s, "env_vars.0.other", &Value::Text("X".into())).is_err());
        assert!(edit_project(&mut s, "env_vars.add", &Value::Text("X".into())).is_err());
        let v: Value =
            serde_json::from_value(serde_json::json!({ "name": "CI", "value": "1" })).unwrap();
        assert_eq!(v, var("CI", "1"));
    }

    #[test]
    fn the_env_vars_gallery_has_every_frame_state() {
        let g = env_vars_gallery();
        let problem = |name: &str| {
            let cell = g.cells.iter().find(|c| c.name == name).unwrap();
            env_rows(&cell.row.control)
                .iter()
                .filter_map(|r| r.problem.clone())
                .collect::<Vec<_>>()
        };
        assert!(env_rows(&g.cells[0].row.control).is_empty());
        assert!(problem("a few rows").is_empty());
        assert_eq!(problem("invalid name").len(), 1);
        assert_eq!(problem("duplicate name").len(), 1);
        assert_eq!(problem("differing case · Windows").len(), 1);
        assert!(problem("differing case · macOS and Linux").is_empty());
    }

    const GB: u64 = 1024 * 1024 * 1024;
    const NOW: i64 = 1_760_000_000_000;

    fn shop() -> ProjectInfo {
        ProjectInfo {
            key: "SHOP".into(),
            name: "shop-web".into(),
            ..info()
        }
    }

    fn apfs() -> CloneSupport {
        CloneSupport {
            available: true,
            file_system: "apfs".into(),
            volume: "/".into(),
        }
    }

    fn shop_build(copying: &[&str], clearing: bool) -> WarmBuildInfo {
        WarmBuildInfo {
            repo: "shop-web".into(),
            path: "/data/warm/shop-web".into(),
            project: Some("SHOP".into()),
            size_bytes: 42 * GB / 10,
            built_at: Some("3e1f9a2".into()),
            from: Some("SHOP-20".into()),
            kept_at: Some(NOW - 2 * 3_600_000),
            status: otaq_core::warm::store::WarmStatus {
                copying: copying.iter().map(|t| (*t).to_owned()).collect(),
                clearing,
            },
            entries: vec![],
            limit: None,
        }
    }

    /// The switch row as the project page draws it, checked to sit right after Environment variables.
    fn warm_row(on: bool, support: CloneSupport, builds: Vec<WarmBuildInfo>) -> Row {
        let settings = ProjectSettings {
            warm_builds: on,
            ..ProjectSettings::default()
        };
        let list = WarmList {
            support,
            mode: Default::default(),
            builds,
        };
        let Page::Project { sections, .. } = project_page(&ProjectInputs {
            info: &shop(),
            settings: &settings,
            problems: &[],
            global: &GlobalSettings::default(),
            warm: Some(WarmInputs {
                list: &list,
                now: NOW,
            }),
            project_agent: "Claude Code",
            tool_connected: true,
            skills: SkillInputs::none(Os::Mac),
        }) else {
            panic!()
        };
        let rows: Vec<&Row> = sections.iter().flat_map(|s| &s.rows).collect();
        let at = rows.iter().position(|r| r.field == "warm_builds").unwrap();
        assert_eq!(
            rows[at - 1].field,
            "env_vars",
            "right after Environment variables"
        );
        assert_eq!(
            rows[at + 1].field,
            "resource_create",
            "the last of Commands, before Ticket resources"
        );
        rows[at].clone()
    }

    #[test]
    fn the_warm_build_switch_is_on_by_default_and_asks_only_when_it_would_delete_a_warm_build() {
        let row = warm_row(true, apfs(), Vec::new());
        assert_eq!(row.label, "Start new worktrees from a warm build");
        assert_eq!(
            row.description,
            "New tickets start from the last finished ticket’s build, so their first build only redoes what changed."
        );
        // On, with nothing to delete: turning it off asks nothing.
        assert_eq!(
            row.control,
            Control::WarmBuild {
                on: true,
                disabled: false,
                reason: None,
                clearing: None,
                confirm: None,
            }
        );
        // Another project's warm build isn't this one's.
        let other = WarmBuildInfo {
            project: Some("BILL".into()),
            ..shop_build(&[], false)
        };
        let Control::WarmBuild { confirm, .. } = warm_row(true, apfs(), vec![other]).control else {
            panic!()
        };
        assert_eq!(confirm, None);

        // With a warm build: the confirmation, "up to" the size, and the shared-space line.
        let Control::WarmBuild { on, confirm, .. } =
            warm_row(true, apfs(), vec![shop_build(&[], false)]).control
        else {
            panic!()
        };
        assert!(on);
        let confirm = confirm.expect("asks first");
        assert_eq!(confirm.title, "Clear shop-web’s warm build (up to 4.2 GB)?");
        assert_eq!(
            confirm.text,
            "Turning this off deletes shop-web’s warm build, built at 3e1f9a2, 2 hours ago. New tickets will run \
             setup and build from nothing. Ticket worktrees and your repo aren’t touched."
        );
        assert_eq!(
            confirm.shared,
            "Tickets copied from it share part of this space, so it may free less."
        );
        assert_eq!(
            confirm.line.text,
            "/data/warm/shop-web · up to 4.2 GB (delete)"
        );
        assert_eq!(confirm.line.icon, IconName::Flame);
        assert_eq!(confirm.confirm, "Turn off and clear up to 4.2 GB");

        // Off: turning it on never asks.
        let Control::WarmBuild { on, confirm, .. } = warm_row(false, apfs(), Vec::new()).control
        else {
            panic!()
        };
        assert!(!on);
        assert_eq!(confirm, None);
    }

    #[test]
    fn the_warm_build_switch_says_when_its_clear_waits_for_a_copy() {
        let Control::WarmBuild {
            on,
            clearing,
            confirm,
            ..
        } = warm_row(false, apfs(), vec![shop_build(&["SHOP-21"], true)]).control
        else {
            panic!()
        };
        assert!(!on);
        assert_eq!(
            clearing.as_deref(),
            Some("Clearing… (waiting for SHOP-21’s copy)")
        );
        assert_eq!(confirm, None);
        // A seed running without a Clear waiting: nothing to say.
        let Control::WarmBuild { clearing, .. } =
            warm_row(true, apfs(), vec![shop_build(&["SHOP-21"], false)]).control
        else {
            panic!()
        };
        assert_eq!(clearing, None);
    }

    #[test]
    fn where_the_disk_cant_clone_the_switch_works_and_the_build_is_handed_over() {
        let cannot = |file_system: &str, volume: &str| CloneSupport {
            available: false,
            file_system: file_system.into(),
            volume: volume.into(),
        };
        for support in [
            cannot("NTFS", "C:"),
            cannot("ext4", "/home"),
            cannot("exfat", "/Volumes/Untitled"),
            cannot("", ""),
        ] {
            // The switch is the stored setting, enabled; the description says the build is renamed (M19-b).
            let row = warm_row(true, support.clone(), Vec::new());
            assert_eq!(
                row.description,
                "This disk can’t copy-on-write; the build is handed over to the next ticket."
            );
            assert_eq!(
                row.control,
                Control::WarmBuild {
                    on: true,
                    disabled: false,
                    reason: None,
                    clearing: None,
                    confirm: None,
                }
            );
            let Control::WarmBuild { on, .. } =
                warm_row(false, support.clone(), Vec::new()).control
            else {
                panic!()
            };
            assert!(!on);
            // Turning it off with a build in the stock asks first, as where the disk can clone.
            let Control::WarmBuild { confirm, .. } =
                warm_row(true, support, vec![shop_build(&[], false)]).control
            else {
                panic!()
            };
            assert!(confirm.is_some());
        }
    }

    #[test]
    fn the_warm_build_switch_saves_as_a_flag() {
        let mut s = ProjectSettings::default();
        assert!(s.warm_builds);
        let change = edit_project(&mut s, "warm_builds", &Value::Flag(false)).unwrap();
        assert_eq!(change, ProjectChange::Settings);
        assert!(!s.warm_builds);
        edit_project(&mut s, "warm_builds", &Value::Flag(true)).unwrap();
        assert!(s.warm_builds);
        assert!(edit_project(&mut s, "warm_builds", &Value::Text("off".into())).is_err());

        let gallery = warm_switch_gallery();
        assert_eq!(gallery.cells.len(), 7);
    }
}
