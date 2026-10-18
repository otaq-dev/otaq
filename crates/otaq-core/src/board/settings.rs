//! Typed settings, global and per project (SET-01; d0003, d0023; SDD §20). Everything lives in the app's
//! database and nothing is ever written into a repository (d0023). Invalid values are stored as given and
//! flagged ([`SettingProblem`]), never silently fixed.
//!
//! Global settings are rows of the `settings` table, one per top-level field below (JSON values; a missing row
//! means the default). Project settings are the project's `settings_json`. Empty per-project values fall back
//! to the global ones (agent instructions, workflow skills, validation, rounds, Auto-merge, Project agent
//! instructions; d0040, d0050, d0063, d0064, d0068).
//!
//! The features that use these values arrive in their own milestones (limits T-0080, notifications T-0117,
//! Run T-0133, workflow skills T-0139, "let finished tickets answer" T-0153).

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{Board, BoardError};
use crate::attention::notify::Kind;
use crate::data_dir::DataDir;
use crate::domain::stored_enum;
use crate::naming;
use crate::store::now_ms;
use crate::tools::Os;
pub use crate::workflow_skills::{ImportedSkill, ResolvedSkill, ResolvedSkills};

/// The optional agent limit and back-pressure (d0016): off by default; when on, at most 5 agents, and new starts
/// wait while 3 items need the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentLimits {
    pub enabled: bool,
    pub max_concurrency: u32,
    pub back_pressure: u32,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            enabled: false,
            max_concurrency: 5,
            back_pressure: 3,
        }
    }
}

/// One workflow skill setting (WSK-01, d0092): the global default of a step, or one project's override of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SkillSetting {
    /// Otaq's built-in for the step. Stored as Otaq's own default (`/otaq:commit`), as before d0092.
    #[default]
    BuiltIn,
    /// A project's step that follows the global setting (stored empty). Read as the built-in in the global
    /// settings.
    UseGlobal,
    /// An imported skill folder; agents use Otaq's copy of it (WSK-03). Boxed: the settings travel in requests.
    Imported(Box<ImportedSkill>),
    /// A typed name stored before d0092 that isn't Otaq's own (`/my-commit`): "Not imported" (WSK-07). Never
    /// used to run anything, and never stored anew.
    Legacy(String),
}

impl SkillSetting {
    /// The imported skill, if this setting holds one.
    pub fn imported(&self) -> Option<&ImportedSkill> {
        match self {
            Self::Imported(skill) => Some(skill),
            _ => None,
        }
    }

    /// How a stored text reads for `step` (WSK-07): Otaq's own default (`/otaq:commit`) is the built-in, empty is
    /// `empty`, anything else a legacy value.
    fn from_text(text: &str, step: WorkflowStep, empty: Self) -> Self {
        let text = text.trim();
        if text.is_empty() {
            empty
        } else if text == step.own_default() {
            Self::BuiltIn
        } else {
            Self::Legacy(text.to_owned())
        }
    }

    fn into_stored(self, step: WorkflowStep) -> StoredSkill {
        match self {
            Self::BuiltIn => StoredSkill::Text(step.own_default()),
            Self::UseGlobal => StoredSkill::Text(String::new()),
            Self::Legacy(text) => StoredSkill::Text(text),
            Self::Imported(skill) => StoredSkill::Imported(skill),
        }
    }
}

/// A workflow skill setting as stored in the settings JSON: a text (Otaq's own default, empty, or a typed name
/// from before d0092) or an imported skill's record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StoredSkill {
    Imported(Box<ImportedSkill>),
    Text(String),
}

impl Default for StoredSkill {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

/// The four settings as stored, for both the global and a project's.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct StoredSkills {
    commit: StoredSkill,
    push: StoredSkill,
    open_pr: StoredSkill,
    validate: StoredSkill,
}

impl StoredSkills {
    fn read(self, empty: SkillSetting) -> [SkillSetting; 4] {
        let read = |stored: StoredSkill, step| match stored {
            StoredSkill::Imported(skill) => SkillSetting::Imported(skill),
            StoredSkill::Text(text) => SkillSetting::from_text(&text, step, empty.clone()),
        };
        [
            read(self.commit, WorkflowStep::Commit),
            read(self.push, WorkflowStep::Push),
            read(self.open_pr, WorkflowStep::OpenPr),
            read(self.validate, WorkflowStep::Validate),
        ]
    }

    fn write([commit, push, open_pr, validate]: [SkillSetting; 4]) -> Self {
        Self {
            commit: commit.into_stored(WorkflowStep::Commit),
            push: push.into_stored(WorkflowStep::Push),
            open_pr: open_pr.into_stored(WorkflowStep::OpenPr),
            validate: validate.into_stored(WorkflowStep::Validate),
        }
    }
}

/// The global Commit, Push, Open PR and Validate workflow skills (d0050, d0063, d0092). A missing or empty value is
/// the built-in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredSkills", into = "StoredSkills")]
pub struct WorkflowSkills {
    pub commit: SkillSetting,
    pub push: SkillSetting,
    pub open_pr: SkillSetting,
    /// What the validator runs (d0063).
    pub validate: SkillSetting,
}

impl From<StoredSkills> for WorkflowSkills {
    fn from(stored: StoredSkills) -> Self {
        let [commit, push, open_pr, validate] = stored.read(SkillSetting::BuiltIn);
        Self {
            commit,
            push,
            open_pr,
            validate,
        }
    }
}

impl From<WorkflowSkills> for StoredSkills {
    fn from(s: WorkflowSkills) -> Self {
        Self::write([s.commit, s.push, s.open_pr, s.validate])
    }
}

impl WorkflowSkills {
    /// Otaq's own workflow skill that a stored value names, recognised by today's defaults as Settings shows
    /// them: `/otaq:commit` → `commit` (ADP-05). `None` for a skill the user chose (`/my-commit`).
    pub fn own_skill_of(stored: &str) -> Option<&'static str> {
        let stored = stored.trim();
        WorkflowStep::ALL
            .into_iter()
            .map(WorkflowStep::own_skill)
            .find(|&name| naming::skill(name) == stored)
    }
}

/// A workflow step whose skill the user can replace (d0050, d0063). On the wire as its settings field (`open_pr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStep {
    Commit,
    Push,
    OpenPr,
    Validate,
}

impl WorkflowStep {
    pub const ALL: [WorkflowStep; 4] = [Self::Commit, Self::Push, Self::OpenPr, Self::Validate];

    /// The settings field: `commit`, `push`, `open_pr`, `validate`.
    pub fn field(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Push => "push",
            Self::OpenPr => "open_pr",
            Self::Validate => "validate",
        }
    }

    /// Otaq's own skill for the step as stored and as Settings shows it: `/otaq:commit` (the adapter names it to
    /// the agent in its tool's syntax).
    pub fn own_default(self) -> String {
        naming::skill(self.own_skill())
    }

    /// Otaq's own skill for the step, by its name in the plugin: `commit`, `push`, `open-pr`, `validate`.
    pub fn own_skill(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Push => "push",
            Self::OpenPr => "open-pr",
            Self::Validate => "validate",
        }
    }
}

