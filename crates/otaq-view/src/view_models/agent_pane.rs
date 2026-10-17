//! The Project agent pane (T-0326, T-0342; SHL-04, PAG-01, PAG-05, PAG-06, PAG-07, TKT-08; d0087, d0086, d0068,
//! d0060, d0077, d0018;
//! *18 · Project agents & terminals → Row · Pane: empty, hidden, tool problems, menu (0087)*, *Row · More pane
//! states (0086 · V)*, *Row · Switching agents, attention (0086 · V)*): everything the pane between the rail and the
//! main area draws, for one project and the agent it shows.
//!
//! The pane shows one Project agent, or the **empty pane** "Ask the Project agent". The empty pane is the page's own
//! state, never a session or a row: no agent exists until the first message is sent, and that message is the
//! go-ahead for its tokens (d0087). A running agent has its header (switcher, state pill, Recently ended, +, …), the
//! agent line, Created and changed, its terminal and the footer. The page only draws; which agent it shows is page
//! state, and [`PaneView::opens_on`] says where a project opens the first time.
//!
//! The agent line names the named agent (M22): on the empty pane the one the Project agent role resolves to for the
//! project, which the first message starts ("Strong · starts when you send"); on a running agent the one its
//! session started on ("Strong · 52k"). The tool follows the name only once two tools are connected and differs
//! from it ("Standard · Codex"); with one tool the line is the name alone.

use otaq_core::agent_tools::Tool;
use otaq_core::domain::{Status, TicketKey};
use otaq_core::named_agents::NamedAgent;
use otaq_core::project_agents::{AgentState, CreatedAndChanged, ProjectAgent, ProjectAgentList};
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::keymap::{KeyAction, Platform};
use crate::view_models::card::tokens_label;
use crate::view_models::menu::MenuItem;
use crate::view_models::project_agent_page::{
    self as agent_page, Recent, ResumeButton, column_name,
};
use crate::view_models::tool_avatar::{self, AvatarTool, ToolAvatarView};

/// A Project agent waits for you (a permission prompt or a question) while its terminal doesn't have your focus.
pub fn agent_waits(agent: &ProjectAgent) -> bool {
    agent.state == AgentState::Running
        && (agent.question.is_some() || agent.needs_you.is_some())
        && !agent.focused
}

/// The empty pane's header.
pub const TITLE: &str = "Project agent";

/// The example prompts: a fixed generic set, the same for every project (d0087, Q-E). A click fills the box and
/// never sends.
pub const EXAMPLES: [&str; 4] = [
    "Turn this into tickets: \u{2026}",
    "Why is a ticket stuck?",
    "Plan an epic for \u{2026}",
    "Import issues from our tracker",
];

/// Created and changed is folded to one line below this window width: the 1280 size class (Q-C splits the classes
/// at 1360 and 1680; T-0327's `layout.rs` holds the split, and this follows it).
pub const FOLD_BELOW_WIDTH: f32 = 1360.;

/// The … menu's items, as the page sends them back.
pub const RENAME: &str = "rename";
pub const END: &str = "end";
pub const CLOSE_ENDED: &str = "close_ended";
pub const RECENT: &str = "recent";

/// A header button: its icon, its tooltip and its keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneButton")]
pub struct Button {
    pub icon: IconName,
    pub label: &'static str,
    /// "⇧⌘A".
    pub keys: Option<String>,
}

/// The pane's header: the bot icon, the switcher (the name ▾), the state pill, Recently ended, + and ….
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PaneHeader")]
pub struct Header {
    pub icon: IconName,
    /// The shown agent's name, or "Project agent" on the empty pane.
    pub title: String,
    /// "New agent" until it is named reads muted.
    pub unnamed: bool,
    /// The name opens the switcher (▾): the project has a Project agent to switch to.
    pub switcher: bool,
    /// The shown agent's state; none on the empty pane.
    pub pill: Option<crate::view_models::status::PillView>,
    pub recent: Button,
    /// + shows the empty pane (as ⇧⌘A does); nothing starts.
    pub new: Button,
    pub more: Button,
}

/// The line under the header: the tool avatar (hidden while one tool is connected), the named agent, then the tokens
/// or "· starts when you send", and Rename on a running agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneAgentLine")]
pub struct AgentLine {
    pub avatar: Option<ToolAvatarView>,
    /// The named agent: "Strong", "Claude Code" (the starter entry); "Standard · Codex" once two tools are
    /// connected and the tool isn't the name.
    pub agent: String,
    /// "· 52k", "· starts when you send".
    pub note: String,
    pub rename: Option<&'static str>,
}

fn avatar(tool: Tool, model: Option<&str>) -> Option<ToolAvatarView> {
    tool_avatar::shown().then(|| ToolAvatarView::of(tool, model))
}

/// The agent line's name: the named agent, with its tool after it only while two or more tools are connected and
/// the tool isn't the name (0086: no tool name while one tool is connected or when it equals the agent's name).
pub fn agent_name(name: &str, tool: Tool) -> String {
    let tool_name = AvatarTool::from(tool).name();
    if tool_avatar::shown() && !name.eq_ignore_ascii_case(tool_name) {
        format!("{name} \u{b7} {tool_name}")
    } else {
        name.to_owned()
    }
}

