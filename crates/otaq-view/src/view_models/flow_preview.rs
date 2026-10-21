//! The flow preview (FLW-04; d0108 "Every change is previewed, then applied", second gap review rule 3; *18 → Row ·
//! The preview: next to the Project agent* `s18-27`/`s18-28`, *a whole flow, a change* `s18-29`/`s18-30`, *warnings,
//! problems, no change* `s18-31`/`s18-32`, *back to the previous flow, a rename* `s18-35`/`s18-36`): what the sheet
//! over the main area shows for a project's open proposal (T-0413's [`Proposal`]).
//!
//! **A whole flow** (a preset, a copy, an import, a project's first flow) is a table of its stages, each with its
//! why line, then AGENTS IT ADDS; **a change** lists only what differs ("shop-web · stage QA (your check) after
//! Security", "Security reviewer · model: opus → fable" with its "Used in"). Both show the warnings, the problems
//! (Apply stays off until each is fixed: a skill folder or a local model picked here), the import clashes (Keep mine ·
//! Take theirs · Add as), and when it applies. The user's picks ([`Choices`]) live in the page until Apply sends them;
//! this view is rebuilt with them on every pick.

use std::collections::{BTreeMap, HashSet};

use otaq_core::agent_tools::Tool;
use otaq_core::agents::in_use::StageUse;
use otaq_core::agents::{AgentList, AgentModel, ModelRef, shipped};
use otaq_core::board::tickets::Ticket;
use otaq_core::flow::apply::still_working;
use otaq_core::flow::proposal::{
    AgentChange, AgentChangeKind, AgentField, Applies, Change, ClashChoice, Fix, Level, Proposal,
    ProposedBy, SAME, Scope, Source,
};
use otaq_core::flow::{
    AppliedBy, CheckBy, Column, Delivers, DeliveryMode, Flow, FlowAgent, FlowFile, FlowVersion,
    Stage, StageKind, StageOption, yaml,
};
use otaq_core::naming::PRODUCT_NAME;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::agent_page::Choice;
use super::agents::{AgentTools, DEFAULT, tool_model};
use super::model_picker::{self, Picker};
use crate::icons::IconName;

/// The line under every title but "No changes".
pub const NOTHING_CHANGES: &str = "nothing changes until you apply it";
/// The footer's hint while a Project agent proposed it.
pub const AGENT_WAITS: &str = "The agent waits for your answer in its terminal";
pub const APPLY: &str = "Apply";
pub const CANCEL: &str = "Cancel";
pub const CLOSE: &str = "Close";
pub const ASK_FOR_MORE_CHANGES: &str = "Ask for more changes";
/// The whole table's first line: the columns every flow has.
pub const ALWAYS_THERE: &str =
    "Backlog \u{2192} Ready for Dev \u{2192} \u{2026} \u{2192} Done: always first and last";
pub const AGENTS_IT_ADDS: &str = "AGENTS IT ADDS";
/// A whole table's options cell, and an agent column with nothing in it.
pub const NONE: &str = "\u{2014}";
pub const NO_AGENT: &str = "No agent";
pub const NEW: &str = "new";

/// The user's picks in the preview, sent with Apply (T-0413's `ApplyChoices`, field for field): a pick per import
/// clash by the agent's name (Keep mine when left out), a folder per missing skill, a model per agent whose model
/// isn't listed (`default`, `claude`, `claude:opus`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(rename = "FlowPreviewChoices")]
pub struct Choices {
    #[serde(default)]
    pub clashes: BTreeMap<String, ClashPick>,
    #[serde(default)]
    pub skill_folders: BTreeMap<String, String>,
    #[serde(default)]
    pub models: BTreeMap<String, String>,
}

/// One import clash's pick (T-0413's `ClashChoice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowPreviewClashPick")]
pub enum ClashPick {
    #[default]
    KeepMine,
    TakeTheirs,
    AddAs,
}

impl From<ClashPick> for ClashChoice {
    fn from(pick: ClashPick) -> Self {
        match pick {
            ClashPick::KeepMine => Self::KeepMine,
            ClashPick::TakeTheirs => Self::TakeTheirs,
            ClashPick::AddAs => Self::AddAs,
        }
    }
}

impl Choices {
    /// What Apply sends (`flow.apply`'s `choices`).
    pub fn apply_choices(&self) -> otaq_core::flow::proposal::ApplyChoices {
        otaq_core::flow::proposal::ApplyChoices {
            clashes: self
                .clashes
                .iter()
                .map(|(name, pick)| (name.clone(), (*pick).into()))
                .collect(),
            skill_folders: self.skill_folders.clone(),
            models: self.models.clone(),
        }
    }
}

/// One stage of a whole flow's table: STAGE · AGENT · MODEL · OPTIONS A TICKET CAN PICK, then its why line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewStageRow")]
pub struct StageRow {
    pub id: String,
    /// Its kind: bot, shield-check, user-check, git-pull-request or git-merge.
    pub icon: IconName,
    pub name: String,
    /// "Agent", "Check · agent · 3 rounds", "Check · you", "Delivery · PR · automatic".
    pub kind: String,
    /// The agent's name, "You" for your check, "No agent" (muted) for a Local merge.
    pub agent: String,
    pub agent_muted: bool,
    /// "Claude Code · sonnet", "default · Claude Code · sonnet"; none for your check and a Local merge.
    pub model: Option<String>,
    /// "Quick developer", "Quick developer, skip", or "—" (muted).
    pub options: String,
    pub options_muted: bool,
    /// Why the flow has it, naming its source (violet).
    pub why: Option<String>,
}

/// A whole flow's table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewTable")]
pub struct Table {
    /// STAGE, AGENT · MODEL, OPTIONS A TICKET CAN PICK.
    pub headings: Vec<String>,
    /// "Backlog → Ready for Dev → … → Done: always first and last".
    pub always: String,
    pub rows: Vec<StageRow>,
}

/// One line of a change: a stage change or one field of a changed agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewChangeRow")]
pub struct ChangeRow {
    pub icon: IconName,
    /// "shop-web · stage QA (your check) after Security", "Security reviewer · model: opus → fable".
    pub text: String,
    /// The muted line under it: "Used in: shop-web (Security)", "applies when Dev next starts".
    pub detail: Option<String>,
}

/// An agent the proposal adds, expandable to its instructions and skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewAgentRow")]
pub struct AgentRow {
    pub name: String,
    /// "Claude Code · sonnet · grill".
    pub summary: String,
    /// "new".
    pub tag: String,
    /// The instructions in quotes; none when it has none.
    pub instructions: Option<String>,
    pub skills_label: String,
    pub skills: Vec<String>,
    pub why: Option<String>,
}

/// AGENTS IT ADDS, and the ones already there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewAgents")]
pub struct AgentsSection {
    pub heading: String,
    pub rows: Vec<AgentRow>,
    /// "None · already there: Developer · Validator" when it adds none, else "Already there: Griller · Validator".
    pub already: Option<String>,
}

/// A local model for an agent whose tool isn't connected or whose model isn't listed: a tool, then the tool's model
/// picker (M32's, `s15-30`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewModelFix")]
pub struct ModelFix {
    pub agent: String,
    pub tool_label: String,
    /// The default model, then each connected tool; ids `default`, `claude`…
    pub tools: Vec<Choice>,
    /// The tool's picker once one is picked.
    pub picker: Option<Picker>,
    pub model_label: String,
}

/// What the user can do about a problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "FlowPreviewFix")]
pub enum FixView {
    /// "Import its folder…": the system folder picker, then [`Choices::skill_folders`].
    Folder {
        skill: String,
        label: String,
        /// The folder picker's title.
        picker_title: String,
        /// The folder picked, shown in place of the problem's words.
        picked: Option<String>,
    },
    /// "Pick another model": [`ModelFix`] opens under the line.
    Model { label: String, fix: Box<ModelFix> },
}

/// A problem: Apply stays off until every one is fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewProblem")]
pub struct ProblemRow {
    pub text: String,
    pub fix: Option<FixView>,
    /// A folder or a model was picked for it.
    pub fixed: bool,
}

