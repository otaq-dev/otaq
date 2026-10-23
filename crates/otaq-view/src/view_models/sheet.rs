//! The ticket sheet (SHT-01, SHT-02; d0087, d0095, d0097; canvas `s25-02`, `s25-03`, `s25-10`): a 1040 px sheet over
//! the board (920 px beside the agent panel), or the whole content area at full page (⤢, "← Board"). Its header is
//! the breadcrumb (project / [epic /] key), the title and status, **Move…** and ⋯; under it the **details line**
//! (Assignee, Model, Working, Priority, Branch, Epic) and the flow path; then the tabs **Overview · Session · Files ·
//! Activity**, `1`–`4` and `g o` / `g s` / `g f` / `g a` (an epic: Overview · Lead · Files in review · Activity).
//!
//! Until each tab's own ticket lands, a tab shows today's panel content ([`Tab`], registered by its screen): Session
//! the Terminal, Run and Shell tabs (Agent · Run · Shell), Activity History, Lead the Lead tab. [`SheetTab::contents`]
//! says which. Overview is its own (`overview.rs`, T-0465), and so is Files (`files_tab.rs`, T-0468).

use otaq_core::agents::facts::{PathKind, PathStage, TicketAgents};
use otaq_core::agents::store::SessionAgentRun;
use otaq_core::agents::{Agent, SessionRole};
use otaq_core::board::tickets::{Base, Ticket};
use otaq_core::domain::{Place, StatusSet};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::agents::TOOL_DEFAULT;
use crate::view_models::board_page::{DragAnswer, Verdict};
use crate::view_models::card::tokens_label;
use crate::view_models::flow_columns::Columns;
use crate::view_models::menu::MenuItem;
use crate::view_models::panel::{self, Tab};
use crate::view_models::priority;
use crate::view_models::stage_choices;
use crate::view_models::tool_avatar::ToolAvatarView;
use crate::view_models::tool_problems::Tools;

/// The sheet's width over the board (SHT-01).
pub const WIDTH: f32 = 1040.;
/// Its width with the Project agent's panel open beside it.
pub const WIDTH_BESIDE_AGENT: f32 = 920.;
/// What it keeps clear on each side of it; a narrow window narrows the sheet.
pub const MARGIN: f32 = 18.;

/// The sheet's width over a main area `main_width` px wide: 1040 px, 920 px beside the agent panel, never wider than
/// the main area less its margins.
pub fn width(beside_agent: bool, main_width: f32) -> f32 {
    let wanted = if beside_agent {
        WIDTH_BESIDE_AGENT
    } else {
        WIDTH
    };
    wanted.min(main_width - 2. * MARGIN).max(0.)
}

/// The sheet's tabs (SHT-02). `SheetTab` on the page, where each tab's screen registers its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SheetTab")]
pub enum SheetTab {
    Overview,
    Session,
    Files,
    Activity,
    /// An epic's lead (d0083), in the place of a ticket's Session.
    Lead,
}

impl SheetTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Session => "Session",
            Self::Files => "Files",
            Self::Activity => "Activity",
            Self::Lead => "Lead",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Overview => IconName::FileText,
            Self::Session => IconName::SquareTerminal,
            Self::Files => IconName::FolderOpen,
            Self::Activity => IconName::History,
            Self::Lead => IconName::Bot,
        }
    }

    /// Its key's number: `1`–`4`, and `g o` / `g s` / `g f` / `g a` (KeyAction::SwitchTab). An epic's Lead takes
    /// Session's.
    pub fn number(self) -> u8 {
        match self {
            Self::Overview => 1,
            Self::Session | Self::Lead => 2,
            Self::Files => 3,
            Self::Activity => 4,
        }
    }

    /// Today's panel tabs it shows until its own ticket lands, the one it opens on first (T-0466 Session and
    /// Activity, T-0468 Files, T-0469 the epic's). Overview has its own body (T-0465); `Tab::Ticket` stays only as
    /// the selection's name for it (⋯ Edit ticket… asks for it), never a panel tab of its own.
    pub fn contents(self) -> &'static [Tab] {
        match self {
            Self::Overview => &[Tab::Ticket],
            Self::Session => &[Tab::Terminal, Tab::Run, Tab::Shell],
            Self::Files => &[Tab::Files],
            Self::Activity => &[Tab::History],
            Self::Lead => &[Tab::Lead],
        }
    }
}

