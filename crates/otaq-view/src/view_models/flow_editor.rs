//! The flow editor (T-0471; FED-01, FED-03, FED-04, FED-07; d0115 "One flow editor", d0108, d0087, d0086; *27 → Row ·
//! One component* `s27-02`, *Row · In onboarding* `s27-03`, *Row · The agent changes it* `s27-04`, *Row · In the
//! project* `s27-05`, *Row · Changes that wait* `s27-07`): one component for onboarding's Flow step and for changing a
//! project's flow, in three parts.
//!
//! - **Start from** (left): the flow in use (in the project), the four presets, **Made for <project>** (an Ask per
//!   connected tool, each finished proposal its own entry, never a session count) and **Import a flow…** (a file, or
//!   another project's flow). Picking one copies it into the draft ([`Pick`]).
//! - **The flow** (middle): the animation of exactly what is in the draft ("Watch it", folded while a step's details
//!   are open), the steps as a list (drag to reorder, **+** between them on hover, **Add a stage**), the selected
//!   step's [`Details`] (Name, Kind, agent, Skill, "A ticket can", Remove stage…) and the legend of the four kinds.
//!   A step the Project agent changed is tinted with its mark and an Undo (the draft's marks, T-0470); problems from
//!   the draft's check show on their step and keep Apply off with the reason. The change line says when the draft
//!   would reach tickets ([`DraftApplies`], FED-08).
//! - **The Project agent** (right, [`AgentPanel`]): before anything is asked, "Not started", "<tool> · <model> ·
//!   starts when you ask", Otaq's own hint and fixed chips. **Nothing starts until the user types or clicks a chip**
//!   (d0087); then it is the project's Project agent on the `flow` skill: its state, tokens and terminal.
//!
//! Every user edit is a [`StageEdit`] the desktop applies to the draft's document with [`edited`] and sends whole as
//! `flow.draft_update`; Start from, Undo and Undo all are the draft's own edits.
//!
//! **Captions** (d0115 "No description field"): a step's caption is its skill's description; Ready for Dev, a check
//! by you and Delivery have Otaq's fixed caption for their kind. Otaq's shipped skills are written for agents, so
//! their captions are Otaq's own words ([`shipped_caption`], Q-B).

use std::collections::BTreeMap;

use otaq_core::agent_tools::Tool;
use otaq_core::agents::Agent;
use otaq_core::flow::draft::{DraftApplies, Inside, Mark, StageMark};
use otaq_core::flow::engine::back_to;
use otaq_core::flow::model::{
    CheckBy, Column, Delivers, FlowAgent, FlowFile, Stage, StageKind, StageOption,
};
use otaq_core::flow::presets::presets;
use otaq_core::flow::proposal::{About, Change, Proposal, Source};
use otaq_core::project_agents::first_flow::{FlowSetup, FlowSetupRow};
use otaq_core::project_agents::{AgentState, ProjectAgent};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::key_labels::KeyLabels;
use crate::keymap::KeyAction;
use crate::theme::Status as Legend;
use crate::view_models::card::tokens_label;
use crate::view_models::menu::MenuItem;
use crate::view_models::status::PillView;
use crate::view_models::tool_avatar::ToolAvatarView;
use crate::view_models::tool_problems::Tools;

/// Where the editor opened: onboarding's Flow step (Use this flow · Skip, use the default) or the project (Cancel ·
/// Apply). `FlowEditorFrom` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowEditorFrom")]
pub enum Opened {
    Onboarding,
    Project,
}

/// The editor's three kinds a step can be switched between (Delivery stays the last step's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "FlowEditorKind")]
pub enum EditorKind {
    /// "An agent works".
    Works,
    /// "An agent checks".
    AgentChecks,
    /// "You check".
    YouCheck,
}

/// One edit of the user's in the flow part (`flow_editor.edit`'s `edit`). `FlowEditorStageEdit` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "edit", rename_all = "snake_case")]
#[ts(rename = "FlowEditorStageEdit")]
pub enum StageEdit {
    Rename {
        stage: String,
        name: String,
    },
    Kind {
        stage: String,
        kind: EditorKind,
    },
    Agent {
        stage: String,
        agent: String,
    },
    /// A ticket can …: `option` is `skip` or another agent's name.
    Option {
        stage: String,
        option: String,
        on: bool,
    },
    /// Drag to reorder: the stage at index `to` among the stages (never after the Delivery stage).
    Move {
        stage: String,
        to: usize,
    },
    /// **+** after a step, or Add a stage: a new stage after `after` (`None`: first, right after Ready for Dev).
    Add {
        #[serde(default)]
        #[ts(optional)]
        after: Option<String>,
    },
    /// Remove stage….
    Remove {
        stage: String,
    },
}

/// What picking a Start from entry copies into the draft. `FlowEditorPick` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "pick", rename_all = "snake_case")]
#[ts(rename = "FlowEditorPick")]
pub enum Pick {
    /// The flow in use (`flow_in_use`).
    FlowInUse,
    /// A shipped preset (`start_from` a preset file).
    Preset { file: String },
    /// A tool's finished proposal under Made for <project> (`entry`).
    Entry { proposal: u32 },
    /// Another project's flow (Import a flow… → from a project).
    Project { project: String },
}

/// The agent's default for each kind (FED-04: "Developer for An agent works, Validator for An agent checks").
pub const WORKS_AGENT: &str = "Developer";
pub const CHECKS_AGENT: &str = "Validator";
/// The skill a new stage of each kind starts with: the default agent's own (an Agent stage needs one to read, Q-D).
pub const WORKS_SKILL: &str = "work-ticket";
pub const CHECKS_SKILL: &str = "validate";
/// An agent check's fails in a row before the card stops (the presets' 3).
pub const CHECK_ROUNDS: u32 = 3;
pub const NEW_STAGE: &str = "New stage";

/// The id a step row of Ready for Dev carries (it isn't a stage).
pub const READY_ID: &str = "ready";

/// The panel's line before anything was asked.
pub const STARTS_WHEN_YOU_ASK: &str = "\u{b7} starts when you ask";
pub const PANEL_TITLE: &str = "Project agent \u{b7} the flow";
pub const NOT_STARTED: &str = "Not started";
pub const TYPING_GOES: &str = "Typing goes to the agent";

/// The step's fixed captions (`s27-03`, `s27-05`).
pub const READY_CAPTION: &str =
    "The ticket you wrote waits here until an agent is free. Tickets start in priority order.";
pub const YOU_CHECK_CAPTION: &str =
    "You look at the change and approve it, or send it back with a note.";
pub const MERGE_CAPTION: &str = "Approved work is merged into main. The ticket is done.";
pub const PR_CAPTION: &str = "A pull request opens on GitHub; your team reviews it there.";

/// Shipped skills' captions in Otaq's words (Q-B): their SKILL.md descriptions are written for the agent.
pub fn shipped_caption(skill: &str, back: Option<&str>) -> Option<String> {
    Some(match skill {
        "work-ticket" => {
            "An agent writes the code in the ticket\u{2019}s own copy of the repo (a worktree)."
                .to_owned()
        }
        "grill" => {
            "An agent asks you questions until the ticket is clear, before any code is written."
                .to_owned()
        }
        "validate" => format!(
            "A checking agent runs the tests and lint. If something fails, the ticket goes back to {}.",
            back.unwrap_or("the stage before")
        ),
        "open-pr" => PR_CAPTION.to_owned(),
        _ => return None,
    })
}

/// A preset's line in Start from (`s27-03`), by its file.
fn preset_line(file: &str) -> &'static str {
    match file {
        "solo-autopilot.yaml" => "No review; merges when checks pass",
        "grill-first.yaml" => "Settle the ticket before code",
        "team-pr.yaml" => "Your team reviews the PR",
        _ => "Agents build and check; you review",
    }
}

/// A badge beside an entry's name: "In use" (approved colours) or "New" (a tool's proposal).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorBadge")]
pub struct Badge {
    pub label: &'static str,
    pub pill: PillView,
}

/// One entry of Start from. `FlowEditorEntry` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorEntry")]
pub struct Entry {
    /// The React key: `in-use`, the preset's file, `proposal-3`.
    pub key: String,
    /// "shop-web’s flow", "Otaq default", "By Claude Code".
    pub name: String,
    pub badge: Option<Badge>,
    /// Its steps, Ready for Dev counted.
    pub count: usize,
    pub line: String,
    /// What the draft holds now: outlined.
    pub chosen: bool,
    pub avatar: Option<ToolAvatarView>,
    pub pick: Pick,
}

/// A connected tool's row under Made for <project>. `FlowEditorToolRow` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorToolRow")]
pub struct ToolRow {
    #[ts(type = "string")]
    pub tool: Tool,
    pub avatar: ToolAvatarView,
    pub name: &'static str,
    /// "Connected", "Connected · not asked", "Working", "Plan limit", "Not signed in", "Ended without a proposal".
    pub status: String,
    /// "Ask"; `None` while it works or can't be asked.
    pub ask: Option<&'static str>,
}

/// Made for <project> (FED-03). `FlowEditorMadeFor` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorMadeFor")]
pub struct MadeFor {
    /// "MADE FOR SHOP-WEB".
    pub title: String,
    pub text: &'static str,
    /// Each finished proposal: "By Claude Code · From your Claude Code sessions".
    pub entries: Vec<Entry>,
    pub tools: Vec<ToolRow>,
}

/// Import a flow…: a file, or another project's flow. `FlowEditorImport` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorImport")]
pub struct Import {
    pub label: &'static str,
    /// `file`, then a header and `project:<KEY>` per other project.
    pub menu: Vec<MenuItem>,
    /// The open dialog's title.
    pub file_title: &'static str,
}

/// Start from (left). `FlowEditorStartFrom` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorStartFrom")]
pub struct StartFrom {
    pub title: &'static str,
    /// The flow in use (in the project), then the presets.
    pub entries: Vec<Entry>,
    pub made_for: MadeFor,
    pub import: Import,
    pub note: &'static str,
}

/// A step's mark from the Project agent: "Added", "New", "Changed", "Moved", with its why line and Undo.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorMark")]
pub struct MarkView {
    pub label: &'static str,
    /// "Changed by Checkout slowness: From 41 /security-review runs in shop-web" (hover).
    pub why: String,
    pub undo: &'static str,
}

/// One tile of the animation and its caption. `FlowEditorTile` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorTile")]
pub struct Tile {
    pub id: String,
    pub name: String,
    pub icon: IconName,
    /// "Waits", "Agent", "Check", "You", "Delivery" (long flows); "An agent works"… (five steps or fewer).
    pub kind: String,
    /// What the walking ticket says there: "Waiting", "Working · Developer", "Your check", "PR open".
    pub ticket: String,
    pub tone: PillView,
    /// "Dev · an agent works".
    pub caption_title: String,
    /// "New step" for a step the agent added.
    pub caption_mark: Option<&'static str>,
    pub caption: Option<String>,
}

