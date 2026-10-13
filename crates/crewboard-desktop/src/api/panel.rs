//! `panel.*`: the ticket side panel (UI-04, CLS-01, CLS-03, CLS-05, CLS-06, TKT-08, SES-02, ATT-12, REV-06, GIT-04;
//! d0008, d0033, d0044, d0052, d0060, d0061, d0043; *05 · Ticket side panel*). One view per open ticket: the header
//! (key, title, priority, status, branch, base, last signal, tokens), previous / next in the column, the action bar
//! and the "…" menu of its state, the answer options of an Asks-you question, the queued notice, the conflict strip,
//! the Start interrupted callout and the tab strip with its indicators. The buttons that talk to the service land
//! here too, so their wording (toasts, refusals) stays in Rust; the ones that open a tab or a dialog are the page's.

use std::time::{SystemTime, UNIX_EPOCH};

use crewboard_core::board::conflicts::Conflict;
use crewboard_core::board::moves::DropTarget;
use crewboard_core::board::questions::Choice;
use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{TicketKey, ValidationAction};
use crewboard_core::usage::TicketUsage;
use crewboard_core::protocol::{
    CloseParams, CommentsParams, DepParams, KeyParams, MoveParams, ProjectParams, Reply, Request,
    SendToFixParams, SessionListParams, SubmitByUserParams, UsageParams, ValidationActionParams,
};
use crewboard_core::review::ReturnOutcome;
use crewboard_core::session::deliveries::Delivery;
use crewboard_core::session::{SessionInfo, SessionKind};
use crewboard_view::icons::IconName;
use crewboard_view::keymap::{KeyAction, Platform};
use crewboard_view::view_models::answer::{self, AnswerOutcome};
use crewboard_view::view_models::answer_options::AnswerOptions;
use crewboard_view::view_models::approve;
use crewboard_view::view_models::card::{
    CardActionKind, TOKENS_UNKNOWN, age_label, tokens_label,
};
use crewboard_view::view_models::close;
use crewboard_view::view_models::comments;
use crewboard_view::view_models::epic_actions;
use crewboard_view::view_models::menu::MenuItem;
use crewboard_view::view_models::panel::{self as vm, Indicator, Tab};
use crewboard_view::view_models::panel_actions::{self as actions, Action, Look};
use crewboard_view::view_models::pr_dialog;
use crewboard_view::view_models::priority::{self, PriorityView};
use crewboard_view::view_models::queue;
use crewboard_view::view_models::status::{self, PillView};
use crewboard_view::view_models::terminal_strip::{self as strip, Click, UnsentPrompt};
use crewboard_view::view_models::toasts::{NewToast, ToastLevel};
use crewboard_view::view_models::validation_panel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "panel.view" => reply(&view(app, parse(params)?).await?),
        "panel.action" => reply(&action(app, parse(params)?).await?),
        "panel.menu" => reply(&menu_choice(app, parse(params)?).await?),
        "panel.priority" => reply(&set_priority(app, parse(params)?).await?),
        "panel.close_dialog" => reply(&close_dialog(app, parse(params)?).await?),
        "panel.close" => reply(&close_ticket(app, parse(params)?).await?),
        "panel.submit_dialog" => reply(&submit_dialog(app, parse(params)?).await?),
        "panel.submit" => reply(&submit(app, parse(params)?).await?),
        "panel.mark_closed" => reply(&mark_closed(app, parse(params)?).await?),
        "panel.answer" => reply(&answer(app, parse(params)?).await?),
        "panel.send_to_fix" => reply(&send_to_fix(app, parse(params)?).await?),
        "panel.card" => reply(&card(app, parse(params)?).await?),
        "panel.reopen_parent" => reply(&reopen_parent(app, parse(params)?).await?),
        "panel.remove_dependency" => reply(&remove_dependency(app, parse(params)?).await?),
        "panel.setup_log" => reply(&setup_log(parse(params)?)),
        "panel.answer_agent" => reply(&answer_agent(app, parse(params)?).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn key_of(text: &str) -> Result<TicketKey, RequestError> {
    text.parse()
        .map_err(|_| RequestError::plain(format!("not a ticket key: {text}")))
}

/// What most `panel.*` requests take: the open ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelParams")]
pub struct Params {
    pub key: String,
}

/// A tooltip: its name and its shortcut on this platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelHint")]
pub struct Hint {
    pub text: String,
    pub keys: Option<String>,
}

impl Hint {
    fn new(text: impl Into<String>, action: KeyAction) -> Self {
        Self {
            text: text.into(),
            keys: action.keys(Platform::current()),
        }
    }
}

/// The branch chip: the name shortened in the middle, the whole name on hover and for Copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelBranch")]
pub struct Branch {
    pub shown: String,
    pub full: String,
}

/// "40s ago", in the warning colour when a working agent has been silent too long (ATT-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelSignal")]
pub struct Signal {
    pub text: String,
    pub stale: bool,
}

/// "182k", or "—" with the reason when Claude Code's file can't be read (USG-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelTokens")]
pub struct Tokens {
    pub text: String,
    pub tooltip: Option<&'static str>,
}

/// A button of the action bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelButton")]
pub struct Button {
    pub action: Action,
    pub label: String,
    pub icon: Option<IconName>,
    pub look: Look,
}

/// What a tab shows after its name (the page's `TabIndicator`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "PanelTabMark")]
pub enum Mark {
    Live,
    LiveAttention,
    Problem,
    Note { text: String },
}

/// One tab of the strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelTabView")]
pub struct TabView {
    pub tab: Tab,
    pub label: &'static str,
    pub icon: IconName,
    pub keys: Option<String>,
    pub mark: Option<Mark>,
}

/// One answer option (ATT-12): the recommended one is picked when the list opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelAnswerRow")]
pub struct AnswerRow {
    pub number: u32,
    pub label: String,
    pub recommended: bool,
    pub picked: bool,
    pub other: bool,
}

/// The question with its options in the header (*SHOP-21 · Asks you · answer options*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelAnswer")]
pub struct Answer {
    pub question: String,
    pub text: String,
    pub rows: Vec<AnswerRow>,
    pub recommended_label: &'static str,
    pub other_placeholder: &'static str,
    pub hint: &'static str,
    pub in_terminal: &'static str,
    pub send: &'static str,
}

/// Reopen parent / Remove dependency for a child of a closed parent (CLS-04), with the removal's confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelParentChoice")]
pub struct ParentChoice {
    pub parent: String,
    pub reopen: String,
    pub remove: String,
    pub confirm_title: String,
    pub confirm_body: String,
    pub confirm_label: &'static str,
}

/// What a Queued ticket waits for, as a sentence (T-0083).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelNotice")]
pub struct Notice {
    pub text: String,
    pub parent_closed: bool,
    pub choices: Vec<ParentChoice>,
}

/// "Conflicts with main · 2 files (…)" in the header (GIT-04), with Send to agent to fix when it applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelConflict")]
pub struct ConflictStrip {
    pub line: String,
    pub fix: Option<&'static str>,
}

/// The Start interrupted callout (SES-02, *16 → Start interrupted · SHOP-28 panel*).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelStartProblem")]
pub struct StartProblem {
    pub heading: String,
    pub message: String,
    pub has_log: bool,
    pub show_log: &'static str,
    pub hide_log: &'static str,
    pub retry_heading: &'static str,
    pub retry_does: Vec<&'static str>,
}

/// *SHOP-32 · Validation failed 3 times*: the note box Answer the agent… opens (VAL-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelAnswerAgent")]
pub struct AnswerAgentBox {
    pub heading: &'static str,
    pub placeholder: &'static str,
    pub footer: String,
    pub cancel: &'static str,
    pub send: &'static str,
}

/// A plain confirmation's texts (Mark closed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelConfirm")]
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub confirm: String,
}