/// An agent of the same name with other content (second gap review rule 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewClash")]
pub struct ClashRow {
    pub agent: String,
    /// "Security reviewer is in the file and already here, with other instructions."
    pub text: String,
    /// Keep mine (ids `keep_mine`, `take_theirs`, `add_as`), selected as picked.
    pub options: Vec<Choice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowPreviewLevel")]
pub enum WarningLevel {
    Amber,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewWarning")]
pub struct WarningRow {
    pub level: WarningLevel,
    pub text: String,
}

/// Ask for more changes: one line of text for the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewAsk")]
pub struct Ask {
    pub label: String,
    pub placeholder: String,
    pub send: String,
    pub back: String,
}

/// Apply, off while a problem is left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewApply")]
pub struct ApplyButton {
    pub label: String,
    pub enabled: bool,
    /// Why it is off.
    pub off: Option<String>,
}

/// The preview of one project's open proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "FlowPreviewView")]
pub struct View {
    /// The project's key, and the proposal Apply answers.
    pub project: String,
    pub proposal: u32,
    /// "A flow for shop-web", "Changes to billing-api's flow", "No changes".
    pub title: String,
    /// "Proposed by the Project agent · nothing changes until you apply it".
    pub subtitle: String,
    pub close_label: String,
    /// "This is the same as your current flow." (green): nothing else shows.
    pub same: Option<String>,
    pub table: Option<Table>,
    pub changes: Vec<ChangeRow>,
    pub agents: Option<AgentsSection>,
    pub problems: Vec<ProblemRow>,
    pub clashes: Vec<ClashRow>,
    pub warnings: Vec<WarningRow>,
    /// When it applies: "The new stage waits for a quiet board: …", "Applies now: shop-web has no tickets yet."
    pub applies: Option<String>,
    /// "The agent waits for your answer in its terminal" while a Project agent proposed it.
    pub agent_waits: Option<String>,
    pub cancel: Option<String>,
    /// Ask for more changes: only for a Project agent's proposal.
    pub ask: Option<Ask>,
    pub apply: Option<ApplyButton>,
    /// "Close" in place of Cancel and Apply when nothing differs.
    pub close: Option<String>,
    /// The picks as Apply sends them: every clash with its pick (Keep mine by default), only the folders and models
    /// still asked for.
    pub choices: Choices,
}

/// What the view is built from.
pub struct Facts<'a> {
    pub proposal: &'a Proposal,
    /// The project's current flow version: whether this is its first flow, and when it last changed (Back).
    pub current: Option<&'a FlowVersion>,
    /// The flow the proposal is compared with (the waiting change's, else the current one).
    pub base: &'a Flow,
    /// The project's tickets: the ones a stage change waits for.
    pub tickets: &'a [Ticket],
    pub agents: &'a AgentList,
    pub tools: &'a AgentTools,
    pub choices: &'a Choices,
    pub now: i64,
}

/// A shipped preset's name ("Solo autopilot"); its file's name when Otaq doesn't ship it any more.
pub fn preset_name(file: &str) -> String {
    otaq_core::flow::presets::preset(file).map_or_else(|| file.to_owned(), |p| p.name.clone())
}

/// "today", "yesterday", "Oct 18" (local time).
fn day_words(at: i64, now: i64) -> String {
    use otaq_core::local_time::local;
    let (Some(when), Some(today)) = (local(at), local(now)) else {
        return "the last".to_owned();
    };
    let same = |a: &otaq_core::local_time::LocalTime, b: &otaq_core::local_time::LocalTime| {
        (a.year, a.month, a.day) == (b.year, b.month, b.day)
    };
    if same(&when, &today) {
        return "today".to_owned();
    }
    if local(now - 86_400_000).is_some_and(|y| same(&when, &y)) {
        return "yesterday".to_owned();
    }
    super::clock::month_day(&when)
}

/// Whether the preview shows the whole flow as a table rather than what changes: a preset, a copy, a file the user
/// imports, or the project's first flow (its current one is the shipped default).
fn shows_whole(proposal: &Proposal, current: Option<&FlowVersion>) -> bool {
    match &proposal.source {
        Source::Preset { .. } | Source::Project { .. } => true,
        Source::Document { file: Some(_) } if proposal.proposed_by == ProposedBy::User => true,
        Source::Document { .. } => current.is_none_or(|v| v.applied_by == AppliedBy::Shipped),
        Source::Previous { .. } | Source::AgentsFile => false,
    }
}

/// The title and the subtitle's first part.
fn title(
    proposal: &Proposal,
    current: Option<&FlowVersion>,
    whole: bool,
    now: i64,
) -> (String, String) {
    let project = &proposal.project_name;
    let by = match &proposal.proposed_by {
        ProposedBy::ProjectAgent { .. } => "Proposed by the Project agent".to_owned(),
        ProposedBy::User => "Proposed by you".to_owned(),
    };
    match &proposal.source {
        Source::Preset { file } => (
            format!("{} for {project}", preset_name(file)),
            "Preset".to_owned(),
        ),
        Source::Document { file: Some(file) } if proposal.proposed_by == ProposedBy::User => {
            (format!("A flow imported from {file}"), "Import".to_owned())
        }
        Source::Project { name, .. } => (
            format!("A flow for {project}"),
            format!("Copied from {name}"),
        ),
        Source::Previous { .. } => (
            format!("Back to {project}\u{2019}s previous flow"),
            match current {
                Some(v) => format!(
                    "The flow before {}\u{2019}s change",
                    day_words(v.applied_at, now)
                ),
                None => "The flow before the last change".to_owned(),
            },
        ),
        Source::AgentsFile => (
            format!("{PRODUCT_NAME}\u{2019}s shipped agents"),
            format!("From {}", otaq_core::naming::AGENTS_FILE),
        ),
        Source::Document { .. } if whole => (format!("A flow for {project}"), by),
        Source::Document { .. } => (format!("Changes to {project}\u{2019}s flow"), by),
    }
}

fn kind_icon(stage: &Stage) -> IconName {
    match stage.kind {
        StageKind::Agent => IconName::Bot,
        StageKind::Check { by: CheckBy::Agent } => IconName::ShieldCheck,
        StageKind::Check { by: CheckBy::You } => IconName::UserCheck,
        StageKind::Delivery => match stage.delivery.map(|d| d.delivers) {
            Some(Delivers::Pr) => IconName::GitPullRequest,
            _ => IconName::GitMerge,
        },
    }
}

/// "Agent", "Check · agent · 3 rounds", "Check · you", "Delivery · PR · automatic", with "shown in Dev" for a stage
/// that shares the column before it.
fn kind_line(flow: &Flow, stage: &Stage) -> String {
    let mut parts: Vec<String> = match stage.kind {
        StageKind::Agent => vec!["Agent".to_owned()],
        StageKind::Check { by } => {
            let mut parts = vec!["Check".to_owned(), by.as_str().to_owned()];
            if let Some(rounds) = stage.rounds {
                parts.push(match rounds {
                    1 => "1 round".to_owned(),
                    n => format!("{n} rounds"),
                });
            }
            parts
        }
        StageKind::Delivery => {
            let mut parts = vec!["Delivery".to_owned()];
            if let Some(d) = stage.delivery {
                parts.push(
                    match d.delivers {
                        Delivers::Pr => "PR",
                        Delivers::Local => "Local merge",
                        Delivers::Either => "PR or Local merge",
                    }
                    .to_owned(),
                );
                parts.push(
                    match d.mode {
                        DeliveryMode::Automatic => "automatic",
                        DeliveryMode::Click => "click",
                    }
                    .to_owned(),
                );
            }
            parts
        }
    };
    if stage.column == Column::WithPrevious
        && let Some(index) = flow.stages.iter().position(|s| s.id == stage.id)
        && let Some(owner) = flow.column_owner(index).and_then(|i| flow.stages.get(i))
        && owner.id != stage.id
    {
        parts.push(format!("shown in {}", owner.name));
    }
    parts.join(" \u{b7} ")
}

/// An agent's model in words: "Claude Code · sonnet", "default · Claude Code · sonnet".
fn model_words(model: &AgentModel, tools: &AgentTools, default: Option<&ModelRef>) -> String {
    match model {
        AgentModel::Default => match default {
            Some(model) => format!("{DEFAULT} \u{b7} {}", tool_model(model, tools)),
            None => DEFAULT.to_owned(),
        },
        AgentModel::Tool { tool, model } => tool_model(
            &ModelRef {
                tool: *tool,
                model: model.clone(),
            },
            tools,
        ),
    }
}

/// The agent the proposal names, as it will be: the document's (with a picked model), else the stored one.
struct Named<'a> {
    file: &'a FlowFile,
    facts: &'a Facts<'a>,
}

impl Named<'_> {
    fn model(&self, name: &str) -> Option<String> {
        let tools = self.facts.tools;
        let default = self.facts.agents.default_model.as_ref();
        let same = |a: &str| a.eq_ignore_ascii_case(name);
        if let Some(agent) = self.file.agents.iter().find(|a| same(&a.name)) {
            let text = self
                .facts
                .choices
                .models
                .iter()
                .find(|(n, _)| same(n))
                .map_or(agent.model.as_str(), |(_, m)| m.as_str());
            return Some(match shipped::model_of(text) {
                Ok(model) => model_words(&model, tools, default),
                Err(_) => text.trim().to_owned(),
            });
        }
        self.facts
            .agents
            .agents
            .iter()
            .find(|a| same(&a.name))
            .map(|a| model_words(&a.model, tools, default))
    }
}

