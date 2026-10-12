//! Settings as the Tauri page draws it (T-0234; SET-01, PRJ-03, PRJ-04, SCH-03, USG-02, ATT-11, REV-09, REV-11, MSG-05,
//! UI-11; *15 · Settings*): the nav, the global page and each project page as sections of rows, every row with its
//! label, description and control already worded, and the rules for applying an edit ([`edit_global`],
//! [`edit_project`]). The page only lays the rows out and sends back `field` + value; the service still flags invalid
//! values (a missing env file, a URL that isn't one, a skill that isn't `/name`), shown in red under their field.
//!
//! Section ids are the page registry's (`ui/src/app/registry/settings.ts`): the Settings screen draws the sections
//! it has registered, so a later ticket adds a section here and registers its id there (T-0240: Validation &
//! Auto-merge, Project agent instructions).

use crewboard_core::board::projects::ProjectUpdate;
use crewboard_core::board::settings::{
    GlobalSettings, ProjectSettings, SettingProblem, TERMINAL_FONT_SIZES, TerminalRenderer,
    WorkflowSkills, WorkflowStep,
};
use crewboard_core::board::snapshot::ProjectInfo;
use crewboard_core::naming::{COMMAND_NAME, PRODUCT_NAME};
use crewboard_core::pr::provider::PrProvider;
use crewboard_core::scheduler::SchedulerStatus;
use crewboard_core::tools::ToolsReport;
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
use crate::view_models::limits::{
    BACK_PRESSURE_ITEMS, MAX_AGENTS, right_now, settings_rows, toggled,
};
use crate::view_models::onboarding::{OnboardingModel, RowIcon};
use crate::view_models::settings::{
    GLOBAL_SECTIONS, PROJECT_ICON, ProviderTone, env_file_problem, instructions_placeholder,
    problem_for, provider_row, remove_text,
};
use crate::view_models::theme_credits;

/// A section of the global page; its id is the page registry's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "GlobalSectionId")]
pub enum GlobalSection {
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
    Tools,
}

/// A section of a project page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "ProjectSectionId")]
pub enum ProjectSection {
    /// Name, key, repository, base branch.
    General,
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
    PrProvider,
    RemoveProject,
}

/// Where a nav row goes: the global page at one of its sections, or a project's page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "page", rename_all = "snake_case")]
#[ts(rename = "SettingsTarget")]
pub enum Target {
    Global { section: GlobalSection },
    Project { key: String },
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

/// A workflow skill's field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsSkillRow")]
pub struct SkillRow {
    pub field: String,
    pub label: String,
    pub value: String,
    pub placeholder: String,
    pub problem: Option<String>,
}

/// One env file, its field `env_files.<index>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsEnvFile")]
pub struct EnvFileRow {
    pub value: String,
    pub problem: Option<String>,
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
    Stepper {
        value: u32,
        min: u32,
        max: u32,
        /// " px" after the number.
        suffix: String,
        disabled: bool,
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
    Skills {
        rows: Vec<SkillRow>,
    },
    /// ↵ in the last field adds a file (`env_files.add`); × removes one (`env_files.remove`).
    EnvFiles {
        rows: Vec<EnvFileRow>,
        add_placeholder: String,
        remove_label: String,
    },
    /// The base branch: a dropdown of the repository's branches (`settings.branches`).
    Branch {
        value: String,
    },
    /// The 15 themes (`theme.picker`, `theme.pick`).
    Themes,
    /// The PR provider, detected when the page opens (`settings.provider`).
    Provider,
    /// The tools checklist (`settings.tools`) and Re-check.
    Tools {
        recheck_label: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SettingsReveal")]
pub enum Reveal {
    Database,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SettingsButtonAction")]
pub enum ButtonAction {
    /// Opens Recovery and Cleanup.
    Cleanup,
    /// Opens the Theme credits dialog.
    Credits,
    /// Asks before removing the project.
    RemoveProject,
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
    /// A button after the control: Run command's **Ask Claude** (T-0232), which opens the run-command dialog.
    pub button: Option<RowButton>,
}

/// A row's button and what it opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsRowButton")]
pub struct RowButton {
    pub label: String,
    pub icon: IconName,
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

/// The page on the right: "General" (the global sections, one scrolling page) or a project.
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
}

/// What a project page is built from.
pub struct ProjectInputs<'a> {
    pub info: &'a ProjectInfo,
    pub settings: &'a ProjectSettings,
    pub problems: &'a [SettingProblem],
    pub global: &'a GlobalSettings,
}

/// The nav labels and icons of the global sections this ticket draws (`settings::GLOBAL_SECTIONS` order).
const DRAWN: [GlobalSection; 8] = [
    GlobalSection::AgentLimits,
    GlobalSection::AgentInstructions,
    GlobalSection::WorkflowSkills,
    GlobalSection::Usage,
    GlobalSection::Notifications,
    GlobalSection::Data,
    GlobalSection::Appearance,
    GlobalSection::Tools,
];

/// The nav: every global section the page has, then every project.
pub fn nav(sections: &[GlobalSection], projects: &[ProjectInfo], alerts: &[String]) -> Vec<NavRow> {
    let globals = sections.iter().filter_map(|id| {
        let i = DRAWN.iter().position(|d| d == id)?;
        let (label, icon) = GLOBAL_SECTIONS[i];
        Some(NavRow {
            label: label.to_owned(),
            icon,
            target: Target::Global { section: *id },
            alert: false,
        })
    });
    let projects = projects.iter().map(|p| NavRow {
        label: p.name.clone(),
        icon: PROJECT_ICON,
        target: Target::Project { key: p.key.clone() },
        alert: alerts.contains(&p.key),
    });
    globals.chain(projects).collect()
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
            format!(
                "Given to every agent in every project through {COMMAND_NAME} prime (0040). A project can override it in its own settings."
            ),
            multiline(
                &s.agent_instructions,
                "Your rules for every agent, e.g. “Ask before coding.”",
            ),
        )],
    );