/// The panel's sizes (`panel.rs`): what a drag of its edge needs, sent up front.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PanelSizes")]
pub struct Sizes {
    pub default_width: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub min_board: f32,
    pub icon_tabs_below: f32,
}

pub const SIZES: Sizes = Sizes {
    default_width: vm::DEFAULT_WIDTH,
    min_width: vm::MIN_WIDTH,
    max_width: vm::MAX_WIDTH,
    min_board: vm::MIN_BOARD,
    icon_tabs_below: vm::ICON_TABS_BELOW,
};

/// The open ticket's panel.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PanelView")]
pub struct View {
    pub key: String,
    pub title: String,
    pub epic: bool,
    pub priority: PriorityView,
    pub priority_hint: String,
    pub priority_menu: Vec<MenuItem>,
    pub pill: PillView,
    pub branch: Option<Branch>,
    pub copy_branch: &'static str,
    pub base: String,
    pub signal: Option<Signal>,
    pub tokens: Option<Tokens>,
    /// "agent 61k · validation 23k (2 rounds)" once a validator ran (VAL-14).
    pub token_split: Option<String>,
    pub previous: Option<String>,
    pub next: Option<String>,
    pub previous_hint: Hint,
    pub next_hint: Hint,
    pub expand_hint: Hint,
    pub collapse_hint: Hint,
    pub close_hint: Hint,
    pub more_hint: &'static str,
    /// The left edge's name for screen readers; it resizes with ←/→ too.
    pub resize_hint: &'static str,
    pub buttons: Vec<Button>,
    /// The "▾" beside Approve: the approve choices (Local merge / Open PR) in the `local_merge` overlay.
    pub approve_more: Option<&'static str>,
    /// Why Auto-merge stopped, under the bar (AMG-03, AMG-04).
    pub stop_message: Option<String>,
    /// Validation failed N times: Answer the agent…'s note box.
    pub answer_agent: Option<AnswerAgentBox>,
    /// The first parent, for View parent.
    pub parent: Option<String>,
    pub menu: Vec<MenuItem>,
    /// `crewboard attach SHOP-19`, for Copy attach command.
    pub attach_command: String,
    pub answer: Option<Answer>,
    pub notice: Option<Notice>,
    pub conflict: Option<ConflictStrip>,
    pub start_problem: Option<StartProblem>,
    pub mark_closed: Option<Confirm>,
    /// The compact running pill next to the tokens while the Run is up (*13 → Card & panel-header controls*).
    pub run: Option<RunPill>,
    pub tabs: Vec<TabView>,
    /// The ticket's agent session runs.
    pub live_agent: bool,
    pub sizes: Sizes,
}

/// "● :4020 ↗ ▢": the Run's port, Open (only with an Open URL) and Stop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelRunPill")]
pub struct RunPill {
    pub port: String,
    pub url: Option<String>,
    pub open_hint: String,
    pub stop_hint: &'static str,
}

/// What `panel.view` answers: the view, or "SHOP-25 is gone." once the ticket was deleted.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "PanelLoad")]
pub struct Load {
    pub view: Option<View>,
    pub gone: String,
}

/// What the view is made of, gathered from the service (one ticket, its column's neighbours, its sessions).
pub struct Facts<'a> {
    pub ticket: &'a Ticket,
    pub tickets: &'a [Ticket],
    pub sessions: &'a [SessionInfo],
    pub conflict: Option<&'a Conflict>,
    pub has_log: bool,
    /// Pending line comments: "Send back (2)" (T-0226).
    pub pending: usize,
    /// The ticket's tokens with the validators' share (`usage.summary`).
    pub usage: Option<&'a TicketUsage>,
    /// The project's Validation rounds (for Answer the agent).
    pub rounds: u32,
    pub now: i64,
    /// The ticket's Run (T-0232): the header's running pill and the Run tab's marker.
    pub run: Option<&'a crewboard_core::terminals::run::RunInfo>,
}

fn running_agent(sessions: &[SessionInfo]) -> bool {
    sessions
        .iter()
        .any(|s| s.kind == SessionKind::Agent && s.is_running())
}

fn mark(indicator: Indicator) -> Option<Mark> {
    match indicator {
        Indicator::None => None,
        Indicator::Dot { attention: false } => Some(Mark::Live),
        Indicator::Dot { attention: true } => Some(Mark::LiveAttention),
        Indicator::Text(text) => Some(Mark::Note { text }),
        Indicator::Count(n) => Some(Mark::Note {
            text: n.to_string(),
        }),
        Indicator::Problem => Some(Mark::Problem),
    }
}

fn answer_of(ticket: &Ticket) -> Option<Answer> {
    if ticket.status != crewboard_core::domain::Status::AsksYou || ticket.flags.your_turn {
        return None;
    }
    let question = ticket.question.as_ref()?;
    let options = AnswerOptions::of(question)?;
    Some(Answer {
        question: question.id.clone(),
        text: question.text.clone(),
        rows: options
            .rows()
            .into_iter()
            .map(|r| AnswerRow {
                number: r.number,
                label: r.label,
                recommended: r.recommended,
                picked: r.picked,
                other: r.other,
            })
            .collect(),
        recommended_label: "Recommended",
        other_placeholder: "Type your answer",
        hint: "1\u{2013}4 pick \u{b7} \u{21b5} sends \u{b7} logged in Messages",
        in_terminal: "Answer in terminal",
        send: "Send answer",
    })
}

fn notice_of(ticket: &Ticket, tickets: &[Ticket]) -> Option<Notice> {
    let text = queue::panel_notice(ticket, tickets)?;
    let choices = close::closed_parent_choices(ticket, tickets)
        .into_iter()
        .map(|c| {
            let (confirm_title, confirm_body) =
                close::remove_dependency_text(&ticket.key, &c.parent);
            ParentChoice {
                parent: c.parent.to_string(),
                reopen: c.reopen,
                remove: c.remove,
                confirm_title,
                confirm_body,
                confirm_label: "Remove dependency",
            }
        })
        .collect();
    Some(Notice {
        text,
        parent_closed: ticket.status == crewboard_core::domain::Status::ParentClosed,
        choices,
    })
}