fn option_words(stage: &Stage) -> Vec<String> {
    stage
        .options
        .iter()
        .map(|o| match o {
            StageOption::Agent(name) => name.clone(),
            StageOption::Skip => StageOption::SKIP.to_owned(),
        })
        .collect()
}

fn table(flow: &Flow, proposal: &Proposal, named: &Named) -> Table {
    let why = |id: &str| {
        proposal.why.iter().find_map(|w| match &w.about {
            otaq_core::flow::proposal::About::Stage { id: of, .. } if of == id => {
                Some(w.text.clone())
            }
            _ => None,
        })
    };
    let rows = flow
        .stages
        .iter()
        .map(|stage| {
            let (agent, agent_muted, model) = match (stage.kind, &stage.agent) {
                (StageKind::Check { by: CheckBy::You }, _) => ("You".to_owned(), false, None),
                (_, Some(agent)) => (agent.clone(), false, named.model(agent)),
                (_, None) => (NO_AGENT.to_owned(), true, None),
            };
            let options = option_words(stage);
            StageRow {
                id: stage.id.clone(),
                icon: kind_icon(stage),
                name: stage.name.clone(),
                kind: kind_line(flow, stage),
                agent,
                agent_muted,
                model,
                options_muted: options.is_empty(),
                options: if options.is_empty() {
                    NONE.to_owned()
                } else {
                    options.join(", ")
                },
                why: why(&stage.id),
            }
        })
        .collect();
    Table {
        headings: vec![
            "STAGE".to_owned(),
            "AGENT \u{b7} MODEL".to_owned(),
            "OPTIONS A TICKET CAN PICK".to_owned(),
        ],
        always: ALWAYS_THERE.to_owned(),
        rows,
    }
}

fn change_icon(change: &Change) -> IconName {
    match change {
        Change::Added { .. } => IconName::Plus,
        Change::Removed => IconName::Minus,
        Change::Moved { .. } => IconName::ChevronsUpDown,
        Change::Kind { .. } => IconName::RefreshCw,
        _ => IconName::Pencil,
    }
}

/// "Used in: shop-web (Security, Dev) · billing-api (Dev (option))".
pub fn used_in(uses: &[StageUse]) -> Option<String> {
    if uses.is_empty() {
        return None;
    }
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for u in uses {
        let project = u
            .project
            .clone()
            .unwrap_or_else(|| format!("{PRODUCT_NAME}\u{2019}s default flow"));
        let stage = if u.option {
            format!("{} (option)", u.stage)
        } else {
            u.stage.clone()
        };
        match groups.iter_mut().find(|(p, _)| *p == project) {
            Some((_, stages)) => stages.push(stage),
            None => groups.push((project, vec![stage])),
        }
    }
    let parts: Vec<String> = groups
        .into_iter()
        .map(|(p, stages)| format!("{p} ({})", stages.join(", ")))
        .collect();
    Some(format!("Used in: {}", parts.join(" \u{b7} ")))
}

fn clip(text: &str) -> String {
    const MOST: usize = 90;
    let text = text.trim();
    if text.chars().count() <= MOST {
        return text.to_owned();
    }
    let cut: String = text.chars().take(MOST - 1).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// What changes in an agent's instructions: `+ “Fail on any unescaped SQL”`, lines gone with `−`.
fn instructions_change(from: &str, to: &str) -> String {
    let (from, to) = (from.trim(), to.trim());
    if !from.is_empty()
        && let Some(rest) = to.strip_prefix(from)
        && !rest.trim().is_empty()
    {
        return format!("+ \u{201c}{}\u{201d}", clip(rest));
    }
    let lines = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let (old, new) = (lines(from), lines(to));
    let old_set: HashSet<&String> = old.iter().collect();
    let new_set: HashSet<&String> = new.iter().collect();
    let mut parts: Vec<String> = new
        .iter()
        .filter(|l| !old_set.contains(l))
        .map(|l| format!("+ \u{201c}{}\u{201d}", clip(l)))
        .collect();
    parts.extend(
        old.iter()
            .filter(|l| !new_set.contains(l))
            .map(|l| format!("\u{2212} \u{201c}{}\u{201d}", clip(l))),
    );
    if parts.is_empty() {
        "reworded".to_owned()
    } else {
        parts.join(" ")
    }
}

/// A model change as short as it reads: "opus → fable" on one tool, else both in words.
fn model_change(from: &str, to: &str, tools: &AgentTools) -> String {
    match (shipped::model_of(from), shipped::model_of(to)) {
        (
            Ok(AgentModel::Tool { tool: a, model: m }),
            Ok(AgentModel::Tool { tool: b, model: n }),
        ) if a == b => {
            let show =
                |m: Option<String>| m.unwrap_or_else(|| super::agents::TOOL_DEFAULT.to_owned());
            format!("{} \u{2192} {}", show(m), show(n))
        }
        (Ok(a), Ok(b)) => format!(
            "{} \u{2192} {}",
            model_words(&a, tools, None),
            model_words(&b, tools, None)
        ),
        _ => format!("{} \u{2192} {}", from.trim(), to.trim()),
    }
}

fn field_text(name: &str, field: &AgentField, tools: &AgentTools) -> String {
    let change = match field.field.as_str() {
        "instructions" => instructions_change(&field.from, &field.to),
        "model" => model_change(&field.from, &field.to, tools),
        _ => format!("{} \u{2192} {}", field.from, field.to),
    };
    format!("{name} \u{b7} {}: {change}", field.field)
}

fn agent_rows(changed: &AgentChange, tools: &AgentTools) -> Vec<ChangeRow> {
    let detail = used_in(&changed.used_in);
    changed
        .fields
        .iter()
        .map(|f| ChangeRow {
            icon: IconName::Pencil,
            text: field_text(&changed.name, f, tools),
            detail: detail.clone(),
        })
        .collect()
}

fn joined_words(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn clash_row(changed: &AgentChange, source: &Source, picks: &Choices) -> Option<ClashRow> {
    let clash = changed.clash.as_ref()?;
    let place = match source {
        Source::Preset { .. } => "the preset",
        Source::AgentsFile => "the shipped agents file",
        _ => "the file",
    };
    let fields: Vec<&str> = changed.fields.iter().map(|f| f.field.as_str()).collect();
    let pick = picks
        .clashes
        .get(&changed.name)
        .copied()
        .unwrap_or_default();
    let mut options = vec![
        Choice {
            id: "keep_mine".to_owned(),
            label: "Keep mine".to_owned(),
            selected: pick == ClashPick::KeepMine,
        },
        Choice {
            id: "take_theirs".to_owned(),
            label: "Take theirs".to_owned(),
            selected: pick == ClashPick::TakeTheirs,
        },
    ];
    if let Some(add_as) = &clash.add_as {
        options.push(Choice {
            id: "add_as".to_owned(),
            label: format!("Add as \u{201c}{add_as}\u{201d}"),
            selected: pick == ClashPick::AddAs,
        });
    }
    Some(ClashRow {
        agent: changed.name.clone(),
        text: format!(
            "{} is in {place} and already here, with other {}.",
            changed.name,
            joined_words(&fields)
        ),
        options,
    })
}

fn agent_summary(agent: &FlowAgent, named: &Named) -> String {
    let mut parts = vec![
        named
            .model(&agent.name)
            .unwrap_or_else(|| agent.model.clone()),
    ];
    parts.extend(agent.skills.iter().map(|s| skill_word(s)));
    parts.join(" \u{b7} ")
}

/// A skill as the preview names it: a folder by its last part.
fn skill_word(skill: &str) -> String {
    std::path::Path::new(skill)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|_| std::path::Path::new(skill).is_absolute())
        .unwrap_or(skill)
        .to_owned()
}

fn agents_section(
    file: &FlowFile,
    proposal: &Proposal,
    named: &Named,
    whole: bool,
) -> Option<AgentsSection> {
    let of = |name: &str| {
        file.agents
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    };
    let why = |name: &str| {
        proposal.why.iter().find_map(|w| match &w.about {
            otaq_core::flow::proposal::About::Agent { name: of }
                if of.eq_ignore_ascii_case(name) =>
            {
                Some(w.text.clone())
            }
            _ => None,
        })
    };
    let rows: Vec<AgentRow> = proposal
        .diff
        .agents
        .iter()
        .filter(|a| a.change == AgentChangeKind::Added)
        .filter_map(|a| of(&a.name))
        .map(|agent| AgentRow {
            name: agent.name.clone(),
            summary: agent_summary(agent, named),
            tag: NEW.to_owned(),
            instructions: (!agent.instructions.trim().is_empty())
                .then(|| format!("\u{201c}{}\u{201d}", agent.instructions.trim())),
            skills_label: "Skills".to_owned(),
            skills: agent.skills.iter().map(|s| skill_word(s)).collect(),
            why: why(&agent.name),
        })
        .collect();
    let there: Vec<&str> = proposal
        .diff
        .agents
        .iter()
        .filter(|a| a.change == AgentChangeKind::Same)
        .map(|a| a.name.as_str())
        .collect();
    if rows.is_empty() && !(whole && !file.agents.is_empty()) {
        return None;
    }
    let already = match (rows.is_empty(), there.is_empty()) {
        (true, true) => Some("None".to_owned()),
        (true, false) => Some(format!(
            "None \u{b7} already there: {}",
            there.join(" \u{b7} ")
        )),
        (false, false) if whole => Some(format!("Already there: {}", there.join(" \u{b7} "))),
        _ => None,
    };
    Some(AgentsSection {
        heading: AGENTS_IT_ADDS.to_owned(),
        rows,
        already,
    })
}

const TOOL_LABEL: &str = "Tool";
const MODEL_LABEL: &str = "Model";

fn model_fix(agent: &str, facts: &Facts) -> ModelFix {
    let tools = facts.tools;
    let picked = facts
        .choices
        .models
        .get(agent)
        .and_then(|m| shipped::model_of(m).ok());
    let tool = match &picked {
        Some(AgentModel::Tool { tool, .. }) => Some(*tool),
        _ => None,
    };
    let mut choices = vec![Choice {
        id: DEFAULT.to_owned(),
        label: super::agent_page::DEFAULT_MODEL.to_owned(),
        selected: picked == Some(AgentModel::Default),
    }];
    choices.extend(
        Tool::ALL
            .iter()
            .filter(|t| tools.is_connected(**t))
            .map(|t| Choice {
                id: t.as_str().to_owned(),
                label: t.display_name().to_owned(),
                selected: tool == Some(*t),
            }),
    );
    let picker = tool.map(|tool| {
        let model = match &picked {
            Some(AgentModel::Tool { model, .. }) => model.as_deref(),
            _ => None,
        };
        model_picker::picker(tool, tools.state(tool), model, false, None)
    });
    ModelFix {
        agent: agent.to_owned(),
        tool_label: TOOL_LABEL.to_owned(),
        tools: choices,
        picker,
        model_label: MODEL_LABEL.to_owned(),
    }
}

/// "Security reviewer: Claude Code isn't connected", "Security reviewer: opus isn't in Claude Code's model list".
fn model_problem(agent: &str, file: &FlowFile, tools: &AgentTools, message: &str) -> String {
    let Some(text) = file
        .agents
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(agent))
        .map(|a| a.model.as_str())
    else {
        return message.to_owned();
    };
    match shipped::model_of(text) {
        Ok(AgentModel::Tool { tool, .. }) if !tools.is_connected(tool) => {
            format!("{agent}: {} isn\u{2019}t connected", tool.display_name())
        }
        Ok(AgentModel::Tool {
            tool,
            model: Some(model),
        }) => format!(
            "{agent}: {model} isn\u{2019}t in {}\u{2019}s model list",
            tool.display_name()
        ),
        _ => message.to_owned(),
    }
}