    let defaults = WorkflowSkills::default();
    let skills = section(
        GlobalSection::WorkflowSkills,
        Some("Workflow skills"),
        vec![row(
            "",
            "Workflow skills",
            "Default skills for Commit, Push, Open PR and Validate (0063). Projects can override each one.",
            Control::Skills {
                rows: WorkflowStep::ALL
                    .into_iter()
                    .map(|step| {
                        skill_row(
                            step,
                            s.workflow_skills.get(step),
                            defaults.get(step),
                            input.problems,
                        )
                    })
                    .collect(),
            },
        )],
    );

    let usage = section(
        GlobalSection::Usage,
        Some("Usage"),
        vec![
            row(
                "",
                "Token count",
                "Always shown in the header (today, all projects) and per ticket.",
                Control::Note {
                    text: "Always on".into(),
                },
            ),
            row(
                "dollar_estimate",
                "Dollar estimate",
                "For API-key users. Off by default. Shown as “≈ $38.20 today” next to tokens; it’s an estimate.",
                Control::Toggle {
                    on: s.dollar_estimate,
                },
            ),
        ],
    );

    let notifications = section(
        GlobalSection::Notifications,
        Some("Notifications"),
        vec![row(
            "notifications",
            "OS notifications for blocking items",
            "Permission prompts, agent questions and interrupted sessions. Nothing else ever notifies.",
            Control::Toggle {
                on: s.notifications,
            },
        )],
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
        let folder = logs.trim_end_matches('/').trim_end_matches("logs");
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
        intro: Some(format!(
            "{PRODUCT_NAME} uses the tools already on your machine. It never installs or updates them."
        )),
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
            agent_limits,
            instructions,
            skills,
            usage,
            notifications,
            data,
            appearance,
            tools,
        ],
    }
}

fn skill_row(
    step: WorkflowStep,
    value: &str,
    placeholder: &str,
    problems: &[SettingProblem],
) -> SkillRow {
    let field = format!("workflow_skills.{}", step.field());
    SkillRow {
        problem: problem_for(problems, &field),
        field,
        label: step_label(step).to_owned(),
        value: value.to_owned(),
        placeholder: placeholder.to_owned(),
    }
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
            format!(
                "Per-project override, given to every agent in this project through {COMMAND_NAME} prime. Empty = the global default is used."
            ),
            multiline(
                &s.agent_instructions,
                &instructions_placeholder(input.global),
            ),
        )],
    );
    let skills = section(
        ProjectSection::WorkflowSkills,
        None,
        vec![row(
            "",
            "Workflow skills",
            "Commit, Push, Open PR and Validate run these skills. Replace one with your own (e.g. /my-pr). Empty = the global default.",
            Control::Skills {
                rows: WorkflowStep::ALL
                    .into_iter()
                    .map(|step| skill_row(step, s.workflow_skills.get(step), "Use global", p))
                    .collect(),
            },
        )],
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
                button: Some(RowButton {
                    label: "Ask Claude".into(),
                    icon: IconName::Sparkles,
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
        ],
    );
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
        sections: vec![general, instructions, skills, answers, commands, provider, remove],
    }
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
    pub name: String,
    pub icon: IconName,
    pub look: ToolLook,
    /// "required" / "optional".
    pub need: String,
    pub status: String,
    pub explanation: Option<String>,
    pub command: Option<String>,
}