pub fn view_of(f: &Facts) -> View {
    let ticket = f.ticket;
    let platform = Platform::current();
    let live = running_agent(f.sessions);
    // Run is built here (T-0232); `actions::buttons` leaves it out.
    let buttons: Vec<Button> = actions::all_buttons(ticket)
        .into_iter()
        .filter(|(a, _)| a.arrives_with().is_none() || *a == Action::Run)
        .map(|(action, look)| Button {
            action,
            label: match action {
                Action::SendBack => comments::send_back_label(f.pending),
                Action::ViewParent => ticket.depends_on.first().map_or_else(
                    || action.label().to_owned(),
                    |p| format!("View parent {p}"),
                ),
                _ => action.label().to_owned(),
            },
            icon: action.icon(),
            look,
        })
        .collect();
    let has_epics = !epic_actions::epic_choices(f.tickets).is_empty();
    let has_branch = ticket.work.as_ref().is_some_and(|w| w.branch.is_some());
    let signal = ticket.flags.last_signal_at.map(|at| Signal {
        text: format!("{} ago", age_label(f.now - at)),
        stale: crewboard_core::signals::is_stale(ticket.status, Some(at), f.now),
    });
    let tokens = match (ticket.flags.tokens, ticket.flags.tokens_unknown) {
        (Some(t), _) => Some(Tokens {
            text: tokens_label(t),
            tooltip: None,
        }),
        (None, true) => Some(Tokens {
            text: "\u{2014}".to_owned(),
            tooltip: Some(TOKENS_UNKNOWN),
        }),
        (None, false) => None,
    };
    let start_problem = vm::start_problem(ticket).map(|p| StartProblem {
        heading: p.heading,
        message: p.message,
        has_log: f.has_log,
        show_log: "View the setup log",
        hide_log: "Hide the setup log",
        retry_heading: "RETRY START DOES",
        retry_does: vm::RETRY_START_DOES.to_vec(),
    });
    let mark_closed = (ticket.status == crewboard_core::domain::Status::PrOpen)
        .then_some(ticket.flags.pr_number)
        .flatten()
        .map(|n| {
            let (title, body) = pr_dialog::mark_closed_confirm(n);
            Confirm {
                title,
                body: body.to_owned(),
                confirm: Action::MarkClosed.label().to_owned(),
            }
        });
    View {
        key: ticket.key.to_string(),
        title: ticket.title.clone(),
        epic: ticket.is_epic(),
        priority: priority::view(ticket.priority),
        priority_hint: match (&ticket.epic, ticket.priority_inherited) {
            (Some(epic), true) => priority::inherited_hint(epic),
            _ => "Priority".to_owned(),
        },
        priority_menu: priority::menu_items(ticket.priority),
        pill: {
            let mut pill = PillView::of(status::shown(ticket));
            if !ticket.flags.your_turn
                && let Some(label) = validation_panel::pill_label(ticket)
            {
                pill.label = label;
            }
            pill
        },
        branch: vm::branch_name(ticket).map(|full| Branch {
            shown: vm::middle_truncate(&full, 28),
            full,
        }),
        copy_branch: "Copy branch name",
        base: match &ticket.base {
            Base::Branch(b) => b.clone(),
            Base::Epic(epic) => crewboard_core::naming::branch(&epic.to_string()),
        },
        signal,
        tokens,
        token_split: f
            .usage
            .and_then(|u| validation_panel::token_split(ticket, u)),
        previous: vm::neighbour(f.tickets, &ticket.key, -1).map(|k| k.to_string()),
        next: vm::neighbour(f.tickets, &ticket.key, 1).map(|k| k.to_string()),
        previous_hint: Hint::new("Previous ticket", KeyAction::PreviousTicket),
        next_hint: Hint::new("Next ticket", KeyAction::NextTicket),
        expand_hint: Hint::new("Expand", KeyAction::ExpandPanel),
        collapse_hint: Hint::new("Back to the board", KeyAction::ExpandPanel),
        close_hint: Hint::new("Close", KeyAction::ClosePanel),
        more_hint: "More",
        resize_hint: "Panel width",
        approve_more: buttons
            .iter()
            .any(|b| b.action == Action::Approve)
            .then_some("Approve choices"),
        stop_message: validation_panel::stop_message(ticket),
        answer_agent: validation_panel::answer_agent_text(ticket, f.rounds).map(|t| AnswerAgentBox {
            heading: t.heading,
            placeholder: t.placeholder,
            footer: t.footer,
            cancel: t.cancel,
            send: t.send,
        }),
        buttons,
        parent: ticket.depends_on.first().map(ToString::to_string),
        menu: actions::menu(ticket, has_epics, has_branch, !f.sessions.is_empty()),
        attach_command: format!(
            "{} attach {}",
            crewboard_core::naming::COMMAND_NAME,
            ticket.key
        ),
        answer: answer_of(ticket),
        notice: notice_of(ticket, f.tickets),
        conflict: f
            .conflict
            .filter(|_| ticket.flags.conflicts_with.is_some())
            .map(|c| ConflictStrip {
                line: approve::conflict_line(c),
                fix: approve::offers_fix(ticket, Some(c)).then_some("Send to agent to fix"),
            }),
        start_problem,
        mark_closed,
        run: crewboard_view::view_models::run::header_pill(f.run).map(|(port, url)| RunPill {
            port: format!(":{port}"),
            open_hint: url
                .as_deref()
                .map_or_else(String::new, crewboard_view::view_models::run::open_label),
            url,
            stop_hint: "Stop",
        }),
        tabs: vm::indicators(ticket, f.sessions)
            .into_iter()
            .map(|(tab, indicator)| match (tab, f.run) {
                (Tab::Run, Some(run)) => (tab, crewboard_view::view_models::run::tab_indicator(Some(run))),
                _ => (tab, indicator),
            })
            .map(|(tab, indicator)| TabView {
                tab,
                label: tab.label(),
                icon: tab.icon(),
                keys: Tab::ALL
                    .iter()
                    .position(|t| *t == tab)
                    .and_then(|i| KeyAction::SwitchTab(i as u8 + 1).keys(platform)),
                mark: mark(indicator),
            })
            .collect(),
        live_agent: live,
        sizes: SIZES,
    }
}

async fn ticket(app: &App, key: &str) -> Result<Option<Ticket>, RequestError> {
    match app
        .service
        .send(Request::TicketGet(KeyParams { key: key.to_owned() }))
        .await
    {
        Ok(Reply::Ticket(t)) => Ok(Some(*t)),
        Ok(other) => Err(unexpected("ticket.get", &other)),
        Err(e) if e.not_found => Ok(None),
        Err(e) => Err(e),
    }
}

async fn tickets(app: &App, project: &str) -> Result<Vec<Ticket>, RequestError> {
    match app
        .service
        .send(Request::TicketList(ProjectParams {
            project: project.to_owned(),
        }))
        .await?
    {
        Reply::Tickets(list) => Ok(list),
        other => Err(unexpected("ticket.list", &other)),
    }
}

async fn sessions(app: &App, key: &str) -> Result<Vec<SessionInfo>, RequestError> {
    match app
        .service
        .send(Request::SessionList(SessionListParams {
            key: Some(key.to_owned()),
        }))
        .await?
    {
        Reply::Sessions(list) => Ok(list),
        other => Err(unexpected("session.list", &other)),
    }
}

/// The ticket with the rest of its project, or a "gone" refusal.
async fn ticket_and_project(app: &App, key: &str) -> Result<(Ticket, Vec<Ticket>), RequestError> {
    let Some(ticket) = ticket(app, key).await? else {
        return Err(RequestError::plain(format!("{key} is gone.")));
    };
    let all = tickets(app, &ticket.key.project.to_string()).await?;
    Ok((ticket, all))
}

fn setup_log_path(key: &str) -> Option<std::path::PathBuf> {
    crewboard_core::data_dir::DataDir::from_env()
        .ok()
        .and_then(|d| d.setup_log_path(key).ok())
        .filter(|p| p.is_file())
}