fn problems(file: &FlowFile, facts: &Facts) -> Vec<ProblemRow> {
    // A skill both an agent and its stage name is one problem with one fix.
    let mut asked = HashSet::new();
    facts
        .proposal
        .problems
        .iter()
        .filter(|p| match &p.fix {
            Some(Fix::ImportSkillFolder { skill }) => asked.insert(skill.to_lowercase()),
            _ => true,
        })
        .map(|p| match &p.fix {
            Some(Fix::ImportSkillFolder { skill }) => {
                let picked = facts.choices.skill_folders.get(skill).cloned();
                ProblemRow {
                    // A folder that can't be imported keeps the core's reason.
                    text: if p.message.contains("can\u{2019}t be imported")
                        || p.message.contains("can't be imported")
                    {
                        p.message.clone()
                    } else {
                        format!("{skill} isn\u{2019}t on this machine")
                    },
                    fixed: picked.is_some(),
                    fix: Some(FixView::Folder {
                        skill: skill.clone(),
                        label: "Import its folder\u{2026}".to_owned(),
                        picker_title: format!("The folder of {skill}"),
                        picked,
                    }),
                }
            }
            Some(Fix::PickModel { agent }) => ProblemRow {
                text: model_problem(agent, file, facts.tools, &p.message),
                fixed: facts.choices.models.contains_key(agent),
                fix: Some(FixView::Model {
                    label: "Pick another model".to_owned(),
                    fix: Box::new(model_fix(agent, facts)),
                }),
            },
            None => ProblemRow {
                text: p.message.clone(),
                fix: None,
                fixed: false,
            },
        })
        .collect()
}

/// "8 tickets in shop-web are still working".
fn working_words(n: usize, project: &str) -> String {
    match n {
        1 => format!("1 ticket in {project} is still working"),
        n => format!("{n} tickets in {project} are still working"),
    }
}