/// The sheet's tabs for `ticket`, in order: Overview · Session · Files · Activity; an epic shows Overview · Lead ·
/// Activity, with Files once it is in review (its branch's diff, d0059).
pub fn tabs(ticket: &Ticket) -> Vec<SheetTab> {
    if !ticket.is_epic() {
        return vec![
            SheetTab::Overview,
            SheetTab::Session,
            SheetTab::Files,
            SheetTab::Activity,
        ];
    }
    let review = panel::epic_tabs(ticket).contains(&Tab::Files);
    [SheetTab::Overview, SheetTab::Lead]
        .into_iter()
        .chain(review.then_some(SheetTab::Files))
        .chain([SheetTab::Activity])
        .collect()
}

/// The shown tab whose key is `n`.
pub fn numbered(shown: &[SheetTab], n: u8) -> Option<SheetTab> {
    shown.iter().copied().find(|t| t.number() == n)
}

/// The tab the sheet opens on (SHT-02): Overview, and Files (on All changes) while the ticket waits in your check
/// (SHT-06) and has the tab.
pub fn opens_on(ticket: &Ticket) -> SheetTab {
    if super::files_tab::waits_for_you(ticket) && tabs(ticket).contains(&SheetTab::Files) {
        SheetTab::Files
    } else {
        SheetTab::Overview
    }
}

/// Session's switch until T-0466: "Agent", "Run", "Shell".
pub fn part_label(content: Tab) -> &'static str {
    match content {
        Tab::Terminal => "Agent",
        other => other.label(),
    }
}

/// One crumb of the breadcrumb: the project, the epic (opens it), the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SheetCrumb")]
pub struct Crumb {
    pub text: String,
    /// The ticket a click opens (the epic's crumb).
    pub opens: Option<String>,
    /// The key, in the mono font.
    pub key: bool,
}

/// "40.2" for an epic child (its epic is the crumb before it); other keys whole.
fn short_key(ticket: &Ticket) -> String {
    match ticket.key.child {
        Some(child) => format!("{}.{child}", ticket.key.number),
        None => ticket.key.to_string(),
    }
}

/// `shop-web / SHOP-22`, `shop-web / Wishlist / 40.2` (SHT-01, SHT-07). At full page the project's crumb is the
/// page's "← Board".
pub fn breadcrumb(ticket: &Ticket, project: &str, epic_title: Option<&str>) -> Vec<Crumb> {
    let epic = ticket.epic.as_ref().map(|epic| Crumb {
        text: epic_title.map_or_else(|| epic.to_string(), str::to_owned),
        opens: Some(epic.to_string()),
        key: epic_title.is_none(),
    });
    [Crumb {
        text: project.to_owned(),
        opens: None,
        key: false,
    }]
    .into_iter()
    .chain(epic)
    .chain([Crumb {
        text: short_key(ticket),
        opens: None,
        key: true,
    }])
    .collect()
}

/// What a click on a detail does.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "SheetDetailChange")]
pub enum DetailChange {
    /// Priority's menu (`panel.priority`).
    Priority { items: Vec<MenuItem> },
    /// The stage's agent before it starts (`panel.set_choice`, AGM-08).
    Choice { stage: String, items: Vec<MenuItem> },
    /// The epic: opens its sheet.
    Open { key: String },
}