pub async fn view(app: &App, params: Params) -> Result<Load, RequestError> {
    let gone = format!("{} is gone.", params.key);
    let Some(ticket) = ticket(app, &params.key).await? else {
        return Ok(Load { view: None, gone });
    };
    let all = tickets(app, &ticket.key.project.to_string()).await?;
    let sessions = sessions(app, &params.key).await?;
    let conflict = if ticket.flags.conflicts_with.is_some() {
        match app
            .service
            .send(Request::TicketConflict(KeyParams {
                key: params.key.clone(),
            }))
            .await
        {
            Ok(Reply::Conflict(c)) => c,
            _ => None,
        }
    } else {
        None
    };
    // The Run, once there is a worktree to run in (T-0232).
    let run = if ticket.work.as_ref().is_some_and(|w| w.worktree.is_some()) {
        match app
            .service
            .send(Request::RunStatus(KeyParams {
                key: params.key.clone(),
            }))
            .await
        {
            Ok(Reply::Run(run)) => Some(*run),
            _ => None,
        }
    } else {
        None
    };
    let has_log = ticket.status == crewboard_core::domain::Status::StartInterrupted
        && setup_log_path(&params.key).is_some();
    let pending = if ticket.status == crewboard_core::domain::Status::ReadyForReview {
        match app
            .service
            .send(Request::ReviewComments(CommentsParams {
                key: params.key.clone(),
                round: None,
            }))
            .await
        {
            Ok(Reply::ReviewComments(list)) => list.pending as usize,
            _ => 0,
        }
    } else {
        0
    };
    let usage = if ticket.started_at.is_some() {
        ticket_usage(app, &ticket).await
    } else {
        None
    };
    let rounds = if ticket.status == crewboard_core::domain::Status::ValidationFailed {
        validation_rounds(app, &ticket).await
    } else {
        0
    };
    let view = view_of(&Facts {
        ticket: &ticket,
        tickets: &all,
        sessions: &sessions,
        conflict: conflict.as_ref(),
        has_log,
        pending,
        usage: usage.as_ref(),
        rounds,
        now: now_ms(),
        run: run.as_ref(),
    });
    Ok(Load {
        view: Some(view),
        gone,
    })
}

/// The ticket's row of `usage.summary`; `None` when it can't be read (the header then shows no split).
async fn ticket_usage(app: &App, ticket: &Ticket) -> Option<TicketUsage> {
    match app
        .service
        .send(Request::UsageSummary(UsageParams {
            project: Some(ticket.key.project.to_string()),
        }))
        .await
    {
        Ok(Reply::Usage(summary)) => summary.tickets.into_iter().find(|u| u.id == ticket.id),
        _ => None,
    }
}

/// The project's Validation rounds, after the global fallback.
pub(super) async fn validation_rounds(app: &App, ticket: &Ticket) -> u32 {
    match app
        .service
        .send(Request::ProjectSettingsGet(ProjectParams {
            project: ticket.key.project.to_string(),
        }))
        .await
    {
        Ok(Reply::ProjectSettings(reply)) => reply.effective.validation_rounds,
        _ => crewboard_core::board::settings::DEFAULT_VALIDATION_ROUNDS,
    }
}

/// The setup log of a Start interrupted ticket, when there is one.
pub fn setup_log(params: Params) -> Option<String> {
    setup_log_path(&params.key).and_then(|p| std::fs::read_to_string(p).ok())
}

fn toast_error(app: &App, title: String, error: &RequestError) {
    app.notices.toast(NewToast::new(
        ToastLevel::Error,
        title,
        Some(error.message.clone()),
    ));
}

/// What `panel.action` takes: a button that talks to the service.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelActionParams")]
pub struct ActionParams {
    pub key: String,
    pub action: Action,
}

/// Start, Retry start, Resume, Retry, Reopen, Mark merged and Open PR in browser. A refusal shows as a toast (and
/// rejects, so the button stops spinning). The other buttons open a tab or a dialog on the page.
pub async fn action(app: &App, params: ActionParams) -> Result<(), RequestError> {
    let key = params.key;
    let k = KeyParams { key: key.clone() };
    let (request, failed) = match params.action {
        Action::Start => (
            Request::TicketMove(MoveParams {
                key: key.clone(),
                target: DropTarget::Dev,
                confirmed: false,
            }),
            format!("{key} can\u{2019}t start"),
        ),
        Action::RetryStart => (
            Request::TicketRetryStart(k),
            format!("{key}\u{2019}s start can\u{2019}t be retried"),
        ),
        Action::Resume => (
            Request::TicketResume(k),
            format!("{key} can\u{2019}t be resumed"),
        ),
        Action::RetryResume => (
            Request::TicketRetryResume(k),
            format!("{key} can\u{2019}t be resumed"),
        ),
        Action::Reopen => (
            Request::TicketReopen(k),
            format!("{key} can\u{2019}t be reopened"),
        ),
        Action::MarkMerged => (
            Request::TicketMarkMerged(k),
            format!("Mark merged of {key} failed"),
        ),
        Action::MoveToReviewAnyway => (
            validation_request(&key, ValidationAction::MoveToReviewAnyway, ""),
            format!("{key} can\u{2019}t be moved to Review"),
        ),
        Action::AnotherRound => (
            validation_request(&key, ValidationAction::AnotherRound, ""),
            format!("{key} can\u{2019}t get another round"),
        ),
        Action::RetryValidation => (
            validation_request(&key, ValidationAction::Retry, ""),
            format!("{key}\u{2019}s validation can\u{2019}t be retried"),
        ),
        Action::RetryMerge => (
            validation_request(&key, ValidationAction::RetryMerge, ""),
            format!("{key} can\u{2019}t be merged"),
        ),
        Action::MoveToReview => (
            validation_request(&key, ValidationAction::MoveToReview, ""),
            format!("{key} can\u{2019}t be moved to Review"),
        ),
        Action::OpenPrInBrowser => {
            let url = ticket(app, &key)
                .await?
                .and_then(|t| t.work)
                .and_then(|w| w.pr_url)
                .ok_or_else(|| RequestError::plain(format!("{key} has no PR")))?;
            return open::that_detached(&url)
                .map_err(|e| RequestError::plain(format!("couldn't open {url}: {e}")));
        }
        other => {
            return Err(RequestError::plain(format!(
                "{other:?} is the page's (a tab or a dialog)"
            )));
        }
    };
    match app.service.send(request).await {
        Ok(Reply::MarkedMerged(marked)) => {
            app.notices.toast(NewToast::new(
                ToastLevel::Success,
                pr_dialog::merged_toast(&key, marked.started.len()),
                None,
            ));
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(e) => {
            toast_error(app, failed, &e);
            Err(e)
        }
    }
}

fn validation_request(key: &str, action: ValidationAction, note: &str) -> Request {
    Request::TicketValidationAction(ValidationActionParams {
        key: key.to_owned(),
        action,
        note: note.to_owned(),
    })
}

/// What `panel.menu` takes: a "…" menu item the service carries out (Pin / Unpin, Stop validation, Skip to
/// Review, Validate and submit).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelMenuParams")]
pub struct MenuParams {
    pub key: String,
    pub item: String,
}

pub async fn menu_choice(app: &App, params: MenuParams) -> Result<(), RequestError> {
    use crewboard_view::view_models::card_menu::{PIN, UNPIN};
    let k = KeyParams {
        key: params.key.clone(),
    };
    let key = params.key.clone();
    let (request, failed) = match params.item.as_str() {
        PIN => (Request::TicketPin(k), format!("{key} wasn\u{2019}t pinned")),
        UNPIN => (Request::TicketUnpin(k), format!("{key} wasn\u{2019}t unpinned")),
        actions::STOP_VALIDATION => (
            validation_request(&key, ValidationAction::Stop, ""),
            format!("{key}\u{2019}s validation can\u{2019}t be stopped"),
        ),
        actions::SKIP_TO_REVIEW => (
            validation_request(&key, ValidationAction::Skip, ""),
            format!("{key} can\u{2019}t skip to Review"),
        ),
        close::VALIDATE_AND_SUBMIT => (
            validation_request(&key, ValidationAction::ValidateAndSubmit, ""),
            format!("{key} can\u{2019}t be validated"),
        ),
        other => return Err(RequestError::plain(format!("not a service item: {other}"))),
    };
    match app.service.send(request).await {
        Ok(_) => Ok(()),
        Err(e) => {
            toast_error(app, failed, &e);
            Err(e)
        }
    }
}

/// What `panel.answer_agent` takes: the user's note for the agent.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelAnswerAgentParams")]
pub struct AnswerAgentParams {
    pub key: String,
    pub note: String,
}

/// Answer the agent (VAL-07): the note goes with the last findings and the round counter restarts. A refusal
/// stays in the box (the page shows it).
pub async fn answer_agent(app: &App, params: AnswerAgentParams) -> Result<(), RequestError> {
    let note = params.note.trim();
    if note.is_empty() {
        return Err(RequestError::plain("Write what the agent should do first."));
    }
    app.service
        .send(validation_request(&params.key, ValidationAction::AnswerAgent, note))
        .await
        .map(|_| ())
}

/// What `panel.priority` takes: the chosen row of the header's Priority menu.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelPriorityParams")]
pub struct PriorityParams {
    pub key: String,
    pub item: String,
}

/// A pick of the header's Priority menu is a `ticket.update` (TKT-09, d0065).
pub async fn set_priority(app: &App, params: PriorityParams) -> Result<(), RequestError> {
    let Some(p) = priority::from_menu_id(&params.item) else {
        return Err(RequestError::plain(format!(
            "not a priority: {}",
            params.item
        )));
    };
    let key = key_of(&params.key)?;
    match app.service.send(priority::request(&key, p)).await {
        Ok(_) => Ok(()),
        Err(e) => {
            app.notices
                .toast(NewToast::new(ToastLevel::Error, e.message.clone(), None));
            Err(e)
        }
    }
}

/// One box of the close confirmation: the running agent (amber) or a dependent ticket (red).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelCloseLine")]
pub struct CloseLine {
    pub agent: bool,
    /// activity (the agent), shield-off (agent and validator, d0063) or link-2 (a dependent).
    pub icon: IconName,
    pub text: String,
}

/// *Close ticket · confirm* (d0033): it names the running agent and the tickets that wait for this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelCloseDialog")]
pub struct CloseDialog {
    pub title: String,
    pub lines: Vec<CloseLine>,
    pub footer: String,
    pub confirm: String,
}

pub fn close_dialog_of(ticket: &Ticket, tickets: &[Ticket], running: bool) -> CloseDialog {
    let text = close::close_text(ticket, tickets, running);
    let validating = close::validating_stop_line(ticket).is_some();
    CloseDialog {
        title: text.title,
        lines: text
            .lines
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                // `close_text` puts the running agent (or agent and validator) first.
                let agent = (running || validating) && i == 0;
                CloseLine {
                    agent,
                    icon: match (agent, validating) {
                        (true, true) => IconName::ShieldOff,
                        (true, false) => IconName::Activity,
                        (false, _) => IconName::Link2,
                    },
                    text,
                }
            })
            .collect(),
        footer: text.footer,
        confirm: text.confirm,
    }
}