/// When it applies, in one line (FLW-05, FLW-06).
fn applies(file: &FlowFile, whole: bool, facts: &Facts) -> Option<String> {
    let proposal = facts.proposal;
    let project = &proposal.project_name;
    let diff = &proposal.diff;
    let mut parts: Vec<String> = Vec::new();
    if diff.against_waiting {
        parts.push(
            "Another change already waits for a quiet board: this one joins it, and both apply together."
                .to_owned(),
        );
    }
    if diff.waits {
        let working = still_working(facts.tickets, facts.base, &file.flow, &HashSet::new()).len();
        let what = if whole {
            "The new flow"
        } else if diff.stages.len() == 1 && matches!(diff.stages[0].change, Change::Added { .. }) {
            "The new stage"
        } else {
            "The stage change"
        };
        parts.push(if working > 0 {
            format!(
                "{what} waits for a quiet board: {}, and no new ticket starts meanwhile.",
                working_words(working, project)
            )
        } else if facts.tickets.is_empty() {
            format!("Applies now: {project} has no tickets yet.")
        } else {
            format!("Applies now: no ticket in {project} is working.")
        });
    }
    if proposal.scope() == Scope::Stages && !diff.stages.is_empty() {
        parts.push("Agents stay as they are.".to_owned());
    }
    if diff
        .agents
        .iter()
        .any(|a| a.change == AgentChangeKind::Changed)
    {
        parts.push("Agent changes apply to the next session that starts.".to_owned());
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The picks as Apply sends them: each clash's pick (Keep mine by default); a folder or model only where one is
/// still asked for.
fn choices_kept(proposal: &Proposal, picks: &Choices) -> Choices {
    let mut kept = Choices::default();
    for a in &proposal.diff.agents {
        if a.clash.is_some() {
            kept.clashes.insert(
                a.name.clone(),
                picks.clashes.get(&a.name).copied().unwrap_or_default(),
            );
        }
    }
    for p in &proposal.problems {
        match &p.fix {
            Some(Fix::ImportSkillFolder { skill }) => {
                if let Some(folder) = picks.skill_folders.get(skill) {
                    kept.skill_folders.insert(skill.clone(), folder.clone());
                }
            }
            Some(Fix::PickModel { agent }) => {
                if let Some(model) = picks.models.get(agent) {
                    kept.models.insert(agent.clone(), model.clone());
                }
            }
            None => {}
        }
    }
    kept
}

/// The preview.
pub fn view(facts: &Facts) -> View {
    let proposal = facts.proposal;
    let file = yaml::read(&proposal.yaml).unwrap_or_default();
    let named = Named { file: &file, facts };
    let scope = proposal.scope();
    let whole = shows_whole(proposal, facts.current);
    let by_agent = matches!(proposal.proposed_by, ProposedBy::ProjectAgent { .. });
    let (title, by) = title(proposal, facts.current, whole, facts.now);
    let agent_waits = by_agent.then(|| AGENT_WAITS.to_owned());
    let ask = by_agent.then(|| Ask {
        label: ASK_FOR_MORE_CHANGES.to_owned(),
        placeholder: "What else should the agent change?".to_owned(),
        send: "Send to the agent".to_owned(),
        back: "Back".to_owned(),
    });
    let choices = choices_kept(proposal, facts.choices);
    if proposal.diff.same {
        return View {
            project: proposal.project.clone(),
            proposal: proposal.id,
            title: "No changes".to_owned(),
            subtitle: by,
            close_label: CLOSE.to_owned(),
            same: Some(SAME.to_owned()),
            table: None,
            changes: Vec::new(),
            agents: None,
            problems: Vec::new(),
            clashes: Vec::new(),
            warnings: Vec::new(),
            applies: None,
            agent_waits,
            cancel: None,
            ask,
            apply: None,
            close: Some(CLOSE.to_owned()),
            choices,
        };
    }
    let table = (whole && scope.stages()).then(|| table(&file.flow, proposal, &named));
    let mut changes: Vec<ChangeRow> = Vec::new();
    if !whole {
        changes.extend(proposal.diff.stages.iter().map(|c| ChangeRow {
            icon: change_icon(&c.change),
            text: format!("{} \u{b7} {}", proposal.project_name, c.text),
            detail: (c.applies != Applies::QuietBoard).then(|| c.applies.text()),
        }));
    }
    let mut clashes = Vec::new();
    for changed in proposal
        .diff
        .agents
        .iter()
        .filter(|a| a.change == AgentChangeKind::Changed)
    {
        changes.extend(agent_rows(changed, facts.tools));
        clashes.extend(clash_row(changed, &proposal.source, facts.choices));
    }
    let agents = if scope.agents() {
        agents_section(&file, proposal, &named, whole)
    } else {
        None
    };
    let problems = problems(&file, facts);
    let mut warnings: Vec<WarningRow> = proposal
        .diff
        .warnings
        .iter()
        .map(|w| WarningRow {
            level: match w.level {
                Level::Amber => WarningLevel::Amber,
                Level::Red => WarningLevel::Red,
            },
            text: w.text.clone(),
        })
        .collect();
    warnings.extend(proposal.diff.removed_choices.iter().map(|r| WarningRow {
        level: WarningLevel::Amber,
        text: format!("{}.", r.text),
    }));
    let left = problems.iter().filter(|p| !p.fixed).count();
    View {
        project: proposal.project.clone(),
        proposal: proposal.id,
        title,
        subtitle: format!("{by} \u{b7} {NOTHING_CHANGES}"),
        close_label: CLOSE.to_owned(),
        same: None,
        table,
        changes,
        agents,
        applies: applies(&file, whole, facts),
        problems,
        clashes,
        warnings,
        agent_waits,
        cancel: Some(CANCEL.to_owned()),
        ask,
        apply: Some(ApplyButton {
            label: APPLY.to_owned(),
            enabled: left == 0,
            off: (left > 0).then(|| match left {
                1 => "Fix the problem above to apply it".to_owned(),
                _ => "Fix the problems above to apply it".to_owned(),
            }),
        }),
        close: None,
        choices,
    }
}

/// Onboarding's Use this flow (Apply: the new project's board is empty, so it applies at once).
pub const USE_THIS_FLOW: &str = "Use this flow";
/// Onboarding's Ask for changes: the user tells the agent in its terminal.
pub const ASK_FOR_CHANGES: &str = "Ask for changes";
/// Onboarding's Use the default (Cancel: the agent gets Cancel).
pub const USE_THE_DEFAULT: &str = "Use the default";
/// The subtitle once the agent proposed again after the user's answer.
pub const UPDATED_AFTER_YOUR_ANSWER: &str = "Updated after your answer";

/// The preview inside onboarding's Flow step (ONB-06, d0108; *s14-16*, *s14-17*): Use the default · Ask for changes ·
/// Use this flow in place of Cancel · Ask for more changes · Apply, and the agent named in the subtitle ("Proposed by
/// Flow setup (Project agent)"). `first` is the proposal the step showed first: a later one is the agent's answer
/// ("Updated after your answer"). "No changes" (the default itself) closes as Use the default.
pub fn onboarding(mut view: View, proposal: &Proposal, first: Option<u32>) -> View {
    if let Some(ask) = &mut view.ask {
        ask.label = ASK_FOR_CHANGES.to_owned();
    }
    if let Some(apply) = &mut view.apply {
        apply.label = USE_THIS_FLOW.to_owned();
    }
    if view.cancel.is_some() {
        view.cancel = Some(USE_THE_DEFAULT.to_owned());
    }
    if view.close.is_some() {
        view.close = Some(USE_THE_DEFAULT.to_owned());
    }
    if let ProposedBy::ProjectAgent { name, .. } = &proposal.proposed_by
        && view.same.is_none()
    {
        let by = if first.is_some_and(|f| f != proposal.id) {
            UPDATED_AFTER_YOUR_ANSWER.to_owned()
        } else if name.trim().is_empty() {
            "Proposed by the Project agent".to_owned()
        } else {
            format!("Proposed by {name} (Project agent)")
        };
        view.subtitle = format!("{by} \u{b7} {NOTHING_CHANGES}");
    }
    view
}

#[cfg(test)]
mod tests {
    use otaq_core::agents::{AgentList, ModelRef};
    use otaq_core::domain::Status;
    use otaq_core::flow::proposal::{
        self, About, AgentChange, Clash, FlowDiff, ProposalProblem, RemovedChoice, Warning,
        stage_changes, warnings, why_lines,
    };
    use otaq_core::flow::{default_flow, yaml};

    use super::*;
    use crate::view_models::agents::fixtures;
    use crate::view_models::card_fixtures::{NOW, ticket};

    fn tools() -> AgentTools {
        AgentTools {
            connected: vec![Tool::Claude, Tool::Codex],
            lists: Vec::new(),
            failed: Vec::new(),
        }
    }

    fn version(applied_by: AppliedBy, applied_at: i64) -> FlowVersion {
        FlowVersion {
            project: "SHOP".into(),
            version: 2,
            yaml: String::new(),
            applied_by,
            agent_name: None,
            preset: None,
            applied_at,
            current: true,
        }
    }

    fn no_agents() -> AgentList {
        AgentList {
            agents: Vec::new(),
            default_model: None,
            fallbacks: Vec::new(),
        }
    }

    fn agent_by() -> ProposedBy {
        ProposedBy::ProjectAgent {
            agent: "a1".into(),
            session: "s1".into(),
            name: "Flow setup".into(),
            tool: otaq_core::agent_tools::Tool::Claude,
        }
    }

    fn proposal(
        project: &str,
        yaml_text: &str,
        source: Source,
        by: ProposedBy,
        base: &Flow,
    ) -> Proposal {
        let (file, written) = yaml::read_with_written_ids(yaml_text).unwrap();
        let mut file = file;
        proposal::match_ids(base, &mut file.flow, &written);
        let stages = stage_changes(base, &file.flow, false);
        let waits = stages.iter().any(|c| c.change.is_shape());
        Proposal {
            id: 3,
            project: "SHOP".into(),
            project_name: project.into(),
            source,
            proposed_by: by,
            yaml: yaml::write(&file),
            problems: Vec::new(),
            diff: FlowDiff {
                against_waiting: false,
                same: false,
                warnings: warnings(&file.flow, "main", Some("GitHub")),
                agents: file
                    .agents
                    .iter()
                    .map(|a| AgentChange {
                        name: a.name.clone(),
                        change: AgentChangeKind::Added,
                        fields: Vec::new(),
                        used_in: Vec::new(),
                        clash: None,
                    })
                    .collect(),
                stages,
                removed_choices: Vec::new(),
                waits,
            },
            why: why_lines(&file),
            created_at: 0,
        }
    }

    fn build(
        proposal: &Proposal,
        current: Option<&FlowVersion>,
        tickets: &[Ticket],
        choices: &Choices,
    ) -> View {
        let base = default_flow();
        let tools = tools();
        let agents = AgentList {
            agents: Vec::new(),
            default_model: Some(ModelRef {
                tool: Tool::Claude,
                model: Some("sonnet".into()),
            }),
            fallbacks: Vec::new(),
        };
        view(&Facts {
            proposal,
            current,
            base: &base,
            tickets,
            agents: &agents,
            tools: &tools,
            choices,
            now: NOW,
        })
    }

    /// `s18-29`'s shop-web flow, as onboarding's Project agent proposes it.
    const SHOP_WEB: &str = r#"agents:
  - name: Griller
    instructions: Ask before coding.
    skills: [grill]
    model: claude:sonnet
  - name: Validator
    skills: [validate]
    model: codex:gpt-5.5
  - name: Security reviewer
    instructions: Never edit files. Read the diff and the code it touches. Fail on unescaped SQL, secrets in code or a missing auth check; say where.
    skills: [security-review]
    model: claude:opus
  - name: Rust developer
    skills: [work-ticket, commit-compact]
    model: claude:sonnet
  - name: Quick developer
    skills: [work-ticket]
    model: claude:haiku
  - name: Shipper
    skills: [open-pr]
    model: claude:haiku
flow:
  - name: Grill
    kind: agent
    agent: Griller
    skill: grill
    why: "You asked for clarification before coding in 34 of 50 sessions (Claude Code: shop-web, billing-api)"
  - name: Dev
    kind: agent
    agent: Rust developer
    skill: work-ticket
    options: [Quick developer]
    why: "Most of your sessions in shop-web end with /work-ticket and /commit-compact (Claude Code)"
  - name: Validate
    kind: check
    by: agent
    agent: Validator
    skill: validate
    rounds: 3
    why: "Most of your shop-web sessions run the tests before /commit-compact (Claude Code)"
  - name: Security
    kind: check
    by: agent
    agent: Security reviewer
    skill: security-review
    rounds: 3
    why: "From 41 /security-review runs in shop-web and billing-api"
  - name: Review
    kind: check
    by: you
    why: "Your PRs on GitHub always get one review before merging (gh)"
  - name: Done
    kind: delivery
    agent: Shipper
    mode: automatic
    delivers: pr
    why: "No evidence found; added from the Team PR preset"
"#;

    /// `s18-29`: a whole flow, one row per stage with its why line, the agents it adds, the amber warning, and when
    /// it applies.
    #[test]
    fn a_whole_flow_s18_29() {
        let p = proposal(
            "shop-web",
            SHOP_WEB,
            Source::Document { file: None },
            agent_by(),
            &default_flow(),
        );
        let shipped = version(AppliedBy::Shipped, 0);
        let v = build(&p, Some(&shipped), &[], &Choices::default());
        assert_eq!(v.title, "A flow for shop-web");
        assert_eq!(
            v.subtitle,
            "Proposed by the Project agent \u{b7} nothing changes until you apply it"
        );
        let t = v.table.as_ref().expect("a whole flow is a table");
        assert_eq!(
            t.headings,
            ["STAGE", "AGENT \u{b7} MODEL", "OPTIONS A TICKET CAN PICK"]
        );
        assert_eq!(
            t.always,
            "Backlog \u{2192} Ready for Dev \u{2192} \u{2026} \u{2192} Done: always first and last"
        );
        let rows: Vec<(&str, &str, &str, Option<&str>, &str)> = t
            .rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.kind.as_str(),
                    r.agent.as_str(),
                    r.model.as_deref(),
                    r.options.as_str(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "Grill",
                    "Agent",
                    "Griller",
                    Some("Claude Code \u{b7} sonnet"),
                    NONE
                ),
                (
                    "Dev",
                    "Agent",
                    "Rust developer",
                    Some("Claude Code \u{b7} sonnet"),
                    "Quick developer"
                ),
                (
                    "Validate",
                    "Check \u{b7} agent \u{b7} 3 rounds",
                    "Validator",
                    Some("Codex \u{b7} gpt-5.5"),
                    NONE
                ),
                (
                    "Security",
                    "Check \u{b7} agent \u{b7} 3 rounds",
                    "Security reviewer",
                    Some("Claude Code \u{b7} opus"),
                    NONE
                ),
                ("Review", "Check \u{b7} you", "You", None, NONE),
                (
                    "Done",
                    "Delivery \u{b7} PR \u{b7} automatic",
                    "Shipper",
                    Some("Claude Code \u{b7} haiku"),
                    NONE
                ),
            ]
        );
        let icons: Vec<IconName> = t.rows.iter().map(|r| r.icon).collect();
        assert_eq!(
            icons,
            [
                IconName::Bot,
                IconName::Bot,
                IconName::ShieldCheck,
                IconName::ShieldCheck,
                IconName::UserCheck,
                IconName::GitPullRequest
            ]
        );
        assert_eq!(
            t.rows[0].why.as_deref(),
            Some(
                "You asked for clarification before coding in 34 of 50 sessions (Claude Code: shop-web, billing-api)"
            )
        );
        assert_eq!(
            t.rows[5].why.as_deref(),
            Some("No evidence found; added from the Team PR preset")
        );
        assert!(t.rows[0].options_muted && !t.rows[1].options_muted);

        let agents = v.agents.as_ref().expect("AGENTS IT ADDS");
        assert_eq!(agents.heading, "AGENTS IT ADDS");
        let summaries: Vec<(&str, &str)> = agents
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.summary.as_str()))
            .collect();
        assert_eq!(
            summaries,
            [
                ("Griller", "Claude Code \u{b7} sonnet \u{b7} grill"),
                ("Validator", "Codex \u{b7} gpt-5.5 \u{b7} validate"),
                (
                    "Security reviewer",
                    "Claude Code \u{b7} opus \u{b7} security-review"
                ),
                (
                    "Rust developer",
                    "Claude Code \u{b7} sonnet \u{b7} work-ticket \u{b7} commit-compact"
                ),
                (
                    "Quick developer",
                    "Claude Code \u{b7} haiku \u{b7} work-ticket"
                ),
                ("Shipper", "Claude Code \u{b7} haiku \u{b7} open-pr"),
            ]
        );
        assert!(agents.rows.iter().all(|r| r.tag == "new"));
        assert_eq!(
            agents.rows[2].instructions.as_deref(),
            Some(
                "\u{201c}Never edit files. Read the diff and the code it touches. Fail on unescaped SQL, secrets in code or a missing auth check; say where.\u{201d}"
            )
        );
        assert_eq!(agents.rows[2].skills_label, "Skills");
        assert_eq!(agents.rows[2].skills, ["security-review"]);
        assert_eq!(agents.already, None);

        assert_eq!(
            v.warnings,
            [WarningRow {
                level: WarningLevel::Amber,
                text: "Done opens a PR on GitHub without asking you.".into()
            }]
        );
        assert_eq!(
            v.applies.as_deref(),
            Some("Applies now: shop-web has no tickets yet.")
        );
        assert_eq!(
            v.agent_waits.as_deref(),
            Some("The agent waits for your answer in its terminal")
        );
        assert_eq!(v.cancel.as_deref(), Some("Cancel"));
        assert_eq!(
            v.ask.as_ref().map(|a| a.label.as_str()),
            Some("Ask for more changes")
        );
        let apply = v.apply.as_ref().unwrap();
        assert_eq!((apply.label.as_str(), apply.enabled), ("Apply", true));
        assert!(v.changes.is_empty() && v.problems.is_empty() && v.close.is_none());
    }

    /// `s18-29`'s second frame: a later change lists what changes, with Used in, and when it waits.
    #[test]
    fn a_later_change_s18_29() {
        let base = default_flow();
        let text = "flow:
  - name: Dev
    kind: agent
    agent: Developer
    skill: work-ticket
  - name: Validate
    kind: check
    by: agent
    agent: Validator
    skill: validate
    rounds: 3
  - name: Review
    kind: check
    by: you
  - name: QA
    kind: check
    by: you
  - name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    skill: open-pr
    mode: click
    delivers: either
";
        let mut p = proposal(
            "shop-web",
            text,
            Source::Document { file: None },
            agent_by(),
            &base,
        );
        p.diff.agents = vec![AgentChange {
            name: "Security reviewer".into(),
            change: AgentChangeKind::Changed,
            fields: vec![
                AgentField {
                    field: "instructions".into(),
                    from: "Never edit files.".into(),
                    to: "Never edit files. Fail on any unescaped SQL".into(),
                },
                AgentField {
                    field: "model".into(),
                    from: "claude:opus".into(),
                    to: "claude:fable".into(),
                },
            ],
            used_in: vec![StageUse {
                project: Some("shop-web".into()),
                stage: "Security".into(),
                option: false,
            }],
            clash: None,
        }];
        let changed = version(AppliedBy::User, NOW);
        let working: Vec<Ticket> = (1..=8)
            .map(|n| {
                let mut t = ticket(&format!("SHOP-{n}"), "work", Status::Working);
                t.place = otaq_core::domain::Place::InStage("dev".into());
                t
            })
            .collect();
        let v = build(&p, Some(&changed), &working, &Choices::default());
        assert_eq!(v.title, "Changes to shop-web\u{2019}s flow");
        assert!(v.table.is_none());
        let texts: Vec<(&str, Option<&str>)> = v
            .changes
            .iter()
            .map(|c| (c.text.as_str(), c.detail.as_deref()))
            .collect();
        assert_eq!(
            texts,
            [
                ("shop-web \u{b7} stage QA (your check) after Review", None),
                (
                    "Security reviewer \u{b7} instructions: + \u{201c}Fail on any unescaped SQL\u{201d}",
                    Some("Used in: shop-web (Security)")
                ),
                (
                    "Security reviewer \u{b7} model: opus \u{2192} fable",
                    Some("Used in: shop-web (Security)")
                ),
            ]
        );
        assert_eq!(v.changes[0].icon, IconName::Plus);
        assert_eq!(
            v.applies.as_deref(),
            Some(
                "The new stage waits for a quiet board: 8 tickets in shop-web are still working, and no new \
                 ticket starts meanwhile. Agent changes apply to the next session that starts."
            )
        );
        assert!(v.agents.is_none(), "it adds no agent");
    }

    /// `s18-31`: a preset's amber warning, the red one on a change the user asked for, the problems that keep Apply
    /// off until fixed, an import clash with Keep mine picked, and "No changes".
    #[test]
    fn warnings_and_problems_s18_31() {
        let solo = "agents:
  - name: Developer
    skills: [work-ticket]
    model: default
  - name: Validator
    skills: [validate]
    model: default
flow:
  - name: Dev
    kind: agent
    agent: Developer
    skill: work-ticket
  - name: Validate
    kind: check
    by: agent
    agent: Validator
    skill: validate
    rounds: 3
  - name: Done
    kind: delivery
    mode: automatic
    delivers: local
";
        let mut p = proposal(
            "billing-api",
            solo,
            Source::Preset {
                file: "solo-autopilot.yaml".into(),
            },
            ProposedBy::User,
            &default_flow(),
        );
        for a in &mut p.diff.agents {
            a.change = AgentChangeKind::Same;
        }
        let v = build(
            &p,
            Some(&version(AppliedBy::User, NOW)),
            &[],
            &Choices::default(),
        );
        assert_eq!(v.title, "Solo autopilot for billing-api");
        assert_eq!(
            v.subtitle,
            "Preset \u{b7} nothing changes until you apply it"
        );
        let rows = &v.table.as_ref().unwrap().rows;
        assert_eq!(rows[2].kind, "Delivery \u{b7} Local merge \u{b7} automatic");
        assert_eq!(
            (rows[2].agent.as_str(), rows[2].agent_muted),
            ("No agent", true)
        );
        assert_eq!(rows[2].icon, IconName::GitMerge);
        assert_eq!(
            rows[0].model.as_deref(),
            Some("default \u{b7} Claude Code \u{b7} sonnet")
        );
        let agents = v.agents.as_ref().unwrap();
        assert!(agents.rows.is_empty());
        assert_eq!(
            agents.already.as_deref(),
            Some("None \u{b7} already there: Developer \u{b7} Validator")
        );
        assert_eq!(
            v.warnings,
            [WarningRow {
                level: WarningLevel::Amber,
                text: "Done merges into main without asking you.".into()
            }]
        );
        assert!(
            v.ask.is_none() && v.agent_waits.is_none(),
            "a preset has no agent to ask"
        );
        assert_eq!(v.cancel.as_deref(), Some("Cancel"));

        // A change the user asked for drops Validate: red.
        let dropped = "flow:
  - name: Dev
    kind: agent
    agent: Developer
    skill: work-ticket
  - name: Done
    kind: delivery
    mode: automatic
    delivers: local
";
        let p = proposal(
            "docs-site",
            dropped,
            Source::Document { file: None },
            agent_by(),
            &default_flow(),
        );
        let v = build(
            &p,
            Some(&version(AppliedBy::User, NOW)),
            &[],
            &Choices::default(),
        );
        assert_eq!(
            v.warnings,
            [WarningRow {
                level: WarningLevel::Red,
                text: "Done merges into main without any check before it.".into()
            }]
        );
        assert!(
            v.changes
                .iter()
                .any(|c| c.text == "docs-site \u{b7} stage Validate removed"
                    && c.icon == IconName::Minus)
        );

        // An import with problems: Apply stays off until each fix is picked.
        let mut p = proposal(
            "shop-web",
            SHOP_WEB,
            Source::Document {
                file: Some("shop-web.flow.yaml".into()),
            },
            ProposedBy::User,
            &default_flow(),
        );
        p.problems = vec![
            ProposalProblem {
                about: About::Stage {
                    id: "security".into(),
                    name: "Security".into(),
                },
                message: "Security names the skill `security-review`, which isn't on this machine".into(),
                fix: Some(Fix::ImportSkillFolder {
                    skill: "security-review".into(),
                }),
            },
            ProposalProblem {
                about: About::Agent {
                    name: "Security reviewer".into(),
                },
                message: "Security reviewer's model opus isn't in Claude Code's model list: pick a model from it".into(),
                fix: Some(Fix::PickModel {
                    agent: "Security reviewer".into(),
                }),
            },
            ProposalProblem {
                about: About::Flow,
                message: "If Grill and Dev are both skipped, a Fail at Review has nowhere to go.".into(),
                fix: None,
            },
        ];
        // The agent and its stage both name the missing skill: one problem.
        let twice = ProposalProblem {
            about: About::Agent {
                name: "Security reviewer".into(),
            },
            ..p.problems[0].clone()
        };
        p.problems.insert(1, twice);
        p.diff.agents[2] = AgentChange {
            name: "Security reviewer".into(),
            change: AgentChangeKind::Changed,
            fields: vec![AgentField {
                field: "instructions".into(),
                from: "Be careful.".into(),
                to: "Never edit files.".into(),
            }],
            used_in: Vec::new(),
            clash: Some(Clash {
                add_as: Some("Security reviewer (shop-web.flow.yaml)".into()),
            }),
        };
        let mut no_claude = tools();
        no_claude.connected = vec![Tool::Codex];
        let base = default_flow();
        let agents = no_agents();
        let built = |p: &Proposal, choices: &Choices, tools: &AgentTools| {
            view(&Facts {
                proposal: p,
                current: None,
                base: &base,
                tickets: &[],
                agents: &agents,
                tools,
                choices,
                now: NOW,
            })
        };
        let v = built(&p, &Choices::default(), &no_claude);
        assert_eq!(v.title, "A flow imported from shop-web.flow.yaml");
        assert_eq!(
            v.subtitle,
            "Import \u{b7} nothing changes until you apply it"
        );
        let texts: Vec<&str> = v.problems.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "security-review isn\u{2019}t on this machine",
                "Security reviewer: Claude Code isn\u{2019}t connected",
                "If Grill and Dev are both skipped, a Fail at Review has nowhere to go.",
            ]
        );
        assert!(matches!(
            &v.problems[0].fix,
            Some(FixView::Folder { label, .. }) if label == "Import its folder\u{2026}"
        ));
        assert!(matches!(
            &v.problems[1].fix,
            Some(FixView::Model { label, .. }) if label == "Pick another model"
        ));
        let apply = v.apply.as_ref().unwrap();
        assert!(!apply.enabled);
        assert_eq!(
            apply.off.as_deref(),
            Some("Fix the problems above to apply it")
        );
        assert_eq!(
            v.clashes,
            [ClashRow {
                agent: "Security reviewer".into(),
                text: "Security reviewer is in the file and already here, with other instructions."
                    .into(),
                options: vec![
                    Choice {
                        id: "keep_mine".into(),
                        label: "Keep mine".into(),
                        selected: true
                    },
                    Choice {
                        id: "take_theirs".into(),
                        label: "Take theirs".into(),
                        selected: false
                    },
                    Choice {
                        id: "add_as".into(),
                        label: "Add as \u{201c}Security reviewer (shop-web.flow.yaml)\u{201d}"
                            .into(),
                        selected: false
                    },
                ]
            }]
        );
        assert_eq!(
            v.choices.clashes.get("Security reviewer"),
            Some(&ClashPick::KeepMine),
            "Keep mine is sent by default"
        );

        // A folder and a model picked: only the rule problem is left.
        let mut picks = Choices::default();
        picks.skill_folders.insert(
            "security-review".into(),
            "/tmp/skills/security-review".into(),
        );
        picks
            .models
            .insert("Security reviewer".into(), "codex:gpt-5.5".into());
        picks
            .clashes
            .insert("Security reviewer".into(), ClashPick::AddAs);
        let v = built(&p, &picks, &no_claude);
        assert_eq!(
            v.problems.iter().map(|p| p.fixed).collect::<Vec<_>>(),
            [true, true, false]
        );
        assert!(!v.apply.as_ref().unwrap().enabled);
        assert_eq!(
            v.apply.as_ref().unwrap().off.as_deref(),
            Some("Fix the problem above to apply it")
        );
        let Some(FixView::Model { fix, .. }) = &v.problems[1].fix else {
            panic!("a model fix")
        };
        assert!(fix.tools.iter().any(|t| t.id == "codex" && t.selected));
        assert!(fix.picker.is_some(), "Codex's picker");
        assert_eq!(v.choices, picks);
        p.problems.retain(|p| p.fix.is_some());
        let v = built(&p, &picks, &no_claude);
        assert!(v.apply.as_ref().unwrap().enabled);

        // Nothing differs.
        let mut same = proposal(
            "shop-web",
            "flow: []\n",
            Source::Document { file: None },
            agent_by(),
            &base,
        );
        same.diff = FlowDiff {
            same: true,
            ..FlowDiff::default()
        };
        let v = build(&same, None, &[], &Choices::default());
        assert_eq!(v.title, "No changes");
        assert_eq!(v.subtitle, "Proposed by the Project agent");
        assert_eq!(
            v.same.as_deref(),
            Some("This is the same as your current flow.")
        );
        assert_eq!(v.close.as_deref(), Some("Close"));
        assert!(v.apply.is_none() && v.cancel.is_none());
        assert_eq!(
            v.ask.as_ref().map(|a| a.label.as_str()),
            Some("Ask for more changes")
        );
    }

    /// `s18-35`: Back lists only the project's stages and says agents stay; a rename is a rename.
    #[test]
    fn back_and_rename_s18_35() {
        let with_qa = "flow:
  - name: Dev
    kind: agent
    agent: Developer
    skill: work-ticket
  - name: Validate
    kind: check
    by: agent
    agent: Validator
    skill: validate
    rounds: 3
    column: with-previous
  - name: Security
    kind: check
    by: agent
    agent: Security reviewer
    skill: security-review
    rounds: 3
  - name: QA
    kind: check
    by: you
  - name: Review
    kind: check
    by: you
  - name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    skill: open-pr
    mode: click
    delivers: either
";
        let current = yaml::read(with_qa).unwrap().flow;
        let previous = with_qa.replace("  - name: QA\n    kind: check\n    by: you\n", "");
        let mut p = proposal(
            "shop-web",
            &previous,
            Source::Previous { version: 1 },
            ProposedBy::User,
            &current,
        );
        p.diff.waits = true;
        let yesterday = version(AppliedBy::ProjectAgent, NOW - 86_400_000);
        let tools = tools();
        let agents = no_agents();
        let v = view(&Facts {
            proposal: &p,
            current: Some(&yesterday),
            base: &current,
            tickets: &[],
            agents: &agents,
            tools: &tools,
            choices: &Choices::default(),
            now: NOW,
        });
        assert_eq!(v.title, "Back to shop-web\u{2019}s previous flow");
        assert_eq!(
            v.subtitle,
            "The flow before yesterday\u{2019}s change \u{b7} nothing changes until you apply it"
        );
        assert!(v.table.is_none() && v.agents.is_none(), "stages only");
        assert_eq!(
            v.changes
                .iter()
                .map(|c| (c.icon, c.text.as_str()))
                .collect::<Vec<_>>(),
            [(IconName::Minus, "shop-web \u{b7} stage QA removed")]
        );
        assert_eq!(
            v.applies.as_deref(),
            Some("Applies now: shop-web has no tickets yet. Agents stay as they are.")
        );
        assert!(v.ask.is_none() && v.agent_waits.is_none());
        assert_eq!(
            (
                v.cancel.as_deref(),
                v.apply.as_ref().map(|a| a.label.as_str())
            ),
            (Some("Cancel"), Some("Apply"))
        );

        // billing-api's Project agent renames Review to QA.
        let base = default_flow();
        let renamed = "flow:
  - name: Dev
    kind: agent
    agent: Developer
    skill: work-ticket
  - name: Validate
    kind: check
    by: agent
    agent: Validator
    skill: validate
    rounds: 3
  - name: QA
    kind: check
    by: you
  - name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    skill: open-pr
    mode: click
    delivers: either
";
        let p = proposal(
            "billing-api",
            renamed,
            Source::Document { file: None },
            agent_by(),
            &base,
        );
        let v = build(
            &p,
            Some(&version(AppliedBy::User, NOW)),
            &[],
            &Choices::default(),
        );
        assert_eq!(v.title, "Changes to billing-api\u{2019}s flow");
        assert_eq!(
            v.subtitle,
            "Proposed by the Project agent \u{b7} nothing changes until you apply it"
        );
        assert_eq!(
            v.changes
                .iter()
                .map(|c| (c.icon, c.text.as_str(), c.detail.as_deref()))
                .collect::<Vec<_>>(),
            [(
                IconName::Pencil,
                "billing-api \u{b7} stage Review \u{2192} QA",
                Some("applies at once")
            )]
        );
        assert_eq!(v.applies, None, "a rename doesn't wait");
        assert_eq!(v.agent_waits.as_deref(), Some(AGENT_WAITS));
        assert!(v.ask.is_some());
    }

    #[test]
    fn removed_choices_warn_in_amber() {
        let mut p = proposal(
            "shop-web",
            SHOP_WEB,
            Source::Document { file: None },
            agent_by(),
            &default_flow(),
        );
        p.diff.removed_choices = vec![RemovedChoice {
            stage: "Dev".into(),
            chose: "Quick developer".into(),
            runs: "Rust developer".into(),
            tickets: vec!["SHOP-2".parse().unwrap()],
            text: "1 ticket chose Quick developer for Dev; it will use Rust developer".into(),
        }];
        let v = build(&p, None, &[], &Choices::default());
        assert!(v.warnings.contains(&WarningRow {
            level: WarningLevel::Amber,
            text: "1 ticket chose Quick developer for Dev; it will use Rust developer.".into()
        }));
    }

    #[test]
    fn preset_names_and_used_in() {
        assert_eq!(preset_name("solo-autopilot.yaml"), "Solo autopilot");
        assert_eq!(preset_name("team-pr.yaml"), "Team PR");
        assert_eq!(preset_name("gone.yaml"), "gone.yaml");
        let uses = [
            StageUse {
                project: Some("shop-web".into()),
                stage: "Security".into(),
                option: false,
            },
            StageUse {
                project: Some("billing-api".into()),
                stage: "Dev".into(),
                option: true,
            },
            StageUse {
                project: Some("shop-web".into()),
                stage: "Dev".into(),
                option: false,
            },
        ];
        assert_eq!(
            used_in(&uses).as_deref(),
            Some("Used in: shop-web (Security, Dev) \u{b7} billing-api (Dev (option))")
        );
        let _ = fixtures::claude_list;
        let _ = Warning {
            level: Level::Red,
            stage: None,
            text: String::new(),
        };
    }

    #[test]
    fn choices_convert_to_apply_choices() {
        let mut picks = Choices::default();
        picks.clashes.insert("A".into(), ClashPick::TakeTheirs);
        picks.models.insert("A".into(), "claude:opus".into());
        let apply = picks.apply_choices();
        assert_eq!(apply.clashes.get("A"), Some(&ClashChoice::TakeTheirs));
        assert_eq!(
            apply.models.get("A").map(String::as_str),
            Some("claude:opus")
        );
        let json = serde_json::to_value(&picks).unwrap();
        let back: otaq_core::flow::proposal::ApplyChoices = serde_json::from_value(json).unwrap();
        assert_eq!(back, apply);
    }

    /// `s14-16`/`s14-17`: the onboarding preview's buttons, the agent named, and "Updated after your answer" once
    /// it proposed again; a preset (the user's) has no Ask for changes.
    #[test]
    fn the_onboarding_buttons_s14_16() {
        let p = proposal(
            "shop-web",
            SHOP_WEB,
            Source::Document { file: None },
            agent_by(),
            &default_flow(),
        );
        let shipped = version(AppliedBy::Shipped, 0);
        let v = onboarding(
            build(&p, Some(&shipped), &[], &Choices::default()),
            &p,
            None,
        );
        assert_eq!(v.title, "A flow for shop-web");
        assert_eq!(
            v.subtitle,
            "Proposed by Flow setup (Project agent) \u{b7} nothing changes until you apply it"
        );
        assert_eq!(v.cancel.as_deref(), Some("Use the default"));
        assert_eq!(
            v.ask.as_ref().map(|a| a.label.as_str()),
            Some("Ask for changes")
        );
        assert_eq!(
            v.apply.as_ref().map(|a| a.label.as_str()),
            Some("Use this flow")
        );
        assert!(v.apply.as_ref().is_some_and(|a| a.enabled));
        assert_eq!(
            v.agent_waits.as_deref(),
            Some("The agent waits for your answer in its terminal")
        );
        assert_eq!(
            v.applies.as_deref(),
            Some("Applies now: shop-web has no tickets yet.")
        );
        // The first proposal again: unchanged. A later one: the agent's answer.
        let again = onboarding(
            build(&p, Some(&shipped), &[], &Choices::default()),
            &p,
            Some(3),
        );
        assert_eq!(again.subtitle, v.subtitle);
        let updated = onboarding(
            build(&p, Some(&shipped), &[], &Choices::default()),
            &p,
            Some(2),
        );
        assert_eq!(
            updated.subtitle,
            "Updated after your answer \u{b7} nothing changes until you apply it"
        );

        // A preset picked in the step: the user's own, so no Ask for changes and no agent waiting.
        let team = otaq_core::flow::presets::preset("team-pr.yaml").unwrap();
        let preset = proposal(
            "shop-web",
            &yaml::write(&team.read()),
            Source::Preset {
                file: "team-pr.yaml".into(),
            },
            ProposedBy::User,
            &default_flow(),
        );
        let v = onboarding(
            build(&preset, Some(&shipped), &[], &Choices::default()),
            &preset,
            None,
        );
        assert_eq!(v.title, "Team PR for shop-web");
        assert_eq!(
            v.subtitle,
            "Preset \u{b7} nothing changes until you apply it"
        );
        assert_eq!(v.cancel.as_deref(), Some("Use the default"));
        assert!(v.ask.is_none());
        assert!(v.agent_waits.is_none());
        assert_eq!(
            v.apply.as_ref().map(|a| a.label.as_str()),
            Some("Use this flow")
        );
    }
}