/// One step of the list. `FlowEditorStep` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorStep")]
pub struct StepRow {
    /// The stage's id; [`READY_ID`] for Ready for Dev.
    pub id: String,
    pub name: String,
    pub icon: IconName,
    /// "Waits", "Agent works", "Agent checks", "You check", "Delivery · PR".
    pub kind: String,
    /// The agent's avatar and name, "You" for a check by you, "PR" / "merge" for the Delivery.
    pub avatar: Option<ToolAvatarView>,
    pub who: Option<String>,
    pub mark: Option<MarkView>,
    /// A stage the flow in use doesn't have, added in the editor (a column's Add a stage after <stage>, **+**, Add a
    /// stage): drawn dashed until it applies (FED-09, `s27-05`). A stage the agent added has its mark instead.
    pub added: bool,
    /// Its problem from the check, in the error colour.
    pub problem: Option<String>,
    pub selected: bool,
    /// Ready for Dev and the Delivery stay where they are.
    pub draggable: bool,
    /// Ready for Dev isn't a stage: no details.
    pub selectable: bool,
    /// The caption on hover.
    pub caption: Option<String>,
    /// What **+** below this step sends as `add`'s `after` (`None` for Ready for Dev: first). `None` after the
    /// Delivery, which nothing follows ([`StepRow::can_add_after`] false).
    pub add_after: Option<String>,
    pub can_add_after: bool,
}

/// One kind of the legend: "An agent works · writes code, asks you questions".
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorLegendRow")]
pub struct LegendRow {
    pub icon: IconName,
    pub name: &'static str,
    pub text: &'static str,
}

/// A segment of the Kind control.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorKindOption")]
pub struct KindOption {
    pub kind: EditorKind,
    pub label: &'static str,
}

/// The agent field: who runs it, the picker, and the hint.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorAgentField")]
pub struct AgentField {
    pub label: &'static str,
    pub avatar: Option<ToolAvatarView>,
    pub name: String,
    /// "default for An agent works", or the model ("gpt-5.5").
    pub note: Option<String>,
    /// Every agent: the stored ones, then those the draft brings ("new").
    pub items: Vec<MenuItem>,
    pub hint: &'static str,
}

/// The Skill field. T-0472 fills its empty state (Pick a skill… · Ask the agent to write it).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorSkillField")]
pub struct SkillField {
    pub label: &'static str,
    pub skill: Option<String>,
    /// "No skill yet. An agent stage needs one: it’s what the agent follows in Plan." when it has none.
    pub empty: Option<String>,
}

/// One line of "A ticket can".
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorCanRow")]
pub struct CanRow {
    /// `skip`, an agent's name, or `back` (fixed: the flow sends failed checks here).
    pub option: String,
    pub label: String,
    pub on: bool,
    pub enabled: bool,
}

/// The selected step's details (FED-04). `FlowEditorDetails` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorDetails")]
pub struct Details {
    pub stage: String,
    /// "Dev".
    pub title: String,
    pub icon: IconName,
    pub remove: Option<&'static str>,
    pub name_label: &'static str,
    pub name: String,
    pub kind_label: &'static str,
    /// The three kinds; empty for the Delivery (its kind is fixed, [`Details::fixed_kind`]).
    pub kinds: Vec<KindOption>,
    pub kind: Option<EditorKind>,
    pub fixed_kind: Option<&'static str>,
    pub agent: Option<AgentField>,
    pub skill: Option<SkillField>,
    pub can_label: &'static str,
    pub can: Vec<CanRow>,
    pub problem: Option<String>,
    pub mark: Option<MarkView>,
}

/// The flow (middle). `FlowEditorFlow` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorFlow")]
pub struct FlowPart {
    /// "shop-web’s flow".
    pub title: String,
    /// "7 steps · in use", "8 steps · 1 change, not applied", "5 steps · starting from Otaq default".
    pub summary: String,
    pub watch: &'static str,
    /// A step's details are open: the animation folds to one line.
    pub folded: bool,
    pub fold_note: &'static str,
    pub tiles: Vec<Tile>,
    pub steps: Vec<StepRow>,
    pub add_stage: &'static str,
    /// What Add a stage sends as `after`: the stage before the Delivery.
    pub add_stage_after: Option<String>,
    pub hint: &'static str,
    pub legend_title: &'static str,
    pub legend: Vec<LegendRow>,
    pub details: Option<Details>,
    /// Problems about the whole flow or an agent (a stage's show on it).
    pub problems: Vec<String>,
    /// When the draft would reach tickets (FED-08); `None` with nothing to apply and in onboarding.
    pub change_line: Option<String>,
    /// "Undo all" while the agent's changes can be taken back.
    pub undo_all: Option<&'static str>,
}

/// A fixed chip in the agent panel: the words it sends, or the tool picker. `FlowEditorChipAction` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "FlowEditorChipAction")]
pub enum ChipAction {
    /// The chip's words are the first message (asking).
    Send { message: String },
    /// "Make one for shop-web…": which tool should look.
    ChooseTool,
    /// "Ask Claude Code": the only connected tool.
    Ask {
        #[ts(type = "string")]
        tool: Tool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorChip")]
pub struct Chip {
    pub label: String,
    pub action: ChipAction,
}

/// "Which tool should read how you work?" (`s27-03`). `FlowEditorChooser` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorChooser")]
pub struct Chooser {
    pub title: &'static str,
    pub text: &'static str,
    pub tools: Vec<ChooserTool>,
    /// "Both, and compare · Two flows under Made for shop-web"; `None` in the project (Q-A).
    pub both: Option<(String, String)>,
    pub foot: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorChooserTool")]
pub struct ChooserTool {
    #[ts(type = "string")]
    pub tool: Tool,
    pub avatar: ToolAvatarView,
    pub name: &'static str,
    pub detail: &'static str,
}

/// The agent panel (FED-07). `FlowEditorAgent` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorAgent")]
pub struct AgentPanel {
    pub title: &'static str,
    pub pill: PillView,
    pub avatar: Option<ToolAvatarView>,
    /// "Claude Code · sonnet".
    pub line: String,
    /// "· starts when you ask", "· 12k".
    pub note: String,
    /// The running agent's id; `None` before anything was asked.
    pub agent: Option<String>,
    /// Its session, which the terminal attaches to.
    pub session: Option<String>,
    /// Otaq's own hint (paragraphs), before anything is asked.
    pub hint: Vec<String>,
    pub chips: Vec<Chip>,
    pub chooser: Option<Chooser>,
    pub foot: &'static str,
    pub keys: Option<String>,
    pub keys_label: &'static str,
}

/// The editor's buttons. `FlowEditorFooter` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorFooter")]
pub struct Footer {
    /// "Cancel" (drops the draft), "Skip, use the default" (onboarding).
    pub cancel: &'static str,
    /// "No changes", "Apply to shop-web", "Use this flow".
    pub apply: String,
    pub apply_enabled: bool,
    /// Why Apply is off: the first problem.
    pub reason: Option<String>,
}

/// Everything the editor draws. `FlowEditorView` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorView")]
pub struct View {
    pub project: String,
    pub from: Opened,
    /// The draft's proposal (Apply takes it); `0` while it is the flow as it will be.
    pub proposal: u32,
    pub start_from: StartFrom,
    pub flow: FlowPart,
    pub agent: AgentPanel,
    pub footer: Footer,
}

/// What [`view`] reads.
pub struct Facts<'a> {
    pub project: &'a str,
    pub project_name: &'a str,
    pub opened: Opened,
    /// The draft (`flow.draft`).
    pub proposal: &'a Proposal,
    pub marks: &'a [StageMark],
    pub can_undo_all: bool,
    pub swap: bool,
    pub applies: &'a DraftApplies,
    /// The draft's document, read.
    pub file: &'a FlowFile,
    /// The flow as it will be (the In use entry's count).
    pub in_use: &'a FlowFile,
    /// Every stored agent.
    pub stored: &'a [Agent],
    pub tools: &'a Tools,
    /// The project's tools and open proposals (`onboarding.flow_setup`).
    pub setup: &'a FlowSetup,
    /// The other projects (key, name), for Import a flow… → from a project.
    pub projects: &'a [(String, String)],
    /// Imported skills' descriptions, by the skill's name.
    pub captions: &'a BTreeMap<String, String>,
    pub selected: Option<&'a str>,
    /// The panel's agent, once one was asked.
    pub agent: Option<&'a ProjectAgent>,
    /// The Project agent role's agent: what the first message starts.
    pub new_agent: Option<&'a Agent>,
    pub keys: &'a KeyLabels,
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The step count Start from and the summary show: the stages and Ready for Dev.
fn steps(file: &FlowFile) -> usize {
    file.flow.stages.len() + 1
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

fn editor_kind(stage: &Stage) -> Option<EditorKind> {
    match stage.kind {
        StageKind::Agent => Some(EditorKind::Works),
        StageKind::Check { by: CheckBy::Agent } => Some(EditorKind::AgentChecks),
        StageKind::Check { by: CheckBy::You } => Some(EditorKind::YouCheck),
        StageKind::Delivery => None,
    }
}

fn kind_label(kind: EditorKind) -> &'static str {
    match kind {
        EditorKind::Works => "An agent works",
        EditorKind::AgentChecks => "An agent checks",
        EditorKind::YouCheck => "You check",
    }
}

/// "PR" or "merge": what the Delivery does.
fn delivers_word(stage: &Stage) -> &'static str {
    match stage.delivery.map(|d| d.delivers) {
        Some(Delivers::Pr) => "PR",
        _ => "merge",
    }
}

/// The list's kind word: "Agent works", "Agent checks", "You check", "Delivery · PR".
fn list_kind(stage: &Stage) -> String {
    match stage.kind {
        StageKind::Agent => "Agent works".to_owned(),
        StageKind::Check { by: CheckBy::Agent } => "Agent checks".to_owned(),
        StageKind::Check { by: CheckBy::You } => "You check".to_owned(),
        StageKind::Delivery => format!("Delivery \u{b7} {}", delivers_word(stage)),
    }
}

/// The caption title's kind: "an agent works", "you check", "delivery".
fn caption_kind(stage: &Stage) -> &'static str {
    match stage.kind {
        StageKind::Agent => "an agent works",
        StageKind::Check { by: CheckBy::Agent } => "an agent checks",
        StageKind::Check { by: CheckBy::You } => "you check",
        StageKind::Delivery => "delivery",
    }
}

/// A step's caption (d0115): its skill's description, Otaq's fixed caption for a check by you and the Delivery.
fn caption(file: &FlowFile, stage: &Stage, captions: &BTreeMap<String, String>) -> Option<String> {
    match stage.kind {
        StageKind::Check { by: CheckBy::You } => Some(YOU_CHECK_CAPTION.to_owned()),
        StageKind::Delivery => Some(
            match stage.delivery.map(|d| d.delivers) {
                Some(Delivers::Pr) => PR_CAPTION,
                _ => MERGE_CAPTION,
            }
            .to_owned(),
        ),
        _ => {
            let skill = stage.skill.as_deref()?;
            captions.get(skill).cloned().or_else(|| {
                let back = back_to(&file.flow, &stage.id, &[]).map(|s| s.name.as_str());
                shipped_caption(skill, back)
            })
        }
    }
}

/// The agent a stage names, as stored (its tool for the avatar).
fn stored_agent<'a>(stored: &'a [Agent], name: &str) -> Option<&'a Agent> {
    stored.iter().find(|a| a.name.eq_ignore_ascii_case(name))
}

