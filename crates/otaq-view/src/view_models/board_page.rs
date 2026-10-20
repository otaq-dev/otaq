//! The board as the Tauri page draws it (T-0220, T-0324, T-0404; UI-02, SHL-06, SHL-01, TKT-06, CLS-03, CLS-06,
//! REV-14, STG-08; d0087, d0108): [`board::build`]'s columns of the project's flow (on the default flow today's
//! five) turned into plain, serialized shapes with every text already worded (counts, hints, the Review column's two
//! sections and the Approved note, the Dev section and fold, the Done rail, the empty board, the "Created or changed
//! by" chip), each with its kind and each section's own drop target, so the page draws any flow without naming a
//! column. The drag is the UIT-04 way: [`drag_answer_in`] gives every drop target's verdict for one ticket at once
//! (every column, with the Review column split into its body, which means Ready for Review, and its Approved
//! section), from the core's `check_move_in` on the project's flow ([`drag::feedback_in`], [`drag::drop_hint_in`]),
//! so the page draws the whole drag without asking again; a drop then asks once more what happens
//! ([`drop_step_in`]): a move, an epic queue (EPC-12: the epic card, one of its rows or a "not queued" chip, which
//! the page follows with the queue toast), the Park confirmation ([`ParkDialog`]) or the approve choice
//! ([`ApproveMenu`]). An epic's track dragged onto Backlog takes back (EPC-14): its waiting children at once, or,
//! when every child started, Park all after its confirmation ([`ParkAllDialog`]).

use std::borrow::Cow;

use otaq_core::board::epic_flight::TakeBackPlan;
use otaq_core::board::moves::DropTarget;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Status, TicketKey};
use otaq_core::flow::Flow;
use otaq_core::pr::provider::PrProvider;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::key_labels::KeyLabels;
use crate::keymap::Platform;
use crate::view_models::approve::{self, ApproveChoice};
use crate::view_models::board::{
    self, BoardInput, CardEntry, ColumnView, DevSections, HeaderTooltip, SectionView, StripColumn,
};
use crate::view_models::card_page::{EpicCardView, TicketCardView};
use crate::view_models::chain;
use crate::view_models::close;
use crate::view_models::drag::{self, DropAction, DropFeedback};
use crate::view_models::empty_board;
use crate::view_models::epic_strip::Strip;
use crate::view_models::filter::Token;
use crate::view_models::flow_columns::{self, ColumnKind, Columns};
use crate::view_models::search_page::FilterState;