impl WorkflowSkills {
    pub fn get(&self, step: WorkflowStep) -> &SkillSetting {
        match step {
            WorkflowStep::Commit => &self.commit,
            WorkflowStep::Push => &self.push,
            WorkflowStep::OpenPr => &self.open_pr,
            WorkflowStep::Validate => &self.validate,
        }
    }

    pub fn get_mut(&mut self, step: WorkflowStep) -> &mut SkillSetting {
        match step {
            WorkflowStep::Commit => &mut self.commit,
            WorkflowStep::Push => &mut self.push,
            WorkflowStep::OpenPr => &mut self.open_pr,
            WorkflowStep::Validate => &mut self.validate,
        }
    }
}

impl WorkflowSkillOverrides {
    pub fn get(&self, step: WorkflowStep) -> &SkillSetting {
        match step {
            WorkflowStep::Commit => &self.commit,
            WorkflowStep::Push => &self.push,
            WorkflowStep::OpenPr => &self.open_pr,
            WorkflowStep::Validate => &self.validate,
        }
    }

    pub fn get_mut(&mut self, step: WorkflowStep) -> &mut SkillSetting {
        match step {
            WorkflowStep::Commit => &mut self.commit,
            WorkflowStep::Push => &mut self.push,
            WorkflowStep::OpenPr => &mut self.open_pr,
            WorkflowStep::Validate => &mut self.validate,
        }
    }
}

stored_enum! {
    /// Auto-merge (d0064): validated tickets Local-merge without a click. Off by default.
    #[derive(Default)]
    pub enum AutoMerge ("auto-merge") {
        /// A validated ticket waits in Ready for Review.
        #[default]
        Off = "off",
        /// A validated epic child merges into its epic branch (local, never pushed).
        EpicChildren = "epic_children",
        /// Standalone tickets merge into their base branch too (local, never pushed).
        All = "all",
    }
}

impl AutoMerge {
    /// The settings screen's words (*15 · Settings*).
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::EpicChildren => "Epic children only",
            Self::All => "All tickets",
        }
    }
}

/// Validation rounds the user may pick (d0063); anything else is flagged and never used.
pub const VALIDATION_ROUNDS: std::ops::RangeInclusive<u32> = 1..=5;
/// The rounds when none is set.
pub const DEFAULT_VALIDATION_ROUNDS: u32 = 3;

/// The imported skill in force for a step (WSK-07), before its copy is checked: the project's own import; for a
/// project step that uses the global one or holds a legacy value, the global import; `None` for the built-in (a
/// global built-in or legacy value, or a project's step set to Otaq's own default).
pub fn skill_in_force<'a>(
    global: &'a WorkflowSkills,
    own: &'a WorkflowSkillOverrides,
    step: WorkflowStep,
) -> Option<&'a ImportedSkill> {
    match own.get(step) {
        SkillSetting::Imported(skill) => Some(skill),
        SkillSetting::BuiltIn => None,
        SkillSetting::UseGlobal | SkillSetting::Legacy(_) => global.get(step).imported(),
    }
}

/// The skill a step runs (REV-09, WSK-07): [`skill_in_force`] when Otaq's copy of it (under `data`) is present and
/// valid, else the step's built-in (WSK-04: a missing or broken copy means the built-in).
pub fn resolve_skill(
    global: &WorkflowSkills,
    own: &WorkflowSkillOverrides,
    step: WorkflowStep,
    data: &DataDir,
) -> ResolvedSkill {
    skill_in_force(global, own, step)
        .and_then(|skill| {
            let folder = data.skill_path(&skill.id).ok()?;
            crate::workflow_skills::copy_problem(&folder)
                .is_none()
                .then(|| ResolvedSkill::Imported {
                    name: skill.name.clone(),
                    folder,
                })
        })
        .unwrap_or(ResolvedSkill::BuiltIn(step))
}

stored_enum! {
    /// One of the 15 bundled themes (d0069, d0070), stored as its name: the design's palette key, with Otaq's
    /// two versions told apart. The dark themes first, then the light ones, as the pickers list them.
    pub enum Theme ("theme") {
        OtaqDark = naming::THEME_DARK,
        CatppuccinFrappe = "catppuccin-frappe",
        CatppuccinMacchiato = "catppuccin-macchiato",
        CatppuccinMocha = "catppuccin-mocha",
        TokyoNight = "tokyo-night",
        Dracula = "dracula",
        Nord = "nord",
        GruvboxDark = "gruvbox-dark",
        RosePine = "rose-pine",
        OneDark = "one-dark",
        OtaqLight = naming::THEME_LIGHT,
        CatppuccinLatte = "catppuccin-latte",
        Daylight = "daylight",
        SolarizedLight = "solarized-light",
        RosePineDawn = "rose-pine-dawn",
    }
}

impl Theme {
    /// Whether the theme draws a dark window.
    pub fn is_dark(self) -> bool {
        !matches!(
            self,
            Self::OtaqLight
                | Self::CatppuccinLatte
                | Self::Daylight
                | Self::SolarizedLight
                | Self::RosePineDawn
        )
    }

    /// A theme stored by d0069 as a dark or light theme, where both Otaq versions were `otaq`. A name
    /// this version doesn't know, or one of the other kind, reads as Otaq.
    fn from_0069(value: Option<&Value>, dark: bool) -> Self {
        let otaq = if dark {
            Self::OtaqDark
        } else {
            Self::OtaqLight
        };
        match value.and_then(Value::as_str) {
            Some(naming::THEME_KEY) | None => otaq,
            Some(name) => name
                .parse()
                .ok()
                .filter(|theme: &Self| theme.is_dark() == dark)
                .unwrap_or(otaq),
        }
    }
}

/// The theme setting (d0070): one theme, used all the time. Until one is picked (a fresh install, or System mode
/// from before d0070) it holds a theme for a dark OS and one for a light OS; the app picks by the OS at its next
/// start and stores that theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeSetting {
    Picked(Theme),
    ByOs { dark: Theme, light: Theme },
}

impl Default for ThemeSetting {
    /// Otaq Dark on a dark OS, Otaq Light on a light one.
    fn default() -> Self {
        Self::ByOs {
            dark: Theme::OtaqDark,
            light: Theme::OtaqLight,
        }
    }
}

impl ThemeSetting {
    /// The theme to use, given whether the OS is dark.
    pub fn resolve(self, os_dark: bool) -> Theme {
        match self {
            Self::Picked(theme) => theme,
            Self::ByOs { dark, light } => {
                if os_dark {
                    dark
                } else {
                    light
                }
            }
        }
    }