/// What a new Project agent starts on: the named agent the role resolves to for the project, else (no named agents
/// yet) the new-session tool under its own name.
fn starts_on(new_agent: Option<&NamedAgent>) -> (String, Tool, Option<&str>) {
    match new_agent {
        Some(agent) => (agent.name.clone(), agent.tool, agent.model.as_deref()),
        None => {
            let tool = new_agent_tool();
            (AvatarTool::from(tool).name().to_owned(), tool, None)
        }
    }
}

/// The name of what a new Project agent starts on ([`starts_on`]): "Strong", or "Claude Code" before any named agent
/// exists. Ask the agent's tooltip names it too.
pub fn new_agent_name(new_agent: Option<&NamedAgent>) -> String {
    starts_on(new_agent).0
}

/// A Project agent's named agent, as its session saved it at start (T-0336); the tool's name for a session from
/// before named agents.
fn ran_on(agent: &ProjectAgent) -> String {
    match agent.session.as_ref().and_then(|s| s.named_agent.as_ref()) {
        Some(named) => agent_name(&named.name, agent.tool),
        None => AvatarTool::from(agent.tool).name().to_owned(),
    }
}

/// An example prompt: its label, and what a click puts in the box (an ending "…" is left for the user to write).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneExample")]
pub struct Example {
    pub label: &'static str,
    pub fill: &'static str,
}

fn example(label: &'static str) -> Example {
    Example {
        label,
        fill: label.strip_suffix('\u{2026}').unwrap_or(label),
    }
}

/// The empty pane, "Ask the Project agent" (*No Project agent yet (0087)*, `SbFln`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneEmpty")]
pub struct Empty {
    pub line: AgentLine,
    pub title: &'static str,
    pub text: String,
    /// Each fills the box; none sends.
    pub examples: Vec<Example>,
    /// Above [`PaneView::recent`]'s rows ("name · yesterday · Resume"); hidden with none.
    pub recent_title: &'static str,
    pub placeholder: String,
    /// "Enter starts Strong" (with the tool once two are connected: "Enter starts Standard · Codex").
    pub hint: String,
    pub send: &'static str,
}

/// The tool a new agent starts on while there are no named agents (before the starter entry exists): the first.
fn new_agent_tool() -> Tool {
    Tool::ALL[0]
}

/// The empty pane, naming what the first message starts: `new_agent`, the Project agent role's named agent for the
/// project (`None` before any named agent exists).
pub fn empty(project_name: &str, new_agent: Option<&NamedAgent>) -> Empty {
    let (name, tool, model) = starts_on(new_agent);
    // "Strong"; "Standard · Codex" once two tools are connected, in the line and the hint alike (`SbFln`).
    let name = agent_name(&name, tool);
    Empty {
        line: AgentLine {
            avatar: avatar(tool, model),
            agent: name.clone(),
            note: "\u{b7} starts when you send".to_owned(),
            rename: None,
        },
        title: "Ask the Project agent",
        text: format!(
            "It reads and runs code in {project_name} (never edits) and creates or changes Backlog tickets. Nothing \
             runs and no tokens are used until you send."
        ),
        examples: EXAMPLES.into_iter().map(example).collect(),
        recent_title: "RECENTLY ENDED",
        placeholder: format!("Ask about {project_name}, or describe work\u{2026}"),
        hint: format!("Enter starts {name}"),
        send: "Send",
    }
}

/// A ticket the agent edited, with what the row shows of it (the service lists only its key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditedTicket {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
}

/// One row of Created and changed: the key (a link that opens the panel), the title, where it is, and Delete on a
/// created ticket that never went to Dev (TKT-08, the 0060 confirmation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneCreatedRow")]
pub struct CreatedRow {
    pub key: String,
    pub title: String,
    /// "Ready for Dev", "Backlog".
    pub place: &'static str,
    /// Created (square-plus) or changed (pencil).
    pub icon: IconName,
    pub edited: bool,
    pub delete: Option<&'static str>,
}

/// Show on board (Q-G, d0087): the board shows only the tickets this agent created or changed, with the chip
/// "Created or changed by <agent> ✕"; ✕ or Esc clears it. The page passes `agent` as `board.view`'s `agent`; the
/// board makes the chip (`board_page::AgentFilter`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneShowOnBoard")]
pub struct ShowOnBoard {
    pub label: &'static str,
    /// The Project agent the filter is by.
    pub agent: String,
}

/// Created and changed at the top of the pane (collapsible; one line at the 1280 class).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneCreated")]
pub struct Created {
    /// "CREATED 2 · CHANGED 1".
    pub line: String,
    /// Whether it opens folded to that one line; the page keeps the user's own toggle.
    pub folded: bool,
    pub show_on_board: ShowOnBoard,
    pub rows: Vec<CreatedRow>,
}

/// `None` while the agent created and changed nothing.
pub fn created(
    agent: &ProjectAgent,
    c: &CreatedAndChanged,
    edited: &[EditedTicket],
    window_width: Option<f32>,
) -> Option<Created> {
    if c.created.is_empty() && c.edited.is_empty() {
        return None;
    }
    let mut rows: Vec<CreatedRow> = c
        .created
        .iter()
        .map(|t| CreatedRow {
            key: t.key.to_string(),
            title: t.title.clone(),
            place: column_name(t.status),
            icon: IconName::SquarePlus,
            edited: false,
            delete: t.deletable.then_some("Delete"),
        })
        .collect();
    rows.extend(c.edited.iter().map(|key| {
        let known = edited.iter().find(|t| t.key == *key);
        CreatedRow {
            key: key.to_string(),
            title: known.map_or_else(String::new, |t| t.title.clone()),
            place: known.map_or("", |t| column_name(t.status)),
            icon: IconName::Pencil,
            edited: true,
            delete: None,
        }
    }));
    Some(Created {
        line: format!(
            "CREATED {} \u{b7} CHANGED {}",
            c.created.len(),
            c.edited.len()
        ),
        folded: window_width.is_some_and(|w| w < FOLD_BELOW_WIDTH),
        show_on_board: ShowOnBoard {
            label: "Show on board",
            agent: agent.id.clone(),
        },
        rows,
    })
}