/// One item of the details line: "Assignee  CX Rust developer", "Branch  otaq/SHOP-22 from main".
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "SheetDetail")]
pub struct Detail {
    pub label: &'static str,
    pub value: String,
    /// The tool's avatar, with several tools connected (SHL-08).
    pub avatar: Option<ToolAvatarView>,
    /// The working dot before the value.
    pub working: bool,
    /// The value in the mono font (the branch).
    pub mono: bool,
    /// Muted text after the value: "from main".
    pub note: Option<String>,
    /// Clickable where the value may be changed.
    pub change: Option<DetailChange>,
}

impl Detail {
    fn new(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
            avatar: None,
            working: false,
            mono: false,
            note: None,
            change: None,
        }
    }
}

/// What the details line is made of.
pub struct DetailFacts<'a> {
    pub ticket: &'a Ticket,
    /// The ticket's agents (`ticket.agents`); `None` when they couldn't be read.
    pub agents: Option<&'a TicketAgents>,
    pub tools: &'a Tools,
    /// The epic of a child, by title.
    pub epic_title: Option<&'a str>,
    pub now: i64,
}

const DASH: &str = "\u{2014}";

/// "3 min", "1 h 5 min": how long, in whole minutes (at least one).
pub fn minutes_label(ms: i64) -> String {
    let minutes = (ms / 60_000).max(1);
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// "Codex · gpt-5.5", "Claude Code · tool default".
fn model_text(tool: otaq_core::agent_tools::Tool, model: Option<&str>) -> String {
    format!(
        "{} \u{b7} {}",
        tool.display_name(),
        model.filter(|m| !m.is_empty()).unwrap_or(TOOL_DEFAULT)
    )
}

/// The ticket's running session (the newest one running).
fn running(agents: &TicketAgents) -> Option<&SessionAgentRun> {
    agents
        .sessions
        .iter()
        .rev()
        .find(|s| s.running && s.role != SessionRole::ProjectAgent)
}

/// The stage whose agent the ticket has next: the one it is in or waits before; from Backlog the first it runs.
fn next_stage<'a>(ticket: &Ticket, path: &'a [PathStage]) -> Option<&'a PathStage> {
    let runs = |s: &&PathStage| s.agent.is_some() && !s.skipped;
    match &ticket.place {
        Place::InStage(id) | Place::Waiting(Some(id)) => path.iter().find(|s| &s.stage_id == id),
        Place::Backlog | Place::Waiting(None) => path.iter().find(runs),
        Place::Done => None,
    }
}

/// Assignee and Model: the agent on it now (its running session), you in your check, else the agent its next stage
/// runs (changeable until that stage starts), else the last one that ran.
fn assignee(f: &DetailFacts) -> (Detail, Detail) {
    let ticket = f.ticket;
    let empty = || (Detail::new("Assignee", DASH), Detail::new("Model", DASH));
    if ticket.status.set() == StatusSet::HumanCheck {
        return (Detail::new("Assignee", "You"), Detail::new("Model", DASH));
    }
    let Some(agents) = f.agents else {
        return empty();
    };
    if let Some(session) = running(agents) {
        let model = session.model_ran.as_deref().or(session.model.as_deref());
        return (
            Detail {
                avatar: session.tool.and_then(|t| f.tools.avatar(t, model)),
                ..Detail::new(
                    "Assignee",
                    session
                        .agent_name
                        .clone()
                        .unwrap_or_else(|| DASH.to_owned()),
                )
            },
            Detail::new(
                "Model",
                session
                    .tool
                    .map_or_else(|| DASH.to_owned(), |t| model_text(t, model)),
            ),
        );
    }
    let of_agent = |agent: &Agent, change: Option<DetailChange>| {
        let on = agent.model.on(f.tools.default_model.as_ref());
        (
            Detail {
                avatar: on
                    .as_ref()
                    .and_then(|m| f.tools.avatar(m.tool, m.model.as_deref())),
                change,
                ..Detail::new("Assignee", agent.name.clone())
            },
            Detail::new(
                "Model",
                on.map_or_else(
                    || DASH.to_owned(),
                    |m| model_text(m.tool, m.model.as_deref()),
                ),
            ),
        )
    };
    if let Some(stage) = next_stage(ticket, &agents.path)
        && stage.kind != PathKind::HumanCheck
        && let Some(agent) = &stage.agent
    {
        // Its agent may be changed until the stage starts (AGM-08): the stage's choice, as the Choices picker.
        let change = stage_choices::panel_choices(agents)
            .and_then(|c| c.rows.into_iter().find(|r| r.stage_id == stage.stage_id))
            .filter(|r| !r.items.is_empty() && r.lock.is_none())
            .map(|r| DetailChange::Choice {
                stage: r.stage_id,
                items: r.items,
            });
        return of_agent(agent, change);
    }
    if ticket.is_epic()
        && let Some(lead) = &agents.lead
    {
        let (mut who, model) = of_agent(lead, None);
        who.label = "Lead";
        return (who, model);
    }
    match agents.sessions.last() {
        Some(last) => {
            let model = last.model_ran.as_deref().or(last.model.as_deref());
            (
                Detail {
                    avatar: last.tool.and_then(|t| f.tools.avatar(t, model)),
                    ..Detail::new(
                        "Assignee",
                        last.agent_name.clone().unwrap_or_else(|| DASH.to_owned()),
                    )
                },
                Detail::new(
                    "Model",
                    last.tool
                        .map_or_else(|| DASH.to_owned(), |t| model_text(t, model)),
                ),
            )
        }
        None => empty(),
    }
}