    /// The setting d0069 stored as a mode (`appearance`) and two themes: Dark and Light mode keep the theme they
    /// showed, System mode keeps both until the OS decides. `None` when nothing of it was stored.
    fn from_0069(stored: &Map<String, Value>) -> Option<Self> {
        if !THEME_KEYS_0069.iter().any(|key| stored.contains_key(*key)) {
            return None;
        }
        let dark = Theme::from_0069(stored.get("dark_theme"), true);
        let light = Theme::from_0069(stored.get("light_theme"), false);
        Some(match stored.get("appearance").and_then(Value::as_str) {
            Some("dark") => Self::Picked(dark),
            Some("light") => Self::Picked(light),
            _ => Self::ByOs { dark, light },
        })
    }
}

/// The settings rows d0069 used for the theme, replaced by `theme` (d0070).
const THEME_KEYS_0069: [&str; 3] = ["appearance", "dark_theme", "light_theme"];

/// Settings for every project (SDD §20, left column).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettings {
    pub agent_limits: AgentLimits,
    /// The user's rules for agents, printed by `otaq prime` (d0040). Empty by default.
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkills,
    /// A separate validator checks every submit (d0063). On by default.
    pub validation: bool,
    /// Rounds before "Validation failed N times" (d0063): 1–5, 3 by default.
    pub validation_rounds: u32,
    /// Off by default (d0064).
    pub auto_merge: AutoMerge,
    /// What every Project agent follows, printed by `otaq prime --project` (d0068). Empty by default.
    pub project_agent_instructions: String,
    /// OS notifications for items that block an agent (d0016). On by default.
    pub notifications: bool,
    /// One checkbox per kind of notification under the main switch (d0090): all on by default.
    pub notification_kinds: NotificationKinds,
    /// Notifications play the OS's default sound (d0090). On by default.
    pub notification_sound: bool,
    /// One theme, used all the time (d0070).
    pub theme: ThemeSetting,
    pub terminal_font_size: u32,
    /// How terminals draw (Q37, Q38): one setting for every terminal.
    pub terminal_renderer: TerminalRenderer,
    /// The description editor shows the raw Markdown source instead of formatted text (d0078 D9): every editor opens
    /// in the mode last chosen with its "Markdown" toggle. Off by default; not a row of the Settings page.
    pub description_source_mode: bool,
    /// Anonymous usage data (d0085): asked once, after first run; nothing is ever sent unless the answer was yes.
    pub usage_data: UsageData,
}

/// The terminal font size when none is set.
pub const DEFAULT_TERMINAL_FONT_SIZE: u32 = 13;
/// The terminal font sizes accepted without a warning.
pub const TERMINAL_FONT_SIZES: std::ops::RangeInclusive<u32> = 8..=32;

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            agent_limits: AgentLimits::default(),
            agent_instructions: String::new(),
            workflow_skills: WorkflowSkills::default(),
            validation: true,
            validation_rounds: DEFAULT_VALIDATION_ROUNDS,
            auto_merge: AutoMerge::Off,
            project_agent_instructions: String::new(),
            notifications: true,
            notification_kinds: NotificationKinds::default(),
            notification_sound: true,
            theme: ThemeSetting::default(),
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
            terminal_renderer: TerminalRenderer::default(),
            description_source_mode: false,
            usage_data: UsageData::NotAsked,
        }
    }
}

/// Which kinds of OS notification go out (d0090, one Settings checkbox each). An unticked kind still turns turquoise
/// and lands in the inbox; only the OS notification is left out. All on by default, also for settings stored before
/// the checkboxes existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationKinds {
    pub permission: bool,
    pub question: bool,
    pub turn_ended: bool,
    pub interrupted: bool,
    pub validation_decision: bool,
    pub auto_merge_stopped: bool,
    pub tool_unavailable: bool,
}

impl Default for NotificationKinds {
    fn default() -> Self {
        Self {
            permission: true,
            question: true,
            turn_ended: true,
            interrupted: true,
            validation_decision: true,
            auto_merge_stopped: true,
            tool_unavailable: true,
        }
    }
}

impl NotificationKinds {
    /// The checkbox of `kind`.
    pub fn allows(&self, kind: Kind) -> bool {
        match kind {
            Kind::Permission => self.permission,
            Kind::Question => self.question,
            Kind::TurnEnded => self.turn_ended,
            Kind::Interrupted => self.interrupted,
            Kind::ValidationDecision => self.validation_decision,
            Kind::AutoMergeStopped => self.auto_merge_stopped,
            Kind::ToolUnavailable => self.tool_unavailable,
        }
    }

    /// Ticks or unticks the checkbox of `kind`.
    pub fn set(&mut self, kind: Kind, on: bool) {
        let slot = match kind {
            Kind::Permission => &mut self.permission,
            Kind::Question => &mut self.question,
            Kind::TurnEnded => &mut self.turn_ended,
            Kind::Interrupted => &mut self.interrupted,
            Kind::ValidationDecision => &mut self.validation_decision,
            Kind::AutoMergeStopped => &mut self.auto_merge_stopped,
            Kind::ToolUnavailable => &mut self.tool_unavailable,
        };
        *slot = on;
    }
}

/// Whether the app may send anonymous usage data (d0085). Until the user answers, it never sends anything.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageData {
    /// Not asked yet: the "Help improve Otaq" dialog shows once; nothing is sent meanwhile.
    #[default]
    NotAsked,
    On,
    Off,
}

impl UsageData {
    /// Only an explicit yes sends anything.
    pub fn sends(self) -> bool {
        self == Self::On
    }
}

/// The terminal renderer: xterm.js's two (UIT-05). WebGL draws on the GPU and keeps the CPU low; DOM draws with the
/// page's elements. A broken WebGL driver draws garbage without any error (the M14 test build, in a Linux VM), so
/// nothing falls back by itself: this setting picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalRenderer {
    Webgl,
    Dom,
}

impl Default for TerminalRenderer {
    /// DOM on Linux, WebGL on macOS and Windows (Q37, deferred as option 1). The service and the app run on the same
    /// machine, so the service's platform is the app's.
    fn default() -> Self {
        if cfg!(target_os = "linux") {
            Self::Dom
        } else {
            Self::Webgl
        }
    }
}

/// Per-project workflow skill overrides (d0092); a missing or empty value means "use the global one".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredSkills", into = "StoredSkills")]
pub struct WorkflowSkillOverrides {
    pub commit: SkillSetting,
    pub push: SkillSetting,
    pub open_pr: SkillSetting,
    pub validate: SkillSetting,
}

impl Default for WorkflowSkillOverrides {
    fn default() -> Self {
        StoredSkills::default().into()
    }
}