/// The tools checklist (`settings.tools`), or why the check failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SettingsTools")]
pub struct ToolsView {
    pub rows: Vec<ToolView>,
    pub error: Option<String>,
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
        "dollar_estimate" => s.dollar_estimate = value.flag()?,
        "notifications" => s.notifications = value.flag()?,
        "terminal_font_size" => s.terminal_font_size = clamp(value.number()?, TERMINAL_FONT_SIZES),
        "terminal_renderer" => {
            s.terminal_renderer = match value.text()? {
                "webgl" => TerminalRenderer::Webgl,
                "dom" => TerminalRenderer::Dom,
                other => return Err(format!("unknown terminal renderer: {other}")),
            }
        }
        f => match step_of(f) {
            Some(step) => {
                let v = value.text()?.trim().to_owned();
                let skills = &mut s.workflow_skills;
                *match step {
                    WorkflowStep::Commit => &mut skills.commit,
                    WorkflowStep::Push => &mut skills.push,
                    WorkflowStep::OpenPr => &mut skills.open_pr,
                    WorkflowStep::Validate => &mut skills.validate,
                } = v;
            }
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
        "setup_command" => s.setup_command = value.text()?.to_owned(),
        "run_command" => s.run_command = value.text()?.to_owned(),
        "open_url" => s.open_url = value.text()?.trim().to_owned(),
        "let_finished_tickets_answer" => s.let_finished_tickets_answer = value.flag()?,
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
            } else if let Some(step) = step_of(f) {
                let v = value.text()?.trim().to_owned();
                let skills = &mut s.workflow_skills;
                *match step {
                    WorkflowStep::Commit => &mut skills.commit,
                    WorkflowStep::Push => &mut skills.push,
                    WorkflowStep::OpenPr => &mut skills.open_pr,
                    WorkflowStep::Validate => &mut skills.validate,
                } = v;
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
        };
        match global_page(&input) {
            Page::Global { sections, .. } => sections,
            Page::Project { .. } => unreachable!(),
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
        assert_eq!(nav.len(), 9);
        assert_eq!(nav[0].label, "Agent limits");
        assert_eq!(nav[8].target, Target::Project { key: "BILL".into() });
        assert!(nav[8].alert);
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
        };
        let Page::Global { sections, .. } = global_page(&input) else {
            unreachable!()
        };
        assert!(
            sections[0]
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
        edit_global(
            &mut s,
            "workflow_skills.open_pr",
            &Value::Text(" /my-pr ".into()),
        )
        .unwrap();
        assert_eq!(s.workflow_skills.open_pr, "/my-pr");
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
                message: ".env.local doesn't exist in the repository".into(),
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
        });
        let Page::Project {
            title, sections, ..
        } = page
        else {
            panic!()
        };
        assert_eq!(title, "billing-api");
        let rows: Vec<&Row> = sections.iter().flat_map(|s| &s.rows).collect();
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
        edit_project(&mut s, "workflow_skills.commit", &Value::Text("/c".into())).unwrap();
        assert_eq!(s.workflow_skills.commit, "/c");
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
        assert_eq!(json["page"]["sections"][0]["id"], "agent_limits");
        assert_eq!(
            json["page"]["sections"][0]["rows"][0]["control"]["kind"],
            "toggle"
        );
        assert_eq!(
            json["nav"][0]["target"],
            serde_json::json!({ "page": "global", "section": "agent_limits" })
        );
        let v: Value = serde_json::from_value(serde_json::json!(true)).unwrap();
        assert_eq!(v, Value::Flag(true));
        let v: Value = serde_json::from_value(serde_json::json!(4)).unwrap();
        assert_eq!(v, Value::Number(4));
    }
}