/// Working: "3 min · 64k tokens" while its agent runs, "waiting for you · 12 min" in your check; nothing otherwise.
fn working(f: &DetailFacts) -> Option<Detail> {
    let ticket = f.ticket;
    if ticket.status.set() == StatusSet::HumanCheck {
        let since = ticket.status_since.unwrap_or(ticket.updated_at);
        return Some(Detail::new(
            "Working",
            format!("waiting for you \u{b7} {}", minutes_label(f.now - since)),
        ));
    }
    let session = f.agents.and_then(running)?;
    let time = minutes_label(f.now - session.started_at);
    let value = match ticket.flags.tokens {
        Some(tokens) => format!("{time} \u{b7} {} tokens", tokens_label(tokens)),
        None => time,
    };
    Some(Detail {
        working: true,
        ..Detail::new("Working", value)
    })
}

/// Branch: "otaq/SHOP-22 from main", "otaq/PAY-123/40.2 from the epic’s branch"; an epic's is shared by its children.
fn branch(ticket: &Ticket) -> Detail {
    if ticket.is_epic() {
        let name = ticket
            .work
            .as_ref()
            .and_then(|w| w.branch.clone())
            .unwrap_or_else(|| otaq_core::naming::branch(&ticket.key.to_string()));
        return Detail {
            mono: true,
            note: Some("shared".to_owned()),
            ..Detail::new("Branch", name)
        };
    }
    Detail {
        mono: true,
        note: Some(match &ticket.base {
            Base::Branch(b) => format!("from {b}"),
            Base::Epic(_) => "from the epic\u{2019}s branch".to_owned(),
        }),
        ..Detail::new("Branch", panel::branch_name(ticket).unwrap_or_default())
    }
}

/// The details line (SHT-01): Assignee, Model, Working (while it works or waits for you), Priority, Branch, and an
/// epic child's Epic.
pub fn details(f: &DetailFacts) -> Vec<Detail> {
    let ticket = f.ticket;
    let (who, model) = assignee(f);
    let priority = Detail {
        change: Some(DetailChange::Priority {
            items: priority::menu_items(ticket.priority),
        }),
        ..Detail::new("Priority", priority::view(ticket.priority).short)
    };
    let epic = ticket.epic.as_ref().map(|epic| Detail {
        change: Some(DetailChange::Open {
            key: epic.to_string(),
        }),
        ..Detail::new(
            "Epic",
            f.epic_title.map_or_else(|| epic.to_string(), str::to_owned),
        )
    });
    [who, model]
        .into_iter()
        .chain(working(f))
        .chain([priority, branch(ticket)])
        .chain(epic)
        .collect()
}