pub async fn close_dialog(app: &App, params: Params) -> Result<CloseDialog, RequestError> {
    let (ticket, all) = ticket_and_project(app, &params.key).await?;
    let running = running_agent(&sessions(app, &params.key).await?);
    Ok(close_dialog_of(&ticket, &all, running))
}

/// Close ticket's confirm (CLS-01): the service stops the agent first. A refusal shows inside the dialog.
pub async fn close_ticket(app: &App, params: Params) -> Result<(), RequestError> {
    app.service
        .send(Request::TicketClose(CloseParams {
            key: params.key,
            confirm: true,
        }))
        .await
        .map(|_| ())
}

/// *Submit for review · dialog* (CLS-05, d0052).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelSubmitDialog")]
pub struct SubmitDialog {
    pub title: String,
    pub intro: String,
    pub note_label: &'static str,
    pub note_placeholder: &'static str,
    pub footer: String,
    pub confirm: &'static str,
}

pub async fn submit_dialog(app: &App, params: Params) -> Result<SubmitDialog, RequestError> {
    let Some(ticket) = ticket(app, &params.key).await? else {
        return Err(RequestError::plain(format!("{} is gone.", params.key)));
    };
    let text = close::submit_text(&ticket, now_ms());
    Ok(SubmitDialog {
        title: text.title,
        intro: text.intro,
        note_label: "Note for the review summary (optional)",
        note_placeholder: "What the reviewer should know",
        footer: text.footer,
        confirm: "Submit for review",
    })
}

/// What `panel.submit` takes: the note for the review summary.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelSubmitParams")]
pub struct SubmitParams {
    pub key: String,
    pub note: String,
}

/// Submit for review: a refusal ("2 files are not committed…") shows inside the dialog.
pub async fn submit(app: &App, params: SubmitParams) -> Result<(), RequestError> {
    app.service
        .send(Request::TicketSubmitByUser(SubmitByUserParams {
            key: params.key,
            note: params.note,
        }))
        .await
        .map(|_| ())
}

/// Mark closed, once confirmed (REV-13).
pub async fn mark_closed(app: &App, params: Params) -> Result<(), RequestError> {
    let key = params.key;
    match app
        .service
        .send(Request::TicketMarkClosed(CloseParams {
            key: key.clone(),
            confirm: true,
        }))
        .await
    {
        Ok(_) => {
            let (title, text) = pr_dialog::marked_closed_toast(&key);
            app.notices.toast(NewToast::new(
                ToastLevel::Success,
                title,
                Some(text.to_owned()),
            ));
            Ok(())
        }
        Err(e) => {
            toast_error(app, format!("Mark closed of {key} failed"), &e);
            Err(e)
        }
    }
}

/// The unsent-text question (SES-07, d0048): Clear it and send, or Keep it and cancel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "PanelUnsent")]
pub struct Unsent {
    pub title: String,
    pub body: String,
    pub clear: &'static str,
}

impl Unsent {
    pub(super) fn of(prompt: &UnsentPrompt) -> Self {
        Self {
            title: prompt.title(),
            body: format!("{} {}.", prompt.explanation(), prompt.sending),
            clear: "Clear it and send",
        }
    }
}

/// How a send went: done, or the terminal has unsent text and the user is asked first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
#[ts(rename = "PanelSent")]
pub enum Sent {
    Done,
    Ask(Unsent),
}

/// What `panel.answer` takes: the open question, the picked number (Other… is the last), its text, and whether
/// the terminal's typed line is cleared first.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelAnswerParams")]
pub struct AnswerParams {
    pub key: String,
    pub question: String,
    pub pick: u32,
    #[serde(default)]
    pub other: String,
    #[serde(default)]
    pub clear: bool,
}

/// The choice an answer sends, by the options' own rules (`answer_options.rs`); `None` when there is nothing to send.
pub fn choice_of(ticket: &Ticket, params: &AnswerParams) -> Option<Choice> {
    let question = ticket.question.as_ref().filter(|q| q.id == params.question)?;
    let mut options = AnswerOptions::of(question)?;
    options.press(params.pick).then_some(())?;
    options.choice(&params.other)
}

