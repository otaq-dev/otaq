//! The Project agent pane (T-0326; SHL-04, PAG-01, PAG-05, PAG-06, TKT-08; d0087, d0068, d0060, d0077, d0018;
//! *18 · Project agents & terminals → Row · Pane: empty, hidden, tool problems, menu (0087)*, *Row · More pane
//! states (0086 · V)*, *Row · Switching agents, attention (0086 · V)*): everything the pane between the rail and the
//! main area draws, for one project and the agent it shows.
//!
//! The pane shows one Project agent, or the **empty pane** "Ask the Project agent". The empty pane is the page's own
//! state, never a session or a row: no agent exists until the first message is sent, and that message is the
//! go-ahead for its tokens (d0087). A running agent has its header (switcher, state pill, Recently ended, +, …), the
//! agent line, Created and changed, its terminal and the footer. The page only draws; which agent it shows is page
//! state, and [`PaneView::opens_on`] says where a project opens the first time.

use otaq_core::agent_tools::Tool;
use otaq_core::domain::{Status, TicketKey};
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
use crate::view_models::project_terminal::agent_waits;
use crate::view_models::tool_avatar::{self, AvatarTool, ToolAvatarView};

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

/// The line under the header: the tool avatar (hidden while one tool is connected), the tool, then the tokens or
/// "· starts when you send", and Rename on a running agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneAgentLine")]
pub struct AgentLine {
    pub avatar: Option<ToolAvatarView>,
    /// "Claude Code" (M22 puts the named agent before it: "Standard · Claude Code").
    pub tool: String,
    /// "· 52k", "· starts when you send".
    pub note: String,
    pub rename: Option<&'static str>,
}

fn avatar(tool: Tool, model: Option<&str>) -> Option<ToolAvatarView> {
    tool_avatar::shown().then(|| ToolAvatarView::of(tool, model))
}

/// The empty pane, "Ask the Project agent" (*No Project agent yet (0087)*, `SbFln`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PaneEmpty")]
pub struct Empty {
    pub line: AgentLine,
    pub title: &'static str,
    pub text: String,
    /// Each fills the box; none sends.
    pub examples: Vec<&'static str>,
    /// Above [`PaneView::recent`]'s rows ("name · yesterday · Resume"); hidden with none.
    pub recent_title: &'static str,
    pub placeholder: String,
    /// "Enter starts Claude Code".
    pub hint: String,
    pub send: &'static str,
}

/// The project agent tool a new agent starts on. Until named agents (M22) and other tools (M23), the first tool.
fn new_agent_tool() -> Tool {
    Tool::ALL[0]
}

pub fn empty(project_name: &str) -> Empty {
    let tool = new_agent_tool();
    let name = AvatarTool::from(tool).name();
    Empty {
        line: AgentLine {
            avatar: avatar(tool, None),
            tool: name.to_owned(),
            note: "\u{b7} starts when you send".to_owned(),
            rename: None,
        },
        title: "Ask the Project agent",
        text: format!(
            "It reads and runs code in {project_name} (never edits) and creates or changes Backlog tickets. Nothing \
             runs and no tokens are used until you send."
        ),
        examples: EXAMPLES.to_vec(),
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
            tool: AvatarTool::from(a.tool).name().to_owned(),
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
        empty: shown.is_none().then(|| empty(f.project_name)),
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
        assert_eq!(e.line.tool, "Claude Code");
        assert_eq!(e.line.note, "\u{b7} starts when you send");
        assert_eq!(e.line.avatar, None, "one tool: avatars hidden");
        assert_eq!(e.line.rename, None);
        assert_eq!(e.title, "Ask the Project agent");
        assert_eq!(
            e.text,
            "It reads and runs code in pocket-notes (never edits) and creates or changes Backlog tickets. Nothing \
             runs and no tokens are used until you send."
        );
        assert_eq!(
            e.examples,
            [
                "Turn this into tickets: \u{2026}",
                "Why is a ticket stuck?",
                "Plan an epic for \u{2026}",
                "Import issues from our tracker"
            ]
        );
        assert_eq!(
            e.placeholder,
            "Ask about pocket-notes, or describe work\u{2026}"
        );
        assert_eq!(e.hint, "Enter starts Claude Code");
        assert_eq!(e.recent_title, "RECENTLY ENDED");
        assert_eq!(
            empty("notes-api").examples,
            e.examples,
            "the same for every project"
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
    fn the_agent_line_has_the_tool_and_tokens() {
        let agents = list(vec![agent("a1", "Search slowness", 1)]);
        let line = view_of(&agents, Some("a1")).running.unwrap().line;
        assert_eq!(
            (line.tool.as_str(), line.note.as_str(), line.rename),
            ("Claude Code", "\u{b7} 52k", Some("Rename"))
        );
        assert_eq!(line.avatar, None);
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
}