/// The line under the terminal: where typing goes, or how it ended with Resume (the click is the go-ahead, d0018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneFooter")]
pub struct Footer {
    pub text: &'static str,
    /// "⌘L".
    pub keys: Option<String>,
    pub keys_label: &'static str,
    pub resume: Option<ResumeButton>,
    /// After Resume: "or ⇧⌘A for a new agent".
    pub resume_note: Option<String>,
}

pub fn footer(agent: &ProjectAgent, platform: Platform) -> Footer {
    let (text, resume_note) = match agent.state {
        AgentState::Running => ("Typing goes to the agent", None),
        AgentState::Ended => (
            "Ended \u{b7} Resume continues this conversation",
            Some(match KeyAction::NewProjectAgent.keys(platform) {
                Some(keys) => format!("or {keys} for a new agent"),
                None => "or + for a new agent".to_owned(),
            }),
        ),
        AgentState::EndedByRestart => (
            "Ended by restart \u{b7} Resume continues it",
            Some("continues the same conversation".to_owned()),
        ),
    };
    Footer {
        text,
        keys: KeyAction::FocusPane.keys(platform),
        keys_label: "focus",
        resume: agent_page::resume_button(agent),
        resume_note,
    }
}

/// The shown agent: its line, Created and changed, its terminal and the footer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneAgent")]
pub struct Running {
    pub line: AgentLine,
    pub created: Option<Created>,
    /// The session its terminal attaches to (`terminal.attach`); none when it is gone.
    pub session: Option<String>,
    /// It ended: the terminal keeps its scrollback, read-only.
    pub ended: bool,
    pub footer: Footer,
}

/// One row of the switcher: a Project agent of the project (not an epic lead), with its state.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PaneSwitcherRow")]
pub struct SwitcherRow {
    pub id: String,
    pub name: String,
    pub unnamed: bool,
    pub pill: crate::view_models::status::PillView,
    /// Hidden while one tool is connected.
    pub avatar: Option<ToolAvatarView>,
    pub selected: bool,
}

/// The switcher (`h038v`, `b8kIUd`): the project's open Project agents, newest first, then the empty pane.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PaneSwitcher")]
pub struct Switcher {
    pub rows: Vec<SwitcherRow>,
    pub new_label: &'static str,
    pub new_keys: Option<String>,
}

/// Newest first; closed agents aren't in `open`, so they aren't listed.
pub fn switcher(agents: &[ProjectAgent], shown: Option<&str>, platform: Platform) -> Switcher {
    let mut newest: Vec<&ProjectAgent> = agents.iter().collect();
    newest.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    Switcher {
        rows: newest
            .into_iter()
            .map(|a| SwitcherRow {
                id: a.id.clone(),
                name: a.name.clone(),
                unnamed: a.name_source.is_none(),
                pill: agent_page::state_pill(a),
                avatar: avatar(a.tool, a.model.as_deref()),
                selected: shown == Some(a.id.as_str()),
            })
            .collect(),
        new_label: "New Project agent (empty)",
        new_keys: KeyAction::NewProjectAgent.keys(platform),
    }
}

/// The … menu (`Ir8TU`): Rename · End agent (asks first mid-turn) · Close ended agents (disabled with none) ·
/// Recently ended….
pub fn menu(shown: Option<&ProjectAgent>, agents: &[ProjectAgent]) -> Vec<MenuItem> {
    let any_ended = agents.iter().any(|a| a.state != AgentState::Running);
    vec![
        MenuItem::new(RENAME, "Rename")
            .icon(IconName::Pencil)
            .disabled(shown.is_none()),
        MenuItem::new(END, "End agent")
            .icon(IconName::CircleStop)
            .hint("asks first if mid-turn")
            .disabled(!shown.is_some_and(|a| a.state == AgentState::Running)),
        MenuItem::new(CLOSE_ENDED, "Close ended agents")
            .icon(IconName::X)
            .hint("ended agents only")
            .disabled(!any_ended),
        MenuItem::new(RECENT, "Recently ended\u{2026}")
            .icon(IconName::History)
            .separated(),
    ]
}

/// Where a project's pane opens when the page has no choice for it yet: its newest running agent, else the empty
/// pane.
pub fn opens_on(agents: &[ProjectAgent]) -> Option<String> {
    agents
        .iter()
        .filter(|a| a.state == AgentState::Running)
        .max_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)))
        .map(|a| a.id.clone())
}