/// The avatar and model of the agent `name`: a stored one's tool and model; one the draft brings has none yet.
fn agent_on(name: &str, f: &Facts) -> (Option<ToolAvatarView>, Option<String>) {
    let Some(agent) = stored_agent(f.stored, name) else {
        return (None, None);
    };
    match agent.model.on(f.tools.default_model.as_ref()) {
        Some(on) => (
            Some(ToolAvatarView::of(on.tool, on.model.as_deref())),
            on.model,
        ),
        None => (None, None),
    }
}

fn mark_label(mark: Mark) -> &'static str {
    match mark {
        Mark::Added => "Added",
        Mark::New => "New",
        Mark::Changed => "Changed",
        Mark::Moved => "Moved",
        Mark::Removed => "Removed",
    }
}

fn mark_view(mark: &StageMark) -> MarkView {
    let why = match &mark.why {
        Some(why) => format!("Changed by {}: {why}", mark.by),
        None => format!("Changed by {}", mark.by),
    };
    MarkView {
        label: mark_label(mark.mark),
        why,
        undo: "Undo",
    }
}

fn mark_of<'m>(marks: &'m [StageMark], id: &str) -> Option<&'m StageMark> {
    marks
        .iter()
        .find(|m| m.stage_id == id && m.mark != Mark::Removed)
}

/// Each stage's problem from the draft's check (its first).
fn stage_problem(proposal: &Proposal, id: &str) -> Option<String> {
    proposal.problems.iter().find_map(|p| match &p.about {
        About::Stage { id: of, .. } if of == id => Some(p.message.clone()),
        _ => None,
    })
}

/// What the walking ticket says in a stage, and its colour.
fn ticket_state(stage: &Stage, long: bool) -> (String, PillView) {
    let who = |word: &str| match (long, &stage.agent) {
        (true, Some(agent)) => format!("{word} \u{b7} {agent}"),
        _ => word.to_owned(),
    };
    match stage.kind {
        StageKind::Agent if stage.skill.as_deref() == Some("grill") => (
            "Asks you a question".to_owned(),
            PillView::legend(Legend::AsksYou, None),
        ),
        StageKind::Agent => (who("Working"), PillView::legend(Legend::AgentWorking, None)),
        StageKind::Check { by: CheckBy::Agent } => (
            who("Checking"),
            PillView::legend(Legend::AgentWorking, None),
        ),
        StageKind::Check { by: CheckBy::You } => (
            if long {
                "Your check \u{b7} You".to_owned()
            } else {
                "Your check".to_owned()
            },
            PillView::legend(Legend::ReadyForReview, None),
        ),
        StageKind::Delivery => (
            match stage.delivery.map(|d| d.delivers) {
                Some(Delivers::Pr) => "PR open",
                _ => "Merged",
            }
            .to_owned(),
            PillView::legend(Legend::Approved, None),
        ),
    }
}

/// The animation's tile kind: long words while the flow has five steps or fewer (`s27-03`), short ones beyond.
fn tile_kind(stage: &Stage, long: bool) -> String {
    match (stage.kind, long) {
        (StageKind::Agent, true) => "An agent works",
        (StageKind::Agent, false) => "Agent",
        (StageKind::Check { by: CheckBy::Agent }, true) => "An agent checks",
        (StageKind::Check { by: CheckBy::Agent }, false) => "Check",
        (StageKind::Check { by: CheckBy::You }, true) => "You check",
        (StageKind::Check { by: CheckBy::You }, false) => "You",
        (StageKind::Delivery, _) => "Delivery",
    }
    .to_owned()
}

fn tiles(f: &Facts) -> Vec<Tile> {
    let long = steps(f.file) <= 5;
    let ready = Tile {
        id: READY_ID.to_owned(),
        name: if long { "Ready for Dev" } else { "Ready" }.to_owned(),
        icon: IconName::Hourglass,
        kind: "Waits".to_owned(),
        ticket: "Waiting".to_owned(),
        tone: PillView::legend(Legend::Queued, None),
        caption_title: "Ready for Dev \u{b7} waits".to_owned(),
        caption_mark: None,
        caption: Some(READY_CAPTION.to_owned()),
    };
    let stages = f.file.flow.stages.iter().map(|stage| {
        let (ticket, tone) = ticket_state(stage, long);
        let added =
            mark_of(f.marks, &stage.id).is_some_and(|m| matches!(m.mark, Mark::Added | Mark::New));
        Tile {
            id: stage.id.clone(),
            name: stage.name.clone(),
            icon: kind_icon(stage),
            kind: tile_kind(stage, long),
            ticket,
            tone,
            caption_title: format!("{} \u{b7} {}", stage.name, caption_kind(stage)),
            caption_mark: added.then_some("New step \u{b7}"),
            caption: caption(f.file, stage, f.captions),
        }
    });
    std::iter::once(ready).chain(stages).collect()
}

/// Where **+** after the step at `index` (of the stages) adds, and whether it can: never after the Delivery.
fn steps_list(f: &Facts) -> Vec<StepRow> {
    let stages = &f.file.flow.stages;
    let ready = StepRow {
        id: READY_ID.to_owned(),
        name: "Ready for Dev".to_owned(),
        icon: IconName::Hourglass,
        kind: "Waits".to_owned(),
        avatar: None,
        who: None,
        mark: None,
        added: false,
        problem: None,
        selected: false,
        draggable: false,
        selectable: false,
        caption: Some(READY_CAPTION.to_owned()),
        add_after: None,
        can_add_after: !stages.first().is_some_and(Stage::is_delivery),
    };
    let rows = stages.iter().map(|stage| {
        let (avatar, who) = match stage.kind {
            StageKind::Check { by: CheckBy::You } => (None, Some("You".to_owned())),
            StageKind::Delivery => (None, Some(delivers_word(stage).to_owned())),
            _ => match &stage.agent {
                Some(name) => (agent_on(name, f).0, Some(name.clone())),
                None => (None, None),
            },
        };
        StepRow {
            id: stage.id.clone(),
            name: stage.name.clone(),
            icon: kind_icon(stage),
            kind: list_kind(stage),
            avatar,
            who,
            mark: mark_of(f.marks, &stage.id).map(mark_view),
            added: mark_of(f.marks, &stage.id).is_none()
                && f.in_use.flow.stage(&stage.id).is_none(),
            problem: stage_problem(f.proposal, &stage.id),
            selected: f.selected == Some(stage.id.as_str()),
            draggable: !stage.is_delivery(),
            selectable: true,
            caption: caption(f.file, stage, f.captions),
            add_after: Some(stage.id.clone()),
            can_add_after: !stage.is_delivery(),
        }
    });
    std::iter::once(ready).chain(rows).collect()
}

fn legend() -> Vec<LegendRow> {
    vec![
        LegendRow {
            icon: IconName::Bot,
            name: "An agent works",
            text: "writes code, asks you questions",
        },
        LegendRow {
            icon: IconName::ShieldCheck,
            name: "An agent checks",
            text: "tests, security; sends it back",
        },
        LegendRow {
            icon: IconName::UserCheck,
            name: "You check",
            text: "approve or send back",
        },
        LegendRow {
            icon: IconName::GitMerge,
            name: "Delivery",
            text: "merge, or open a PR",
        },
    ]
}

/// The agents the field offers: the stored ones, then the ones only the draft brings ("new").
fn agent_items(f: &Facts, current: &str) -> Vec<MenuItem> {
    let stored = f.stored.iter().map(|a| {
        MenuItem::new(a.name.clone(), a.name.clone()).checked(a.name.eq_ignore_ascii_case(current))
    });
    let brought = f
        .file
        .agents
        .iter()
        .filter(|a: &&FlowAgent| stored_agent(f.stored, &a.name).is_none())
        .map(|a| {
            MenuItem::new(a.name.clone(), a.name.clone())
                .hint("new")
                .checked(a.name.eq_ignore_ascii_case(current))
        });
    stored.chain(brought).collect()
}

fn agent_field(f: &Facts, stage: &Stage) -> Option<AgentField> {
    let name = stage.agent.as_deref()?;
    let (avatar, model) = agent_on(name, f);
    let default = match stage.kind {
        StageKind::Agent => Some((WORKS_AGENT, "An agent works")),
        StageKind::Check { by: CheckBy::Agent } => Some((CHECKS_AGENT, "An agent checks")),
        _ => None,
    };
    let note = match default {
        Some((agent, kind)) if agent.eq_ignore_ascii_case(name) => {
            Some(format!("default for {kind}"))
        }
        _ => model,
    };
    Some(AgentField {
        label: "Agent",
        avatar,
        name: name.to_owned(),
        note,
        items: agent_items(f, name),
        hint: "Change it here, or manage agents in Settings \u{2192} Agents.",
    })
}