impl From<StoredSkills> for WorkflowSkillOverrides {
    fn from(stored: StoredSkills) -> Self {
        let [commit, push, open_pr, validate] = stored.read(SkillSetting::UseGlobal);
        Self {
            commit,
            push,
            open_pr,
            validate,
        }
    }
}

impl From<WorkflowSkillOverrides> for StoredSkills {
    fn from(s: WorkflowSkillOverrides) -> Self {
        Self::write([s.commit, s.push, s.open_pr, s.validate])
    }
}

/// Settings of one project (SDD §20, right column). Name and base branch are project fields (`update_project`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectSettings {
    pub setup_command: String,
    pub run_command: String,
    pub open_url: String,
    /// Files copied from the repository into each worktree, relative to the repository root.
    pub env_files: Vec<String>,
    /// Overrides the global agent instructions when not empty.
    pub agent_instructions: String,
    pub workflow_skills: WorkflowSkillOverrides,
    /// Let finished tickets answer other agents' questions (d0030). On by default.
    pub let_finished_tickets_answer: bool,
    /// Validation on or off for this project; `None` uses the global setting (d0063). Stored as `null` so the
    /// field-by-field read keeps a set value.
    pub validation: Option<bool>,
    /// `None` uses the global rounds.
    pub validation_rounds: Option<u32>,
    /// `None` uses the global Auto-merge (d0064).
    pub auto_merge: Option<AutoMerge>,
    /// Overrides the global Project agent instructions when not empty (d0068).
    pub project_agent_instructions: String,
    /// Set in everything Otaq starts in the project, in row order (ENV-01, d0082). Empty by default.
    pub env_vars: Vec<EnvVar>,
    /// "Start new worktrees from a warm build" (WRM-05, d0082). On by default; warm builds are on only where the
    /// data folder's disk can clone too.
    pub warm_builds: bool,
    /// Ticket resources (M19-d, d0087): the command that creates a ticket's own resource (a database, say) before
    /// setup, in the ticket's worktree. Empty: none, and Otaq sets no resource variables.
    pub resource_create: String,
    /// Removes what Create made, at Done and Close; runs only for a resource Otaq recorded.
    pub resource_remove: String,
    /// Refreshes the project's template (in the main checkout) after Done; Creates wait meanwhile.
    pub resource_template_refresh: String,
    /// Globs of the files that change the schema (`db/migrations/**`), relative to the repository root (M19-e).
    pub schema_paths: Vec<String>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            setup_command: String::new(),
            run_command: String::new(),
            open_url: String::new(),
            env_files: Vec::new(),
            agent_instructions: String::new(),
            workflow_skills: WorkflowSkillOverrides::default(),
            let_finished_tickets_answer: true,
            validation: None,
            validation_rounds: None,
            auto_merge: None,
            project_agent_instructions: String::new(),
            env_vars: Vec::new(),
            warm_builds: true,
            resource_create: String::new(),
            resource_remove: String::new(),
            resource_template_refresh: String::new(),
            schema_paths: Vec::new(),
        }
    }
}

/// One project environment variable row, `NAME = value` (ENV-01). Stored and shown as written, in plain text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

impl EnvVar {
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }
}

/// The problem field of the environment variable row at `index`: `env_vars.2`.
pub fn env_var_field(index: usize) -> String {
    format!("env_vars.{index}")
}

/// The problem of a row with an empty name (*15 · Settings → Row · Environment variables · edit, remove, errors*).
pub const ENV_VAR_EMPTY_NAME: &str = "Add a name, or remove the row";

/// The problem of a name containing `=`: "“MAX=JOBS” can’t be a name: a name can’t contain “=”. This row isn’t
/// set until it’s fixed."
pub fn env_var_equals_message(name: &str) -> String {
    format!(
        "“{name}” can’t be a name: a name can’t contain “=”. This row isn’t set until it’s fixed."
    )
}

/// The problem of a name containing a null character, shown as `\0`.
pub fn env_var_null_message(name: &str) -> String {
    format!(
        "“{}” can’t be a name: a name can’t contain a null character. This row isn’t set until it’s fixed.",
        name.replace('\0', "\\0")
    )
}

/// The problem of a row repeating an earlier row's name: "NODE_OPTIONS is already set above. Only the first row
/// applies; remove one of them."
pub fn env_var_duplicate_message(name: &str) -> String {
    format!("{name} is already set above. Only the first row applies; remove one of them.")
}

/// Whether two variable names are the same on `os`: Windows compares them ignoring case (ENV-03).
pub fn same_env_name(a: &str, b: &str, os: Os) -> bool {
    if os == Os::Windows {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// Each row's problem, if it has one, by row index (ENV-03): an empty name, a name containing `=` or a null
/// character, or a name an earlier row already has (the first applies; on Windows ignoring case). Nothing else is
/// a problem: spaces, a leading digit and any value are allowed.
pub fn env_var_problems(env_vars: &[EnvVar], os: Os) -> Vec<Option<String>> {
    env_vars
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let name = row.name.as_str();
            if name.is_empty() {
                Some(ENV_VAR_EMPTY_NAME.to_owned())
            } else if name.contains('=') {
                Some(env_var_equals_message(name))
            } else if name.contains('\0') {
                Some(env_var_null_message(name))
            } else if env_vars[..index]
                .iter()
                .any(|earlier| same_env_name(&earlier.name, name, os))
            {
                Some(env_var_duplicate_message(name))
            } else {
                None
            }
        })
        .collect()
}

/// The values a project's agents actually use, after falling back to the global settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveSettings {
    pub agent_instructions: String,
    /// The skill each step runs ([`resolve_skill`]).
    pub workflow_skills: ResolvedSkills,
    pub validation: bool,
    /// Always within 1–5: a flagged value falls back to the global one, then to 3.
    pub validation_rounds: u32,
    pub auto_merge: AutoMerge,
    /// Auto-merge is set but does nothing because validation is off for the project (the settings say so,
    /// d0064).
    pub auto_merge_inactive: bool,
    pub project_agent_instructions: String,
}

/// The project's override when it isn't empty, else the global value.
fn own_or_global(own: &str, global: &str) -> String {
    if own.trim().is_empty() {
        global.to_owned()
    } else {
        own.to_owned()
    }
}

/// The rounds a project uses: its own when valid, else the global value when valid, else 3 (a flagged value is
/// never used).
pub fn effective_rounds(own: Option<u32>, global: u32) -> u32 {
    own.into_iter()
        .chain([global])
        .find(|r| VALIDATION_ROUNDS.contains(r))
        .unwrap_or(DEFAULT_VALIDATION_ROUNDS)
}

/// Why a rounds value can't be used, if it can't.
fn rounds_problem(rounds: u32) -> Option<SettingProblem> {
    (!VALIDATION_ROUNDS.contains(&rounds)).then(|| {
        problem(
            "validation_rounds",
            &format!(
                "Use {} to {} validation rounds",
                VALIDATION_ROUNDS.start(),
                VALIDATION_ROUNDS.end()
            ),
        )
    })
}