/// A page column's id (SHL-06, STG-08): `"backlog"`, `"ready_for_dev"`, the id of a stage with its own column
/// (the default flow's `"dev"` and `"review"`), `"done"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(rename = "BoardColumnId")]
pub struct ColumnId(#[ts(type = "string")] pub Cow<'static, str>);

impl ColumnId {
    pub const BACKLOG: Self = Self::fixed(flow_columns::BACKLOG);
    pub const READY_FOR_DEV: Self = Self::fixed(flow_columns::READY_FOR_DEV);
    /// The default flow's Dev and Review columns.
    pub const DEV: Self = Self::fixed(otaq_core::flow::default::DEV);
    pub const REVIEW: Self = Self::fixed(otaq_core::flow::default::REVIEW);
    pub const DONE: Self = Self::fixed(flow_columns::DONE);

    const fn fixed(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for ColumnId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

/// Where a card can be dropped (d0087, UIT-04, STG-08): a column, or a section of a human check's column after
/// its own. On the default flow `"review"` is the Review column outside its Approved section, which means Ready
/// for Review (only PR rework, from Approved, is allowed there); `"approved"` is the Approved section (from Ready
/// for Review it opens the Approve choice).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(rename = "BoardDropTarget")]
pub struct DropTargetId(#[ts(type = "string")] pub Cow<'static, str>);

impl DropTargetId {
    pub const BACKLOG: Self = Self::fixed(flow_columns::BACKLOG);
    pub const READY_FOR_DEV: Self = Self::fixed(flow_columns::READY_FOR_DEV);
    pub const DEV: Self = Self::fixed(otaq_core::flow::default::DEV);
    pub const REVIEW: Self = Self::fixed(otaq_core::flow::default::REVIEW);
    pub const APPROVED: Self = Self::fixed(flow_columns::APPROVED);
    pub const DONE: Self = Self::fixed(flow_columns::DONE);

    const fn fixed(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Every target of `flow`'s board, in board order: what a drag answers for.
    pub fn all(flow: &Flow) -> Vec<Self> {
        Columns::of(flow)
            .targets()
            .into_iter()
            .map(Self::new)
            .collect()
    }
}

/// A section of a human check's column (STG-08): on the default flow `"ready_for_review"` or `"approved"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(rename = "BoardReviewSectionId")]
pub struct ReviewSectionId(#[ts(type = "string")] pub Cow<'static, str>);

impl ReviewSectionId {
    pub const READY_FOR_REVIEW: Self = Self(Cow::Borrowed(flow_columns::READY_FOR_REVIEW));
    pub const APPROVED: Self = Self(Cow::Borrowed(flow_columns::APPROVED));
}

/// A card on the board. `needs_you` feeds the "7 above · 6 need you" chip of a scrolled column.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "BoardCard")]
pub enum BoardCard {
    Ticket {
        card: Box<TicketCardView>,
        needs_you: bool,
    },
    Epic {
        card: Box<EpicCardView>,
    },
}

impl BoardCard {
    pub fn key(&self) -> &str {
        match self {
            Self::Ticket { card, .. } => &card.key,
            Self::Epic { card } => &card.key,
        }
    }
}

/// A quiet line under a column's title with its icon (Approved: "Otaq doesn’t watch PRs. Mark it when it
/// lands.").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardNote")]
pub struct Note {
    pub icon: IconName,
    pub text: String,
}

/// Dev's parts: the floated cards (pins and needs-you), then the working cards and the fold.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardDevSections")]
pub struct DevPage {
    pub top: Vec<BoardCard>,
    pub working_count: String,
    pub working: Vec<BoardCard>,
    /// "2 more · working" while folded; a click unfolds.
    pub fold: Option<String>,
    /// The keys behind the fold: the fold chip turns accent when one of them is in the highlighted chain (T-0229).
    pub folded: Vec<String>,
}

/// A section of the Review column (*Row · Review column (0086 · V)*): its header ("READY FOR REVIEW 2"), its cards
/// in that state's order, and a quiet line when empty. Whether it is folded is the page's own; the count stays.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardReviewSection")]
pub struct SectionPage {
    pub id: ReviewSectionId,
    /// The drop target it is, apart from its column (Approved: `"approved"`); `null` for the column's own section.
    pub target: Option<DropTargetId>,
    /// "READY FOR REVIEW", "APPROVED".
    pub title: String,
    /// "2", "1/2" under a filter.
    pub count: String,
    pub cards: Vec<BoardCard>,
    /// "1 hidden by filter".
    pub hidden: Option<String>,
    /// Shown when the section has no card: "Nothing to review", "Nothing approved".
    pub empty: String,
    /// Approved, while a card in it has an open PR: "Otaq doesn’t watch PRs. Mark it when it lands."
    pub note: Option<Note>,
    /// Approved only: shown in place of `empty` while a Ready for Review card is dragged.
    pub drag_hint: Option<String>,
    /// The header's tooltip: "Fold Approved" / "Unfold Approved" (the page picks by its own fold state).
    pub fold: String,
    pub unfold: String,
}

/// A stage folded into an agent column, after its count in the header (FLW-12, Q270): "· Validating 1".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardFoldedStage")]
pub struct FoldedStage {
    /// "Validating".
    pub tag: String,
    pub count: String,
}

/// Who runs a column's stages, on hovering its title (FLW-12): a bold first line and detail lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardColumnTooltip")]
pub struct ColumnTooltip {
    /// "Grill · Griller · grill".
    pub title: String,
    /// "Claude Code · sonnet", "Options: skip".
    pub detail: Vec<String>,
}

impl From<HeaderTooltip> for ColumnTooltip {
    fn from(t: HeaderTooltip) -> Self {
        Self {
            title: t.title,
            detail: t.detail,
        }
    }
}

/// The stage strip above a long flow's board that doesn't fit (FLW-12; *Row · A flow with many stages*): every
/// column with its count and the stages folded into it. The page outlines the columns in view and scrolls to one
/// on a click.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardStageStrip")]
pub struct StageStrip {
    /// "Columns": the strip's name for assistive tech.
    pub label: String,
    pub columns: Vec<StageStripColumn>,
}

/// One column in the stage strip: "Dev 1" over "Validate 1".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardStageStripColumn")]
pub struct StageStripColumn {
    pub column: ColumnId,
    pub title: String,
    pub count: String,
    pub folded: Vec<StageStripFolded>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardStageStripFolded")]
pub struct StageStripFolded {
    pub name: String,
    pub count: String,
}

impl From<StripColumn> for StageStripColumn {
    fn from(c: StripColumn) -> Self {
        Self {
            column: c.column,
            title: c.title,
            count: c.count,
            folded: c
                .folded
                .into_iter()
                .map(|(name, count)| StageStripFolded { name, count })
                .collect(),
        }
    }
}

pub const STAGE_STRIP_LABEL: &str = "Columns";

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardColumn")]
pub struct ColumnPage {
    pub id: ColumnId,
    /// What it holds: the look (the icon's ink, the strip's label over Backlog) and the drops (⌘D starts on the
    /// first agent column, Park drops on Backlog) follow it.
    pub kind: ColumnKind,
    pub title: String,
    pub icon: IconName,
    /// The icon in the muted colour (a flow that isn't the default's shape, FLW-12); else in its kind's status
    /// colour.
    pub quiet_icon: bool,
    /// Review: both sections together. An agent column with a folded stage: its own stage's cards.
    pub count: String,
    /// An agent column's folded stages, after its count: "· Validating 1".
    pub folded: Vec<FoldedStage>,
    /// Who runs it, on hovering the title; `None` for Backlog, Ready for Dev and Done.
    pub tooltip: Option<ColumnTooltip>,
    /// Under the title: Ready for Dev's "Starts when ready, in priority order".
    pub hint: Option<String>,
    /// Shown when the column has no card (Review's sections have their own lines).
    pub empty_hint: String,
    /// Drawn as the 34 px rail: Done folded, or an empty stage of a long flow (FLW-12).
    pub collapsed: bool,
    /// Done only: the header button's (and the rail's) tooltip ("Collapse Done" / "Expand Done"); a click on it
    /// folds or opens Done. An empty stage's rail has none: it opens when a card comes.
    pub toggle: Option<String>,
    /// "6 hidden by filter".
    pub hidden: Option<String>,
    /// Every column but Dev and Review.
    pub cards: Vec<BoardCard>,
    /// Review only: Ready for Review, then Approved.
    pub sections: Vec<SectionPage>,
    /// Done only: the epics' folded merged children (EPC-16); each fold's cards follow each other in `cards`.
    pub folds: Vec<DoneFold>,
    pub dev: Option<DevPage>,
    /// Backlog's "Add ticket": the + in its header (and its row); the same as ⌘N.
    pub add: Option<String>,
}

/// An epic's merged children as one line in Done (EPC-16): "Wishlist · 4 of 6 merged". The page draws the line
/// before the first of `keys` and hides those cards until the line is clicked (opened is the page's own, not
/// saved).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardDoneFold")]
pub struct DoneFold {
    pub epic: String,
    pub label: String,
    pub keys: Vec<String>,
}

/// The chip of a column scrolled down: `"{n} above"`, then `" · {n} need you"` when any of them need the user. The
/// page counts the cards above its view and fills in `{n}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardScrollChip")]
pub struct ScrollChip {
    pub above: String,
    pub need_you: String,
}

pub const SCROLL_CHIP_ABOVE: &str = "{n} above";
pub const SCROLL_CHIP_NEED_YOU: &str = "{n} need you";

/// The page's board.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardView")]
pub struct BoardPage {
    pub columns: Vec<ColumnPage>,
    /// The centred card of a project with no tickets (the columns stay behind it).
    pub empty: Option<empty_board::View>,
    pub scroll_chip: ScrollChip,
    /// The chain of every linked card, for the highlight on hover or selection (DEP-05, T-0229).
    pub chains: Vec<chain::View>,
    /// The "Epics in flight" strip above the columns, while an epic is a track (EPC-10, T-0269); its stations
    /// follow `columns`.
    pub strip: Option<Strip>,
    /// The focused epic (an `epic:` filter, EPC-18, Q55): its cards stay, the other cards, tracks and Done lines
    /// dim, and ↑↓ walk only its cards.
    pub focus: Option<EpicFocus>,
    /// The "Created or changed by <agent> ✕" chip while the board is filtered by a Project agent (d0087).
    pub agent_filter: Option<AgentChip>,
    /// The columns fill the board's width; below this many px per column the board scrolls sideways (SHL-01): 200,
    /// or a long flow's compact 184 (FLW-12).
    pub min_column_width: f32,
    /// The stage strip above a long flow that doesn't fit (FLW-12).
    pub stage_strip: Option<StageStrip>,
    /// Done as a rail.
    pub done_rail_width: f32,
    /// The Board tab's count ("Board 26", Q84): the cards in every column but Done, whatever the filter.
    pub tab_count: String,
}

/// The chip of the agent filter (d0087; Q-G): ✕ or Esc sets `Params::agent` back to `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardAgentChip")]
pub struct AgentChip {
    /// The Project agent's id, as in `Params::agent`.
    pub agent: String,
    /// "Created or changed by Checkout slowness".
    pub label: String,
    /// The ✕'s tooltip: "Clear filter · Esc".
    pub clear: String,
}

/// What the agent filter keeps: a Project agent's name and the tickets in its "Created and changed" (PAG-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFilter {
    pub agent: String,
    pub name: String,
    pub keys: Vec<TicketKey>,
}

impl AgentFilter {
    /// The filter of the agent `agent` named `name`, from its created and edited tickets.
    pub fn of(
        agent: &str,
        name: &str,
        created: &otaq_core::project_agents::CreatedAndChanged,
    ) -> Self {
        Self {
            agent: agent.to_owned(),
            name: name.to_owned(),
            keys: created
                .created
                .iter()
                .map(|c| c.key.clone())
                .chain(created.edited.iter().cloned())
                .collect(),
        }
    }

    fn chip(&self) -> AgentChip {
        AgentChip {
            agent: self.agent.clone(),
            label: format!("Created or changed by {}", self.name),
            clear: AGENT_FILTER_CLEAR.to_owned(),
        }
    }
}

pub const AGENT_FILTER_CLEAR: &str = "Clear filter · Esc";

/// What a focused epic keeps at full strength (*03 → epic flows 2 → 6 · Focus on Wishlist*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardEpicFocus")]
pub struct EpicFocus {
    /// The focused epics (one, unless a typed `epic:` names several by the same start): their tracks and Done
    /// lines stay.
    pub epics: Vec<String>,
    /// The cards that stay: the epic's own and its children's.
    pub keys: Vec<String>,
}

/// The filter after focusing (`key`: a selected epic, epic child or track, EPC-18) or leaving the focus (`None`,
/// Esc). `None` when nothing changes: a standalone ticket has no epic, and Esc without a focus does nothing.
pub fn focus_filter(
    state: &FilterState,
    key: Option<&TicketKey>,
    tickets: &[Ticket],
) -> Option<FilterState> {
    let mut filter = state.filter();
    match key {
        Some(key) => filter.focus(crate::view_models::filter::epic_of(key, tickets)?),
        None => {
            if !filter.unfocus() {
                return None;
            }
        }
    }
    Some(FilterState {
        tokens: filter.tokens.iter().map(Token::label).collect(),
        text: filter.text,
    })
}

/// What the page sends with `board.view`: the project, the board's size (for the Done rail and Dev's fold) and the
/// user's own choices, kept per project by the page.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "BoardParams")]
pub struct Params {
    pub project: String,
    pub width: f32,
    pub height: f32,
    /// The user folded (`true`) or opened (`false`) Done; `None` follows the width.
    pub done_collapsed: Option<bool>,
    pub dev_unfolded: bool,
    /// The header's search and filter (T-0221): cards that don't match hide, and each column says how many.
    #[serde(default)]
    pub filter: FilterState,
    /// A Project agent's id: only the tickets it created or changed show (d0087, "Show on board" in the pane). It
    /// combines with `filter`; ✕ on its chip or Esc sets it back to `None`.
    #[serde(default)]
    pub agent: Option<String>,
}

/// The project the board shows, for the empty board's wording.
pub struct ProjectFacts<'a> {
    /// The project's flow: the board's columns (STG-08).
    pub flow: &'a Flow,
    pub name: &'a str,
    pub repo_path: &'a str,
    pub base_branch: &'a str,
    /// Its Project agents by id and name (cards name the one that created them).
    pub agent_names: &'a [(String, String)],
    /// Interrupted tickets that were validating when the service restarted (T-0244).
    pub was_validating: &'a [TicketKey],
    /// Its epic leads (`lead.list`): the Lead row on epic cards, the lead's line on tracks (d0083, T-0289).
    pub leads: &'a [otaq_core::project_agents::lead::Lead],
    /// `Params::agent`'s filter, read from the service; `None` without one (or when the agent is gone).
    pub agent_filter: Option<&'a AgentFilter>,
    /// The cards' named agents (`board.agents`, T-0345): the footer's agent part.
    pub agents: Option<&'a otaq_core::agents::facts::BoardAgents>,
    /// Every agent (`agents.list`): the column headers' tooltips name each stage's tool and model (FLW-12).
    pub agent_list: Option<&'a otaq_core::agents::AgentList>,
    /// Why no ticket starts from Ready for Dev (FLW-06: [`board::WAITING_FOR_THE_FLOW_CHANGE`] while a flow change
    /// waits); `None` while starts run.
    pub starts_held: Option<&'a str>,
}

pub const APPROVED_NOTE: &str =
    "doesn\u{2019}t watch PRs. Mark it once it\u{2019}s merged on GitHub.";

fn card(entry: CardEntry, tickets: &[Ticket]) -> Option<BoardCard> {
    Some(match entry {
        CardEntry::Ticket(vm) => {
            let ticket = tickets.iter().find(|t| t.key == vm.key)?;
            BoardCard::Ticket {
                needs_you: ticket.needs_user(),
                card: Box::new(TicketCardView::from_vm(vm, ticket)),
            }
        }
        CardEntry::Epic(vm) => BoardCard::Epic {
            card: Box::new(EpicCardView::from_vm(vm)),
        },
    })
}

fn cards(entries: Vec<CardEntry>, tickets: &[Ticket]) -> Vec<BoardCard> {
    entries
        .into_iter()
        .filter_map(|e| card(e, tickets))
        .collect()
}

fn hidden_text(n: usize) -> Option<String> {
    (n > 0).then(|| format!("{n} hidden by filter"))
}

fn section_page(view: SectionView, tickets: &[Ticket]) -> SectionPage {
    let approved = view.delivery;
    let pr_open = view.cards.iter().any(|c| match c {
        CardEntry::Ticket(vm) => tickets
            .iter()
            .any(|t| t.key == vm.key && t.status == Status::PrOpen),
        CardEntry::Epic(_) => false,
    });
    let name = view.name;
    SectionPage {
        id: view.id,
        target: view.target,
        title: view.title.to_owned(),
        count: view.count,
        hidden: hidden_text(view.hidden),
        empty: view.empty.to_owned(),
        note: (approved && pr_open).then(|| Note {
            icon: IconName::Info,
            text: format!("{} {APPROVED_NOTE}", otaq_core::naming::PRODUCT_NAME),
        }),
        drag_hint: view.drag_hint.map(str::to_owned),
        fold: format!("Fold {name}"),
        unfold: format!("Unfold {name}"),
        cards: cards(view.cards, tickets),
    }
}

fn column_page(view: ColumnView, tickets: &[Ticket]) -> ColumnPage {
    let kind = view.kind;
    let dev = view.dev.map(|d: DevSections| DevPage {
        top: cards(d.top, tickets),
        working_count: d.working_count.to_string(),
        working: cards(d.working, tickets),
        fold: d.fold,
        folded: d.folded.iter().map(ToString::to_string).collect(),
    });
    ColumnPage {
        id: view.id,
        kind,
        title: view.title,
        icon: view.icon,
        quiet_icon: view.quiet_icon,
        count: view.count,
        folded: match kind {
            ColumnKind::Agent => view
                .folded
                .into_iter()
                .map(|f| FoldedStage {
                    tag: f.tag,
                    count: f.count,
                })
                .collect(),
            _ => Vec::new(),
        },
        tooltip: view.tooltip.map(ColumnTooltip::from),
        hint: view.hint.map(str::to_owned),
        empty_hint: view.empty_hint.to_owned(),
        collapsed: view.collapsed,
        toggle: (kind == ColumnKind::Done).then(|| {
            if view.collapsed {
                "Expand Done"
            } else {
                "Collapse Done"
            }
            .to_owned()
        }),
        hidden: hidden_text(view.hidden),
        cards: cards(view.cards, tickets),
        sections: view
            .sections
            .into_iter()
            .map(|s| section_page(s, tickets))
            .collect(),
        folds: view
            .folds
            .into_iter()
            .map(|f| DoneFold {
                epic: f.epic.to_string(),
                label: f.label,
                keys: f.keys.iter().map(ToString::to_string).collect(),
            })
            .collect(),
        dev,
        add: (kind == ColumnKind::Backlog).then(|| "Add ticket".to_owned()),
    }
}

/// Puts each epic card's lead on it (d0083): the Lead row, Open lead, and "added by Wishlist lead" rows.
fn with_leads(
    columns: &mut [ColumnView],
    tickets: &[Ticket],
    leads: &[otaq_core::project_agents::lead::Lead],
) {
    for column in columns {
        let dev = column
            .dev
            .iter_mut()
            .flat_map(|d| d.top.iter_mut().chain(d.working.iter_mut()));
        let sections = column.sections.iter_mut().flat_map(|s| s.cards.iter_mut());
        for entry in column.cards.iter_mut().chain(dev).chain(sections) {
            let CardEntry::Epic(vm) = entry else { continue };
            let Some(epic) = tickets.iter().find(|t| t.key == vm.key) else {
                continue;
            };
            let lead = leads.iter().find(|l| l.epic == vm.key);
            vm.set_lead(epic, tickets, lead);
        }
    }
}

/// The board for `tickets` of one project, at `params`' size.
pub fn page(
    tickets: &[Ticket],
    project: &ProjectFacts,
    params: &Params,
    now_ms: i64,
    keys: &KeyLabels,
) -> BoardPage {
    let filter = params.filter.filter();
    // An `epic:` token dims instead of hiding (Q55).
    let mut visible = filter.hiding().visible(tickets);
    // The agent filter (d0087) narrows whatever the search shows.
    let agent_filter = project
        .agent_filter
        .filter(|f| params.agent.as_deref() == Some(f.agent.as_str()));
    if let Some(agent) = agent_filter {
        visible = Some(
            visible
                .unwrap_or_else(|| tickets.iter().map(|t| t.key.clone()).collect())
                .into_iter()
                .filter(|k| agent.keys.contains(k))
                .collect(),
        );
    }
    let strong = filter.strong(tickets);
    let done_collapsed = board::done_collapsed(params.done_collapsed, params.width);
    let input = BoardInput {
        flow: project.flow,
        tickets,
        visible: visible.as_deref(),
        strong: strong.as_deref(),
        project_base: Some(project.base_branch),
        now_ms,
        dev_budget: (!params.dev_unfolded).then(|| board::dev_budget(params.height)),
        done_collapsed,
        agent_names: project.agent_names,
        was_validating: project.was_validating,
        agents: project.agents,
        agent_list: project.agent_list,
        starts_held: project.starts_held,
    };
    let mut built = board::build(&input);
    with_leads(&mut built, tickets, project.leads);
    let layout = board::lay_out(&mut built, params.width);
    BoardPage {
        strip: Strip::with_leads(tickets, project.flow, project.leads)
            .map(|s| s.with_done_rail(done_collapsed)),
        columns: built.into_iter().map(|c| column_page(c, tickets)).collect(),
        empty: empty_board::shows(tickets.len(), false)
            .then(|| empty_board::view(project.name, keys)),
        scroll_chip: ScrollChip {
            above: SCROLL_CHIP_ABOVE.to_owned(),
            need_you: SCROLL_CHIP_NEED_YOU.to_owned(),
        },
        chains: chain::views(tickets),
        focus: strong.map(|keys| EpicFocus {
            epics: filter
                .focused_epics(tickets)
                .iter()
                .map(ToString::to_string)
                .collect(),
            keys: keys.iter().map(ToString::to_string).collect(),
        }),
        agent_filter: agent_filter.map(AgentFilter::chip),
        min_column_width: layout.column_width,
        stage_strip: layout.strip.map(|columns| StageStrip {
            label: STAGE_STRIP_LABEL.to_owned(),
            columns: columns.into_iter().map(StageStripColumn::from).collect(),
        }),
        done_rail_width: board::DONE_RAIL_WIDTH,
        tab_count: tickets
            .iter()
            .filter(|t| board::is_card(t, tickets) && !t.place.is_done())
            .count()
            .to_string(),
    }
}

/// How a column looks while the card is over it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "DragVerdict")]
pub enum Verdict {
    /// The card's own column: no styling, dropping does nothing.
    Home,
    /// Accent outline and the hint under the pointer, with a second line under it when there is one ("SHOP-45.1
    /// keeps running").
    Allowed {
        icon: IconName,
        hint: String,
        note: Option<String>,
    },
    /// Red outline, a ban cursor and the reason; dropping does nothing.
    Forbidden {
        title: String,
        detail: Option<String>,
    },
}

/// One drop target's verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DropTargetVerdict")]
pub struct TargetVerdict {
    pub target: DropTargetId,
    pub verdict: Verdict,
}

/// The answer to `board.drag`, asked once when a drag starts: a verdict for every drop target (every column,
/// Review's Approved section on its own). `tickets` are the project's: an epic's answer counts the children it
/// queues ("Queue 2 children") or, for its track over Backlog, what it takes back; other tickets need none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "DragAnswer")]
pub struct DragAnswer {
    pub key: String,
    pub targets: Vec<TargetVerdict>,
}

/// [`drag_answer_in`] on the default flow.
pub fn drag_answer(ticket: &Ticket, tickets: &[Ticket]) -> DragAnswer {
    drag_answer_in(&otaq_core::flow::default_file().flow, ticket, tickets)
}

/// The drag answer on `flow`'s board (the project's): one verdict per drop target, in board order.
pub fn drag_answer_in(flow: &Flow, ticket: &Ticket, tickets: &[Ticket]) -> DragAnswer {
    let columns = Columns::of(flow);
    DragAnswer {
        key: ticket.key.to_string(),
        targets: DropTargetId::all(flow)
            .into_iter()
            .map(|target| {
                let verdict = match columns.drop_target(target.as_str()) {
                    Some(column) => verdict(flow, ticket, tickets, column),
                    None => not_yet(&columns, ticket, &target),
                };
                TargetVerdict { target, verdict }
            })
            .collect(),
    }
}

/// A stage today's moves don't reach (a custom flow's, M33): the card's own column is home, the others say so.
fn not_yet(columns: &Columns, ticket: &Ticket, target: &DropTargetId) -> Verdict {
    if columns.spot(ticket).column == target.as_str() {
        return Verdict::Home;
    }
    Verdict::Forbidden {
        title: NOT_YET.to_owned(),
        detail: None,
    }
}

/// The reason over a column today's moves don't reach.
pub const NOT_YET: &str = "Moving a card here isn\u{2019}t available yet";

fn verdict(flow: &Flow, ticket: &Ticket, tickets: &[Ticket], column: DropTarget) -> Verdict {
    match drag::feedback_in(flow, ticket, tickets, column) {
        DropFeedback::Home => Verdict::Home,
        DropFeedback::Forbidden { title, detail } => Verdict::Forbidden {
            title,
            detail: detail.map(str::to_owned),
        },
        DropFeedback::Allowed => {
            let (icon, hint) = drag::drop_hint_in(flow, ticket, tickets, column)
                .unwrap_or((IconName::Check, String::new()));
            Verdict::Allowed {
                icon,
                hint,
                note: drag::drop_note(ticket, tickets, column),
            }
        }
    }
}

/// One thing a confirmation keeps: "otaq/SHOP-25", "worktree · kept with its files".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ConfirmRow")]
pub struct Row {
    pub icon: IconName,
    pub text: String,
}

/// *03 → Park · Dev → Backlog (allowed)*: "Park SHOP-25?", what stops and what is kept (CLS-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ParkDialog")]
pub struct ParkDialog {
    pub key: String,
    pub title: String,
    pub message: String,
    pub kept: Vec<Row>,
    pub confirm: String,
    pub confirm_icon: IconName,
}

pub fn park_dialog(ticket: &Ticket) -> ParkDialog {
    let text = close::park_text(ticket);
    ParkDialog {
        key: ticket.key.to_string(),
        title: text.title,
        message: text.message,
        kept: text
            .kept
            .into_iter()
            .map(|(icon, text)| Row { icon, text })
            .collect(),
        confirm: "Park".to_owned(),
        confirm_icon: IconName::SquareParking,
    }
}

/// *03 → epic flows 4 → 11*: "Park 2 running tickets?", built like the Park confirmation (d0052); Cancel is the
/// default, the confirm button sends `board.park_all` for the epic (EPC-14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ParkAllDialog")]
pub struct ParkAllDialog {
    /// The epic.
    pub key: String,
    pub title: String,
    pub message: String,
    pub cancel: String,
    pub confirm: String,
    pub confirm_icon: IconName,
}

/// The Park-all confirmation of a take-back `plan`; `None` unless it parks.
pub fn park_all_dialog(plan: &TakeBackPlan) -> Option<ParkAllDialog> {
    let confirmation = plan.confirmation()?;
    Some(ParkAllDialog {
        key: plan.epic().to_string(),
        title: confirmation.title,
        message: confirmation.body,
        cancel: confirmation.cancel,
        confirm: confirmation.confirm,
        confirm_icon: IconName::SquareParking,
    })
}

/// One choice of the approve popover, with its number key ("1 Local merge").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ApproveItem")]
pub struct ApproveItem {
    pub choice: ApproveChoice,
    pub number: u32,
    pub icon: IconName,
    pub title: String,
    pub detail: String,
    pub enabled: bool,
}

/// *03 → Approve choice on drop*: a drop on Approved never approves by itself; it asks how (UI-02, T-0095).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ApproveMenu")]
pub struct ApproveMenu {
    pub key: String,
    pub title: String,
    pub items: Vec<ApproveItem>,
    pub note: String,
}

fn choice_icon(choice: ApproveChoice) -> IconName {
    match choice {
        ApproveChoice::LocalMerge => IconName::GitMerge,
        ApproveChoice::OpenPr => IconName::GitPullRequest,
        ApproveChoice::PushToPr => IconName::Upload,
    }
}

pub fn approve_menu(
    ticket: &Ticket,
    target: &str,
    tickets: &[Ticket],
    pr: Option<&PrProvider>,
) -> ApproveMenu {
    ApproveMenu {
        key: ticket.key.to_string(),
        title: approve::menu_title(ticket),
        items: approve::options(ticket, target, tickets, pr)
            .into_iter()
            .zip(1..)
            .map(|(o, number)| ApproveItem {
                choice: o.choice,
                number,
                icon: choice_icon(o.choice),
                title: o.title,
                detail: o.detail,
                enabled: o.enabled,
            })
            .collect(),
        note: approve::menu_note(ticket),
    }
}

/// What a drop on `column` asks of the app, before anything runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropStep {
    /// Its own column, or refused: the card flies back.
    Nothing,
    /// Send `ticket.move`.
    Move,
    /// Send `epic.queue` for the epic, or for one `child` of it (a row or a "not queued" chip), then show the
    /// queue toast with its outcome (EPC-12, EPC-15).
    QueueEpic {
        epic: TicketKey,
        child: Option<TicketKey>,
    },
    /// Dev → Backlog: the Park confirmation first.
    Park,
    /// An epic's track on Backlog: send `epic.take_back`, which unqueues its waiting children (EPC-14).
    TakeBack {
        epic: TicketKey,
    },
    /// An epic's track on Backlog with every child started: the Park-all confirmation first.
    ParkAll(ParkAllDialog),
    /// Ready for Review → the Approved section: the Approve choice (Local merge · Open PR); nothing moves until one
    /// is picked, and Esc or Cancel leaves the card where it was (d0087).
    Approve,
    NotYet(String),
}

/// The core's drop target a page target means on `flow`'s board; `None` for a stage today's moves don't reach.
pub fn core_target(flow: &Flow, target: &DropTargetId) -> Option<DropTarget> {
    Columns::of(flow).drop_target(target.as_str())
}

/// [`drop_step_in`] on the default flow.
pub fn drop_step(ticket: &Ticket, tickets: &[Ticket], target: DropTargetId) -> DropStep {
    drop_step_in(
        &otaq_core::flow::default_file().flow,
        ticket,
        tickets,
        &target,
    )
}

/// `tickets` are the project's; only an epic's need them (its track's take back). `flow` is the project's.
pub fn drop_step_in(
    flow: &Flow,
    ticket: &Ticket,
    tickets: &[Ticket],
    target: &DropTargetId,
) -> DropStep {
    let Some(target) = core_target(flow, target) else {
        return DropStep::Nothing;
    };
    match drag::on_drop_in(flow, ticket, tickets, target) {
        DropAction::Nothing => DropStep::Nothing,
        DropAction::Move => DropStep::Move,
        DropAction::QueueEpic { epic, child } => DropStep::QueueEpic { epic, child },
        DropAction::AskPark => DropStep::Park,
        DropAction::TakeBack { epic } => DropStep::TakeBack { epic },
        DropAction::AskParkAll(plan) => match park_all_dialog(&plan) {
            Some(dialog) => DropStep::ParkAll(dialog),
            None => DropStep::Nothing,
        },
        DropAction::ChooseApprove => DropStep::Approve,
        DropAction::NotYet(message) => DropStep::NotYet(message),
    }
}

/// The answer to `board.drop`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "DropOutcome")]
pub enum DropOutcome {
    /// Nothing happened: the card flies back.
    Nothing,
    /// The service carried out the move; the board refetches on its event.
    Moved,
    /// The page shows the confirmation, then sends `board.park`.
    Park { dialog: ParkDialog },
    /// The page shows the Park-all confirmation (Cancel by default), then sends `board.park_all`.
    ParkAll { dialog: ParkAllDialog },
    /// The page shows the popover; a choice opens its dialog.
    Approve { menu: ApproveMenu },
    /// Allowed by the rules, but not built yet: shown as a toast.
    NotYet { message: String },
}

/// Where a card is, as today's board names it: its section in a human check's column ("Ready for Review",
/// "Approved"), else its column ("Dev").
pub fn place_label(columns: &Columns, ticket: &Ticket) -> String {
    let spot = columns.spot(ticket);
    match (columns.get(spot.column), spot.part) {
        (Some(column), Some(part)) if column.has_sections() => match part {
            flow_columns::APPROVED => "Approved".to_owned(),
            flow_columns::READY_FOR_REVIEW => "Ready for Review".to_owned(),
            _ => column.title.clone(),
        },
        (Some(column), _) => column.title.clone(),
        (None, _) => spot.column.to_owned(),
    }
}

/// One Review column state in the gallery.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardGalleryColumn")]
pub struct GalleryColumn {
    pub name: String,
    pub column: ColumnPage,
}

/// One dragged card's verdicts in the gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "BoardGalleryDrag")]
pub struct GalleryDrag {
    /// "SHOP-25 · Dev".
    pub from: String,
    pub answer: DragAnswer,
}

/// The gallery's board section (T-0324): the Review column in its states (*Row · Review column (0086 · V)*), the
/// drop verdicts of a card from each column, the empty board and the agent filter's chip.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "BoardGallery")]
pub struct Gallery {
    pub review: Vec<GalleryColumn>,
    pub drags: Vec<GalleryDrag>,
    pub empty: empty_board::View,
    pub agent_chip: AgentChip,
}

pub fn gallery() -> Gallery {
    use crate::view_models::card_fixtures::{NOW, ticket};
    let flow = otaq_core::flow::default_flow();
    let columns = Columns::of(&flow);
    let facts = ProjectFacts {
        flow: &flow,
        name: "shop-web",
        repo_path: "/Users/me/code/shop-web",
        base_branch: "main",
        agent_names: &[],
        was_validating: &[],
        leads: &[],
        agent_filter: None,
        agents: None,
        agent_list: None,
        starts_held: None,
    };
    let params = Params {
        project: "SHOP".into(),
        width: 1376.,
        height: 856.,
        ..Params::default()
    };
    let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
    pr.flags.pr_number = Some(481);
    let full = vec![
        ticket("SHOP-18", "Checkout API", Status::WaitingForYou),
        ticket("SHOP-23", "Split user full name", Status::WaitingForYou),
        pr.clone(),
    ];
    let review_of = |name: &str, tickets: &[Ticket]| GalleryColumn {
        name: name.to_owned(),
        column: page(
            tickets,
            &facts,
            &params,
            NOW,
            &KeyLabels::base(Platform::Mac),
        )
        .columns
        .into_iter()
        .find(|c| c.kind == ColumnKind::HumanCheck)
        .expect("the Review column"),
    };
    let drag_of = |t: &Ticket| GalleryDrag {
        from: format!("{} · {}", t.key, place_label(&columns, t)),
        answer: drag_answer_in(&flow, t, &[]),
    };
    let one = |t: Ticket| vec![t, ticket("SHOP-24", "Update footer links", Status::Backlog)];
    Gallery {
        review: vec![
            review_of("both sections", &full),
            review_of(
                "an empty Approved section",
                &one(ticket("SHOP-18", "Checkout API", Status::WaitingForYou)),
            ),
            review_of("an empty Ready for Review section", &one(pr.clone())),
            review_of(
                "empty",
                &one(ticket("SHOP-17", "Add Stripe client", Status::Merged)),
            ),
        ],
        drags: [
            ticket("SHOP-24", "Update footer links", Status::Backlog),
            ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working),
            ticket("SHOP-18", "Checkout API", Status::WaitingForYou),
            pr,
            ticket("SHOP-30", "Order history", Status::ClosedNotMerged),
        ]
        .iter()
        .map(drag_of)
        .collect(),
        empty: empty_board::view("shop-web", &KeyLabels::base(Platform::Mac)),
        agent_chip: AgentFilter {
            agent: "a1".into(),
            name: "Checkout slowness".into(),
            keys: Vec::new(),
        }
        .chip(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::{NOW, ticket};
    use otaq_core::domain::Kind;

    fn facts() -> ProjectFacts<'static> {
        ProjectFacts {
            flow: &otaq_core::flow::default_file().flow,
            name: "shop-web",
            repo_path: "/tmp/shop-web",
            base_branch: "main",
            agent_names: &[],
            was_validating: &[],
            leads: &[],
            agent_filter: None,
            agents: None,
            agent_list: None,
            starts_held: None,
        }
    }

    fn params(width: f32) -> Params {
        Params {
            project: "SHOP".into(),
            width,
            height: 856.,
            done_collapsed: None,
            dev_unfolded: false,
            filter: FilterState::default(),
            agent: None,
        }
    }

    fn sample() -> Vec<Ticket> {
        let mut pinned = ticket("SHOP-19", "Price table", Status::Working);
        pinned.flags.pinned_at = Some(10);
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        vec![
            ticket("SHOP-24", "Update footer links", Status::Backlog),
            epic,
            pinned,
            ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou),
            ticket("SHOP-27", "Search results paging", Status::Working),
            ticket("SHOP-21", "Order confirmation email", Status::Queued),
            ticket("SHOP-18", "Checkout API", Status::WaitingForYou),
            pr,
            ticket("SHOP-17", "Add Stripe client", Status::Merged),
        ]
    }

    /// EPC-11 on the page: the epic card carries its rows (draggable before anything starts); once SHOP-45.1 is
    /// queued the epic is no Backlog card and SHOP-45.2 is no card at all, and Backlog counts what it shows.
    #[test]
    fn epics_and_children_show_once() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t
        };
        let mut tickets = vec![
            epic,
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
        ];
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let backlog = &board.columns[0];
        assert_eq!(backlog.count, "1");
        let [BoardCard::Epic { card }] = backlog.cards.as_slice() else {
            panic!("one epic card: {:?}", backlog.cards)
        };
        let rows: Vec<(&str, bool)> = card
            .children
            .iter()
            .map(|c| (c.key.as_str(), c.draggable))
            .collect();
        assert_eq!(rows, [("SHOP-45.1", true), ("SHOP-45.2", true)]);

        tickets[1].status = Status::Queued;
        tickets[1].place = otaq_core::domain::Place::Waiting(None);
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(board.columns[0].cards.is_empty());
        assert_eq!(board.columns[0].count, "0");
        let ready: Vec<&str> = board.columns[1].cards.iter().map(BoardCard::key).collect();
        assert_eq!(ready, ["SHOP-45.1"]);
    }

    /// EPC-10 on the page: no strip while the epic is a card; once SHOP-45.1 is queued the page carries the
    /// strip, one station per shown column but Backlog, with the queued child under Ready for Dev and the other
    /// as a "not queued" chip.
    /// T-0289: the epic card's Lead row (Open lead without a lead) and the track's lead line come from the leads.
    #[test]
    fn the_leads_reach_the_epic_card_and_the_track() {
        use crate::view_models::lead_status::sample;
        use otaq_core::project_agents::lead::LeadStatus;

        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let mut child = ticket("SHOP-45.1", "Search index API", Status::Backlog);
        child.epic = Some("SHOP-45".parse().unwrap());
        let mut tickets = vec![epic, child];
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let [BoardCard::Epic { card }] = board.columns[0].cards.as_slice() else {
            panic!("one epic card")
        };
        assert_eq!(card.lead.as_ref().unwrap().full, "Open lead");

        let leads = [sample(&tickets[0], LeadStatus::CheckingPlan, 0)];
        let with = ProjectFacts {
            leads: &leads,
            ..facts()
        };
        let board = page(
            &tickets,
            &with,
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let [BoardCard::Epic { card }] = board.columns[0].cards.as_slice() else {
            panic!("one epic card")
        };
        assert_eq!(
            card.lead.as_ref().unwrap().full,
            "Search lead · checking the plan"
        );

        tickets[1].status = Status::Queued;
        tickets[1].place = otaq_core::domain::Place::Waiting(None);
        tickets[1].queued_at = Some(1);
        let board = page(
            &tickets,
            &with,
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let track = &board.strip.expect("a track").tracks[0];
        assert_eq!(track.lead.as_ref().unwrap().short, "checking the plan");
    }

    #[test]
    fn the_page_carries_the_strip() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t
        };
        let mut tickets = vec![
            epic,
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
            ticket("SHOP-51", "Fix checkout rounding", Status::Working),
        ];
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert_eq!(board.strip, None);

        tickets[1].status = Status::Queued;
        tickets[1].place = otaq_core::domain::Place::Waiting(None);
        tickets[1].queued_at = Some(1_000);
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let strip = board.strip.clone().expect("a track");
        assert_eq!(strip.title, "Epics in flight · 1");
        let [track] = strip.tracks.as_slice() else {
            panic!("one track: {:?}", strip.tracks)
        };
        assert_eq!(track.key, "SHOP-45");
        let stations: Vec<(ColumnId, Vec<&str>)> = track
            .stations
            .iter()
            .map(|s| {
                (
                    s.column.clone(),
                    s.stops.iter().map(|s| s.key.as_str()).collect(),
                )
            })
            .collect();
        let expected: Vec<(ColumnId, Vec<&str>)> = board.columns[1..]
            .iter()
            .map(|c| {
                let keys = if c.id == ColumnId::READY_FOR_DEV {
                    vec!["SHOP-45.1"]
                } else {
                    vec![]
                };
                (c.id.clone(), keys)
            })
            .collect();
        assert_eq!(stations, expected);
        assert_eq!(track.not_queued[0].key, "SHOP-45.2");
        let json = serde_json::to_value(&board).unwrap();
        assert_eq!(
            json["strip"]["tracks"][0]["stations"][0]["column"],
            "ready_for_dev"
        );
    }

    /// SHL-06, FLW-01: six columns in order (Validate its own); Review holds Ready for Review and Approved as two
    /// sections.
    #[test]
    fn six_columns_reach_the_page_worded() {
        let tickets = sample();
        let page = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let ids: Vec<ColumnId> = page.columns.iter().map(|c| c.id.clone()).collect();
        assert_eq!(
            ids,
            [
                ColumnId::BACKLOG,
                ColumnId::READY_FOR_DEV,
                ColumnId::DEV,
                ColumnId::new("validate"),
                ColumnId::REVIEW,
                ColumnId::DONE
            ]
        );
        let titles: Vec<&str> = page.columns.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Backlog",
                "Ready for Dev",
                "Dev",
                "Validate",
                "Review",
                "Done"
            ]
        );
        assert_eq!(page.min_column_width, 200.);
        assert_eq!(page.done_rail_width, 34.);
        // Q84: every card but Done's SHOP-17 (the epic card counts once).
        assert_eq!(page.tab_count, "8");
        assert_eq!(page.empty, None);
        let backlog = &page.columns[0];
        assert_eq!(backlog.add.as_deref(), Some("Add ticket"));
        assert!(
            backlog
                .cards
                .iter()
                .any(|c| matches!(c, BoardCard::Epic { .. }))
        );
        assert_eq!(
            page.columns[1].hint.as_deref(),
            Some(board::READY_FOR_DEV_HINT)
        );
        let dev = page.columns[2].dev.as_ref().unwrap();
        let top: Vec<&str> = dev.top.iter().map(BoardCard::key).collect();
        assert_eq!(top, ["SHOP-19", "SHOP-22"], "pinned, then needs you");
        assert!(matches!(
            dev.top[1],
            BoardCard::Ticket {
                needs_you: true,
                ..
            }
        ));
        let review = &page.columns[4];
        assert_eq!(review.count, "2", "both sections");
        assert!(review.cards.is_empty(), "the cards are in the sections");
        let [ready, approved] = review.sections.as_slice() else {
            panic!("two sections: {:?}", review.sections)
        };
        assert_eq!(
            (ready.id.clone(), ready.title.as_str(), ready.count.as_str()),
            (ReviewSectionId::READY_FOR_REVIEW, "READY FOR REVIEW", "1")
        );
        assert_eq!(
            (
                approved.id.clone(),
                approved.title.as_str(),
                approved.count.as_str()
            ),
            (ReviewSectionId::APPROVED, "APPROVED", "1")
        );
        assert_eq!(
            ready.cards.iter().map(BoardCard::key).collect::<Vec<_>>(),
            ["SHOP-18"]
        );
        assert_eq!(
            approved
                .cards
                .iter()
                .map(BoardCard::key)
                .collect::<Vec<_>>(),
            ["SHOP-26"]
        );
        assert_eq!(
            approved.note.as_ref().unwrap().text,
            format!(
                "{} doesn’t watch PRs. Mark it once it’s merged on GitHub.",
                otaq_core::naming::PRODUCT_NAME
            )
        );
        assert_eq!(ready.note, None);
        assert_eq!(
            (ready.drag_hint.as_deref(), approved.drag_hint.is_some()),
            (None, true)
        );
        assert_eq!(approved.fold, "Fold Approved");
        assert_eq!(ready.unfold, "Unfold Ready for Review");
        let done = &page.columns[5];
        assert!(!done.collapsed, "1376: Done open");
        assert_eq!(done.toggle.as_deref(), Some("Collapse Done"));
        let json = serde_json::to_value(&page).unwrap();
        assert_eq!(json["columns"][1]["id"], "ready_for_dev");
        assert_eq!(json["columns"][0]["cards"][0]["kind"], "ticket");
        assert_eq!(json["scroll_chip"]["above"], "{n} above");
    }

    /// EPC-16 on the page: SHOP-40.1 and 40.2 merged fold into one line before their cards, SHOP-40.5 closed stays
    /// a card; Done (and its 1280 rail) counts the fold as one.
    #[test]
    fn merged_children_fold_on_the_page() {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-40".parse().unwrap());
            t
        };
        let tickets = vec![
            epic,
            child("SHOP-40.1", Status::Merged),
            child("SHOP-40.2", Status::Merged),
            child("SHOP-40.3", Status::Working),
            child("SHOP-40.5", Status::ClosedNotMerged),
            ticket("SHOP-17", "Add Stripe client", Status::Merged),
        ];
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let done = &board.columns[5];
        assert_eq!(
            done.folds,
            [DoneFold {
                epic: "SHOP-40".into(),
                label: "Wishlist · 2 of 4 merged".into(),
                keys: vec!["SHOP-40.1".into(), "SHOP-40.2".into()],
            }]
        );
        let cards: Vec<&str> = done.cards.iter().map(BoardCard::key).collect();
        assert_eq!(cards, ["SHOP-17", "SHOP-40.1", "SHOP-40.2", "SHOP-40.5"]);
        assert_eq!(done.count, "3");
        let json = serde_json::to_value(&board).unwrap();
        assert_eq!(
            json["columns"][5]["folds"][0]["label"],
            "Wishlist · 2 of 4 merged"
        );
        assert_eq!(json["columns"][0]["folds"], serde_json::json!([]));

        let rail = page(
            &tickets,
            &facts(),
            &params(1216.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(rail.columns[5].collapsed);
        assert_eq!(rail.columns[5].count, "3");
    }

    #[test]
    fn done_folds_at_1280_and_the_user_choice_wins() {
        let tickets = sample();
        let narrow = page(
            &tickets,
            &facts(),
            &params(1216.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(narrow.columns[5].collapsed);
        assert_eq!(narrow.columns[5].toggle.as_deref(), Some("Expand Done"));
        let opened = page(
            &tickets,
            &facts(),
            &Params {
                done_collapsed: Some(false),
                ..params(1216.)
            },
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(!opened.columns[5].collapsed);
    }

    #[test]
    fn dev_folds_by_height_until_unfolded() {
        let mut tickets = sample();
        for n in 50..60 {
            tickets.push(ticket(&format!("SHOP-{n}"), "Work", Status::Working));
        }
        let short = Params {
            height: 400.,
            ..params(1376.)
        };
        let folded = page(
            &tickets,
            &facts(),
            &short,
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let dev = folded.columns[2].dev.as_ref().unwrap();
        assert!(dev.fold.as_deref().unwrap().ends_with("more · working"));
        let open = page(
            &tickets,
            &facts(),
            &Params {
                dev_unfolded: true,
                ..short
            },
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert_eq!(open.columns[2].dev.as_ref().unwrap().fold, None);
    }

    /// The header's filter (T-0221): non-matching cards hide, counts read "shown/total", and each column says how
    /// many it hides.
    #[test]
    fn the_filter_hides_cards_and_columns_say_how_many() {
        let tickets = sample();
        let filtered = page(
            &tickets,
            &facts(),
            &Params {
                filter: FilterState {
                    tokens: vec!["is:needs-you".into()],
                    text: String::new(),
                },
                ..params(1376.)
            },
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let dev = &filtered.columns[2];
        let shown: Vec<&str> = dev
            .dev
            .as_ref()
            .unwrap()
            .top
            .iter()
            .map(BoardCard::key)
            .collect();
        assert_eq!(shown, ["SHOP-22"]);
        assert_eq!(dev.count, "1/3");
        assert_eq!(dev.hidden.as_deref(), Some("2 hidden by filter"));
        assert!(filtered.columns[0].cards.is_empty());
        assert_eq!(filtered.columns[4].count, "1/2");
        assert_eq!(
            filtered.columns[4].sections[1].hidden.as_deref(),
            Some("1 hidden by filter")
        );
        let review: Vec<&str> = filtered.columns[4].sections[0]
            .cards
            .iter()
            .map(BoardCard::key)
            .collect();
        assert_eq!(review, ["SHOP-18"], "review needs you too");
        let unfiltered = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert_eq!(unfiltered.columns[2].hidden, None);
    }

    /// EPC-18, Q55: Focus epic (`g` `e`) on SHOP-40.2 puts `epic:Wishlist` in the filter (another epic's token gives way, other
    /// tokens stay); the board then keeps every card, dims the others and says which epic and cards stay; Esc
    /// takes the token out. A standalone ticket focuses nothing.
    #[test]
    fn focusing_an_epic_dims_the_other_cards() {
        let mut tickets = sample();
        let mut child = ticket("SHOP-40.2", "Wishlist button", Status::Working);
        child.epic = Some("SHOP-40".parse().unwrap());
        tickets.push(child);
        let state = FilterState {
            tokens: vec!["epic:Search".into(), "is:working".into()],
            text: String::new(),
        };
        let key: TicketKey = "SHOP-40.2".parse().unwrap();
        let focused = focus_filter(&state, Some(&key), &tickets).unwrap();
        assert_eq!(focused.tokens, ["is:working", "epic:Wishlist"]);
        let standalone: TicketKey = "SHOP-22".parse().unwrap();
        assert_eq!(focus_filter(&state, Some(&standalone), &tickets), None);
        let left = focus_filter(&focused, None, &tickets).unwrap();
        assert_eq!(left.tokens, ["is:working"]);
        assert_eq!(
            focus_filter(&left, None, &tickets),
            None,
            "nothing to leave"
        );

        let only_epic = FilterState {
            tokens: vec!["epic:Wishlist".into()],
            text: String::new(),
        };
        let board = page(
            &tickets,
            &facts(),
            &Params {
                filter: only_epic,
                ..params(1376.)
            },
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let focus = board.focus.unwrap();
        assert_eq!(focus.epics, ["SHOP-40"]);
        assert_eq!(focus.keys, ["SHOP-40", "SHOP-40.2"]);
        let dev = &board.columns[2];
        assert_eq!(dev.hidden, None, "dimmed, not hidden");
        assert_eq!(dev.count, "1/4");
        let unfocused = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert_eq!(unfocused.focus, None);
    }

    #[test]
    fn an_empty_project_shows_the_empty_board_over_its_columns() {
        let page = page(
            &[],
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        let empty = page.empty.unwrap();
        assert_eq!(empty.title, "shop-web is ready");
        assert_eq!(
            empty.text,
            "Tell the Project agent on the left what you want: it reads the code and writes the tickets. Or write the first one yourself."
        );
        let labels: Vec<&str> = empty.buttons.iter().map(|b| b.label).collect();
        assert_eq!(
            labels,
            ["New ticket"],
            "no Project agent or terminal button"
        );
        assert_eq!(page.columns.len(), 6);
        let review = &page.columns[4];
        let empties: Vec<(&str, &str)> = review
            .sections
            .iter()
            .map(|s| (s.title.as_str(), s.empty.as_str()))
            .collect();
        assert_eq!(
            empties,
            [
                ("READY FOR REVIEW", "Nothing to review"),
                ("APPROVED", "Nothing approved")
            ]
        );
        assert!(
            review.sections[1]
                .drag_hint
                .as_deref()
                .unwrap()
                .starts_with("Drag a ticket here from Ready for Review")
        );
    }

    #[test]
    fn a_drag_answers_every_column_at_once() {
        let backlog = ticket("SHOP-24", "Update footer links", Status::Backlog);
        let answer = drag_answer(&backlog, &[]);
        assert_eq!(answer.targets.len(), 7);
        let verdict = |id: DropTargetId| {
            answer
                .targets
                .iter()
                .find(|c| c.target == id)
                .unwrap()
                .verdict
                .clone()
        };
        assert_eq!(verdict(DropTargetId::BACKLOG), Verdict::Home);
        assert_eq!(
            verdict(DropTargetId::READY_FOR_DEV),
            Verdict::Allowed {
                icon: IconName::Play,
                hint: "Drop to queue · starts in priority order".into(),
                note: None,
            }
        );
        assert_eq!(
            verdict(DropTargetId::REVIEW),
            Verdict::Forbidden {
                title: "Only agents move tickets to Ready for Review".into(),
                detail: Some("Drag to Ready for Dev to queue it.".into())
            },
            "*Drag · over a forbidden column*"
        );
        let json = serde_json::to_value(drag_answer(&backlog, &[])).unwrap();
        assert_eq!(json["targets"][0]["verdict"]["kind"], "home");
        assert_eq!(json["targets"][5]["target"], "approved");
    }

    /// The verdict of `ticket` over `target`.
    fn verdict_on(ticket: &Ticket, target: DropTargetId) -> Verdict {
        drag_answer(ticket, &[])
            .targets
            .into_iter()
            .find(|t| t.target == target)
            .unwrap()
            .verdict
    }

    /// d0087, §7.4: every source column over the Review column (outside Approved: Ready for Review) and over the
    /// Approved section, answered up front.
    #[test]
    fn review_and_approved_verdicts_for_every_source() {
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        let sources = [
            ticket("SHOP-24", "Update footer links", Status::Backlog),
            ticket("SHOP-21", "Order confirmation email", Status::Queued),
            ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working),
            ticket("SHOP-18", "Checkout API", Status::WaitingForYou),
            pr,
            ticket("SHOP-30", "Order history", Status::ClosedNotMerged),
            ticket("SHOP-17", "Add Stripe client", Status::Merged),
        ];
        let kind = |v: &Verdict| match v {
            Verdict::Home => "home".to_owned(),
            Verdict::Allowed { hint, .. } => format!("allowed: {hint}"),
            Verdict::Forbidden { title, .. } => format!("forbidden: {title}"),
        };
        let rows: Vec<(String, String, String)> = sources
            .iter()
            .map(|t| {
                (
                    t.key.to_string(),
                    kind(&verdict_on(t, DropTargetId::REVIEW)),
                    kind(&verdict_on(t, DropTargetId::APPROVED)),
                )
            })
            .collect();
        let only_agents = "forbidden: Only agents move tickets to Ready for Review";
        let approve_first = "forbidden: Approve a ticket from Ready for Review";
        let row = |key: &str, review: &str, approved: &str| {
            (key.to_owned(), review.to_owned(), approved.to_owned())
        };
        assert_eq!(
            rows,
            [
                row("SHOP-24", only_agents, approve_first),
                row("SHOP-21", only_agents, approve_first),
                row("SHOP-25", only_agents, approve_first),
                row(
                    "SHOP-18",
                    "home",
                    "allowed: Drop to approve SHOP-18 → choose Local merge or Open PR"
                ),
                row("SHOP-26", "allowed: Drop to rework · PR #481 stays", "home"),
                row(
                    "SHOP-30",
                    only_agents,
                    "forbidden: Drag it to Backlog to reopen it first"
                ),
                row(
                    "SHOP-17",
                    "forbidden: Merged tickets are locked",
                    "forbidden: Merged tickets are locked"
                ),
            ]
        );
        // Dev → Review: today's reason, and no "queue it" line (only a Backlog card can be queued).
        assert_eq!(
            verdict_on(&sources[2], DropTargetId::REVIEW),
            Verdict::Forbidden {
                title: drag::ONLY_AGENTS_TO_REVIEW.into(),
                detail: None
            }
        );
    }

    /// d0087: Ready for Review → the Approved section opens the Approve choice and moves nothing by itself; Approved
    /// → the Review column is PR rework; a Ready for Review card dropped on its own column does nothing.
    #[test]
    fn approve_choice_and_pr_rework_on_drop() {
        let review = ticket("SHOP-18", "Checkout API", Status::WaitingForYou);
        assert_eq!(
            drop_step(&review, &[], DropTargetId::APPROVED),
            DropStep::Approve,
            "the choice, never a Move"
        );
        assert_eq!(
            drop_step(&review, &[], DropTargetId::REVIEW),
            DropStep::Nothing,
            "home"
        );
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        assert_eq!(
            drop_step(&pr, &[], DropTargetId::REVIEW),
            DropStep::Move,
            "PR rework"
        );
        assert_eq!(
            drop_step(&pr, &[], DropTargetId::APPROVED),
            DropStep::Nothing
        );
        let working = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working);
        for target in [DropTargetId::REVIEW, DropTargetId::APPROVED] {
            assert_eq!(drop_step(&working, &[], target), DropStep::Nothing);
        }
        let targets: Vec<DropTargetId> = drag_answer(&review, &[])
            .targets
            .iter()
            .map(|t| t.target.clone())
            .collect();
        assert_eq!(
            targets,
            [
                DropTargetId::BACKLOG,
                DropTargetId::READY_FOR_DEV,
                DropTargetId::DEV,
                DropTargetId::new("validate"),
                DropTargetId::REVIEW,
                DropTargetId::APPROVED,
                DropTargetId::DONE
            ],
            "both sections answered up front"
        );
        let flow = otaq_core::flow::default_flow();
        let columns = Columns::of(&flow);
        assert!(
            columns
                .get("review")
                .unwrap()
                .targets()
                .contains(&"approved".to_owned())
        );
        assert_eq!(
            columns.drop_target("review"),
            Some(DropTarget::ReadyForReview)
        );
    }

    /// SHL-01 on the page: Done opens from 1360 px of board width; the strip's Done station is a check while Done
    /// is the rail.
    #[test]
    fn done_opens_at_1360_and_the_strip_follows() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t.queued_at = Some(1);
            t
        };
        let tickets = vec![
            epic,
            child("SHOP-45.1", Status::Merged),
            child("SHOP-45.2", Status::WaitingForYou),
            child("SHOP-45.3", Status::Working),
        ];
        let open = page(
            &tickets,
            &facts(),
            &params(1360.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(!open.columns[5].collapsed);
        let stations = |p: &BoardPage| -> Vec<(ColumnId, Option<IconName>, usize)> {
            p.strip.as_ref().unwrap().tracks[0]
                .stations
                .iter()
                .map(|s| (s.column.clone(), s.check, s.stops.len()))
                .collect()
        };
        assert_eq!(
            stations(&open),
            [
                (ColumnId::READY_FOR_DEV, None, 0),
                (ColumnId::DEV, None, 1),
                (ColumnId::new("validate"), None, 0),
                (ColumnId::REVIEW, None, 1),
                (ColumnId::DONE, None, 1),
            ],
            "one Review stop"
        );
        let folded = page(
            &tickets,
            &facts(),
            &params(1359.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(folded.columns[5].collapsed);
        assert_eq!(
            stations(&folded)[4],
            (ColumnId::DONE, Some(IconName::CircleCheck), 1)
        );
        let opened = page(
            &tickets,
            &facts(),
            &Params {
                done_collapsed: Some(false),
                ..params(1359.)
            },
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(!opened.columns[5].collapsed, "the user's choice wins");
        assert_eq!(stations(&opened)[4].1, None);
    }

    #[test]
    fn the_gallery_has_every_review_state() {
        let g = gallery();
        let counts: Vec<(&str, Vec<&str>)> = g
            .review
            .iter()
            .map(|c| {
                (
                    c.column.count.as_str(),
                    c.column.sections.iter().map(|s| s.count.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            counts,
            [
                ("3", vec!["2", "1"]),
                ("1", vec!["1", "0"]),
                ("1", vec!["0", "1"]),
                ("0", vec!["0", "0"])
            ]
        );
        assert_eq!(g.drags.len(), 5);
        assert_eq!(g.drags[1].from, "SHOP-25 · Dev");
        assert!(g.empty.text.starts_with("Tell the Project agent"));
    }

    /// d0087 (Q-G): the agent filter shows only the tickets the agent created or changed, combines with search,
    /// shows its chip, and clears.
    #[test]
    fn the_agent_filter_shows_its_tickets_and_clears() {
        let tickets = sample();
        let created = otaq_core::project_agents::CreatedAndChanged {
            created: vec![otaq_core::project_agents::CreatedTicket {
                key: "SHOP-24".parse().unwrap(),
                title: "Update footer links".into(),
                status: Status::Backlog,
                deletable: true,
            }],
            edited: vec!["SHOP-22".parse().unwrap(), "SHOP-18".parse().unwrap()],
            dependencies: 0,
            summary: String::new(),
        };
        let agent = AgentFilter::of("a1", "Checkout slowness", &created);
        let with = ProjectFacts {
            agent_filter: Some(&agent),
            ..facts()
        };
        let filtered = |filter: FilterState, agent: Option<&str>| {
            page(
                &tickets,
                &with,
                &Params {
                    filter,
                    agent: agent.map(str::to_owned),
                    ..params(1376.)
                },
                NOW,
                &KeyLabels::base(Platform::Mac),
            )
        };
        let shown = |p: &BoardPage| -> Vec<String> {
            let mut keys: Vec<String> = p
                .columns
                .iter()
                .flat_map(|c| {
                    let dev = c.dev.iter().flat_map(|d| d.top.iter().chain(&d.working));
                    let sections = c.sections.iter().flat_map(|s| &s.cards);
                    c.cards.iter().chain(dev).chain(sections)
                })
                .map(|c| c.key().to_owned())
                .collect();
            keys.sort();
            keys
        };
        let by_agent = filtered(FilterState::default(), Some("a1"));
        assert_eq!(shown(&by_agent), ["SHOP-18", "SHOP-22", "SHOP-24"]);
        assert_eq!(
            by_agent.agent_filter,
            Some(AgentChip {
                agent: "a1".into(),
                label: "Created or changed by Checkout slowness".into(),
                clear: "Clear filter · Esc".into(),
            })
        );
        assert_eq!(by_agent.columns[0].count, "1/2");
        assert_eq!(by_agent.columns[4].count, "1/2");

        // With a search: both must match.
        let both = filtered(
            FilterState {
                tokens: vec![],
                text: "footer".into(),
            },
            Some("a1"),
        );
        assert_eq!(shown(&both), ["SHOP-24"]);
        let both = filtered(
            FilterState {
                tokens: vec!["is:needs-you".into()],
                text: String::new(),
            },
            Some("a1"),
        );
        assert_eq!(shown(&both), ["SHOP-18", "SHOP-22"]);

        // Cleared (✕ or Esc): every card, no chip.
        let cleared = filtered(FilterState::default(), None);
        assert_eq!(cleared.agent_filter, None);
        assert_eq!(shown(&cleared).len(), tickets.len(), "every card");
        assert_eq!(cleared.columns[0].count, "2");
        let json = serde_json::to_value(&by_agent).unwrap();
        assert_eq!(
            json["agent_filter"]["label"],
            "Created or changed by Checkout slowness"
        );
    }

    /// *Epic flows 1 → 1* and *epic flows 3 → 8* (EPC-12): the epic card and a row are allowed over Ready for Dev
    /// and Dev with what they queue, and not allowed over Review, Approved and Done; their drops queue through
    /// `epic.queue`.
    #[test]
    fn an_epic_and_a_row_answer_their_drag_and_queue_on_drop() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status: Status| {
            let mut t = ticket(key, "Search child", status);
            t.epic = Some(epic.key.clone());
            t
        };
        let tickets = vec![
            epic.clone(),
            child("SHOP-45.1", Status::Backlog),
            child("SHOP-45.2", Status::Backlog),
        ];
        let verdicts = |answer: DragAnswer| -> Vec<Verdict> {
            answer.targets.into_iter().map(|c| c.verdict).collect()
        };
        let card = verdicts(drag_answer(&epic, &tickets));
        assert_eq!(card[0], Verdict::Home);
        for v in &card[1..3] {
            assert_eq!(
                v,
                &Verdict::Allowed {
                    icon: IconName::Play,
                    hint: "Queue 2 children".into(),
                    note: None,
                },
                "an epic drop queues its children (d0080)"
            );
        }
        for v in &card[3..] {
            assert!(matches!(v, Verdict::Forbidden { .. }), "{v:?}");
        }
        let row = verdicts(drag_answer(&tickets[1], &tickets));
        assert_eq!(
            row[1],
            Verdict::Allowed {
                icon: IconName::Play,
                hint: "Queue SHOP-45.1".into(),
                note: None,
            }
        );
        assert!(
            row[3..]
                .iter()
                .all(|v| matches!(v, Verdict::Forbidden { .. }))
        );
        let mut parked = child("SHOP-45.2", Status::Parked);
        parked.place = otaq_core::domain::Place::Backlog;
        assert_eq!(
            verdicts(drag_answer(&parked, &tickets))[2],
            Verdict::Allowed {
                icon: IconName::Play,
                hint: "Resume SHOP-45.2".into(),
                note: None,
            }
        );

        assert_eq!(
            drop_step(&epic, &[], DropTargetId::READY_FOR_DEV),
            DropStep::QueueEpic {
                epic: epic.key.clone(),
                child: None
            }
        );
        assert_eq!(
            drop_step(&tickets[1], &[], DropTargetId::DEV),
            DropStep::QueueEpic {
                epic: epic.key.clone(),
                child: Some(tickets[1].key.clone())
            }
        );
        assert_eq!(drop_step(&epic, &[], DropTargetId::DONE), DropStep::Nothing);
        assert_eq!(
            drop_step(&tickets[1], &[], DropTargetId::APPROVED),
            DropStep::Nothing
        );
    }

    /// *Epic flows 1 → 3* and *epic flows 4 → 11* (EPC-14): the track's drag accepts Backlog with what it takes
    /// back (two lines when a child keeps running) and Ready for Dev / Dev with what it queues; a drop on Backlog
    /// unqueues at once, or asks first when every child started.
    #[test]
    fn a_track_answers_its_drag_and_takes_back_on_drop() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status: Status| {
            let mut t = ticket(key, "Search child", status);
            t.epic = Some(epic.key.clone());
            t
        };
        let mut tickets = vec![
            epic.clone(),
            child("SHOP-45.1", Status::Working),
            child("SHOP-45.2", Status::Queued),
            child("SHOP-45.3", Status::Backlog),
        ];
        let verdicts = |answer: DragAnswer| -> Vec<Verdict> {
            answer.targets.into_iter().map(|c| c.verdict).collect()
        };
        let track = verdicts(drag_answer(&epic, &tickets));
        assert_eq!(
            track[0],
            Verdict::Allowed {
                icon: IconName::ArrowDownToLine,
                hint: "Unqueue 1 never-started child (45.2)".into(),
                note: Some("SHOP-45.1 keeps running".into()),
            }
        );
        for v in &track[1..3] {
            assert_eq!(
                v,
                &Verdict::Allowed {
                    icon: IconName::Play,
                    hint: "Queue 1 child".into(),
                    note: None,
                },
                "the rest of the epic"
            );
        }
        for v in &track[3..] {
            assert!(matches!(v, Verdict::Forbidden { .. }), "{v:?}");
        }
        let json = serde_json::to_value(drag_answer(&epic, &tickets)).unwrap();
        assert_eq!(
            json["targets"][0]["verdict"]["note"],
            "SHOP-45.1 keeps running"
        );
        assert_eq!(
            drop_step(&epic, &tickets, DropTargetId::BACKLOG),
            DropStep::TakeBack {
                epic: epic.key.clone()
            }
        );
        assert_eq!(
            drop_step(&epic, &tickets, DropTargetId::READY_FOR_DEV),
            DropStep::QueueEpic {
                epic: epic.key.clone(),
                child: None
            }
        );
        assert_eq!(
            drop_step(&epic, &tickets, DropTargetId::DONE),
            DropStep::Nothing
        );

        // State 11: both started and run; the drop asks first.
        tickets.truncate(2);
        tickets.push(child("SHOP-45.2", Status::Working));
        assert_eq!(
            verdicts(drag_answer(&epic, &tickets))[0],
            Verdict::Allowed {
                icon: IconName::SquareParking,
                hint: "Park 2 running children".into(),
                note: None,
            }
        );
        let DropStep::ParkAll(dialog) = drop_step(&epic, &tickets, DropTargetId::BACKLOG) else {
            panic!("Park all asks first");
        };
        assert_eq!(
            dialog,
            ParkAllDialog {
                key: "SHOP-45".into(),
                title: "Park 2 running tickets?".into(),
                message: "Their work is kept. SHOP-45.1 and SHOP-45.2 stop and go back to Backlog with their \
                          conversations."
                    .into(),
                cancel: "Cancel".into(),
                confirm: "Park 2 tickets".into(),
                confirm_icon: IconName::SquareParking,
            }
        );
    }

    /// State 10 (d0066): a child card taken out of Ready for Dev goes back to Backlog without a dialog and shows as
    /// a "not queued" chip on its track, which stays while SHOP-45.1 runs.
    #[test]
    fn a_child_card_taken_out_of_the_queue_becomes_a_chip() {
        let mut epic = ticket("SHOP-45", "Search", Status::Backlog);
        epic.kind = Kind::Epic;
        let child = |key: &str, status| {
            let mut t = ticket(key, "Child", status);
            t.epic = Some("SHOP-45".parse().unwrap());
            t
        };
        let mut tickets = vec![
            epic,
            child("SHOP-45.1", Status::Working),
            child("SHOP-45.2", Status::Queued),
        ];
        assert_eq!(
            drop_step(&tickets[2], &tickets, DropTargetId::BACKLOG),
            DropStep::Move
        );
        // What the service's unqueue leaves (d0066).
        tickets[2].status = Status::Backlog;
        tickets[2].place = otaq_core::domain::Place::Backlog;
        let board = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(board.columns[0].cards.is_empty(), "no Backlog card");
        let strip = board.strip.expect("the track stays");
        let chips: Vec<&str> = strip.tracks[0]
            .not_queued
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(chips, ["SHOP-45.2"]);
    }

    #[test]
    fn drops_say_what_comes_next() {
        let working = ticket("SHOP-25", "Upgrade to Next.js 15", Status::Working);
        assert_eq!(
            drop_step(&working, &[], DropTargetId::BACKLOG),
            DropStep::Park
        );
        let park = park_dialog(&working);
        assert_eq!(park.title, "Park SHOP-25?");
        assert!(park.message.contains("will be stopped"), "{}", park.message);
        assert_eq!(park.kept.len(), 3);
        let review = ticket("SHOP-18", "Checkout API", Status::WaitingForYou);
        assert_eq!(
            drop_step(&review, &[], DropTargetId::APPROVED),
            DropStep::Approve
        );
        let menu = approve_menu(&review, "main", std::slice::from_ref(&review), None);
        assert_eq!(menu.title, "Approve SHOP-18 Checkout API");
        assert_eq!(menu.items[0].choice, ApproveChoice::LocalMerge);
        assert_eq!((menu.items[0].number, menu.items[1].number), (1, 2));
        assert_eq!(menu.items[1].icon, IconName::GitPullRequest);
        assert_eq!(
            menu.note,
            "Both open a confirmation step before anything runs."
        );
        let mut pr = ticket("SHOP-26", "Add sitemap", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        assert_eq!(
            drop_step(&pr, &[], DropTargetId::REVIEW),
            DropStep::Move,
            "PR rework"
        );
        let closed = ticket("SHOP-30", "Order history", Status::ClosedNotMerged);
        assert_eq!(
            drop_step(&closed, &[], DropTargetId::BACKLOG),
            DropStep::Move,
            "Reopen"
        );
        assert_eq!(
            drop_step(&closed, &[], DropTargetId::DONE),
            DropStep::Nothing
        );
        let json = serde_json::to_value(DropOutcome::Park { dialog: park }).unwrap();
        assert_eq!(json["kind"], "park");
        assert_eq!(json["dialog"]["confirm"], "Park");
    }
    /// Frame 12's chain at one storyboard step: SHOP-17 merged, SHOP-18 and SHOP-19 as given, SHOP-20 needs both,
    /// SHOP-21 needs SHOP-20.
    fn storyboard(s18: Status, s19: Status, s20: Status) -> Vec<Ticket> {
        let with = |key: &str, title: &str, status: Status, deps: &[&str]| {
            let mut t = ticket(key, title, status);
            t.depends_on = deps.iter().map(|d| d.parse().unwrap()).collect();
            t
        };
        let mut shop_20 = with("SHOP-20", "Checkout page", s20, &["SHOP-18", "SHOP-19"]);
        if s20 == Status::Working {
            shop_20.work = Some(otaq_core::board::tickets::Work {
                base_sha: Some("3b7d0e1c9a".into()),
                ..Default::default()
            });
        }
        vec![
            with("SHOP-17", "Add Stripe client", Status::Merged, &[]),
            with("SHOP-18", "Checkout API", s18, &["SHOP-17"]),
            with("SHOP-19", "Price table", s19, &[]),
            shop_20,
            with(
                "SHOP-21",
                "Order confirmation email",
                Status::Queued,
                &["SHOP-20"],
            ),
        ]
    }

    /// One card of the page as JSON text, to look for the words the storyboard draws on it.
    fn card_text(page: &BoardPage, key: &str) -> String {
        page.columns
            .iter()
            .flat_map(|c| {
                let dev = c.dev.iter().flat_map(|d| d.top.iter().chain(&d.working));
                let sections = c.sections.iter().flat_map(|s| &s.cards);
                c.cards.iter().chain(dev).chain(sections)
            })
            .find(|c| c.key() == key)
            .map(|c| serde_json::to_string(c).unwrap())
            .unwrap_or_else(|| panic!("{key} is on the board"))
    }

    /// *12 · Stack & dependencies → 2 · The chain runs … 6 · Closed parent*, as the page receives them (the hand
    /// check walks them on the real board).
    #[test]
    fn the_storyboards_reach_the_page() {
        let at = |s18, s19, s20| {
            let tickets = storyboard(s18, s19, s20);
            page(
                &tickets,
                &facts(),
                &params(1376.),
                NOW,
                &KeyLabels::base(Platform::Mac),
            )
        };
        // 1 · Planning: the parents on the situation line.
        let planning = at(Status::Backlog, Status::Backlog, Status::Backlog);
        assert!(card_text(&planning, "SHOP-20").contains("after SHOP-18, SHOP-19"));
        // 2 · The chain runs, then SHOP-18 in Review.
        let runs = at(Status::Working, Status::Working, Status::Queued);
        assert!(card_text(&runs, "SHOP-20").contains("waits for SHOP-18, SHOP-19 to merge"));
        assert!(card_text(&runs, "SHOP-21").contains("waits for SHOP-20 to merge"));
        let review = at(Status::WaitingForYou, Status::Working, Status::Queued);
        assert!(card_text(&review, "SHOP-20").contains("SHOP-18 in Review"));
        // 3 · One parent merged.
        let one = at(Status::Merged, Status::WaitingForYou, Status::Queued);
        assert!(card_text(&one, "SHOP-20").contains("waits for SHOP-19 to merge"));
        // 4 · All parents merged: SHOP-20 works from the new main; SHOP-21 still waits.
        let all = at(Status::Merged, Status::Merged, Status::Working);
        assert!(
            card_text(&all, "SHOP-20").contains("from main @ 3b7d0e1 · all dependencies merged")
        );
        assert!(card_text(&all, "SHOP-21").contains("waits for SHOP-20 to merge"));
        // 5 · PR variant.
        let mut tickets = storyboard(Status::PrOpen, Status::Merged, Status::Queued);
        tickets[1].flags.pr_number = Some(482);
        let pr = page(
            &tickets,
            &facts(),
            &params(1376.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert!(card_text(&pr, "SHOP-20").contains("PR #482 open"));
        assert!(card_text(&pr, "SHOP-18").contains("Mark merged"));
        // 6 · Closed parent never counts as merged.
        let closed = at(
            Status::Merged,
            Status::ClosedNotMerged,
            Status::ParentClosed,
        );
        let shop_20 = card_text(&closed, "SHOP-20");
        assert!(shop_20.contains("Parent closed"), "{shop_20}");
        assert!(shop_20.contains("SHOP-19 was closed without merging"));
        // The highlight of SHOP-20 lights the whole chain.
        let chain = all.chains.iter().find(|c| c.key == "SHOP-20").unwrap();
        assert_eq!(
            chain.members,
            ["SHOP-17", "SHOP-18", "SHOP-19", "SHOP-20", "SHOP-21"]
        );
        assert!(planning.chains.iter().all(|c| c.key != "SHOP-22"));
    }

    /// FLW-12 on the page: the test flow at 1440 with the pane open is long and doesn't fit: 184 px columns, the
    /// empty Grill a rail with no toggle (Done's keeps its own), Dev's header with "Validating 1" and the stage
    /// strip; the default flow keeps 200 px and no strip.
    #[test]
    fn a_long_flow_reaches_the_page_with_its_strip() {
        use otaq_core::domain::Place;
        let flow = crate::view_models::flow_columns::every_kind();
        let at = |key: &str, status, place: Place| {
            let mut t = ticket(key, "Work", status);
            t.place = place;
            t
        };
        let tickets = vec![
            at("SHOP-33", Status::Working, Place::stage_of("dev")),
            at("SHOP-30", Status::Checking, Place::stage_of("validate")),
            at("SHOP-19", Status::Checking, Place::stage_of("security")),
            at("SHOP-23", Status::WaitingForYou, Place::stage_of("qa")),
            at("SHOP-26", Status::PrOpen, Place::stage_of("ship")),
        ];
        let page = super::page(
            &tickets,
            &ProjectFacts {
                flow: &flow,
                ..facts()
            },
            &params(1440. - 64. - 400.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert_eq!(page.min_column_width, board::COMPACT_COLUMN_WIDTH);
        let grill = &page.columns[2];
        assert!(grill.collapsed && grill.toggle.is_none());
        let done = page.columns.last().unwrap();
        assert!(done.collapsed && done.toggle.as_deref() == Some("Expand Done"));
        let dev = &page.columns[3];
        assert_eq!(
            dev.folded,
            [FoldedStage {
                tag: "Validating".into(),
                count: "1".into()
            }]
        );
        assert!(page.columns.iter().all(|c| c.quiet_icon));
        let strip = page.stage_strip.expect("the stage strip");
        assert_eq!(strip.label, "Columns");
        let names: Vec<&str> = strip.columns.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            names,
            [
                "Backlog",
                "Ready for Dev",
                "Grill",
                "Dev",
                "Security",
                "QA",
                "Done · PR",
                "Done"
            ]
        );
        assert_eq!(
            strip.columns[3].folded,
            [StageStripFolded {
                name: "Validate".into(),
                count: "1".into()
            }]
        );

        let default = super::page(
            &tickets,
            &facts(),
            &params(1440. - 64. - 400.),
            NOW,
            &KeyLabels::base(Platform::Mac),
        );
        assert_eq!(default.min_column_width, board::MIN_COLUMN_WIDTH);
        assert_eq!(default.stage_strip, None);
    }
}