/// "A ticket can": skipping it, each other agent it may use, and (fixed) coming back here when a later check fails.
fn can_rows(f: &Facts, stage: &Stage) -> Vec<CanRow> {
    let mut rows = Vec::new();
    let runs_agent = stage.agent.is_some() && !stage.is_delivery();
    if runs_agent {
        let current = stage.agent.as_deref().unwrap_or_default();
        let named: Vec<String> = f
            .stored
            .iter()
            .map(|a| a.name.clone())
            .chain(f.file.agents.iter().map(|a| a.name.clone()))
            .collect();
        let mut seen: Vec<String> = Vec::new();
        for name in named {
            if name.eq_ignore_ascii_case(current)
                || seen.iter().any(|s| s.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let on = stage
                .options
                .iter()
                .any(|o| matches!(o, StageOption::Agent(a) if a.eq_ignore_ascii_case(&name)));
            // The ones it allows, and those of the same kind's default it could.
            if on {
                rows.push(CanRow {
                    label: format!("use {name} instead"),
                    option: name.clone(),
                    on,
                    enabled: true,
                });
            }
            seen.push(name);
        }
    }
    if !stage.is_delivery() {
        rows.push(CanRow {
            option: StageOption::SKIP.to_owned(),
            label: "skip it".to_owned(),
            on: stage.options.contains(&StageOption::Skip),
            enabled: true,
        });
    }
    if stage.is_agent_stage() {
        let later = f.file.flow.position(&stage.id).map_or(0, |p| p + 1);
        let back = f.file.flow.stages[later.min(f.file.flow.stages.len())..]
            .iter()
            .filter(|s| matches!(s.kind, StageKind::Check { .. }))
            .any(|s| back_to(&f.file.flow, &s.id, &[]).is_some_and(|b| b.id == stage.id));
        if back {
            rows.push(CanRow {
                option: "back".to_owned(),
                label: "come back here when a later check fails".to_owned(),
                on: true,
                enabled: false,
            });
        }
    }
    rows
}

fn details(f: &Facts, stage: &Stage) -> Details {
    let kind = editor_kind(stage);
    let kinds = match kind {
        Some(_) => [
            EditorKind::Works,
            EditorKind::AgentChecks,
            EditorKind::YouCheck,
        ]
        .into_iter()
        .map(|k| KindOption {
            kind: k,
            label: kind_label(k),
        })
        .collect(),
        None => Vec::new(),
    };
    let skill = (stage.agent.is_some()).then(|| SkillField {
        label: "Skill",
        skill: stage.skill.clone(),
        empty: stage.skill.is_none().then(|| {
            format!(
                "No skill yet. An agent stage needs one: it\u{2019}s what the agent follows in {}.",
                stage.name
            )
        }),
    });
    Details {
        stage: stage.id.clone(),
        title: stage.name.clone(),
        icon: kind_icon(stage),
        remove: (!stage.is_delivery()).then_some("Remove stage"),
        name_label: "Name",
        name: stage.name.clone(),
        kind_label: "Kind",
        kinds,
        kind,
        fixed_kind: stage.is_delivery().then_some("Delivery"),
        agent: agent_field(f, stage),
        skill,
        can_label: "A ticket can",
        can: can_rows(f, stage),
        problem: stage_problem(f.proposal, &stage.id),
        mark: mark_of(f.marks, &stage.id).map(mark_view),
    }
}

/// The source's name in onboarding's summary: "Otaq default", "Grill first", "By Claude Code".
fn source_name(f: &Facts) -> String {
    if let Some(tool) = f.proposal.proposed_by.tool().filter(|_| f.swap) {
        return format!("By {}", tool.display_name());
    }
    match &f.proposal.source {
        Source::Preset { file } => presets()
            .iter()
            .find(|p| p.file == file)
            .map_or_else(|| file.clone(), |p| p.name.clone()),
        Source::Project { name, .. } => format!("{name}\u{2019}s flow"),
        Source::Document { file: Some(file) } => file.clone(),
        _ => presets()[0].name.clone(),
    }
}

/// The stages the draft changes (each once).
fn changed_stages(proposal: &Proposal) -> usize {
    let mut ids: Vec<&str> = proposal
        .diff
        .stages
        .iter()
        .map(|c| c.stage_id.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.len().max(usize::from(!proposal.diff.same))
}

fn summary(f: &Facts) -> String {
    let count = plural(steps(f.file), "step", "steps");
    match f.opened {
        Opened::Onboarding => match f.marks.first() {
            Some(mark) => format!("{count} \u{b7} {}, changed by {}", source_name(f), mark.by),
            None => format!("{count} \u{b7} starting from {}", source_name(f)),
        },
        Opened::Project if f.proposal.diff.same => format!("{count} \u{b7} in use"),
        Opened::Project => format!(
            "{count} \u{b7} {}, not applied",
            plural(changed_stages(f.proposal), "change", "changes")
        ),
    }
}

/// "Security moved before Validate.", "Docs removed.": what a reorder or removal does, for the change line.
fn shape_words(f: &Facts) -> String {
    let stages = &f.file.flow.stages;
    let mut words: Vec<String> = Vec::new();
    for change in &f.proposal.diff.stages {
        match &change.change {
            Change::Moved { .. } => {
                let at = f.file.flow.position(&change.stage_id);
                let next = at
                    .and_then(|i| stages.get(i + 1))
                    .filter(|s| !s.is_delivery());
                let prev = at
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| stages.get(i));
                words.push(match (next, prev) {
                    (Some(next), _) => format!("{} moved before {}.", change.stage, next.name),
                    (None, Some(prev)) => format!("{} moved after {}.", change.stage, prev.name),
                    (None, None) => format!("{} moved.", change.stage),
                });
            }
            Change::Removed => words.push(format!("{} removed.", change.stage)),
            _ => {}
        }
    }
    words.dedup();
    words.join(" ")
}

/// "the 6 there now (4 in Dev, 2 in Security)": the tickets inside, by stage in the order they come.
fn inside_words(inside: &[Inside]) -> String {
    let mut by: Vec<(&str, usize)> = Vec::new();
    for i in inside {
        match by.iter_mut().find(|(s, _)| *s == i.stage) {
            Some((_, n)) => *n += 1,
            None => by.push((&i.stage, 1)),
        }
    }
    by.iter()
        .map(|(stage, n)| format!("{n} in {stage}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The change line (FED-08; `s27-07`): which part waits, how many tickets are in it, and that Apply now is there.
fn change_line(f: &Facts) -> Option<String> {
    if f.opened == Opened::Onboarding {
        return None;
    }
    Some(match f.applies {
        DraftApplies::Nothing => return None,
        DraftApplies::AtOnce => {
            "This applies at once: tickets that haven\u{2019}t reached a changed step follow \
                                 it; tickets in it or past it finish as they are."
                .to_owned()
        }
        DraftApplies::Span { span, inside } => {
            let what = shape_words(f);
            let what = if what.is_empty() {
                String::new()
            } else {
                format!("{what} ")
            };
            let (first, last) = (&span.first, &span.last);
            if inside.is_empty() {
                format!("{what}No ticket is between {first} and {last} now, so it applies at once.")
            } else {
                let there = match inside.len() {
                    1 => format!("the ticket there now ({}) finishes", inside_words(inside)),
                    n => format!("the {n} there now ({}) finish", inside_words(inside)),
                };
                format!(
                    "{what}This waits until no ticket is between {first} and {last}: {there} on the current \
                     order, and until then no ticket enters {first} to {last}. Tickets before {first} keep moving. \
                     The banner offers Apply now."
                )
            }
        }
        DraftApplies::WholeBoard { inside } if inside.is_empty() => {
            "Another flow: no ticket is at work, so it applies at once.".to_owned()
        }
        DraftApplies::WholeBoard { inside } => format!(
            "Another flow: this waits for the whole board. {} at work now finish on the current flow, and no \
             ticket starts until then. The banner offers Apply now.",
            match inside.len() {
                1 => "The ticket".to_owned(),
                n => format!("The {n} tickets"),
            }
        ),
        DraftApplies::WithTheWaitingChange => {
            "A flow change already waits: this one applies together with it.".to_owned()
        }
    })
}

fn problems(proposal: &Proposal) -> Vec<String> {
    proposal
        .problems
        .iter()
        .filter(|p| !matches!(p.about, About::Stage { .. }))
        .map(|p| p.message.clone())
        .collect()
}

fn flow_part(f: &Facts) -> FlowPart {
    let selected = f.selected.and_then(|id| f.file.flow.stage(id));
    let stages = &f.file.flow.stages;
    let before_delivery = stages
        .iter()
        .rev()
        .find(|s| !s.is_delivery())
        .map(|s| s.id.clone());
    FlowPart {
        title: format!("{}\u{2019}s flow", f.project_name),
        summary: summary(f),
        watch: "Watch it",
        folded: selected.is_some(),
        fold_note: "The animation folds while you edit a stage",
        tiles: tiles(f),
        steps: steps_list(f),
        add_stage: "Add a stage",
        add_stage_after: before_delivery,
        hint: "Click a step to edit it, drag to reorder, or ask the agent.",
        legend_title: "Every step is one of four kinds:",
        legend: legend(),
        details: selected.map(|s| details(f, s)),
        problems: problems(f.proposal),
        change_line: change_line(f),
        undo_all: f.can_undo_all.then_some("Undo all"),
    }
}

fn in_use_badge() -> Badge {
    Badge {
        label: "In use",
        pill: PillView::legend(Legend::Approved, Some("In use")),
    }
}

fn new_badge() -> Badge {
    Badge {
        label: "New",
        pill: PillView::legend(Legend::ReadyForReview, Some("New")),
    }
}

/// Which entry the draft holds: a preset copied in, a tool's proposal, else (unchanged or edited) the flow in use
/// (the default in onboarding).
fn chosen(f: &Facts, pick: &Pick) -> bool {
    let tool = f.proposal.proposed_by.tool().filter(|_| f.swap);
    match (pick, &f.proposal.source) {
        (Pick::Entry { .. }, _) => false,
        (_, _) if tool.is_some() => false,
        (Pick::Preset { file }, Source::Preset { file: from }) if f.swap => file == from,
        (Pick::FlowInUse, _) => !f.swap,
        (Pick::Preset { file }, _) => {
            f.opened == Opened::Onboarding && !f.swap && *file == presets()[0].file
        }
        _ => false,
    }
}

fn start_from(f: &Facts) -> StartFrom {
    let mut entries = Vec::new();
    if f.opened == Opened::Project {
        let pick = Pick::FlowInUse;
        entries.push(Entry {
            key: "in-use".to_owned(),
            name: format!("{}\u{2019}s flow", f.project_name),
            badge: Some(in_use_badge()),
            count: steps(f.in_use),
            line: "In use \u{b7} the board\u{2019}s columns".to_owned(),
            chosen: chosen(f, &pick),
            avatar: None,
            pick,
        });
    }
    for preset in presets() {
        let pick = Pick::Preset {
            file: preset.file.to_owned(),
        };
        entries.push(Entry {
            key: preset.file.to_owned(),
            name: preset.name.clone(),
            badge: None,
            count: steps(&preset.read()),
            line: preset_line(preset.file).to_owned(),
            chosen: chosen(f, &pick),
            avatar: None,
            pick,
        });
    }
    let mut menu = vec![MenuItem::new("file", "A file\u{2026}").icon(IconName::Upload)];
    if !f.projects.is_empty() {
        menu.push(MenuItem::header("FROM A PROJECT").separated());
        menu.extend(f.projects.iter().map(|(key, name)| {
            MenuItem::new(format!("project:{key}"), format!("{name}\u{2019}s flow"))
        }));
    }
    StartFrom {
        title: "START FROM",
        entries,
        made_for: made_for(f),
        import: Import {
            label: "Import a flow\u{2026}",
            menu,
            file_title: "Import a flow",
        },
        note: "Starting from one copies it here; nothing changes until you use it.",
    }
}

fn made_for(f: &Facts) -> MadeFor {
    let draft_tool = f.proposal.proposed_by.tool().filter(|_| f.swap);
    let entries: Vec<Entry> = f
        .setup
        .proposals
        .iter()
        .filter(|p| p.id != f.proposal.id)
        .filter_map(|p| Some((p, p.proposed_by.tool()?)))
        .map(|(p, tool)| {
            let pick = Pick::Entry { proposal: p.id };
            let count = otaq_core::flow::yaml::read(&p.yaml).map_or(0, |file| steps(&file));
            Entry {
                key: format!("proposal-{}", p.id),
                name: format!("By {}", tool.display_name()),
                badge: Some(new_badge()),
                count,
                line: format!("From your {} sessions", tool.display_name()),
                chosen: draft_tool == Some(tool),
                avatar: Some(ToolAvatarView::of(tool, None)),
                pick,
            }
        })
        .collect();
    let any_proposed = !entries.is_empty();
    let working = f
        .agent
        .filter(|a| a.state == AgentState::Running && a.mid_turn)
        .map(|a| a.tool);
    let tools = f
        .setup
        .tools
        .iter()
        .filter(|t| !matches!(t.row, FlowSetupRow::Proposed { .. }))
        .map(|t| {
            let (status, ask) = match &t.row {
                FlowSetupRow::Working { .. } => ("Working".to_owned(), None),
                _ if f.opened == Opened::Project && working == Some(t.tool) => {
                    ("Working".to_owned(), None)
                }
                FlowSetupRow::PlanLimit { .. } => ("Plan limit".to_owned(), None),
                FlowSetupRow::NotSignedIn => ("Not signed in".to_owned(), None),
                FlowSetupRow::Ended { .. } => ("Ended without a proposal".to_owned(), Some("Ask")),
                _ if any_proposed => ("Connected \u{b7} not asked".to_owned(), Some("Ask")),
                _ => ("Connected".to_owned(), Some("Ask")),
            };
            ToolRow {
                tool: t.tool,
                avatar: ToolAvatarView::of(t.tool, None),
                name: t.tool.display_name(),
                status,
                ask,
            }
        })
        .collect();
    MadeFor {
        title: format!("MADE FOR {}", f.project_name.to_uppercase()),
        text: "Ask a connected tool; it reads how you use it.",
        entries,
        tools,
    }
}

/// The connected tools the chooser and the chips offer.
fn askable(f: &Facts) -> Vec<Tool> {
    f.setup
        .tools
        .iter()
        .filter(|t| !matches!(t.row, FlowSetupRow::NotSignedIn))
        .map(|t| t.tool)
        .collect()
}

fn chooser(f: &Facts, tools: &[Tool]) -> Option<Chooser> {
    (tools.len() > 1).then(|| Chooser {
        title: "Which tool should read how you work?",
        text: "Each reads only its own history on this Mac, and this repo.",
        tools: tools
            .iter()
            .map(|&tool| ChooserTool {
                tool,
                avatar: ToolAvatarView::of(tool, None),
                name: tool.display_name(),
                detail: "Reads its own history on this Mac",
            })
            .collect(),
        both: (f.opened == Opened::Onboarding).then(|| {
            (
                "Both, and compare".to_owned(),
                format!("Two flows under Made for {}", f.project_name),
            )
        }),
        foot: "Uses tokens \u{b7} read by that tool\u{2019}s provider",
    })
}

/// The fixed chips (d0115: "fixed text written by Otaq"): about the selected step, else about the flow.
fn chips(f: &Facts, tools: &[Tool]) -> Vec<Chip> {
    let stages = &f.file.flow.stages;
    let send = |label: String| Chip {
        action: ChipAction::Send {
            message: label.clone(),
        },
        label,
    };
    let mut chips = Vec::new();
    let selected = f.selected.and_then(|id| f.file.flow.stage(id));
    if let Some(skill) = selected.and_then(|s| s.skill.as_deref()) {
        chips.push(send(format!("What does {skill} do?")));
    }
    match f.opened {
        Opened::Onboarding => {
            match tools {
                [] => {}
                [tool] => chips.push(Chip {
                    label: format!("Ask {}", tool.display_name()),
                    action: ChipAction::Ask { tool: *tool },
                }),
                _ => chips.push(Chip {
                    label: format!("Make one for {}\u{2026}", f.project_name),
                    action: ChipAction::ChooseTool,
                }),
            }
            let check = stages
                .iter()
                .find(|s| s.is_agent_check())
                .or_else(|| stages.first());
            if let (Some(check), None) = (check, selected) {
                chips.push(send(format!("What\u{2019}s {}?", check.name)));
            }
        }
        Opened::Project => {
            let why = stages
                .iter()
                .rev()
                .find(|s| s.is_agent_check())
                .or_else(|| stages.iter().find(|s| s.is_agent_stage()));
            if let (Some(why), None) = (why, selected) {
                chips.push(send(format!("Why is {} here?", why.name)));
            }
            if let Some(review) = stages
                .iter()
                .find(|s| s.is_human_check() && !s.options.contains(&StageOption::Skip))
            {
                chips.push(send(format!("Make {} optional", review.name)));
            }
        }
    }
    chips
}

/// "Claude Code · sonnet": what the first message starts on (the Project agent role's agent).
fn starts_on(f: &Facts) -> (Option<ToolAvatarView>, String) {
    let on = f
        .new_agent
        .and_then(|a| a.model.on(f.tools.default_model.as_ref()));
    match on {
        Some(on) => {
            let model = on
                .model
                .clone()
                .unwrap_or_else(|| "tool default".to_owned());
            (
                Some(ToolAvatarView::of(on.tool, on.model.as_deref())),
                format!("{} \u{b7} {model}", on.tool.display_name()),
            )
        }
        None => {
            let tool = Tool::ALL[0];
            (
                Some(ToolAvatarView::of(tool, None)),
                tool.display_name().to_owned(),
            )
        }
    }
}

fn panel_pill(agent: &ProjectAgent) -> PillView {
    let pill = |legend: Legend, icon: IconName, label: &str| PillView {
        icon,
        label: label.to_owned(),
        ..PillView::legend(legend, None)
    };
    match agent.state {
        AgentState::Running if agent.mid_turn && agent.needs_you.is_none() => {
            pill(Legend::AgentWorking, IconName::Activity, "Working")
        }
        AgentState::Running if agent.needs_you.is_some() => {
            pill(Legend::NeedsYou, IconName::Hand, "Waiting for you")
        }
        AgentState::Running => pill(
            Legend::AsksYou,
            IconName::MessageCircleQuestionMark,
            "Waiting for you",
        ),
        AgentState::Paused => pill(Legend::Queued, IconName::CirclePause, "Paused"),
        _ => pill(Legend::Queued, IconName::CircleStop, "Ended"),
    }
}

fn agent_panel(f: &Facts) -> AgentPanel {
    let keys = f.keys.label(KeyAction::FocusPane);
    // An agent that ended leaves the panel as before anything was asked: the next words start a new one.
    let running = f.agent.filter(|a| a.state == AgentState::Running);
    if let Some(agent) = running {
        let model = agent
            .model
            .clone()
            .unwrap_or_else(|| "tool default".to_owned());
        return AgentPanel {
            title: PANEL_TITLE,
            pill: panel_pill(agent),
            avatar: Some(ToolAvatarView::of(agent.tool, agent.model.as_deref())),
            line: format!("{} \u{b7} {model}", agent.tool.display_name()),
            note: format!("\u{b7} {}", tokens_label(agent.tokens.shown())),
            agent: Some(agent.id.clone()),
            session: agent.session.as_ref().map(|s| s.id.clone()),
            hint: Vec::new(),
            chips: Vec::new(),
            chooser: None,
            foot: TYPING_GOES,
            keys,
            keys_label: "focus",
        };
    }
    let tools = askable(f);
    let (avatar, line) = starts_on(f);
    let hint = match f.opened {
        Opened::Onboarding => {
            let check = f
                .file
                .flow
                .stages
                .iter()
                .find(|s| s.is_agent_check())
                .map_or("Validate", |s| s.name.as_str());
            vec![
                format!(
                    "Ask about the flow, or to change it: \u{201c}what does {check} do?\u{201d}, \u{201c}add a \
                     security check\u{201d}."
                ),
                format!(
                    "To have one made for {}, ask a connected tool in Start from. Nothing runs, and no tokens are \
                     used, until you ask.",
                    f.project_name
                ),
            ]
        }
        Opened::Project => vec![format!(
            "Ask about {}\u{2019}s flow, or to change it. Nothing on the board changes until you apply, and \
             nothing runs until you ask.",
            f.project_name
        )],
    };
    AgentPanel {
        title: PANEL_TITLE,
        pill: PillView {
            icon: IconName::CircleDashed,
            label: NOT_STARTED.to_owned(),
            ..PillView::legend(Legend::Queued, None)
        },
        avatar,
        line,
        note: STARTS_WHEN_YOU_ASK.to_owned(),
        agent: None,
        session: None,
        hint,
        chips: chips(f, &tools),
        chooser: chooser(f, &tools),
        foot: TYPING_GOES,
        keys,
        keys_label: "focus",
    }
}

fn footer(f: &Facts) -> Footer {
    let reason = f.proposal.problems.first().map(|p| p.message.clone());
    match f.opened {
        Opened::Onboarding => Footer {
            cancel: "Skip, use the default",
            apply: "Use this flow".to_owned(),
            apply_enabled: reason.is_none(),
            reason,
        },
        Opened::Project if f.proposal.diff.same => Footer {
            cancel: "Cancel",
            apply: "No changes".to_owned(),
            apply_enabled: false,
            reason: None,
        },
        Opened::Project => Footer {
            cancel: "Cancel",
            apply: format!("Apply to {}", f.project_name),
            apply_enabled: reason.is_none(),
            reason,
        },
    }
}

pub fn view(f: &Facts) -> View {
    View {
        project: f.project.to_owned(),
        from: f.opened,
        proposal: f.proposal.id,
        start_from: start_from(f),
        flow: flow_part(f),
        agent: agent_panel(f),
        footer: footer(f),
    }
}

/// A stage id from a name, unique in `file`: `Plan` → `plan`, `plan-2`.
fn fresh_id(file: &FlowFile, name: &str) -> String {
    let mut base = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            base.push(c);
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
    }
    let base = match base.trim_end_matches('-') {
        "" => "stage".to_owned(),
        b => b.to_owned(),
    };
    let mut id = base.clone();
    let mut n = 2;
    while file.flow.stage(&id).is_some() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// A name no stage has: "New stage", "New stage 2".
fn fresh_name(file: &FlowFile) -> String {
    let taken = |name: &str| {
        file.flow
            .stages
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(name))
    };
    let mut name = NEW_STAGE.to_owned();
    let mut n = 2;
    while taken(&name) {
        name = format!("{NEW_STAGE} {n}");
        n += 1;
    }
    name
}

/// `stage` as the kind `kind`, keeping its agent and skill unless they were the old kind's defaults.
fn with_kind(stage: &Stage, kind: EditorKind) -> Stage {
    let mut s = stage.clone();
    let was_default = |agent: &str, skill: &str| {
        stage
            .agent
            .as_deref()
            .is_none_or(|a| a.eq_ignore_ascii_case(agent))
            && stage.skill.as_deref().is_none_or(|k| k == skill)
    };
    let defaults = match editor_kind(stage) {
        Some(EditorKind::Works) => was_default(WORKS_AGENT, WORKS_SKILL),
        Some(EditorKind::AgentChecks) => was_default(CHECKS_AGENT, CHECKS_SKILL),
        _ => true,
    };
    let (agent, skill) = match kind {
        EditorKind::Works => (WORKS_AGENT, WORKS_SKILL),
        EditorKind::AgentChecks => (CHECKS_AGENT, CHECKS_SKILL),
        EditorKind::YouCheck => ("", ""),
    };
    match kind {
        EditorKind::YouCheck => {
            s.kind = StageKind::Check { by: CheckBy::You };
            s.agent = None;
            s.skill = None;
            s.rounds = None;
            s.options.retain(|o| *o == StageOption::Skip);
        }
        _ => {
            s.kind = match kind {
                EditorKind::Works => StageKind::Agent,
                _ => StageKind::Check { by: CheckBy::Agent },
            };
            if defaults || s.agent.is_none() {
                s.agent = Some(agent.to_owned());
                s.skill = Some(skill.to_owned());
            }
            s.rounds =
                (kind == EditorKind::AgentChecks).then(|| stage.rounds.unwrap_or(CHECK_ROUNDS));
        }
    }
    s
}

/// The draft's document after `edit`, and the stage to select after it (a new one). Refused with why when the edit
/// doesn't fit the flow (a stage that isn't there, the Delivery moved or removed).
pub fn edited(file: &FlowFile, edit: &StageEdit) -> Result<(FlowFile, Option<String>), String> {
    let mut out = file.clone();
    let missing = |id: &str| format!("The flow has no stage {id}");
    let at = |id: &str| out.flow.position(id).ok_or_else(|| missing(id));
    let mut select = None;
    match edit {
        StageEdit::Rename { stage, name } => {
            let i = at(stage)?;
            let name = name.trim();
            if name.is_empty() {
                return Err("A stage needs a name".to_owned());
            }
            out.flow.stages[i].name = name.to_owned();
        }
        StageEdit::Kind { stage, kind } => {
            let i = at(stage)?;
            if out.flow.stages[i].is_delivery() {
                return Err("The Delivery stage stays the last one".to_owned());
            }
            out.flow.stages[i] = with_kind(&out.flow.stages[i], *kind);
        }
        StageEdit::Agent { stage, agent } => {
            let i = at(stage)?;
            let s = &mut out.flow.stages[i];
            if s.agent.is_none() {
                return Err(format!("No agent runs {}", s.name));
            }
            s.agent = Some(agent.clone());
            s.options
                .retain(|o| !matches!(o, StageOption::Agent(a) if a.eq_ignore_ascii_case(agent)));
        }
        StageEdit::Option { stage, option, on } => {
            let i = at(stage)?;
            let s = &mut out.flow.stages[i];
            let option = if option == StageOption::SKIP {
                StageOption::Skip
            } else {
                StageOption::Agent(option.clone())
            };
            s.options.retain(|o| *o != option);
            if *on {
                s.options.push(option);
            }
        }
        StageEdit::Move { stage, to } => {
            let i = at(stage)?;
            if out.flow.stages[i].is_delivery() {
                return Err("The Delivery stage stays the last one".to_owned());
            }
            let moved = out.flow.stages.remove(i);
            let delivery = out
                .flow
                .stages
                .iter()
                .position(Stage::is_delivery)
                .unwrap_or(out.flow.stages.len());
            out.flow.stages.insert((*to).min(delivery), moved);
        }
        StageEdit::Add { after } => {
            let index = match after {
                Some(id) => at(id)? + 1,
                None => 0,
            };
            let delivery = out
                .flow
                .stages
                .iter()
                .position(Stage::is_delivery)
                .unwrap_or(out.flow.stages.len());
            let name = fresh_name(&out);
            let id = fresh_id(&out, &name);
            out.flow.stages.insert(
                index.min(delivery),
                Stage {
                    id: id.clone(),
                    name,
                    kind: StageKind::Agent,
                    column: Column::Own,
                    agent: Some(WORKS_AGENT.to_owned()),
                    skill: Some(WORKS_SKILL.to_owned()),
                    options: Vec::new(),
                    rounds: None,
                    delivery: None,
                    why: None,
                },
            );
            select = Some(id);
        }
        StageEdit::Remove { stage } => {
            let i = at(stage)?;
            if out.flow.stages[i].is_delivery() {
                return Err("The Delivery stage stays: a flow ends with one".to_owned());
            }
            out.flow.stages.remove(i);
        }
    }
    Ok((out, select))
}

/// The gallery's editors: shop-web's flow in the project, Otaq default in onboarding with two tools.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "FlowEditorGallery")]
pub struct Gallery {
    pub cells: Vec<(String, View)>,
}

pub fn gallery() -> Gallery {
    use fixtures::*;
    let tools = crate::view_models::flow_setup_page::fixtures::tools();
    let setup = crate::view_models::flow_setup_page::fixtures::setup(
        FlowSetupRow::NotAsked,
        FlowSetupRow::NotAsked,
    );
    let keys = KeyLabels::default();
    let captions = BTreeMap::from([(
        "security-review".to_owned(),
        "A security agent reviews the change for risky code.".to_owned(),
    )]);
    let shop = shop_web();
    let default = otaq_default();
    let stored = stored();
    let none = DraftApplies::Nothing;
    let in_project = proposal(&shop, true);
    let in_onboarding = proposal(&default, true);
    let facts = |opened, proposal: &'_ Proposal, file: &'_ FlowFile| -> View {
        view(&Facts {
            project: "SHOP",
            project_name: "shop-web",
            opened,
            proposal,
            marks: &[],
            can_undo_all: false,
            swap: false,
            applies: &none,
            file,
            in_use: file,
            stored: &stored,
            tools: &tools,
            setup: &setup,
            projects: &[],
            captions: &captions,
            selected: None,
            agent: None,
            new_agent: None,
            keys: &keys,
        })
    };
    Gallery {
        cells: vec![
            (
                "In the project".to_owned(),
                facts(Opened::Project, &in_project, &shop),
            ),
            (
                "In onboarding".to_owned(),
                facts(Opened::Onboarding, &in_onboarding, &default),
            ),
        ],
    }
}

/// shop-web's flow and the draft around it, for the tests and the gallery.
pub mod fixtures {
    use otaq_core::agents::{Agent, AgentModel, CameFrom, SkillRef};
    use otaq_core::flow::model::FlowFile;
    use otaq_core::flow::proposal::{FlowDiff, Proposal, ProposedBy, Source};

    /// shop-web on Grill first with a Security check (the ticket file's example): 7 steps.
    pub const SHOP_WEB: &str = "agents:
  - name: Griller
    skills: [grill]
    model: default
  - name: Rust developer
    skills: [work-ticket]
    model: default
  - name: Validator
    skills: [validate]
    model: default
  - name: Security reviewer
    skills: [security-review]
    model: default
flow:
  - id: grill
    name: Grill
    kind: agent
    column: own
    agent: Griller
    skill: grill
  - id: dev
    name: Dev
    kind: agent
    column: own
    agent: Rust developer
    skill: work-ticket
  - id: validate
    name: Validate
    kind: check
    by: agent
    column: own
    agent: Validator
    skill: validate
    rounds: 3
  - id: security
    name: Security
    kind: check
    by: agent
    column: own
    agent: Security reviewer
    skill: security-review
    rounds: 3
  - id: review
    name: Review
    kind: check
    by: you
    column: own
  - id: deliver
    name: Done
    kind: delivery
    column: with-previous
    agent: Rust developer
    skill: open-pr
    mode: click
    delivers: pr
";

    pub fn shop_web() -> FlowFile {
        otaq_core::flow::yaml::read(SHOP_WEB).expect("shop-web's flow reads")
    }

    pub fn otaq_default() -> FlowFile {
        otaq_core::flow::presets::presets()[0].read()
    }

    fn agent(id: &str, name: &str, skill: &str) -> Agent {
        Agent {
            id: id.to_owned(),
            name: name.to_owned(),
            instructions: String::new(),
            skills: vec![SkillRef::Shipped(skill.to_owned())],
            imported: Vec::new(),
            model: AgentModel::Default,
            effort: None,
            role: None,
            came_from: CameFrom::AgentsFile,
            mark: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// The stored agents: Otaq's and shop-web's.
    pub fn stored() -> Vec<Agent> {
        vec![
            agent("a1", "Developer", "work-ticket"),
            agent("a2", "Validator", "validate"),
            agent("a3", "Griller", "grill"),
            agent("a4", "Rust developer", "work-ticket"),
            agent("a5", "Security reviewer", "validate"),
        ]
    }

    /// `file` as the draft's proposal: unchanged when `same`.
    pub fn proposal(file: &FlowFile, same: bool) -> Proposal {
        Proposal {
            id: 0,
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            source: Source::Document { file: None },
            proposed_by: ProposedBy::User,
            yaml: otaq_core::flow::yaml::write(file),
            problems: Vec::new(),
            diff: FlowDiff {
                against_waiting: false,
                same,
                stages: Vec::new(),
                agents: Vec::new(),
                warnings: Vec::new(),
                removed_choices: Vec::new(),
                waits: false,
            },
            why: Vec::new(),
            created_at: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::flow::apply::Span;
    use otaq_core::flow::proposal::{Applies, ProposalProblem, ProposedBy, StageChange};
    use otaq_core::project_agents::first_flow::FlowSetupTool;

    use super::fixtures::*;
    use super::*;
    use crate::view_models::flow_setup_page::fixtures as setup;

    struct Draft {
        proposal: Proposal,
        marks: Vec<StageMark>,
        applies: DraftApplies,
        swap: bool,
        undo: bool,
    }

    fn draft(file: &FlowFile, same: bool) -> Draft {
        Draft {
            proposal: proposal(file, same),
            marks: Vec::new(),
            applies: if same {
                DraftApplies::Nothing
            } else {
                DraftApplies::AtOnce
            },
            swap: false,
            undo: false,
        }
    }

    struct Env {
        stored: Vec<Agent>,
        tools: Tools,
        setup: FlowSetup,
        keys: KeyLabels,
        captions: BTreeMap<String, String>,
    }

    fn env(setup: FlowSetup) -> Env {
        Env {
            stored: stored(),
            tools: setup::tools(),
            setup,
            keys: KeyLabels::default(),
            captions: BTreeMap::from([(
                "security-review".to_owned(),
                "A security agent reviews the change for risky code.".to_owned(),
            )]),
        }
    }

    fn not_asked() -> FlowSetup {
        setup::setup(FlowSetupRow::NotAsked, FlowSetupRow::NotAsked)
    }

    #[allow(clippy::too_many_arguments)]
    fn render(
        e: &Env,
        opened: Opened,
        d: &Draft,
        file: &FlowFile,
        in_use: &FlowFile,
        selected: Option<&str>,
        agent: Option<&ProjectAgent>,
    ) -> View {
        view(&Facts {
            project: "SHOP",
            project_name: "shop-web",
            opened,
            proposal: &d.proposal,
            marks: &d.marks,
            can_undo_all: d.undo,
            swap: d.swap,
            applies: &d.applies,
            file,
            in_use,
            stored: &e.stored,
            tools: &e.tools,
            setup: &e.setup,
            projects: &[("BILL".to_owned(), "billing-api".to_owned())],
            captions: &e.captions,
            selected,
            agent,
            new_agent: None,
            keys: &e.keys,
        })
    }

    fn names(v: &View) -> Vec<&str> {
        v.flow.steps.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn the_editor_from_otaq_default_in_onboarding() {
        let e = env(not_asked());
        let file = otaq_default();
        let d = draft(&file, true);
        let v = render(&e, Opened::Onboarding, &d, &file, &file, None, None);
        assert_eq!(
            names(&v),
            ["Ready for Dev", "Dev", "Validate", "Review", "Done"]
        );
        assert_eq!(v.flow.title, "shop-web\u{2019}s flow");
        let default = format!("{} default", otaq_core::naming::PRODUCT_NAME);
        assert_eq!(
            v.flow.summary,
            format!("5 steps \u{b7} starting from {default}")
        );
        let entries: Vec<(&str, usize, bool)> = v
            .start_from
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.count, e.chosen))
            .collect();
        assert_eq!(
            entries,
            [
                (default.as_str(), 5, true),
                ("Solo autopilot", 4, false),
                ("Grill first", 6, false),
                ("Team PR", 4, false)
            ]
        );
        assert_eq!(
            v.start_from.entries[0].line,
            "Agents build and check; you review"
        );
        // The animation: long words for five steps, fixed captions for Ready, You check and Delivery.
        let tiles: Vec<(&str, &str, &str)> = v
            .flow
            .tiles
            .iter()
            .map(|t| (t.name.as_str(), t.kind.as_str(), t.ticket.as_str()))
            .collect();
        assert_eq!(
            tiles,
            [
                ("Ready for Dev", "Waits", "Waiting"),
                ("Dev", "An agent works", "Working \u{b7} Developer"),
                ("Validate", "An agent checks", "Checking \u{b7} Validator"),
                ("Review", "You check", "Your check \u{b7} You"),
                ("Done", "Delivery", "Merged")
            ]
        );
        assert_eq!(v.flow.tiles[0].caption.as_deref(), Some(READY_CAPTION));
        assert_eq!(
            v.flow.tiles[2].caption.as_deref(),
            Some(
                "A checking agent runs the tests and lint. If something fails, the ticket goes back to Dev."
            )
        );
        assert_eq!(v.flow.tiles[3].caption.as_deref(), Some(YOU_CHECK_CAPTION));
        assert_eq!(v.flow.tiles[4].caption.as_deref(), Some(MERGE_CAPTION));
        assert_eq!(v.flow.change_line, None);
        assert_eq!(v.footer.cancel, "Skip, use the default");
        assert_eq!(v.footer.apply, "Use this flow");
        assert!(v.footer.apply_enabled);
        assert!(v.flow.details.is_none() && !v.flow.folded);
    }

    #[test]
    fn the_editor_from_shop_webs_flow_in_the_project() {
        let e = env(not_asked());
        let file = shop_web();
        let d = draft(&file, true);
        let v = render(&e, Opened::Project, &d, &file, &file, None, None);
        assert_eq!(
            names(&v),
            [
                "Ready for Dev",
                "Grill",
                "Dev",
                "Validate",
                "Security",
                "Review",
                "Done"
            ]
        );
        assert_eq!(v.flow.summary, "7 steps \u{b7} in use");
        let in_use = &v.start_from.entries[0];
        assert_eq!(
            (
                in_use.name.as_str(),
                in_use.count,
                in_use.chosen,
                in_use.line.as_str()
            ),
            (
                "shop-web\u{2019}s flow",
                7,
                true,
                "In use \u{b7} the board\u{2019}s columns"
            )
        );
        assert_eq!(in_use.badge.as_ref().map(|b| b.label), Some("In use"));
        assert!(v.start_from.entries[1..].iter().all(|e| !e.chosen));
        let kinds: Vec<&str> = v.flow.steps.iter().map(|s| s.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "Waits",
                "Agent works",
                "Agent works",
                "Agent checks",
                "Agent checks",
                "You check",
                "Delivery \u{b7} PR"
            ]
        );
        assert_eq!(v.flow.steps[2].who.as_deref(), Some("Rust developer"));
        assert_eq!(v.flow.steps[6].who.as_deref(), Some("PR"));
        assert!(!v.flow.steps[0].draggable && !v.flow.steps[6].draggable);
        assert!(!v.flow.steps[6].can_add_after);
        assert_eq!(v.flow.add_stage_after.as_deref(), Some("review"));
        // Short tile words past five steps; the Security caption is its imported skill's description.
        assert_eq!(v.flow.tiles[1].ticket, "Asks you a question");
        assert_eq!(v.flow.tiles[4].kind, "Check");
        assert_eq!(
            v.flow.tiles[4].caption.as_deref(),
            Some("A security agent reviews the change for risky code.")
        );
        assert_eq!(v.flow.tiles[6].caption.as_deref(), Some(PR_CAPTION));
        assert_eq!(v.flow.tiles[6].ticket, "PR open");
        assert_eq!(v.footer.apply, "No changes");
        assert!(!v.footer.apply_enabled);
        assert_eq!(v.footer.cancel, "Cancel");
        let chips: Vec<&str> = v.agent.chips.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(chips, ["Why is Security here?", "Make Review optional"]);
        assert_eq!(
            v.start_from
                .import
                .menu
                .iter()
                .map(|m| m.label.as_str())
                .collect::<Vec<_>>(),
            [
                "A file\u{2026}",
                "FROM A PROJECT",
                "billing-api\u{2019}s flow"
            ]
        );
    }

    #[test]
    fn a_steps_details() {
        let e = env(not_asked());
        let file = shop_web();
        let d = draft(&file, true);
        let v = render(&e, Opened::Project, &d, &file, &file, Some("dev"), None);
        assert!(v.flow.folded);
        assert!(v.flow.steps[2].selected);
        let details = v.flow.details.expect("Dev's details");
        assert_eq!(details.title, "Dev");
        assert_eq!(details.name, "Dev");
        assert_eq!(details.kind, Some(EditorKind::Works));
        assert_eq!(
            details.kinds.iter().map(|k| k.label).collect::<Vec<_>>(),
            ["An agent works", "An agent checks", "You check"]
        );
        let agent = details.agent.expect("an agent");
        assert_eq!(agent.name, "Rust developer");
        assert_eq!(
            agent.hint,
            "Change it here, or manage agents in Settings \u{2192} Agents."
        );
        assert!(
            agent
                .items
                .iter()
                .any(|i| i.label == "Rust developer" && i.checked)
        );
        assert_eq!(
            details.skill.and_then(|s| s.skill).as_deref(),
            Some("work-ticket")
        );
        let can: Vec<(&str, bool, bool)> = details
            .can
            .iter()
            .map(|c| (c.label.as_str(), c.on, c.enabled))
            .collect();
        assert_eq!(
            can,
            [
                ("skip it", false, true),
                ("come back here when a later check fails", true, false)
            ]
        );
        assert_eq!(details.remove, Some("Remove stage"));
        // The chip asks about the selected step's skill.
        assert_eq!(v.agent.chips[0].label, "What does work-ticket do?");

        // The Delivery: its kind is fixed and it can't be removed; Review has no agent.
        let v = render(&e, Opened::Project, &d, &file, &file, Some("deliver"), None);
        let details = v.flow.details.expect("Done's details");
        assert_eq!(details.fixed_kind, Some("Delivery"));
        assert!(details.kinds.is_empty() && details.remove.is_none());
        let v = render(&e, Opened::Project, &d, &file, &file, Some("review"), None);
        let details = v.flow.details.expect("Review's details");
        assert!(details.agent.is_none() && details.skill.is_none());
        assert_eq!(details.kind, Some(EditorKind::YouCheck));
    }

    #[test]
    fn which_tool_with_two_tools_and_with_one() {
        let e = env(not_asked());
        let file = otaq_default();
        let d = draft(&file, true);
        let v = render(&e, Opened::Onboarding, &d, &file, &file, None, None);
        let tools: Vec<(&str, &str, Option<&str>)> = v
            .start_from
            .made_for
            .tools
            .iter()
            .map(|t| (t.name, t.status.as_str(), t.ask))
            .collect();
        assert_eq!(
            tools,
            [
                ("Claude Code", "Connected", Some("Ask")),
                ("OpenCode", "Connected", Some("Ask"))
            ]
        );
        assert_eq!(v.start_from.made_for.title, "MADE FOR SHOP-WEB");
        assert_eq!(v.agent.chips[0].label, "Make one for shop-web\u{2026}");
        assert_eq!(v.agent.chips[0].action, ChipAction::ChooseTool);
        let chooser = v.agent.chooser.as_ref().expect("the chooser");
        assert_eq!(chooser.title, "Which tool should read how you work?");
        assert_eq!(
            chooser.tools.iter().map(|t| t.name).collect::<Vec<_>>(),
            ["Claude Code", "OpenCode"]
        );
        assert_eq!(
            chooser.both,
            Some((
                "Both, and compare".to_owned(),
                "Two flows under Made for shop-web".to_owned()
            ))
        );
        assert_eq!(
            chooser.foot,
            "Uses tokens \u{b7} read by that tool\u{2019}s provider"
        );
        // No session counts or "most used" anywhere: tools by name only.
        let text = serde_json::to_string(&v.start_from.made_for).unwrap()
            + &serde_json::to_string(&v.agent).unwrap();
        assert!(!text.to_lowercase().contains("most used"));
        assert!(
            !text.contains("sessions"),
            "no session counts before a proposal"
        );

        // One tool: "Ask Claude Code", no chooser.
        let mut one = not_asked();
        one.tools.truncate(1);
        let e = env(one);
        let v = render(&e, Opened::Onboarding, &d, &file, &file, None, None);
        assert_eq!(v.agent.chips[0].label, "Ask Claude Code");
        assert!(v.agent.chooser.is_none());

        // A proposal from Claude Code is its own entry; OpenCode can still be asked.
        let mut asked = setup::setup(
            FlowSetupRow::Proposed {
                agent: setup::CC.into(),
                proposal: 4,
            },
            FlowSetupRow::NotAsked,
        );
        let mut entry = proposal(&shop_web(), false);
        entry.id = 4;
        entry.proposed_by = ProposedBy::ProjectAgent {
            agent: setup::CC.into(),
            session: "s".into(),
            name: "Flow setup \u{b7} Claude Code".into(),
            tool: setup::CLAUDE,
        };
        asked.proposals.push(entry);
        let e = env(asked);
        let v = render(&e, Opened::Onboarding, &d, &file, &file, None, None);
        let made = &v.start_from.made_for;
        assert_eq!(made.entries.len(), 1);
        assert_eq!(
            (
                made.entries[0].name.as_str(),
                made.entries[0].line.as_str(),
                made.entries[0].count
            ),
            ("By Claude Code", "From your Claude Code sessions", 7)
        );
        assert_eq!(made.entries[0].pick, Pick::Entry { proposal: 4 });
        assert_eq!(
            made.tools
                .iter()
                .map(|t| (t.name, t.status.as_str()))
                .collect::<Vec<_>>(),
            [("OpenCode", "Connected \u{b7} not asked")]
        );
        // In the project there is no Both (Q-A).
        let e = env(not_asked());
        let v = render(&e, Opened::Project, &d, &file, &file, None, None);
        assert!(v.agent.chooser.is_some_and(|c| c.both.is_none()));
        let _ = FlowSetupTool {
            tool: setup::CLAUDE,
            row: FlowSetupRow::NotAsked,
        };
    }

    #[test]
    fn a_proposal_tinted_with_undo() {
        let e = env(not_asked());
        let base = otaq_default();
        let mut file = base.clone();
        let (with_grill, _) = edited(&file, &StageEdit::Add { after: None }).unwrap();
        file = with_grill;
        file.flow.stages[0].name = "Grill".into();
        file.flow.stages[0].id = "grill".into();
        file.flow.stages[0].skill = Some("grill".into());
        file.flow.stages[0].agent = Some("Griller".into());
        file.flow
            .stages
            .last_mut()
            .unwrap()
            .delivery
            .as_mut()
            .unwrap()
            .delivers = Delivers::Pr;
        let mut d = draft(&file, false);
        d.marks = vec![
            StageMark {
                stage_id: "grill".into(),
                stage: "Grill".into(),
                mark: Mark::Added,
                by: "Claude Code".into(),
                why: Some("From 12 /grill-me runs".into()),
            },
            StageMark {
                stage_id: "deliver".into(),
                stage: "Done".into(),
                mark: Mark::Changed,
                by: "Claude Code".into(),
                why: None,
            },
        ];
        d.undo = true;
        let v = render(&e, Opened::Onboarding, &d, &file, &base, None, None);
        let grill = &v.flow.steps[1];
        let mark = grill.mark.as_ref().expect("Grill is tinted");
        assert_eq!(mark.label, "Added");
        assert_eq!(mark.why, "Changed by Claude Code: From 12 /grill-me runs");
        assert_eq!(mark.undo, "Undo");
        assert_eq!(
            v.flow.steps.last().unwrap().mark.as_ref().map(|m| m.label),
            Some("Changed")
        );
        assert!(v.flow.steps[2].mark.is_none());
        assert_eq!(v.flow.tiles[1].caption_mark, Some("New step \u{b7}"));
        assert_eq!(v.flow.undo_all, Some("Undo all"));
        assert_eq!(
            v.flow.summary,
            format!(
                "6 steps \u{b7} {} default, changed by Claude Code",
                otaq_core::naming::PRODUCT_NAME
            )
        );
    }

    #[test]
    fn the_idle_agent_panel_starts_nothing() {
        let e = env(not_asked());
        let file = shop_web();
        let d = draft(&file, true);
        let v = render(&e, Opened::Project, &d, &file, &file, None, None);
        let panel = &v.agent;
        assert_eq!(panel.title, "Project agent \u{b7} the flow");
        assert_eq!(panel.pill.label, "Not started");
        assert_eq!(panel.note, "\u{b7} starts when you ask");
        assert!(panel.agent.is_none() && panel.session.is_none());
        assert_eq!(
            panel.hint,
            [
                "Ask about shop-web\u{2019}s flow, or to change it. Nothing on the board changes until you apply, and \
              nothing runs until you ask."
            ]
        );
        assert_eq!(panel.foot, "Typing goes to the agent");
        // The chips' words are fixed: each sends itself as the first message.
        assert!(
            panel
                .chips
                .iter()
                .all(|c| matches!(&c.action, ChipAction::Send { message } if *message == c.label))
        );

        // Once asked: its state, tokens and terminal; the chips go.
        let agent = ProjectAgent {
            id: "pa-1".into(),
            project: "SHOP".into(),
            name: "Flow".into(),
            name_source: None,
            state: AgentState::Running,
            tool: setup::CLAUDE,
            tool_session_id: None,
            model: Some("sonnet".into()),
            got_request: true,
            mid_turn: true,
            limit_stopped: false,
            created_at: 0,
            ended_at: None,
            closed: false,
            question: None,
            needs_you: None,
            focused: false,
            tokens: otaq_core::usage::Tokens {
                input: 12_000,
                ..Default::default()
            },
            session: None,
            epic: None,
            turn_ended_at: None,
        };
        let v = render(&e, Opened::Project, &d, &file, &file, None, Some(&agent));
        assert_eq!(v.agent.pill.label, "Working");
        assert_eq!(v.agent.line, "Claude Code \u{b7} sonnet");
        assert_eq!(v.agent.note, "\u{b7} 12k");
        assert_eq!(v.agent.agent.as_deref(), Some("pa-1"));
        assert!(v.agent.chips.is_empty());
        let waiting = ProjectAgent {
            mid_turn: false,
            ..agent
        };
        let v = render(&e, Opened::Project, &d, &file, &file, None, Some(&waiting));
        assert_eq!(v.agent.pill.label, "Waiting for you");
    }

    #[test]
    fn the_change_line_names_the_span_and_the_tickets_in_it() {
        let e = env(not_asked());
        let in_use = shop_web();
        let (file, _) = edited(
            &in_use,
            &StageEdit::Move {
                stage: "security".into(),
                to: 2,
            },
        )
        .unwrap();
        let mut d = draft(&file, false);
        d.proposal.diff.stages.push(StageChange {
            stage_id: "security".into(),
            stage: "Security".into(),
            change: Change::Moved {
                after: Some("dev".into()),
            },
            text: String::new(),
            label: String::new(),
            applies: Applies::Span {
                first: "Dev".into(),
                last: "Review".into(),
            },
        });
        let inside = |key: &str, stage: &str| Inside {
            ticket: key.parse().unwrap(),
            stage: stage.into(),
        };
        d.applies = DraftApplies::Span {
            span: Span {
                stages: vec![
                    "dev".into(),
                    "validate".into(),
                    "security".into(),
                    "review".into(),
                ],
                first: "Dev".into(),
                last: "Review".into(),
            },
            inside: vec![
                inside("SHOP-22", "Dev"),
                inside("SHOP-33", "Dev"),
                inside("SHOP-29", "Dev"),
                inside("SHOP-30", "Dev"),
                inside("SHOP-19", "Security"),
                inside("SHOP-32", "Security"),
            ],
        };
        let v = render(&e, Opened::Project, &d, &file, &in_use, None, None);
        assert_eq!(
            v.flow.change_line.as_deref(),
            Some(
                "Security moved before Validate. This waits until no ticket is between Dev and Review: the 6 there \
                 now (4 in Dev, 2 in Security) finish on the current order, and until then no ticket enters Dev to \
                 Review. Tickets before Dev keep moving. The banner offers Apply now."
            )
        );
        assert_eq!(v.flow.summary, "7 steps \u{b7} 1 change, not applied");
        assert_eq!(v.footer.apply, "Apply to shop-web");
        assert!(v.footer.apply_enabled);
    }

    #[test]
    fn problems_show_on_their_step_and_keep_apply_off() {
        let e = env(not_asked());
        let file = shop_web();
        let mut d = draft(&file, false);
        d.proposal.problems.push(ProposalProblem {
            about: About::Stage {
                id: "security".into(),
                name: "Security".into(),
            },
            message: "Security names the agent Security reviewer, which isn't one of your agents"
                .into(),
            fix: None,
        });
        let v = render(&e, Opened::Project, &d, &file, &file, None, None);
        assert_eq!(
            v.flow.steps[4].problem.as_deref(),
            Some("Security names the agent Security reviewer, which isn't one of your agents")
        );
        assert!(!v.footer.apply_enabled);
        assert_eq!(v.footer.reason, v.flow.steps[4].problem);
    }

    #[test]
    fn edits_change_the_document() {
        let file = shop_web();
        let ids = |f: &FlowFile| {
            f.flow
                .stages
                .iter()
                .map(|s| s.id.clone())
                .collect::<Vec<_>>()
        };
        // Add after Grill: a new agent stage with the kind's default agent and skill, selected.
        let (added, select) = edited(
            &file,
            &StageEdit::Add {
                after: Some("grill".into()),
            },
        )
        .unwrap();
        assert_eq!(select.as_deref(), Some("new-stage"));
        assert_eq!(ids(&added)[..3], ["grill", "new-stage", "dev"]);
        let new = added.flow.stage("new-stage").unwrap();
        assert_eq!(
            (new.agent.as_deref(), new.skill.as_deref()),
            (Some("Developer"), Some("work-ticket"))
        );
        let (again, select) = edited(&added, &StageEdit::Add { after: None }).unwrap();
        assert_eq!(select.as_deref(), Some("new-stage-2"));
        assert_eq!(again.flow.stages[0].name, "New stage 2");
        // The document reads back.
        assert!(otaq_core::flow::yaml::read(&otaq_core::flow::yaml::write(&again)).is_ok());
        // Move: never past the Delivery.
        let (moved, _) = edited(
            &file,
            &StageEdit::Move {
                stage: "grill".into(),
                to: 99,
            },
        )
        .unwrap();
        assert_eq!(
            ids(&moved),
            ["dev", "validate", "security", "review", "grill", "deliver"]
        );
        assert!(
            edited(
                &file,
                &StageEdit::Move {
                    stage: "deliver".into(),
                    to: 0
                }
            )
            .is_err()
        );
        assert!(
            edited(
                &file,
                &StageEdit::Remove {
                    stage: "deliver".into()
                }
            )
            .is_err()
        );
        let (removed, _) = edited(
            &file,
            &StageEdit::Remove {
                stage: "security".into(),
            },
        )
        .unwrap();
        assert!(removed.flow.stage("security").is_none());
        // Kind: a check by you drops the agent; back to an agent check takes the kind's defaults.
        let (you, _) = edited(
            &file,
            &StageEdit::Kind {
                stage: "validate".into(),
                kind: EditorKind::YouCheck,
            },
        )
        .unwrap();
        let s = you.flow.stage("validate").unwrap();
        assert!(s.is_human_check() && s.agent.is_none() && s.rounds.is_none());
        let (back, _) = edited(
            &you,
            &StageEdit::Kind {
                stage: "validate".into(),
                kind: EditorKind::AgentChecks,
            },
        )
        .unwrap();
        let s = back.flow.stage("validate").unwrap();
        assert_eq!(
            (s.agent.as_deref(), s.skill.as_deref(), s.rounds),
            (Some("Validator"), Some("validate"), Some(3))
        );
        assert!(otaq_core::flow::yaml::read(&otaq_core::flow::yaml::write(&back)).is_ok());
        // Rename, agent, an option.
        let (renamed, _) = edited(
            &file,
            &StageEdit::Rename {
                stage: "dev".into(),
                name: " Build ".into(),
            },
        )
        .unwrap();
        assert_eq!(renamed.flow.stage("dev").unwrap().name, "Build");
        assert!(
            edited(
                &file,
                &StageEdit::Rename {
                    stage: "dev".into(),
                    name: " ".into()
                }
            )
            .is_err()
        );
        let (agent, _) = edited(
            &file,
            &StageEdit::Agent {
                stage: "dev".into(),
                agent: "Developer".into(),
            },
        )
        .unwrap();
        assert_eq!(
            agent.flow.stage("dev").unwrap().agent.as_deref(),
            Some("Developer")
        );
        let (skip, _) = edited(
            &file,
            &StageEdit::Option {
                stage: "review".into(),
                option: "skip".into(),
                on: true,
            },
        )
        .unwrap();
        assert_eq!(
            skip.flow.stage("review").unwrap().options,
            [StageOption::Skip]
        );
        let (unskip, _) = edited(
            &skip,
            &StageEdit::Option {
                stage: "review".into(),
                option: "skip".into(),
                on: false,
            },
        )
        .unwrap();
        assert!(unskip.flow.stage("review").unwrap().options.is_empty());
    }

    #[test]
    fn the_gallery_builds() {
        assert_eq!(gallery().cells.len(), 2);
    }

    /// FED-09 (`s26-08`, `s27-05`): each column ⋯ entry opens the editor over shop-web's flow in use. Edit Grill…
    /// selects Grill with its details; Add a stage after Grill puts a new stage right after it, selected, dashed, its
    /// details open, Apply on; Change the whole flow… opens on the legend with nothing to apply.
    #[test]
    fn each_column_entry_opens_the_editor_in_its_state() {
        let e = env(not_asked());
        let file = shop_web();
        let d = draft(&file, true);

        let edit = render(&e, Opened::Project, &d, &file, &file, Some("grill"), None);
        assert!(edit.flow.steps[1].selected && !edit.flow.steps[1].added);
        assert_eq!(
            edit.flow.details.as_ref().map(|d| d.title.as_str()),
            Some("Grill")
        );

        let (added, select) = edited(
            &file,
            &StageEdit::Add {
                after: Some("grill".into()),
            },
        )
        .unwrap();
        let select = select.expect("the new stage is selected");
        let d = draft(&added, false);
        let add = render(&e, Opened::Project, &d, &added, &file, Some(&select), None);
        let new = &add.flow.steps[2];
        assert_eq!(new.id, select);
        assert!(
            new.selected && new.added,
            "the new stage is dashed and selected"
        );
        assert!(add.flow.steps.iter().filter(|s| s.added).count() == 1);
        assert!(add.flow.details.is_some(), "its details open");
        assert!(add.flow.folded);

        let d = draft(&file, true);
        let whole = render(&e, Opened::Project, &d, &file, &file, None, None);
        assert!(whole.flow.details.is_none() && !whole.flow.folded);
        assert!(whole.flow.steps.iter().all(|s| !s.selected && !s.added));
        assert_eq!(whole.footer.apply, "No changes");
    }
}