/// Send answer (ATT-12): typed into the session as the options word it.
pub async fn answer(app: &App, params: AnswerParams) -> Result<Sent, RequestError> {
    let key = key_of(&params.key)?;
    let Some(ticket) = ticket(app, &params.key).await? else {
        return Err(RequestError::plain(format!("{key} is gone.")));
    };
    let Some(choice) = choice_of(&ticket, &params) else {
        return Err(RequestError::plain(format!(
            "{key}\u{2019}s question has changed"
        )));
    };
    match app
        .service
        .send(answer::request(&key, &choice, params.clear))
        .await
    {
        Ok(Reply::Delivery(delivery)) => Ok(match answer::outcome(&key, &choice, &delivery) {
            AnswerOutcome::Sent => Sent::Done,
            AnswerOutcome::Ask(prompt) => Sent::Ask(Unsent::of(&prompt)),
        }),
        Ok(other) => Err(unexpected("inbox.answer", &other)),
        Err(e) => {
            toast_error(app, format!("The answer to {key} wasn\u{2019}t sent"), &e);
            Err(e)
        }
    }
}

/// What `panel.send_to_fix` takes.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelSendToFixParams")]
pub struct SendToFix {
    pub key: String,
    #[serde(default)]
    pub clear: bool,
}

/// Send to agent to fix (GIT-04): the ticket returns to Dev and its agent rebases.
pub async fn send_to_fix(app: &App, params: SendToFix) -> Result<Sent, RequestError> {
    let key = key_of(&params.key)?;
    match app
        .service
        .send(Request::TicketSendToFix(SendToFixParams {
            key: params.key,
            clear_first: params.clear,
        }))
        .await
    {
        Ok(Reply::Returned(ReturnOutcome::NeedsConfirmation { .. })) => {
            Ok(Sent::Ask(Unsent::of(&UnsentPrompt::for_click(
                key,
                None,
                "The restack skill (rebase and resolve the conflicts)".to_owned(),
                Click::SendToFix,
            ))))
        }
        Ok(_) => Ok(Sent::Done),
        Err(e) => {
            toast_error(app, format!("{key} wasn\u{2019}t sent to its agent"), &e);
            Err(e)
        }
    }
}

/// What `panel.card` takes: a board card's button (T-0220) the panel carries out once it shows the ticket, and
/// whether the terminal's typed line is cleared first (Deliver now).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelCardParams")]
pub struct CardParams {
    pub key: String,
    pub kind: CardActionKind,
    #[serde(default)]
    pub clear: bool,
}

/// A card button that talks to the service, as the GPUI app's card buttons did: Deliver now (with the unsent-text
/// question), Resume, Retry resume, Retry start and Retry (SES-02, d0059), Reopen, Mark merged, and the validation
/// buttons (VAL-07, AMG-03, AMG-04). An answer option, Start from scratch, Push to PR and Mark closed are the page's.
pub async fn card(app: &App, params: CardParams) -> Result<Sent, RequestError> {
    let key = key_of(&params.key)?;
    let panel_action = |action| ActionParams {
        key: params.key.clone(),
        action,
    };
    let validation = |action| {
        Request::TicketValidationAction(ValidationActionParams {
            key: params.key.clone(),
            action,
            note: String::new(),
        })
    };
    use crewboard_core::domain::ValidationAction as V;
    let (request, failed) = match params.kind {
        CardActionKind::Resume => return action(app, panel_action(Action::Resume)).await.map(|()| Sent::Done),
        CardActionKind::RetryResume => {
            return action(app, panel_action(Action::RetryResume)).await.map(|()| Sent::Done);
        }
        CardActionKind::RetryStart | CardActionKind::Retry => {
            return action(app, panel_action(Action::RetryStart)).await.map(|()| Sent::Done);
        }
        CardActionKind::Reopen => return action(app, panel_action(Action::Reopen)).await.map(|()| Sent::Done),
        CardActionKind::MarkMerged => {
            return action(app, panel_action(Action::MarkMerged)).await.map(|()| Sent::Done);
        }
        CardActionKind::DeliverNow => (
            strip::deliver_now_request(
                &key,
                params.clear.then_some(strip::Choice::Clear),
            )
            .ok_or_else(|| RequestError::plain("nothing to deliver"))?,
            format!("{key}\u{2019}s messages weren\u{2019}t delivered"),
        ),
        CardActionKind::RetryValidation => (
            validation(V::Retry),
            format!("{key}\u{2019}s validation can\u{2019}t be retried"),
        ),
        CardActionKind::RetryMerge => (validation(V::RetryMerge), format!("{key} can\u{2019}t be merged")),
        CardActionKind::MoveToReview => (
            validation(V::MoveToReview),
            format!("{key} can\u{2019}t be moved to Review"),
        ),
        other => {
            return Err(RequestError::plain(format!(
                "{other:?} is the page's (a dialog or the answer list)"
            )));
        }
    };
    match app.service.send(request).await {
        Ok(Reply::Delivery(Delivery::NeedsConfirmation { .. })) => {
            let waiting = ticket(app, &params.key)
                .await?
                .map_or(1, |t| t.flags.messages_waiting);
            Ok(Sent::Ask(Unsent::of(&UnsentPrompt::deliver_now(
                key, None, waiting,
            ))))
        }
        Ok(_) => Ok(Sent::Done),
        Err(e) => {
            toast_error(app, failed, &e);
            Err(e)
        }
    }
}

/// What `panel.reopen_parent` and `panel.remove_dependency` take.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "PanelParentParams")]
pub struct ParentParams {
    pub key: String,
    pub parent: String,
}

/// Reopen parent (CLS-04): the closed parent goes back to Backlog.
pub async fn reopen_parent(app: &App, params: ParentParams) -> Result<(), RequestError> {
    let parent = params.parent;
    match app
        .service
        .send(Request::TicketReopen(KeyParams {
            key: parent.clone(),
        }))
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            toast_error(app, format!("{parent} can\u{2019}t be reopened"), &e);
            Err(e)
        }
    }
}