/// Everything `pane.view` answers. `PaneView` on the page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PaneView")]
pub struct PaneView {
    /// The agent shown; `None` for the empty pane (also when the asked one was closed).
    pub agent: Option<String>,
    pub opens_on: Option<String>,
    pub header: Header,
    /// The empty pane, when no agent is shown.
    pub empty: Option<Empty>,
    /// The shown agent.
    pub running: Option<Running>,
    pub switcher: Switcher,
    pub menu: Vec<MenuItem>,
    /// Recently ended (the history icon, the … menu, and the empty pane's list), each with Resume.
    pub recent: Recent,
    /// A Project agent of the project waits for you: the hidden strip's turquoise dot (d0077).
    pub agent_waits: bool,
    /// The hidden strip: a click or ⌘\ shows the pane.
    pub strip: Button,
}

/// What [`view`] reads.
pub struct Facts<'a> {
    pub agents: &'a ProjectAgentList,
    /// The agent the page shows, if any.
    pub shown: Option<&'a str>,
    /// The shown agent's Created and changed, and the tickets it edited.
    pub created: Option<&'a CreatedAndChanged>,
    pub edited: &'a [EditedTicket],
    pub project_name: &'a str,
    /// The named agent the Project agent role resolves to for the project (`named_agents::resolve`), which the
    /// empty pane's first message starts; `None` before any named agent exists.
    pub new_agent: Option<&'a NamedAgent>,
    pub window_width: Option<f32>,
    pub now_ms: i64,
    pub platform: Platform,
}

pub fn view(f: &Facts) -> PaneView {
    let open = &f.agents.open;
    let shown = f.shown.and_then(|id| open.iter().find(|a| a.id == id));
    let header = Header {
        icon: IconName::Bot,
        title: shown.map_or_else(|| TITLE.to_owned(), |a| a.name.clone()),
        unnamed: shown.is_some_and(|a| a.name_source.is_none()),
        switcher: !open.is_empty(),
        pill: shown.map(agent_page::state_pill),
        recent: Button {
            icon: IconName::History,
            label: "Recently ended",
            keys: None,
        },
        new: Button {
            icon: IconName::Plus,
            label: "New Project agent",
            keys: KeyAction::NewProjectAgent.keys(f.platform),
        },
        more: Button {
            icon: IconName::Ellipsis,
            label: "More",
            keys: None,
        },
    };
    let running = shown.map(|a| Running {
        line: AgentLine {
            avatar: avatar(a.tool, a.model.as_deref()),
            agent: ran_on(a),
            note: format!("\u{b7} {}", tokens_label(a.tokens.shown())),
            rename: Some("Rename"),
        },
        created: f
            .created
            .and_then(|c| created(a, c, f.edited, f.window_width)),
        session: a.session.as_ref().map(|s| s.id.clone()),
        ended: a.state != AgentState::Running,
        footer: footer(a, f.platform),
    });
    PaneView {
        agent: shown.map(|a| a.id.clone()),
        opens_on: opens_on(open),
        header,
        empty: shown.is_none().then(|| empty(f.project_name, f.new_agent)),
        running,
        switcher: switcher(open, shown.map(|a| a.id.as_str()), f.platform),
        menu: menu(shown, open),
        recent: agent_page::recent(&f.agents.recent, f.project_name, f.now_ms),
        agent_waits: open.iter().any(agent_waits),
        strip: Button {
            icon: IconName::Bot,
            label: "Show the Project agent",
            keys: KeyAction::TogglePane.keys(f.platform),
        },
    }
}

/// One pane state in the gallery: its name and what `pane.view` answers there.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PaneGalleryState")]
pub struct GalleryState {
    pub name: &'static str,
    pub view: PaneView,
}

/// The gallery's pane section (T-0330, T-0342; *18 → Row · Pane: empty, hidden, tool problems, menu (0087)*, *Row ·
/// More pane states (0086 · V)*, *Row · Switching agents, attention (0086 · V)*): the empty pane on the starter
/// entry and on a project's own Project agent (Strong), a running agent on Strong, one waiting, one ended, one ended
/// by restart, at the 1440 class; the switcher and the … menu are the running view's.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PaneGallery")]
pub struct Gallery {
    pub states: Vec<GalleryState>,
}