/// Where a stage of the flow path stands for the ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "SheetStepState")]
pub enum StepState {
    Done,
    /// The stage it is in now; `attention` when it needs you there.
    Current,
    ToCome,
    /// The ticket skips it (its choice, AGM-08).
    Skipped,
}

/// One stage of the flow path under the details line (`s25-03`: Grill ✓ — Dev ● — Validate — …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "SheetStep")]
pub struct Step {
    pub stage: String,
    pub state: StepState,
    /// It needs you in the stage it is in (the turquoise ring).
    pub attention: bool,
}

/// The flow path as steps: the stages before the one it is in done, that one current, the rest to come; from
/// Backlog every stage is to come, in Done every one is done. None for an epic or an unread flow.
pub fn steps(ticket: &Ticket, agents: Option<&TicketAgents>) -> Vec<Step> {
    let Some(agents) = agents.filter(|_| !ticket.is_epic()) else {
        return Vec::new();
    };
    let index = |id: &str| agents.path.iter().position(|s| s.stage_id == id);
    // Stages before `done_before` are done; `current` is the one it is in.
    let (done_before, current) = match &ticket.place {
        Place::InStage(id) => index(id).map_or((0, None), |i| (i, Some(i))),
        Place::Waiting(Some(id)) => (index(id).unwrap_or(0), None),
        Place::Done => (agents.path.len(), None),
        Place::Backlog | Place::Waiting(None) => (0, None),
    };
    agents
        .path
        .iter()
        .enumerate()
        .map(|(i, stage)| {
            let state = match () {
                _ if stage.skipped => StepState::Skipped,
                _ if Some(i) == current => StepState::Current,
                _ if i < done_before => StepState::Done,
                _ => StepState::ToCome,
            };
            Step {
                stage: stage.stage.clone(),
                state,
                attention: state == StepState::Current && ticket.shows_needs_you(),
            }
        })
        .collect()
}

/// **Move…** (SHT-01): the board's columns as the drag sees them, each with the drag's verdict: its own column
/// checked, an allowed one with what the drop does, a refused one disabled with why. A pick is a drop
/// (`board.drop`), with the same dialogs.
pub fn move_menu(columns: &Columns, answer: &DragAnswer) -> Vec<MenuItem> {
    let title = |target: &str| {
        columns
            .iter()
            .find_map(|c| {
                if c.id == target {
                    Some(c.title.clone())
                } else {
                    c.parts
                        .iter()
                        .skip(1)
                        .find(|p| c.has_sections() && p.id == target)
                        .map(|p| p.stage.name.clone())
                }
            })
            .unwrap_or_else(|| target.to_owned())
    };
    answer
        .targets
        .iter()
        .map(|t| {
            let item = MenuItem::new(t.target.0.to_string(), title(&t.target.0));
            match &t.verdict {
                Verdict::Home => item.checked(true).disabled(true),
                Verdict::Allowed { hint, .. } => item.hint(hint.clone()),
                Verdict::Forbidden { title, .. } => item.disabled(true).hint(title.clone()),
            }
        })
        .collect()
}

/// The board column id the ticket shows in (the drag's own column).
pub fn column_of(columns: &Columns, ticket: &Ticket) -> String {
    columns.column_of(ticket).id.clone()
}