/// A stored value that is flagged as invalid. `field` names the setting (`env_files`, `open_url`, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingProblem {
    pub field: String,
    pub message: String,
}

impl Board {
    pub async fn global_settings(&self) -> Result<GlobalSettings, BoardError> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value_json FROM settings")
            .fetch_all(self.store.pool())
            .await?;
        let mut object = Map::new();
        for (key, value) in rows {
            if let Ok(value) = serde_json::from_str::<Value>(&value) {
                object.insert(key, value);
            }
        }
        if !object.contains_key("theme")
            && let Some(theme) = ThemeSetting::from_0069(&object)
        {
            object.insert(
                "theme".to_owned(),
                serde_json::to_value(theme).expect("theme"),
            );
        }
        Ok(read_or_default(Value::Object(object)))
    }

    /// Stores every global setting.
    pub async fn set_global_settings(&self, settings: &GlobalSettings) -> Result<(), BoardError> {
        let Value::Object(fields) = serde_json::to_value(settings).expect("settings serialize")
        else {
            unreachable!("settings are an object")
        };
        let mut tx = self.store.begin_write().await?;
        for (key, value) in fields {
            sqlx::query(
                "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?)
                 ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value.to_string())
            .bind(now_ms())
            .execute(&mut *tx)
            .await?;
        }
        for key in THEME_KEYS_0069 {
            sqlx::query("DELETE FROM settings WHERE key = ?")
                .bind(key)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn project_settings(&self, project: &str) -> Result<ProjectSettings, BoardError> {
        let project = self.project(project).await?;
        let value: Value = serde_json::from_str(&project.settings_json).unwrap_or_default();
        Ok(read_or_default(value))
    }

    pub async fn set_project_settings(
        &self,
        project: &str,
        settings: &ProjectSettings,
    ) -> Result<(), BoardError> {
        let project = self.project(project).await?;
        let mut json = serde_json::to_value(settings).expect("settings serialize");
        // The role default overrides are saved by their own request (AGT-03); a settings save keeps them.
        let agents = crate::store::projects::PROJECT_AGENT_DEFAULTS;
        if let Some(defaults) = serde_json::from_str::<Value>(&project.settings_json)
            .ok()
            .and_then(|mut stored| stored.get_mut(agents).map(Value::take))
        {
            json[agents] = defaults;
        }
        let json = json.to_string();
        sqlx::query("UPDATE projects SET settings_json = ? WHERE id = ?")
            .bind(json)
            .bind(&project.id)
            .execute(self.store.pool())
            .await?;
        Ok(())
    }

    /// The effective validation rounds of the project with this id (cards show "round 1 of 3").
    pub(crate) async fn validation_rounds_of(&self, project_id: &str) -> Result<u32, BoardError> {
        let global = self.global_settings().await?;
        let json: Option<String> =
            sqlx::query_scalar("SELECT settings_json FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(self.store.pool())
                .await?;
        let own: ProjectSettings =
            read_or_default(serde_json::from_str(&json.unwrap_or_default()).unwrap_or_default());
        Ok(effective_rounds(
            own.validation_rounds,
            global.validation_rounds,
        ))
    }

    /// The project variables that apply (ENV-02), of the project with this id, read now (never cached): a
    /// change reaches what starts after it. An unknown project has none.
    pub async fn project_env(&self, project_id: &str) -> Result<Vec<(String, String)>, BoardError> {
        let json: Option<String> =
            sqlx::query_scalar("SELECT settings_json FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(self.store.pool())
                .await?;
        let own: ProjectSettings =
            read_or_default(serde_json::from_str(&json.unwrap_or_default()).unwrap_or_default());
        Ok(crate::session::env::applied(&own.env_vars, Os::current()))
    }

    /// The user's `git` with the project's variables set in every command it runs (ENV-02), for the git
    /// commands Otaq runs in the project's repository and worktrees, so the user's hooks see them. Otaq sets
    /// none of its own variables there, so a `${…}` naming one becomes empty ([`crate::session::env::substitute`]).
    pub async fn project_git(&self, project: &str) -> Result<crate::git::Git, BoardError> {
        let git = self.git()?;
        let settings = self.project_settings(project).await?;
        let os = Os::current();
        Ok(git.with_env(crate::session::env::substitute(
            &crate::session::env::applied(&settings.env_vars, os),
            &[],
            os,
        )))
    }

    /// The values a project's agents, validators and Project agents use.
    pub async fn effective_settings(&self, project: &str) -> Result<EffectiveSettings, BoardError> {
        let global = self.global_settings().await?;
        let own = self.project_settings(project).await?;
        Ok(effective(&global, &own, &self.data))
    }

    /// Problems with the global settings' values.
    pub async fn global_setting_problems(&self) -> Result<Vec<SettingProblem>, BoardError> {
        let settings = self.global_settings().await?;
        let mut problems = Vec::new();
        if settings.agent_limits.max_concurrency == 0 {
            problems.push(problem(
                "agent_limits.max_concurrency",
                "Allow at least 1 agent",
            ));
        }
        if settings.agent_limits.back_pressure == 0 {
            problems.push(problem(
                "agent_limits.back_pressure",
                "Back-pressure needs at least 1 item",
            ));
        }
        problems.extend(rounds_problem(settings.validation_rounds));
        if !TERMINAL_FONT_SIZES.contains(&settings.terminal_font_size) {
            problems.push(problem(
                "terminal_font_size",
                &format!(
                    "Use a size from {} to {}",
                    TERMINAL_FONT_SIZES.start(),
                    TERMINAL_FONT_SIZES.end()
                ),
            ));
        }
        Ok(problems)
    }

    /// Problems with a project's settings: env files missing from the repository, an Open URL that isn't a URL.
    /// Reads the repository, never writes to it.
    pub async fn project_setting_problems(
        &self,
        project: &str,
    ) -> Result<Vec<SettingProblem>, BoardError> {
        let settings = self.project_settings(project).await?;
        let repo = self.project(project).await?.repo_path;
        let mut problems = Vec::new();
        for file in &settings.env_files {
            if let Some(message) = env_file_problem(Path::new(&repo), file) {
                problems.push(problem("env_files", &message));
            }
        }
        problems.extend(settings.validation_rounds.and_then(rounds_problem));
        for (index, message) in env_var_problems(&settings.env_vars, Os::current())
            .into_iter()
            .enumerate()
        {
            if let Some(message) = message {
                problems.push(problem(&env_var_field(index), &message));
            }
        }
        problems.extend(resource_problems(&settings));
        if !settings.open_url.trim().is_empty() && !looks_like_url(settings.open_url.trim()) {
            problems.push(problem(
                "open_url",
                &format!(
                    "{} isn't a URL; use something like http://localhost:$PORT",
                    settings.open_url.trim()
                ),
            ));
        }
        Ok(problems)
    }
}

/// Resolves a project's settings against the global ones ([`Board::effective_settings`] reads them). Reads only
/// Otaq's skill copies under `data`, to tell a missing or broken one.
pub fn effective(
    global: &GlobalSettings,
    own: &ProjectSettings,
    data: &DataDir,
) -> EffectiveSettings {
    let skill = |step| resolve_skill(&global.workflow_skills, &own.workflow_skills, step, data);
    let validation = own.validation.unwrap_or(global.validation);
    let auto_merge = own.auto_merge.unwrap_or(global.auto_merge);
    EffectiveSettings {
        agent_instructions: own_or_global(&own.agent_instructions, &global.agent_instructions),
        workflow_skills: ResolvedSkills {
            commit: skill(WorkflowStep::Commit),
            push: skill(WorkflowStep::Push),
            open_pr: skill(WorkflowStep::OpenPr),
            validate: skill(WorkflowStep::Validate),
        },
        validation,
        validation_rounds: effective_rounds(own.validation_rounds, global.validation_rounds),
        auto_merge,
        auto_merge_inactive: auto_merge != AutoMerge::Off && !validation,
        project_agent_instructions: own_or_global(
            &own.project_agent_instructions,
            &global.project_agent_instructions,
        ),
    }
}

/// Reads settings from stored JSON; anything missing or unreadable gets its default.
fn read_or_default<T: Default + serde::de::DeserializeOwned + Serialize>(stored: Value) -> T {
    // Merge field by field so one bad value doesn't reset the others.
    let mut merged = serde_json::to_value(T::default()).expect("settings serialize");
    if let (Value::Object(merged), Value::Object(stored)) = (&mut merged, stored) {
        for (key, value) in stored {
            if let Some(slot) = merged.get_mut(&key) {
                let previous = std::mem::replace(slot, value);
                let candidate = Value::Object(merged.clone());
                if serde_json::from_value::<T>(candidate).is_err() {
                    merged.insert(key, previous);
                }
            }
        }
    }
    serde_json::from_value(merged).unwrap_or_default()
}

fn problem(field: &str, message: &str) -> SettingProblem {
    SettingProblem {
        field: field.to_owned(),
        message: message.to_owned(),
    }
}

/// Why an env file can't be copied, if it can't: it must be a relative path to a file inside the repository.
fn env_file_problem(repo: &Path, file: &str) -> Option<String> {
    let path = Path::new(file.trim());
    let escapes =
        path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir));
    if file.trim().is_empty() {
        Some("An env file entry is empty".to_owned())
    } else if escapes {
        Some(format!(
            "{file} must be a path inside the repository, like .env.local"
        ))
    } else if !repo.join(path).is_file() {
        Some(format!("{file} doesn't exist in the repository"))
    } else {
        None
    }
}

/// The message on a Remove or Template refresh command set without a Create command.
pub fn resource_without_create() -> String {
    format!(
        "Remove runs only for resources {} created",
        naming::PRODUCT_NAME
    )
}

/// Problems with a project's ticket resource settings (d0087): a schema path that isn't a relative glob, and a
/// Remove or Template refresh command with no Create command (a warning: nothing is refused).
pub fn resource_problems(settings: &ProjectSettings) -> Vec<SettingProblem> {
    let mut problems: Vec<SettingProblem> = settings
        .schema_paths
        .iter()
        .filter_map(|glob| crate::glob::problem(glob))
        .map(|message| problem("schema_paths", &message))
        .collect();
    if settings.resource_create.trim().is_empty() {
        for (field, value) in [
            ("resource_remove", &settings.resource_remove),
            (
                "resource_template_refresh",
                &settings.resource_template_refresh,
            ),
        ] {
            if !value.trim().is_empty() {
                problems.push(problem(field, &resource_without_create()));
            }
        }
    }
    problems
}

/// A light check: `http://` or `https://` followed by a host, and no spaces. `$PORT` is allowed (d0035).
fn looks_like_url(url: &str) -> bool {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"));
    rest.is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !url.contains(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        for good in [
            "http://localhost:3000",
            "https://example.com/path?q=1",
            "http://localhost:$PORT/",
        ] {
            assert!(looks_like_url(good), "{good}");
        }
        for bad in [
            "localhost:3000",
            "http://",
            "ftp://x",
            "http://a b",
            "https:///x",
        ] {
            assert!(!looks_like_url(bad), "{bad}");
        }
    }

    /// WSK-07: settings stored before d0092 keep reading. Otaq's own defaults are the built-in, empty is the built-in
    /// (global) or "use global" (a project), any other text a legacy value; an imported record reads back as it is.
    #[test]
    fn old_workflow_skill_json_reads_as_built_in_legacy_or_use_global() {
        let global: WorkflowSkills = serde_json::from_value(serde_json::json!({
            "commit": "/my-commit",
            "push": naming::skill("push"),
            "open_pr": "",
        }))
        .unwrap();
        assert_eq!(global.commit, SkillSetting::Legacy("/my-commit".into()));
        assert_eq!(global.push, SkillSetting::BuiltIn);
        assert_eq!(global.open_pr, SkillSetting::BuiltIn);
        assert_eq!(global.validate, SkillSetting::BuiltIn, "missing");
        assert_eq!(
            global,
            WorkflowSkills {
                commit: SkillSetting::Legacy("/my-commit".into()),
                ..Default::default()
            }
        );

        let own: WorkflowSkillOverrides = serde_json::from_value(serde_json::json!({
            "commit": "",
            "push": naming::skill("push"),
            "open_pr": " /my-pr ",
        }))
        .unwrap();
        assert_eq!(own.commit, SkillSetting::UseGlobal);
        assert_eq!(own.push, SkillSetting::BuiltIn);
        assert_eq!(own.open_pr, SkillSetting::Legacy("/my-pr".into()));
        assert_eq!(own.validate, SkillSetting::UseGlobal, "missing");
        // Another step's own skill isn't this step's built-in.
        let odd: WorkflowSkillOverrides =
            serde_json::from_value(serde_json::json!({ "commit": naming::skill("push") })).unwrap();
        assert_eq!(odd.commit, SkillSetting::Legacy(naming::skill("push")));

        // Stored as before: the built-in as Otaq's default, "use global" empty, a legacy value as typed.
        let mut own = WorkflowSkillOverrides {
            push: SkillSetting::BuiltIn,
            open_pr: SkillSetting::Legacy("/my-pr".into()),
            ..Default::default()
        };
        own.commit = SkillSetting::Imported(Box::new(imported("abc")));
        let json = serde_json::to_value(&own).unwrap();
        assert_eq!(json["push"], naming::skill("push"));
        assert_eq!(json["validate"], "");
        assert_eq!(json["open_pr"], "/my-pr");
        assert_eq!(json["commit"]["id"], "abc");
        assert_eq!(json["commit"]["name"], "my-commit");
        assert_eq!(
            serde_json::from_value::<WorkflowSkillOverrides>(json).unwrap(),
            own
        );
        assert_eq!(
            serde_json::to_value(WorkflowSkills::default()).unwrap()["commit"],
            naming::skill("commit")
        );
    }

    fn imported(id: &str) -> ImportedSkill {
        ImportedSkill {
            id: id.to_owned(),
            name: "my-commit".to_owned(),
            original: "/tmp/team-skills/my-commit".to_owned(),
            imported_at: 1,
            fingerprint: String::new(),
            warnings: Vec::new(),
        }
    }

    /// A copy under `data` for `id`, valid or not.
    fn copy(data: &DataDir, id: &str, skill_md: Option<&str>) {
        let folder = data.skill_path(id).unwrap();
        std::fs::create_dir_all(&folder).unwrap();
        if let Some(text) = skill_md {
            std::fs::write(folder.join("SKILL.md"), text).unwrap();
        }
    }

    /// WSK-07: project imported → it; project "use global" or legacy → the global's; global imported → it; global
    /// built-in or legacy → the built-in. A missing or broken copy → the built-in (WSK-04).
    #[test]
    fn workflow_skills_resolve_by_wsk_07() {
        let home = tempfile::tempdir().unwrap();
        let data = DataDir::new(home.path());
        let good = "---\nname: my-commit\n---\nCommit.\n";
        copy(&data, "g", Some(good));
        copy(&data, "p", Some(good));
        copy(&data, "broken", Some("no front matter"));
        copy(&data, "empty", None);
        let built_in = ResolvedSkill::BuiltIn;
        let at = |id: &str| ResolvedSkill::Imported {
            name: "my-commit".into(),
            folder: data.skill_path(id).unwrap(),
        };
        let resolve = |global: &WorkflowSkills, own: &WorkflowSkillOverrides| {
            resolve_skill(global, own, WorkflowStep::Commit, &data)
        };

        let mut global = WorkflowSkills::default();
        let mut own = WorkflowSkillOverrides::default();
        for step in WorkflowStep::ALL {
            assert_eq!(resolve_skill(&global, &own, step, &data), built_in(step));
        }
        // A global legacy value is the built-in, and a project following it too.
        global.commit = SkillSetting::Legacy("/my-commit".into());
        assert_eq!(resolve(&global, &own), built_in(WorkflowStep::Commit));
        // A global import, followed by a project on "use global" and by a project's legacy value.
        global.commit = SkillSetting::Imported(Box::new(imported("g")));
        assert_eq!(resolve(&global, &own), at("g"));
        own.commit = SkillSetting::Legacy("/my-pr".into());
        assert_eq!(resolve(&global, &own), at("g"));
        // A project's own import wins; a project's step on Otaq's own default is the built-in.
        own.commit = SkillSetting::Imported(Box::new(imported("p")));
        assert_eq!(resolve(&global, &own), at("p"));
        own.commit = SkillSetting::BuiltIn;
        assert_eq!(resolve(&global, &own), built_in(WorkflowStep::Commit));
        // A broken, empty or missing copy is the built-in, and doesn't fall back to the global import.
        for id in ["broken", "empty", "gone"] {
            own.commit = SkillSetting::Imported(Box::new(imported(id)));
            assert_eq!(
                resolve(&global, &own),
                built_in(WorkflowStep::Commit),
                "{id}"
            );
        }
        // The value today's naming takes: Otaq's default, or the copy's SKILL.md.
        assert_eq!(built_in(WorkflowStep::Push).stored(), naming::skill("push"));
        assert_eq!(
            at("g").stored(),
            data.skill_path("g")
                .unwrap()
                .join("SKILL.md")
                .display()
                .to_string()
        );
        assert_eq!(at("g").label(), "my-commit");
    }

    /// ADP-05: Otaq's own workflow skills are recognised by their stored value, whatever the step.
    #[test]
    fn own_workflow_skills_by_stored_value() {
        for step in WorkflowStep::ALL {
            assert_eq!(
                WorkflowSkills::own_skill_of(&naming::skill(step.own_skill())),
                Some(step.own_skill())
            );
        }
        assert_eq!(
            WorkflowSkills::own_skill_of(&format!(" {} ", naming::skill("push"))),
            Some("push")
        );
        for user in [
            "/my-commit",
            "",
            &naming::skill("work-ticket"),
            "/their-plugin:commit",
        ] {
            assert_eq!(WorkflowSkills::own_skill_of(user), None, "{user}");
        }
    }

    #[test]
    fn theme_names_are_the_designs_palette_keys() {
        let names: Vec<&str> = Theme::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            names,
            [
                naming::THEME_DARK,
                "catppuccin-frappe",
                "catppuccin-macchiato",
                "catppuccin-mocha",
                "tokyo-night",
                "dracula",
                "nord",
                "gruvbox-dark",
                "rose-pine",
                "one-dark",
                naming::THEME_LIGHT,
                "catppuccin-latte",
                "daylight",
                "solarized-light",
                "rose-pine-dawn",
            ]
        );
        let dark = Theme::ALL.iter().filter(|t| t.is_dark()).count();
        assert_eq!(dark, 10);
        assert!(Theme::ALL[..dark].iter().all(|t| t.is_dark()));
        for theme in Theme::ALL {
            let json = serde_json::to_value(ThemeSetting::Picked(*theme)).unwrap();
            assert_eq!(json, serde_json::json!(theme.as_str()));
            assert_eq!(
                serde_json::from_value::<ThemeSetting>(json).unwrap(),
                ThemeSetting::Picked(*theme)
            );
        }
        let defaults = GlobalSettings::default();
        assert_eq!(defaults.theme.resolve(true), Theme::OtaqDark);
        assert_eq!(defaults.theme.resolve(false), Theme::OtaqLight);
        let by_os = serde_json::to_value(defaults.theme).unwrap();
        assert_eq!(
            by_os,
            serde_json::json!({"dark": naming::THEME_DARK, "light": naming::THEME_LIGHT})
        );
        assert_eq!(
            serde_json::from_value::<ThemeSetting>(by_os).unwrap(),
            defaults.theme
        );
    }

    #[test]
    fn warm_builds_are_on_unless_turned_off() {
        assert!(ProjectSettings::default().warm_builds);
        // A project saved before the switch existed reads as on.
        let old: ProjectSettings = read_or_default(serde_json::json!({"setup_command": "make"}));
        assert!(old.warm_builds);
        assert_eq!(old.setup_command, "make");
        let off: ProjectSettings = read_or_default(serde_json::json!({"warm_builds": false}));
        assert!(!off.warm_builds);
    }

    #[test]
    fn resource_settings_default_to_empty_in_old_settings() {
        let old: ProjectSettings = read_or_default(serde_json::json!({"setup_command": "make"}));
        assert_eq!(old.resource_create, "");
        assert_eq!(old.resource_remove, "");
        assert_eq!(old.resource_template_refresh, "");
        assert!(old.schema_paths.is_empty());
        assert!(resource_problems(&old).is_empty());
        let set: ProjectSettings = read_or_default(serde_json::json!({
            "resource_create": "createdb -T shop_template shop_1",
            "schema_paths": ["db/migrations/**"],
            "warm_builds": "not a flag",
        }));
        assert_eq!(set.schema_paths, ["db/migrations/**"]);
        assert!(set.resource_create.starts_with("createdb"));
        assert!(set.warm_builds, "one bad value keeps the others");
    }

    #[test]
    fn a_picked_theme_is_used_whatever_the_os() {
        let picked = ThemeSetting::Picked(Theme::Dracula);
        assert_eq!(picked.resolve(true), Theme::Dracula);
        assert_eq!(picked.resolve(false), Theme::Dracula);
    }

    #[test]
    fn an_unknown_theme_reads_as_unpicked_and_keeps_the_others() {
        let stored = serde_json::json!({"theme": "monokai", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.theme, ThemeSetting::default());
        assert_eq!(settings.terminal_font_size, 15);
    }

    #[test]
    fn a_0069_mode_keeps_the_theme_it_showed() {
        let read = |stored: Value| {
            let Value::Object(stored) = stored else {
                unreachable!()
            };
            ThemeSetting::from_0069(&stored)
        };
        let both = |appearance: &str| {
            read(serde_json::json!({
                "appearance": appearance,
                "dark_theme": "tokyo-night",
                "light_theme": "daylight",
            }))
        };
        assert_eq!(both("dark"), Some(ThemeSetting::Picked(Theme::TokyoNight)));
        assert_eq!(both("light"), Some(ThemeSetting::Picked(Theme::Daylight)));
        assert_eq!(
            both("system"),
            Some(ThemeSetting::ByOs {
                dark: Theme::TokyoNight,
                light: Theme::Daylight
            })
        );
        // Both Otaq versions were stored as `otaq`; unknown names read as Otaq.
        assert_eq!(
            read(serde_json::json!({"appearance": "light", "light_theme": naming::THEME_KEY})),
            Some(ThemeSetting::Picked(Theme::OtaqLight))
        );
        assert_eq!(
            read(serde_json::json!({"appearance": "dark", "dark_theme": "monokai"})),
            Some(ThemeSetting::Picked(Theme::OtaqDark))
        );
        assert_eq!(
            read(serde_json::json!({"dark_theme": "nord"})),
            Some(ThemeSetting::ByOs {
                dark: Theme::Nord,
                light: Theme::OtaqLight
            })
        );
        assert_eq!(read(serde_json::json!({"notifications": false})), None);
    }

    #[test]
    fn the_terminal_renderer_defaults_to_dom_on_linux_only() {
        let default = GlobalSettings::default().terminal_renderer;
        if cfg!(target_os = "linux") {
            assert_eq!(default, TerminalRenderer::Dom);
        } else {
            assert_eq!(default, TerminalRenderer::Webgl);
        }
        let stored = serde_json::json!({"terminal_renderer": "dom"});
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.terminal_renderer, TerminalRenderer::Dom);
        let stored = serde_json::json!({"terminal_renderer": "vulkan", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert_eq!(settings.terminal_renderer, default);
        assert_eq!(settings.terminal_font_size, 15);
    }

    #[test]
    fn the_description_source_mode_is_off_until_chosen() {
        assert!(!GlobalSettings::default().description_source_mode);
        // Settings stored before the field existed load with it off and keep their own values.
        let settings: GlobalSettings =
            read_or_default(serde_json::json!({"terminal_font_size": 15}));
        assert!(!settings.description_source_mode);
        assert_eq!(settings.terminal_font_size, 15);
        let old: GlobalSettings =
            serde_json::from_value(serde_json::json!({"validation": false})).unwrap();
        assert!(!old.description_source_mode);
        let settings: GlobalSettings =
            read_or_default(serde_json::json!({"description_source_mode": true}));
        assert!(settings.description_source_mode);
    }

    #[test]
    fn usage_data_is_not_asked_and_never_sends_until_the_user_says_yes() {
        let fresh = GlobalSettings::default();
        assert_eq!(fresh.usage_data, UsageData::NotAsked);
        assert!(!fresh.usage_data.sends());
        // Settings stored before the field existed load as not asked.
        let old: GlobalSettings = read_or_default(serde_json::json!({"terminal_font_size": 15}));
        assert_eq!(old.usage_data, UsageData::NotAsked);
        let on: GlobalSettings = read_or_default(serde_json::json!({"usage_data": "on"}));
        assert!(on.usage_data.sends());
        let off: GlobalSettings = read_or_default(serde_json::json!({"usage_data": "off"}));
        assert!(!off.usage_data.sends());
        // A value this version doesn't know never turns sending on.
        let odd: GlobalSettings = read_or_default(serde_json::json!({"usage_data": "maybe"}));
        assert!(!odd.usage_data.sends());
    }

    /// T-0371 (d0090): settings stored before the per-kind checkboxes and the sound read with every kind and the
    /// sound on; one kind turned off is kept, the others stay on.
    #[test]
    fn old_settings_read_with_every_kind_and_the_sound_on() {
        let old: GlobalSettings =
            read_or_default(serde_json::json!({"notifications": false, "terminal_font_size": 15}));
        assert!(!old.notifications);
        assert_eq!(old.notification_kinds, NotificationKinds::default());
        assert!(Kind::ALL.iter().all(|k| old.notification_kinds.allows(*k)));
        assert!(old.notification_sound);
        let fresh = GlobalSettings::default();
        assert!(fresh.notifications && fresh.notification_sound);
        assert!(
            Kind::ALL
                .iter()
                .all(|k| fresh.notification_kinds.allows(*k))
        );

        let one_off: GlobalSettings = read_or_default(serde_json::json!({
            "notification_kinds": {"turn_ended": false},
            "notification_sound": false,
        }));
        assert!(!one_off.notification_kinds.allows(Kind::TurnEnded));
        assert!(one_off.notification_kinds.allows(Kind::Permission));
        assert!(!one_off.notification_sound);
        let mut kinds = NotificationKinds::default();
        kinds.set(Kind::ToolUnavailable, false);
        assert!(!kinds.allows(Kind::ToolUnavailable));
    }

    #[test]
    fn a_bad_stored_value_keeps_the_others() {
        let stored = serde_json::json!({"notifications": "loud", "terminal_font_size": 15});
        let settings: GlobalSettings = read_or_default(stored);
        assert!(settings.notifications);
        assert_eq!(settings.terminal_font_size, 15);
    }
}