pub fn gallery() -> Gallery {
    use otaq_core::project_agents::{CreatedTicket, NameSource};

    const NOW: i64 = 1_791_600_000_000;
    let agent = |id: &str, name: &str, at: i64| ProjectAgent {
        id: id.into(),
        project: "NOTE".into(),
        name: name.into(),
        name_source: Some(NameSource::Agent),
        state: AgentState::Running,
        tool: Tool::ALL[0],
        tool_session_id: None,
        model: None,
        got_request: true,
        mid_turn: true,
        limit_stopped: false,
        created_at: at,
        ended_at: None,
        closed: false,
        question: None,
        needs_you: None,
        focused: false,
        tokens: otaq_core::usage::Tokens {
            input: 44_000,
            output: 8_000,
            ..Default::default()
        },
        session: None,
        epic: None,
    };
    let mut working = agent("a1", "Search slowness", 3);
    working.session = Some(session_on("Strong"));
    let mut asks = agent("a2", "Import GitHub issues", 2);
    asks.needs_you = Some("permission: npm run bench".into());
    let mut idle = agent("a3", "Sync conflicts", 1);
    idle.mid_turn = false;
    let mut ended = agent("a4", "Footer audit", 0);
    ended.state = AgentState::Ended;
    ended.mid_turn = false;
    ended.ended_at = Some(NOW - 3_600_000);
    let mut restarted = agent("a5", "Order history", 0);
    restarted.state = AgentState::EndedByRestart;
    restarted.ended_at = Some(NOW - 86_400_000);
    let mut gone = agent("a6", "Tag cleanup", 0);
    gone.state = AgentState::Ended;
    gone.ended_at = Some(NOW - 2 * 86_400_000);
    let agents = ProjectAgentList {
        open: vec![working, asks, idle, ended, restarted],
        recent: vec![gone],
    };
    let changed = CreatedAndChanged {
        created: vec![
            CreatedTicket {
                key: "NOTE-11".parse().expect("a key"),
                title: "Slow search query".into(),
                status: Status::Queued,
                deletable: false,
            },
            CreatedTicket {
                key: "NOTE-12".parse().expect("a key"),
                title: "Cache note previews".into(),
                status: Status::Backlog,
                deletable: true,
            },
        ],
        edited: vec!["NOTE-5".parse().expect("a key")],
        dependencies: 1,
        summary: String::new(),
    };
    let edited = [EditedTicket {
        key: "NOTE-5".parse().expect("a key"),
        title: "Sync conflicts".into(),
        status: Status::Backlog,
    }];
    let strong = sample_agent("Strong", Some("opus"));
    let state = |name: &'static str, shown: Option<&str>, width: f32| GalleryState {
        name,
        view: view(&Facts {
            agents: &agents,
            shown,
            created: shown.is_some().then_some(&changed),
            edited: &edited,
            project_name: "pocket-notes",
            new_agent: None,
            window_width: Some(width),
            now_ms: NOW,
            platform: Platform::Mac,
        }),
    };
    Gallery {
        states: vec![
            state("Empty · Ask the Project agent", None, 1440.),
            GalleryState {
                name: "Empty · the project's Project agent is Strong",
                view: view(&Facts {
                    agents: &agents,
                    shown: None,
                    created: None,
                    edited: &edited,
                    project_name: "pocket-notes",
                    new_agent: Some(&strong),
                    window_width: Some(1440.),
                    now_ms: NOW,
                    platform: Platform::Mac,
                }),
            },
            state(
                "Running · Created and changed open (1440)",
                Some("a1"),
                1440.,
            ),
            state("Running · folded to one line (1280)", Some("a1"), 1280.),
            state("Waiting for you", Some("a2"), 1440.),
            state("Ended", Some("a4"), 1440.),
            state("Ended by restart", Some("a5"), 1440.),
        ],
    }
}

/// A named agent for the gallery and tests.
fn sample_agent(name: &str, model: Option<&str>) -> NamedAgent {
    NamedAgent {
        id: name.to_lowercase(),
        name: name.to_owned(),
        tool: Tool::ALL[0],
        model: model.map(str::to_owned),
        use_for: String::new(),
        fallback: None,
        mark: None,
        created_at: 0,
        updated_at: 0,
    }
}