/// The header's and the tab bar's words.
pub const MOVE: &str = "Move\u{2026}";
pub const FULL_PAGE: &str = "Full page";
pub const BACK_TO_SHEET: &str = "Back to the sheet";
/// At full page, the breadcrumb's first crumb goes back to the sheet over the board (`s25-10`).
pub const BOARD: &str = "Board";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crate::view_models::stage_choices::fixtures::{
        CLAUDE, CODEX, session, shop_options, shop_path, tools,
    };
    use otaq_core::domain::{Priority, Status};

    const MIN: i64 = 60_000;

    fn agents_of(t: &Ticket, sessions: Vec<SessionAgentRun>) -> TicketAgents {
        crate::view_models::stage_choices::fixtures::ticket(
            &t.key.to_string(),
            t.status,
            t.place.clone(),
            shop_path("rust"),
            shop_options(),
            Vec::new(),
            sessions,
        )
    }

    fn several_tools() -> Tools {
        let mut tools = tools();
        tools.connected = vec![CLAUDE, CODEX];
        tools
    }

    fn shown(details: &[Detail]) -> Vec<(&str, String, Option<&str>)> {
        details
            .iter()
            .map(|d| (d.label, d.value.clone(), d.note.as_deref()))
            .collect()
    }

    /// SHOP-22 "Fix flaky login redirect" asks to run a command in Dev, Rust developer on Codex · gpt-5.5 for 3
    /// minutes and 64k tokens (`s25-03`).
    fn shop_22() -> Ticket {
        let mut t = ticket("SHOP-22", "Fix flaky login redirect", Status::NeedsYou);
        t.place = Place::InStage("dev".into());
        t.priority = Priority::High;
        t.flags.tokens = Some(64_000);
        t.work = Some(otaq_core::board::tickets::Work {
            branch: Some(otaq_core::naming::branch("SHOP-22")),
            ..Default::default()
        });
        t
    }

    #[test]
    fn the_header_and_details_line_for_shop_22() {
        let t = shop_22();
        let mut dev = session(
            "s1",
            SessionRole::Developer,
            "Rust developer",
            CODEX,
            None,
            Some("gpt-5.5"),
        );
        dev.running = true;
        dev.started_at = 100 * MIN;
        dev.ended_at = None;
        let agents = agents_of(&t, vec![dev]);
        let several = several_tools();
        let details = details(&DetailFacts {
            ticket: &t,
            agents: Some(&agents),
            tools: &several,
            epic_title: None,
            now: 103 * MIN + 20_000,
        });
        assert_eq!(
            shown(&details),
            [
                ("Assignee", "Rust developer".into(), None),
                ("Model", "Codex \u{b7} gpt-5.5".into(), None),
                ("Working", "3 min \u{b7} 64k tokens".into(), None),
                ("Priority", "P2".into(), None),
                (
                    "Branch",
                    otaq_core::naming::branch("SHOP-22"),
                    Some("from main")
                ),
            ]
        );
        assert_eq!(
            details[0].avatar.as_ref().map(|a| a.letters.as_str()),
            Some("CX")
        );
        assert!(details[2].working);
        assert!(details[4].mono);
        assert!(matches!(
            details[3].change,
            Some(DetailChange::Priority { .. })
        ));
        assert_eq!(details[0].change, None, "Dev has started");
        let crumbs = breadcrumb(&t, "shop-web", None);
        let texts: Vec<_> = crumbs.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["shop-web", "SHOP-22"]);
        assert!(crumbs[1].key && !crumbs[0].key);
        // One tool connected: no avatar (SHL-08).
        let one = tools();
        let plain = super::details(&DetailFacts {
            ticket: &t,
            agents: Some(&agents),
            tools: &one,
            epic_title: None,
            now: 103 * MIN,
        });
        assert_eq!(plain[0].avatar, None);
    }

    /// A child of the Wishlist epic: the epic in the breadcrumb (key shortened) and its Epic detail linking back,
    /// the branch from the epic's.
    #[test]
    fn a_child_of_an_epic() {
        let mut t = ticket("SHOP-40.2", "Wishlist button", Status::Working);
        t.epic = Some("SHOP-40".parse().unwrap());
        t.base = Base::Epic("SHOP-40".parse().unwrap());
        t.place = Place::InStage("dev".into());
        let crumbs = breadcrumb(&t, "shop-web", Some("Wishlist"));
        let texts: Vec<_> = crumbs.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["shop-web", "Wishlist", "40.2"]);
        assert_eq!(crumbs[1].opens.as_deref(), Some("SHOP-40"));
        let tools = tools();
        let details = details(&DetailFacts {
            ticket: &t,
            agents: None,
            tools: &tools,
            epic_title: Some("Wishlist"),
            now: 0,
        });
        let epic = details.last().unwrap();
        assert_eq!((epic.label, epic.value.as_str()), ("Epic", "Wishlist"));
        assert_eq!(
            epic.change,
            Some(DetailChange::Open {
                key: "SHOP-40".into()
            })
        );
        let branch = details.iter().find(|d| d.label == "Branch").unwrap();
        assert_eq!(
            branch.note.as_deref(),
            Some("from the epic\u{2019}s branch")
        );
        assert_eq!(details[0].value, DASH, "agents unread");
    }

    /// A ticket in Backlog: its first stage's agent, changeable before it starts; no working time, no branch yet.
    #[test]
    fn a_ticket_in_backlog() {
        let t = ticket("SHOP-41", "Fix README typo", Status::Backlog);
        let agents = agents_of(&t, Vec::new());
        let tools = tools();
        let details = details(&DetailFacts {
            ticket: &t,
            agents: Some(&agents),
            tools: &tools,
            epic_title: None,
            now: 0,
        });
        assert_eq!(
            shown(&details),
            [
                ("Assignee", "Griller".into(), None),
                ("Model", "Claude Code \u{b7} sonnet".into(), None),
                ("Priority", "P3".into(), None),
                ("Branch", String::new(), Some("from main")),
            ]
        );
        let Some(DetailChange::Choice { stage, items }) = &details[0].change else {
            panic!("Grill's agent can change: {:?}", details[0].change);
        };
        assert_eq!(stage, "grill");
        assert!(items.iter().any(|i| i.checked));
    }

    /// In your check (`s25-09`): Assignee You, no model, and how long it waits for you.
    #[test]
    fn in_your_check() {
        let mut t = ticket("SHOP-18", "Checkout API", Status::WaitingForYou);
        t.status_since = Some(10 * MIN);
        let tools = tools();
        let details = details(&DetailFacts {
            ticket: &t,
            agents: None,
            tools: &tools,
            epic_title: None,
            now: 22 * MIN,
        });
        assert_eq!(
            &shown(&details)[..3],
            [
                ("Assignee", "You".into(), None),
                ("Model", DASH.into(), None),
                ("Working", "waiting for you \u{b7} 12 min".into(), None),
            ]
        );
    }

    /// SHOP-22's path (`s25-03`): Grill done, Dev current and needing you, the rest to come.
    #[test]
    fn the_flow_path_as_steps() {
        let t = shop_22();
        let agents = agents_of(&t, Vec::new());
        let steps = steps(&t, Some(&agents));
        let shown: Vec<(&str, StepState, bool)> = steps
            .iter()
            .map(|s| (s.stage.as_str(), s.state, s.attention))
            .collect();
        assert_eq!(
            shown,
            [
                ("Grill", StepState::Done, false),
                ("Dev", StepState::Current, true),
                ("Validate", StepState::ToCome, false),
                ("Security", StepState::ToCome, false),
                ("Review", StepState::ToCome, false),
                ("Done", StepState::ToCome, false),
            ]
        );
        let backlog = ticket("SHOP-41", "Fix README typo", Status::Backlog);
        let agents = agents_of(&backlog, Vec::new());
        assert!(
            super::steps(&backlog, Some(&agents))
                .iter()
                .all(|s| s.state == StepState::ToCome)
        );
        assert!(super::steps(&backlog, None).is_empty());
    }

    #[test]
    fn minutes() {
        assert_eq!(minutes_label(10_000), "1 min");
        assert_eq!(minutes_label(3 * MIN + 59_000), "3 min");
        assert_eq!(minutes_label(60 * MIN), "1 h");
        assert_eq!(minutes_label(65 * MIN), "1 h 5 min");
    }

    /// Tabs: a ticket's four, numbered 1–4; an epic's Overview · Lead · Activity with Files in review; Overview on
    /// open; today's panel tab each one shows.
    #[test]
    fn tabs_their_numbers_and_the_tab_on_open() {
        let t = ticket("SHOP-22", "Fix flaky login redirect", Status::Working);
        let shown = tabs(&t);
        assert_eq!(
            shown,
            [
                SheetTab::Overview,
                SheetTab::Session,
                SheetTab::Files,
                SheetTab::Activity
            ]
        );
        let numbers: Vec<u8> = shown.iter().map(|t| t.number()).collect();
        assert_eq!(numbers, [1, 2, 3, 4]);
        assert_eq!(numbered(&shown, 4), Some(SheetTab::Activity));
        assert_eq!(numbered(&shown, 5), None);
        assert_eq!(opens_on(&t), SheetTab::Overview);
        // In your check it opens on Files, where All changes and the review bar are (SHT-06).
        assert_eq!(
            opens_on(&ticket("SHOP-18", "Checkout API", Status::WaitingForYou)),
            SheetTab::Files
        );
        let labels: Vec<_> = shown.iter().map(|t| t.label()).collect();
        assert_eq!(labels, ["Overview", "Session", "Files", "Activity"]);
        assert_eq!(
            SheetTab::Session.contents(),
            [Tab::Terminal, Tab::Run, Tab::Shell]
        );
        assert_eq!(SheetTab::Files.contents(), [Tab::Files]);
        assert_eq!(part_label(Tab::Terminal), "Agent");
        assert_eq!(part_label(Tab::Shell), "Shell");

        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = otaq_core::domain::Kind::Epic;
        assert_eq!(
            tabs(&epic),
            [SheetTab::Overview, SheetTab::Lead, SheetTab::Activity]
        );
        assert_eq!(numbered(&tabs(&epic), 2), Some(SheetTab::Lead));
        epic.status = Status::WaitingForYou;
        epic.place = Place::of(epic.status, None, None);
        assert_eq!(
            tabs(&epic),
            [
                SheetTab::Overview,
                SheetTab::Lead,
                SheetTab::Files,
                SheetTab::Activity
            ]
        );
    }

    /// j / k follow the column's order (pinned first), as the panel's did (`panel::neighbour`).
    #[test]
    fn j_and_k_follow_the_column_order() {
        let mut a = ticket("SHOP-1", "a", Status::Backlog);
        a.created_at = 1;
        let mut b = ticket("SHOP-2", "b", Status::Backlog);
        b.created_at = 2;
        b.flags.pinned_at = Some(10);
        let tickets = vec![a, b];
        let key = |s: &str| s.parse::<otaq_core::domain::TicketKey>().unwrap();
        assert_eq!(
            panel::neighbour(&tickets, &key("SHOP-2"), 1),
            Some(key("SHOP-1"))
        );
        assert_eq!(
            panel::neighbour(&tickets, &key("SHOP-1"), -1),
            Some(key("SHOP-2"))
        );
    }

    #[test]
    fn widths() {
        assert_eq!(width(false, 1376.), WIDTH);
        assert_eq!(width(true, 956.), WIDTH_BESIDE_AGENT);
        assert_eq!(
            width(true, 896.),
            896. - 36.,
            "a narrow main area narrows it"
        );
    }

    /// Move… lists the drag's targets with their verdicts.
    #[test]
    fn move_lists_the_columns_with_the_drags_verdicts() {
        let flow = &otaq_core::flow::default_file().flow;
        let columns = Columns::of(flow);
        let t = ticket("SHOP-41", "Fix README typo", Status::Backlog);
        let answer = crate::view_models::board_page::drag_answer(&t, &[]);
        let menu = move_menu(&columns, &answer);
        let labels: Vec<_> = menu.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels.first(), Some(&"Backlog"));
        assert!(labels.contains(&"Ready for Dev"));
        assert!(menu[0].checked && menu[0].disabled, "its own column");
        let ready = menu.iter().find(|i| i.id == "ready_for_dev").unwrap();
        assert!(!ready.disabled);
        assert_eq!(column_of(&columns, &t), "backlog");
    }
}