/// Remove dependency, once confirmed.
pub async fn remove_dependency(app: &App, params: ParentParams) -> Result<(), RequestError> {
    match app
        .service
        .send(Request::DepRemove(DepParams {
            child: params.key,
            parent: params.parent,
        }))
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            toast_error(
                app,
                "The dependency wasn\u{2019}t removed".to_owned(),
                &e,
            );
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::board::questions::HumanQuestion;
    use crewboard_core::board::tickets::NewTicketInput;
    use crewboard_core::domain::Status;
    use crewboard_core::git::testing::TempRepos;
    use crewboard_core::board::projects::AddProject;
    use crewboard_core::session::SessionState;
    use crewboard_view::view_models::card_fixtures::ticket as fixture;
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    fn agent(state: SessionState) -> SessionInfo {
        SessionInfo {
            id: "s1".into(),
            kind: SessionKind::Agent,
            project: "SHOP".into(),
            ticket: Some("SHOP-19".parse().unwrap()),
            state,
            started_at: 0,
            ended_at: None,
            end_reason: None,
            claude_session_id: None,
            size: None,
        }
    }

    fn facts<'a>(
        ticket: &'a Ticket,
        tickets: &'a [Ticket],
        sessions: &'a [SessionInfo],
    ) -> Facts<'a> {
        Facts {
            ticket,
            tickets,
            sessions,
            conflict: None,
            has_log: false,
            pending: 0,
            usage: None,
            rounds: 3,
            now: 60_000,
            run: None,
        }
    }

    fn labels(view: &View) -> Vec<&str> {
        view.buttons.iter().map(|b| b.label.as_str()).collect()
    }

    #[test]
    fn the_header_and_bar_follow_the_state() {
        let mut working = fixture("SHOP-19", "Price table", Status::Working);
        working.flags.last_signal_at = Some(20_000);
        working.flags.tokens = Some(182_000);
        let all = vec![working.clone()];
        let sessions = [agent(SessionState::Running)];
        let view = view_of(&facts(&working, &all, &sessions));
        assert_eq!((view.key.as_str(), view.title.as_str()), ("SHOP-19", "Price table"));
        assert_eq!(labels(&view), ["Open terminal", "Run"]);
        assert_eq!(view.run, None, "no Run up, no pill");
        assert_eq!(view.signal.as_ref().unwrap().text, "40s ago");
        assert_eq!(view.tokens.as_ref().unwrap().text, "182k");
        assert_eq!(view.tabs[0].mark, Some(Mark::Live));
        assert_eq!(view.tabs[5].mark, Some(Mark::Note { text: "1".into() }));
        assert!(view.live_agent);
        assert_eq!(
            view.attach_command,
            format!("{} attach SHOP-19", crewboard_core::naming::COMMAND_NAME)
        );
        let ids: Vec<&str> = view.menu.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"copy-attach") && ids.contains(&close::CLOSE_TICKET));
        assert_eq!(view.base, "main");

        let mut unknown = fixture("SHOP-30", "New", Status::Backlog);
        unknown.flags.tokens_unknown = true;
        let view = view_of(&facts(&unknown, std::slice::from_ref(&unknown), &[]));
        assert_eq!(labels(&view), ["Start (to Dev)", "Edit"]);
        assert_eq!(view.tokens.unwrap().tooltip, Some(TOKENS_UNKNOWN));
        assert_eq!(view.branch, None);
        assert!(!view.live_agent);
    }

    #[test]
    fn previous_and_next_walk_the_column() {
        let mut a = fixture("SHOP-1", "a", Status::Backlog);
        a.created_at = 1;
        let mut b = fixture("SHOP-2", "b", Status::Backlog);
        b.created_at = 2;
        let all = vec![a.clone(), b.clone()];
        let view = view_of(&facts(&a, &all, &[]));
        assert_eq!((view.previous, view.next), (None, Some("SHOP-2".into())));
    }

    #[test]
    fn a_queued_child_names_its_parent() {
        let parent = fixture("SHOP-18", "Checkout API", Status::ReadyForReview);
        let mut child = fixture("SHOP-20", "Checkout page", Status::Queued);
        child.depends_on = vec![parent.key.clone()];
        let all = vec![parent, child.clone()];
        let view = view_of(&facts(&child, &all, &[]));
        assert_eq!(labels(&view), ["Edit", "View parent SHOP-18"]);
        assert_eq!(view.parent.as_deref(), Some("SHOP-18"));
        let notice = view.notice.unwrap();
        assert!(notice.text.contains("SHOP-18"), "{}", notice.text);
        assert!(!notice.parent_closed && notice.choices.is_empty());
    }

    #[test]
    fn asks_you_with_options_shows_the_list() {
        let mut asks = fixture("SHOP-21", "Order confirmation email", Status::AsksYou);
        asks.question = Some(HumanQuestion {
            id: "q1".into(),
            text: "Which email provider?".into(),
            options: vec!["Postmark".into(), "SES".into(), "Resend".into()],
            recommended: Some(1),
            asked_at: 0,
        });
        let view = view_of(&facts(&asks, std::slice::from_ref(&asks), &[]));
        let answer = view.answer.unwrap();
        assert_eq!(answer.rows.len(), 4);
        assert!(answer.rows[0].recommended && answer.rows[0].picked);
        assert!(answer.rows[3].other);

        let pick = |pick: u32, other: &str| AnswerParams {
            key: "SHOP-21".into(),
            question: "q1".into(),
            pick,
            other: other.into(),
            clear: false,
        };
        assert_eq!(choice_of(&asks, &pick(2, "")), Some(Choice::Option(2)));
        assert_eq!(choice_of(&asks, &pick(4, "  ")), None, "Other… needs text");
        assert_eq!(
            choice_of(&asks, &pick(4, "Mailgun")),
            Some(Choice::Other("Mailgun".into()))
        );
        assert_eq!(choice_of(&asks, &pick(9, "")), None);
        let stale = AnswerParams {
            question: "old".into(),
            ..pick(1, "")
        };
        assert_eq!(choice_of(&asks, &stale), None, "a newer question wins");

        asks.flags.your_turn = true;
        let view = view_of(&facts(&asks, std::slice::from_ref(&asks), &[]));
        assert_eq!(view.answer, None, "focused in its terminal: answered there");
    }

    #[test]
    fn conflicts_show_in_review_with_the_fix() {
        let mut review = fixture("SHOP-18", "Checkout API", Status::ReadyForReview);
        let conflict = Conflict {
            target: "main".into(),
            files: vec!["a.ts".into(), "b.ts".into()],
            reason: crewboard_core::board::conflicts::ConflictReason::Merge,
        };
        let all = vec![review.clone()];
        let with = |t: &Ticket| {
            view_of(&Facts {
                conflict: Some(&conflict),
                ..facts(t, &all, &[])
            })
        };
        assert_eq!(with(&review).conflict, None, "no flag: no strip");
        review.flags.conflicts_with = Some("main".into());
        let strip = with(&review).conflict.unwrap();
        assert_eq!(strip.line, "Conflicts with main \u{b7} 2 files (a.ts, b.ts)");
        assert_eq!(strip.fix, Some("Send to agent to fix"));
    }

    #[test]
    fn a_running_run_shows_its_pill_and_a_failed_one_marks_its_tab() {
        use crewboard_core::terminals::run::{RunInfo, RunState};
        let t = fixture("SHOP-19", "Price table", Status::Working);
        let mut run = RunInfo {
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            key: t.key.clone(),
            title: t.title.clone(),
            state: RunState::Running,
            port: Some(4020),
            command: "pnpm dev --port $PORT".into(),
            open_url: Some("http://localhost:$PORT".into()),
            session: None,
            started_at: None,
            ended_at: None,
            exit_code: None,
            stopped_by: None,
        };
        let all = vec![t.clone()];
        let view = view_of(&Facts {
            run: Some(&run),
            ..facts(&t, &all, &[])
        });
        assert_eq!(
            view.run,
            Some(RunPill {
                port: ":4020".into(),
                url: Some("http://localhost:4020".into()),
                open_hint: "Open localhost:4020".into(),
                stop_hint: "Stop",
            })
        );
        assert_eq!(view.tabs[2].mark, Some(Mark::Note { text: ":4020".into() }));
        run.state = RunState::FailedToStart;
        let view = view_of(&Facts {
            run: Some(&run),
            ..facts(&t, &all, &[])
        });
        assert_eq!(view.run, None);
        assert_eq!(view.tabs[2].mark, Some(Mark::Problem));
    }

    #[test]
    fn send_back_counts_the_pending_comments() {
        let review = fixture("SHOP-18", "Checkout API", Status::ReadyForReview);
        let all = vec![review.clone()];
        let view = view_of(&Facts {
            pending: 2,
            ..facts(&review, &all, &[])
        });
        assert_eq!(labels(&view), ["Approve \u{b7} Local merge", "Send back (2)"]);
        assert_eq!(labels(&view_of(&facts(&review, &all, &[]))), ["Approve \u{b7} Local merge", "Send back"]);
    }

    /// *Row · validation (0063)*: the round in the header, the token split, Answer the agent's box, the Auto-merge
    /// stop message and the Approve caret.
    #[test]
    fn the_validation_states_carry_their_extras() {
        use crewboard_core::domain::ValidationRound;
        use crewboard_core::usage::Tokens;
        let mut t = fixture("SHOP-29", "Coupon field", Status::Validating);
        t.flags.validating_round = Some(ValidationRound { round: 1, of: 3 });
        let tokens = |input| {
            Some(Tokens {
                input,
                ..Tokens::default()
            })
        };
        let usage = TicketUsage {
            id: t.id.clone(),
            tokens: tokens(67_000),
            validation: tokens(9_000),
            rounds: 0,
        };
        let view = view_of(&Facts {
            usage: Some(&usage),
            ..facts(&t, std::slice::from_ref(&t), &[])
        });
        assert_eq!(view.pill.label, "Validating\u{2026} \u{b7} round 1 of 3");
        assert_eq!(
            view.token_split.as_deref(),
            Some("agent 58k \u{b7} validation 9k (1 round)")
        );
        assert_eq!(labels(&view), ["Open terminal \u{b7} Validator", "Run"]);
        assert_eq!(view.menu[0].label, "Stop validation");
        assert_eq!((view.answer_agent, view.approve_more), (None, None));

        let mut failed = fixture("SHOP-32", "Address autocomplete", Status::ValidationFailed);
        failed.validation_round = 3;
        let view = view_of(&facts(&failed, std::slice::from_ref(&failed), &[]));
        assert_eq!(view.pill.label, "Validation failed 3 times");
        assert_eq!(
            labels(&view),
            ["Move to Review anyway", "Give it another round", "Answer the agent\u{2026}"]
        );
        assert!(view.answer_agent.unwrap().footer.ends_with("(3 new rounds)."));

        let mut stopped = fixture("SHOP-40.2", "Wishlist page", Status::AutoMergeStopped);
        stopped.auto_merge_stop = Some(crewboard_core::domain::AutoMergeStop::CheckoutDirty);
        stopped.signal = Some(crewboard_core::board::tickets::AgentSignal {
            text: Some("shop-web has uncommitted changes on main. Clean up and retry.".into()),
            ..Default::default()
        });
        let view = view_of(&facts(&stopped, std::slice::from_ref(&stopped), &[]));
        assert_eq!(labels(&view), ["Retry merge", "Move to Review"]);
        assert_eq!(
            view.stop_message.as_deref(),
            Some("shop-web has uncommitted changes on main. Clean up and retry.")
        );

        let review = fixture("SHOP-18", "Checkout API", Status::ReadyForReview);
        let view = view_of(&facts(&review, std::slice::from_ref(&review), &[]));
        assert_eq!(view.approve_more, Some("Approve choices"));
    }

    #[test]
    fn start_interrupted_and_pr_open_have_their_extras() {
        let t = fixture("SHOP-28", "Cart badge count", Status::StartInterrupted);
        let view = view_of(&Facts {
            has_log: true,
            ..facts(&t, std::slice::from_ref(&t), &[])
        });
        assert_eq!(labels(&view), ["Retry start"]);
        let p = view.start_problem.unwrap();
        assert!(p.has_log && p.retry_does.len() == 3);

        let mut pr = fixture("SHOP-26", "Order history", Status::PrOpen);
        pr.flags.pr_number = Some(481);
        let view = view_of(&facts(&pr, std::slice::from_ref(&pr), &[]));
        assert_eq!(labels(&view), ["Open PR in browser", "Mark merged", "Mark closed"]);
        assert_eq!(view.mark_closed.unwrap().title, "Mark PR #481 as closed?");
    }

    #[test]
    fn the_close_confirmation_names_the_agent_and_the_dependents() {
        let t = fixture("SHOP-19", "Price table", Status::Working);
        let mut child = fixture("SHOP-20", "Checkout page", Status::Queued);
        child.depends_on = vec![t.key.clone()];
        let all = vec![t.clone(), child];
        let running = close_dialog_of(&t, &all, true);
        assert_eq!(running.title, "Close SHOP-19 Price table without merging?");
        assert_eq!(running.confirm, "Stop agent & close");
        assert_eq!(running.lines.len(), 2);
        assert!(running.lines[0].agent && !running.lines[1].agent);
        assert!(running.lines[1].text.starts_with("SHOP-20 Checkout page depends on it"));
        let idle = close_dialog_of(&t, &all, false);
        assert_eq!(idle.confirm, "Close ticket");
        assert!(idle.lines.iter().all(|l| !l.agent));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_panel_reads_and_changes_a_real_ticket() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let project = match h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        {
            Reply::Project(p) => p,
            other => panic!("{other:?}"),
        };
        let created = match h
            .app
            .service
            .send(Request::TicketCreate(NewTicketInput {
                project: project.key.clone(),
                kind: Default::default(),
                title: "Price table".into(),
                body: String::new(),
                priority: None,
                epic: None,
                base_branch: None,
                external_id: None,
            }))
            .await
            .unwrap()
        {
            Reply::Ticket(t) => t,
            other => panic!("{other:?}"),
        };
        let key = created.key.to_string();

        let load = table(&h.app, "panel.view", json!({ "key": key })).await.unwrap();
        assert_eq!(load["view"]["title"], "Price table");
        assert_eq!(load["view"]["buttons"][0]["action"], "start");
        assert_eq!(load["view"]["tabs"][0]["tab"], "terminal");

        let item = load["view"]["priority_menu"][0]["id"].as_str().unwrap().to_owned();
        table(&h.app, "panel.priority", json!({ "key": key, "item": item }))
            .await
            .unwrap();
        table(&h.app, "panel.menu", json!({ "key": key, "item": "pin" }))
            .await
            .unwrap();
        let load = table(&h.app, "panel.view", json!({ "key": key })).await.unwrap();
        assert_eq!(load["view"]["priority"]["number"], 1);
        assert!(
            load["view"]["menu"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["id"] == "unpin")
        );

        let dialog = table(&h.app, "panel.close_dialog", json!({ "key": key }))
            .await
            .unwrap();
        assert_eq!(dialog["confirm"], "Close ticket");
        table(&h.app, "panel.close", json!({ "key": key })).await.unwrap();
        let load = table(&h.app, "panel.view", json!({ "key": key })).await.unwrap();
        assert_eq!(load["view"]["buttons"][0]["action"], "reopen");
        table(&h.app, "panel.action", json!({ "key": key, "action": "reopen" }))
            .await
            .unwrap();
        let load = table(&h.app, "panel.view", json!({ "key": key })).await.unwrap();
        assert_eq!(load["view"]["buttons"][0]["action"], "start");

        assert!(
            table(&h.app, "panel.action", json!({ "key": key, "action": "edit" }))
                .await
                .is_err(),
            "Edit is the page's"
        );

        // A card's Reopen runs through the panel; an answer option stays the page's.
        table(&h.app, "panel.close", json!({ "key": key })).await.unwrap();
        let sent = table(&h.app, "panel.card", json!({ "key": key, "kind": "reopen" }))
            .await
            .unwrap();
        assert_eq!(sent["outcome"], "done");
        let load = table(&h.app, "panel.view", json!({ "key": key })).await.unwrap();
        assert_eq!(load["view"]["buttons"][0]["action"], "start");
        assert!(
            table(
                &h.app,
                "panel.card",
                json!({ "key": key, "kind": { "pick_option": 2 } })
            )
            .await
            .is_err(),
            "an answer option is the page's"
        );
        let gone = table(&h.app, "panel.view", json!({ "key": "SHOP-99" }))
            .await
            .unwrap();
        assert_eq!(gone["view"], Value::Null);
        assert_eq!(gone["gone"], "SHOP-99 is gone.");
        h.service.stop_all().await;
    }
}