/// A Project agent's session started on the named agent `name`, for the gallery and tests.
fn session_on(name: &str) -> otaq_core::session::SessionInfo {
    otaq_core::session::SessionInfo {
        id: "s1".into(),
        kind: otaq_core::session::SessionKind::ProjectAgent,
        project: "NOTE".into(),
        ticket: None,
        state: otaq_core::session::SessionState::Running,
        started_at: 0,
        ended_at: None,
        end_reason: None,
        tool: Some(Tool::ALL[0]),
        tool_session_id: None,
        model: None,
        named_agent: Some(otaq_core::session::SessionAgent {
            id: name.to_lowercase(),
            name: name.to_owned(),
        }),
        model_ran: None,
        size: None,
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::project_agents::{CreatedTicket, NameSource};

    use super::*;
    use crate::theme::Palette;

    fn agent(id: &str, name: &str, created_at: i64) -> ProjectAgent {
        ProjectAgent {
            id: id.into(),
            project: "NOTE".into(),
            name: name.into(),
            name_source: Some(NameSource::Agent),
            state: AgentState::Running,
            tool: Tool::ALL[0],
            tool_session_id: Some("s".into()),
            model: None,
            got_request: true,
            mid_turn: true,
            limit_stopped: false,
            created_at,
            ended_at: None,
            closed: false,
            question: None,
            needs_you: None,
            focused: false,
            tokens: otaq_core::usage::Tokens {
                input: 44_000,
                output: 8_000,
                ..Default::default()
            },
            session: None,
            epic: None,
        }
    }

    fn list(open: Vec<ProjectAgent>) -> ProjectAgentList {
        ProjectAgentList {
            open,
            recent: Vec::new(),
        }
    }

    fn changed() -> CreatedAndChanged {
        CreatedAndChanged {
            created: vec![
                CreatedTicket {
                    key: "NOTE-11".parse().unwrap(),
                    title: "Slow search query".into(),
                    status: Status::Queued,
                    deletable: false,
                },
                CreatedTicket {
                    key: "NOTE-12".parse().unwrap(),
                    title: "Cache note previews".into(),
                    status: Status::Backlog,
                    deletable: true,
                },
            ],
            edited: vec!["NOTE-5".parse().unwrap()],
            dependencies: 1,
            summary: String::new(),
        }
    }

    fn edited() -> Vec<EditedTicket> {
        vec![EditedTicket {
            key: "NOTE-5".parse().unwrap(),
            title: "Sync conflicts".into(),
            status: Status::Backlog,
        }]
    }

    fn view_of(agents: &ProjectAgentList, shown: Option<&str>) -> PaneView {
        view(&facts(agents, shown))
    }

    #[test]
    fn the_empty_pane_says_what_it_will_start() {
        let v = view_of(&list(Vec::new()), None);
        assert_eq!(v.agent, None);
        assert_eq!(v.opens_on, None);
        assert_eq!((v.header.title.as_str(), v.header.pill), (TITLE, None));
        assert!(!v.header.switcher, "nothing to switch to");
        assert!(v.running.is_none());
        let e = v.empty.unwrap();
        assert_eq!(e.line.agent, "Claude Code");
        assert_eq!(e.line.note, "\u{b7} starts when you send");
        assert_eq!(e.line.avatar, None, "one tool: avatars hidden");
        assert_eq!(e.line.rename, None);
        assert_eq!(e.title, "Ask the Project agent");
        assert_eq!(
            e.text,
            "It reads and runs code in pocket-notes (never edits) and creates or changes Backlog tickets. Nothing \
             runs and no tokens are used until you send."
        );
        let examples: Vec<(&str, &str)> = e.examples.iter().map(|x| (x.label, x.fill)).collect();
        assert_eq!(
            examples,
            [
                (
                    "Turn this into tickets: \u{2026}",
                    "Turn this into tickets: "
                ),
                ("Why is a ticket stuck?", "Why is a ticket stuck?"),
                ("Plan an epic for \u{2026}", "Plan an epic for "),
                (
                    "Import issues from our tracker",
                    "Import issues from our tracker"
                )
            ],
            "a click fills the box, leaving the \u{2026} to the user"
        );
        assert_eq!(
            e.placeholder,
            "Ask about pocket-notes, or describe work\u{2026}"
        );
        assert_eq!(e.hint, "Enter starts Claude Code");
        assert_eq!(e.recent_title, "RECENTLY ENDED");
        assert_eq!(
            empty("notes-api", None).examples,
            e.examples,
            "the same for every project"
        );
    }

    #[test]
    fn the_empty_pane_names_the_project_agents_named_agent() {
        // A fresh install: the starter entry, named after its tool, and no tool part.
        let starter = sample_agent("Claude Code", None);
        let e = empty("shop-web", Some(&starter));
        assert_eq!(
            (e.line.agent.as_str(), e.line.note.as_str()),
            ("Claude Code", "\u{b7} starts when you send")
        );
        assert_eq!(e.hint, "Enter starts Claude Code");
        // shop-web's Project agent default is Strong.
        let strong = sample_agent("Strong", Some("opus"));
        let agents = list(Vec::new());
        let v = view(&Facts {
            new_agent: Some(&strong),
            ..facts(&agents, None)
        });
        let e = v.empty.unwrap();
        assert_eq!(
            (e.line.agent.as_str(), e.line.note.as_str()),
            ("Strong", "\u{b7} starts when you send")
        );
        assert_eq!(e.line.avatar, None, "one tool: no avatar");
        assert_eq!(e.hint, "Enter starts Strong");
        assert_eq!(
            agent_name("Strong", Tool::ALL[0]),
            "Strong",
            "one tool: no tool part"
        );
    }

    #[test]
    fn recently_ended_rows_offer_resume() {
        let now = 1_791_600_000_000;
        let mut a = agent("a1", "Search slowness", 1);
        a.state = AgentState::Ended;
        a.ended_at = Some(now - 86_400_000);
        let mut agents = list(Vec::new());
        agents.recent = vec![a];
        let v = view(&Facts {
            now_ms: now,
            ..facts(&agents, None)
        });
        let row = &v.recent.rows[0];
        assert_eq!(
            (row.name.as_str(), row.ended.as_str(), row.resume),
            ("Search slowness", "yesterday", "Resume")
        );
        assert_eq!(row.when, "yesterday \u{b7} ended normally");
        assert!(
            v.empty.is_some(),
            "Recently ended doesn't make an agent shown"
        );
    }

    fn facts<'a>(agents: &'a ProjectAgentList, shown: Option<&'a str>) -> Facts<'a> {
        Facts {
            agents,
            shown,
            created: None,
            edited: &[],
            project_name: "pocket-notes",
            new_agent: None,
            window_width: Some(1440.),
            now_ms: 0,
            platform: Platform::Mac,
        }
    }

    #[test]
    fn the_running_header_follows_each_state() {
        let mut a = agent("a1", "Search slowness", 1);
        let pill = |a: &ProjectAgent| {
            let agents = list(vec![a.clone()]);
            let v = view_of(&agents, Some("a1"));
            let p = v.header.pill.unwrap();
            (p.label, p.palette, p.icon)
        };
        assert_eq!(
            pill(&a),
            ("Working".to_owned(), Palette::Working, IconName::Activity)
        );
        a.needs_you = Some("Bash(npm test)".into());
        assert_eq!(
            pill(&a),
            ("Waiting for you".to_owned(), Palette::Attn, IconName::Hand),
            "a permission prompt is turquoise"
        );
        a.needs_you = None;
        a.question = Some(otaq_core::board::questions::HumanQuestion {
            id: "q1".into(),
            text: "Import closed issues too?".into(),
            options: Vec::new(),
            recommended: None,
            asked_at: 0,
            via_lead: None,
        });
        assert_eq!(
            pill(&a),
            (
                "Waiting for you".to_owned(),
                Palette::Attn,
                IconName::MessageCircleQuestionMark
            ),
            "a question is turquoise"
        );
        a.question = None;
        a.mid_turn = false;
        assert_eq!(
            pill(&a),
            ("Idle".to_owned(), Palette::Queued, IconName::CirclePause),
            "idle isn't needs-you: never turquoise"
        );
        a.state = AgentState::Ended;
        assert_eq!(
            pill(&a),
            ("Ended".to_owned(), Palette::Queued, IconName::CircleStop)
        );
        a.state = AgentState::EndedByRestart;
        assert_eq!(
            pill(&a),
            (
                "Ended by restart".to_owned(),
                Palette::Queued,
                IconName::CircleStop
            )
        );

        let agents = list(vec![agent("a1", "Search slowness", 1)]);
        let v = view_of(&agents, Some("a1"));
        assert_eq!(v.agent.as_deref(), Some("a1"));
        assert!(v.empty.is_none());
        assert_eq!(
            (v.header.title.as_str(), v.header.unnamed, v.header.switcher),
            ("Search slowness", false, true)
        );
        assert_eq!(
            (v.header.new.label, v.header.new.keys.as_deref()),
            ("New Project agent", Some("\u{21e7}\u{2318}A"))
        );
        assert_eq!(
            (v.header.recent.icon, v.header.more.icon),
            (IconName::History, IconName::Ellipsis)
        );
    }

    #[test]
    fn the_agent_line_has_the_sessions_named_agent_and_tokens() {
        let mut a = agent("a1", "Search slowness", 1);
        a.session = Some(session_on("Strong"));
        let agents = list(vec![a.clone()]);
        let strong = sample_agent("Light", Some("haiku"));
        let line = view(&Facts {
            // What the role resolves to now doesn't change a running agent's line.
            new_agent: Some(&strong),
            ..facts(&agents, Some("a1"))
        })
        .running
        .unwrap()
        .line;
        assert_eq!(
            (line.agent.as_str(), line.note.as_str(), line.rename),
            ("Strong", "\u{b7} 52k", Some("Rename"))
        );
        assert_eq!(line.avatar, None);
        // The starter entry.
        a.session = Some(session_on("Claude Code"));
        let agents = list(vec![a.clone()]);
        let line = view_of(&agents, Some("a1")).running.unwrap().line;
        assert_eq!(
            (line.agent.as_str(), line.note.as_str()),
            ("Claude Code", "\u{b7} 52k")
        );
        // A session that saved no named agent: the tool's name.
        a.session = None;
        let agents = list(vec![a]);
        let line = view_of(&agents, Some("a1")).running.unwrap().line;
        assert_eq!(line.agent, "Claude Code");
    }

    #[test]
    fn the_footer_says_where_typing_goes_or_how_it_ended() {
        let mut a = agent("a1", "Search slowness", 1);
        let f = footer(&a, Platform::Mac);
        assert_eq!(
            (f.text, f.keys.as_deref(), f.keys_label, f.resume),
            ("Typing goes to the agent", Some("\u{2318}L"), "focus", None)
        );
        a.state = AgentState::Ended;
        let f = footer(&a, Platform::Mac);
        assert_eq!(f.text, "Ended \u{b7} Resume continues this conversation");
        assert_eq!(
            (
                f.resume.map(|r| (r.label, r.primary)),
                f.resume_note.as_deref()
            ),
            (
                Some(("Resume", false)),
                Some("or \u{21e7}\u{2318}A for a new agent")
            )
        );
        a.state = AgentState::EndedByRestart;
        let f = footer(&a, Platform::Mac);
        assert_eq!(f.text, "Ended by restart \u{b7} Resume continues it");
        assert_eq!(f.resume.map(|r| r.primary), Some(true));
        assert_eq!(
            f.resume_note.as_deref(),
            Some("continues the same conversation")
        );
        let agents = list(vec![a]);
        assert!(view_of(&agents, Some("a1")).running.unwrap().ended);
    }

    #[test]
    fn created_and_changed_folds_at_the_1280_class() {
        let a = agent("a1", "Search slowness", 1);
        let c = created(&a, &changed(), &edited(), Some(1280.)).unwrap();
        assert_eq!(c.line, "CREATED 2 \u{b7} CHANGED 1");
        assert!(c.folded);
        assert!(
            created(&a, &changed(), &edited(), Some(1359.))
                .unwrap()
                .folded
        );
        assert!(
            !created(&a, &changed(), &edited(), Some(1360.))
                .unwrap()
                .folded
        );
        assert!(
            !created(&a, &changed(), &edited(), Some(1920.))
                .unwrap()
                .folded
        );
        let rows: Vec<(&str, &str, &str, bool, Option<&str>)> = c
            .rows
            .iter()
            .map(|r| {
                (
                    r.key.as_str(),
                    r.title.as_str(),
                    r.place,
                    r.edited,
                    r.delete,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("NOTE-11", "Slow search query", "Ready for Dev", false, None),
                (
                    "NOTE-12",
                    "Cache note previews",
                    "Backlog",
                    false,
                    Some("Delete")
                ),
                ("NOTE-5", "Sync conflicts", "Backlog", true, None),
            ]
        );
        assert_eq!(c.rows[2].icon, IconName::Pencil);
        assert_eq!(
            c.show_on_board,
            ShowOnBoard {
                label: "Show on board",
                agent: "a1".into(),
            }
        );
        assert_eq!(
            created(&a, &CreatedAndChanged::default(), &[], None),
            None,
            "nothing created or changed: no section"
        );
    }

    #[test]
    fn the_switcher_lists_newest_first_with_states() {
        let mut waits = agent("a2", "Import GitHub issues", 2);
        waits.needs_you = Some("Allow gh issue list?".into());
        let mut ended = agent("a1", "Footer audit", 1);
        ended.state = AgentState::Ended;
        let mut unnamed = agent("a3", "New agent", 3);
        unnamed.name_source = None;
        let agents = list(vec![ended, waits, unnamed]);
        let v = view_of(&agents, Some("a2"));
        let rows: Vec<(&str, &str, bool)> = v
            .switcher
            .rows
            .iter()
            .map(|r| (r.name.as_str(), r.pill.label.as_str(), r.selected))
            .collect();
        assert_eq!(
            rows,
            [
                ("New agent", "Working", false),
                ("Import GitHub issues", "Waiting for you", true),
                ("Footer audit", "Ended", false),
            ]
        );
        assert!(v.switcher.rows[0].unnamed);
        assert!(v.switcher.rows.iter().all(|r| r.avatar.is_none()));
        assert_eq!(v.switcher.new_label, "New Project agent (empty)");
        assert_eq!(v.switcher.new_keys.as_deref(), Some("\u{21e7}\u{2318}A"));
        assert_eq!(
            v.opens_on.as_deref(),
            Some("a3"),
            "the newest running agent"
        );
    }

    #[test]
    fn a_closed_or_unknown_agent_shows_the_empty_pane() {
        let agents = list(vec![agent("a1", "Search slowness", 1)]);
        let v = view_of(&agents, Some("gone"));
        assert_eq!(v.agent, None);
        assert!(v.empty.is_some());
        assert!(v.header.switcher, "the running agent is one click away");
        assert_eq!(v.opens_on.as_deref(), Some("a1"));
        let mut ended = agent("a1", "Search slowness", 1);
        ended.state = AgentState::Ended;
        assert_eq!(opens_on(&[ended]), None, "only running agents");
    }

    #[test]
    fn the_more_menu_and_its_disabled_items() {
        let ids = |m: &[MenuItem]| -> Vec<(String, bool)> {
            m.iter().map(|i| (i.id.clone(), i.disabled)).collect()
        };
        let running = agent("a1", "Search slowness", 1);
        let m = menu(Some(&running), std::slice::from_ref(&running));
        assert_eq!(
            m.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            [
                "Rename",
                "End agent",
                "Close ended agents",
                "Recently ended\u{2026}"
            ]
        );
        assert_eq!(m[1].hint.as_deref(), Some("asks first if mid-turn"));
        assert_eq!(
            ids(&m),
            [
                (RENAME.to_owned(), false),
                (END.to_owned(), false),
                (CLOSE_ENDED.to_owned(), true),
                (RECENT.to_owned(), false)
            ],
            "Close ended agents is disabled with none ended"
        );
        let mut ended = agent("a2", "Footer audit", 2);
        ended.state = AgentState::EndedByRestart;
        let m = menu(Some(&ended), &[running.clone(), ended.clone()]);
        assert!(m[1].disabled, "an ended agent can't be ended again");
        assert!(!m[2].disabled);
        let m = menu(None, &[]);
        assert!(m[0].disabled && m[1].disabled && m[2].disabled && !m[3].disabled);
    }

    #[test]
    fn the_strip_dot_shows_when_an_agent_waits() {
        let mut a = agent("a1", "Search slowness", 1);
        let agents = list(vec![a.clone()]);
        assert!(!view_of(&agents, None).agent_waits);
        a.needs_you = Some("Allow npm test?".into());
        let agents = list(vec![a.clone()]);
        let v = view_of(&agents, None);
        assert!(v.agent_waits, "even while another one is shown");
        assert_eq!(v.strip.keys.as_deref(), Some("\u{2318}\\"));
        a.focused = true;
        assert!(
            !view_of(&list(vec![a]), None).agent_waits,
            "you are in its terminal"
        );
    }

    #[test]
    fn the_gallery_shows_every_pane_state() {
        let g = gallery();
        let shown: Vec<(Option<&str>, Option<&str>)> = g
            .states
            .iter()
            .map(|s| {
                (
                    s.view.agent.as_deref(),
                    s.view.header.pill.as_ref().map(|p| p.label.as_str()),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (None, None),
                (None, None),
                (Some("a1"), Some("Working")),
                (Some("a1"), Some("Working")),
                (Some("a2"), Some("Waiting for you")),
                (Some("a4"), Some("Ended")),
                (Some("a5"), Some("Ended by restart")),
            ]
        );
        let folded = |i: usize| {
            g.states[i]
                .view
                .running
                .as_ref()
                .and_then(|r| r.created.as_ref())
                .map(|c| c.folded)
        };
        assert_eq!((folded(2), folded(3)), (Some(false), Some(true)));
        let line = |i: usize| {
            g.states[i]
                .view
                .empty
                .as_ref()
                .map(|e| e.line.agent.clone())
                .or_else(|| {
                    g.states[i]
                        .view
                        .running
                        .as_ref()
                        .map(|r| r.line.agent.clone())
                })
        };
        assert_eq!(
            (line(0), line(1), line(2)),
            (
                Some("Claude Code".to_owned()),
                Some("Strong".to_owned()),
                Some("Strong".to_owned())
            )
        );
        assert!(g.states[0].view.agent_waits, "the strip's dot");
    }
}
