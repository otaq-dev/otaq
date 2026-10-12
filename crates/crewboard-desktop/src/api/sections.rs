//! `sections.*`: the side panel's sections and its History tab (UI-04, DEP-05, REC-03, MSG-01, MSG-03, MSG-06,
//! EPC-01, EPC-04, EPC-06, EPC-07, TKT-07; d0045, d0030, d0039, d0044, d0059; *05 · Ticket side panel*). The
//! **Stack** list with parents, children and "waits for" lines (`stack.rs`); the **Messages** between linked agents
//! with their states (`messages.rs`), answerable when one went to the inbox; the **History** tab, sessions with
//! their Claude session IDs and events newest first (`history.rs`); the **epic panel**, children and progress
//! (`epic_review.rs`); and the epic dialogs, Move to epic… and Close epic… (`epic_actions.rs`). Each view is `null`
//! when its section has nothing to show for the ticket.

use std::time::{SystemTime, UNIX_EPOCH};

use crewboard_core::board::epics::progress_of;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{
    AnswerParams, CloseParams, KeyParams, MoveToEpicParams, ProjectParams, Reply, Request,
    SessionListParams,
};
use crewboard_core::session::SessionInfo;
use crewboard_view::icons::IconName;
use crewboard_view::view_models::clock::local_hhmm;
use crewboard_view::view_models::epic_actions::{self, EpicChoice};
use crewboard_view::view_models::epic_review;
use crewboard_view::view_models::history::{self, HistoryRow};
use crewboard_view::view_models::messages::{self, Line, Thread, Tone};
use crewboard_view::view_models::stack;
use crewboard_view::view_models::status::PillView;
use crewboard_view::view_models::toasts::{NewToast, ToastLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "sections.stack" => reply(&stack_view(app, parse(params)?).await?),
        "sections.messages" => reply(&messages_view(app, parse(params)?).await?),
        "sections.answer_message" => reply(&answer_message(app, parse(params)?).await?),
        "sections.history" => reply(&history_view(app, parse(params)?).await?),
        "sections.epic" => reply(&epic_view(app, parse(params)?).await?),
        "sections.move_dialog" => reply(&move_dialog(app, parse(params)?).await?),
        "sections.move_to_epic" => reply(&move_to_epic(app, parse(params)?).await?),
        "sections.close_epic_dialog" => reply(&close_epic_dialog(app, parse(params)?).await?),
        "sections.close_epic" => reply(&close_epic(app, parse(params)?).await?),
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

fn toast(app: &App, level: ToastLevel, title: String, text: Option<String>) {
    app.notices.toast(NewToast::new(level, title, text));
}

/// What every `sections.*` view takes: the ticket the panel shows.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "SectionParams")]
pub struct Params {
    pub key: String,
}

async fn send_key(app: &App, request: Request, what: &str) -> Result<Reply, RequestError> {
    app.service
        .send(request)
        .await
        .map_err(|e| RequestError::plain(format!("{what}: {}", e.message)))
}

/// The ticket, or `None` when it is gone.
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

/// The ticket with the rest of its project; `None` when it is gone.
async fn with_project(app: &App, key: &str) -> Result<Option<(Ticket, Vec<Ticket>)>, RequestError> {
    let Some(ticket) = ticket(app, key).await? else {
        return Ok(None);
    };
    let all = tickets(app, &ticket.key.project.to_string()).await?;
    Ok(Some((ticket, all)))
}

async fn found(app: &App, key: &str) -> Result<(Ticket, Vec<Ticket>), RequestError> {
    with_project(app, key)
        .await?
        .ok_or_else(|| RequestError::plain(format!("{key} is gone.")))
}

// ---------------------------------------------------------------------------------------------------------------
// Stack (DEP-05)

/// One line of the Stack list: indented by its depth, the open ticket marked, a Queued ticket's waiting text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "StackRowView")]
pub struct StackRowView {
    pub key: String,
    pub title: String,
    pub depth: u32,
    pub current: bool,
    pub waits: Option<String>,
    pub pill: PillView,
}

/// The Stack section between the header and the tabs: open with its list when the ticket has links, else one
/// line saying it has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "StackView")]
pub struct StackView {
    pub title: &'static str,
    /// "Depends on SHOP-18, SHOP-19 · SHOP-21 waits on it", or "No dependencies · nothing waits on SHOP-12".
    pub summary: String,
    /// "6 tickets": the list's rows.
    pub count: String,
    pub rows: Vec<StackRowView>,
    /// Opens the Ticket tab's form at Depends on.
    pub add_dependency: &'static str,
    pub show_hint: &'static str,
    pub hide_hint: &'static str,
}

pub fn stack_of(ticket: &Ticket, tickets: &[Ticket]) -> StackView {
    let rows: Vec<StackRowView> = stack::stack_rows(&ticket.key, tickets)
        .into_iter()
        .map(|r| StackRowView {
            key: r.key.to_string(),
            title: r.title,
            depth: u32::try_from(r.depth).unwrap_or(u32::MAX),
            current: r.current,
            waits: r.waits,
            pill: PillView::of(r.status),
        })
        .collect();
    let count = match rows.len() {
        1 => "1 ticket".to_owned(),
        n => format!("{n} tickets"),
    };
    StackView {
        title: "Stack",
        summary: stack::summary(&ticket.key, tickets),
        count,
        rows,
        add_dependency: "+ Add dependency",
        show_hint: "Show the Stack",
        hide_hint: "Hide the Stack",
    }
}

pub async fn stack_view(app: &App, params: Params) -> Result<Option<StackView>, RequestError> {
    Ok(with_project(app, &params.key)
        .await?
        .map(|(ticket, all)| stack_of(&ticket, &all)))
}

// ---------------------------------------------------------------------------------------------------------------
// Messages (MSG-01, MSG-03, MSG-06; d0030)

/// How a message's status line looks (*05 → Message states*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "MessageTone")]
pub enum MessageTone {
    Muted,
    Delivered,
    Done,
    Attention,
}

impl From<Tone> for MessageTone {
    fn from(tone: Tone) -> Self {
        match tone {
            Tone::Muted => Self::Muted,
            Tone::Delivered => Self::Delivered,
            Tone::Done => Self::Done,
            Tone::Attention => Self::Attention,
        }
    }
}

/// "SHOP-40.2 → SHOP-40.1 · 10:42", the text and its status; a `null` end is the user ("you").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "MessageLineView")]
pub struct LineView {
    pub from: Option<String>,
    pub to: Option<String>,
    pub time: String,
    pub text: String,
    pub status: String,
    pub icon: IconName,
    pub tone: MessageTone,
}

impl From<Line> for LineView {
    fn from(l: Line) -> Self {
        Self {
            from: l.from.map(|k| k.to_string()),
            to: l.to.map(|k| k.to_string()),
            time: l.time,
            text: l.text,
            status: l.status,
            icon: l.icon,
            tone: l.tone.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "MessageThreadView")]
pub struct ThreadView {
    pub id: String,
    pub question: LineView,
    pub answer: Option<LineView>,
    /// Sent to the user's inbox: answered here.
    pub answerable: bool,
}

impl From<Thread> for ThreadView {
    fn from(t: Thread) -> Self {
        Self {
            id: t.id,
            question: t.question.into(),
            answer: t.answer.map(Into::into),
            answerable: t.answerable,
        }
    }
}

/// The Messages section; `null` when the ticket has no messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "MessagesView")]
pub struct MessagesView {
    pub title: &'static str,
    /// "1 thread · linked tickets only".
    pub heading: String,
    pub threads: Vec<ThreadView>,
    pub you: &'static str,
    pub answer: &'static str,
    pub placeholder: &'static str,
    pub send: &'static str,
}

pub fn messages_of(threads: Vec<Thread>) -> Option<MessagesView> {
    (!threads.is_empty()).then(|| MessagesView {
        title: "Messages",
        heading: messages::heading(&threads),
        threads: threads.into_iter().map(Into::into).collect(),
        you: "you",
        answer: "Answer",
        placeholder: "Type your answer",
        send: "Send",
    })
}

pub async fn messages_view(app: &App, params: Params) -> Result<Option<MessagesView>, RequestError> {
    let Ok(key) = params.key.parse::<TicketKey>() else {
        return Ok(None);
    };
    let k = || KeyParams {
        key: params.key.clone(),
    };
    let list = match app.service.send(Request::TicketMessages(k())).await {
        Ok(Reply::Messages(list)) => list,
        Ok(other) => return Err(unexpected("ticket.messages", &other)),
        Err(e) if e.not_found => return Ok(None),
        Err(e) => return Err(e),
    };
    let answers = match send_key(app, Request::TicketHumanAnswers(k()), "ticket.human_answers").await? {
        Reply::HumanAnswers(list) => list,
        other => return Err(unexpected("ticket.human_answers", &other)),
    };
    Ok(messages_of(messages::all_threads(
        &list,
        &answers,
        &key,
        now_ms(),
    )))
}

/// What `sections.answer_message` takes: a question that went to the inbox, and the typed answer.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "AnswerMessageParams")]
pub struct AnswerMessage {
    pub id: String,
    pub answer: String,
}

/// The answer to a question sent to the inbox (MSG-06); a refusal toasts and rejects, so the box stays open.
pub async fn answer_message(app: &App, params: AnswerMessage) -> Result<(), RequestError> {
    let answer = params.answer.trim().to_owned();
    if answer.is_empty() {
        return Err(RequestError::plain("Type an answer first."));
    }
    match app
        .service
        .send(Request::MessageAnswer(AnswerParams {
            id: params.id,
            answer,
        }))
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            toast(
                app,
                ToastLevel::Error,
                "The answer wasn\u{2019}t sent".to_owned(),
                Some(e.message.clone()),
            );
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// History (REC-03; d0045)

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "HistoryRowView")]
pub struct HistoryRowView {
    pub title: String,
    pub detail: Option<String>,
    /// "09:42".
    pub when: String,
    pub duration: String,
    pub tokens: String,
    /// The full Claude session ID, for the copy button (`claude --resume <id>`).
    pub claude_session_id: Option<String>,
    pub is_session: bool,
}

impl From<HistoryRow> for HistoryRowView {
    fn from(r: HistoryRow) -> Self {
        Self {
            title: r.title,
            detail: r.detail,
            when: local_hhmm(r.at),
            duration: r.duration,
            tokens: r.tokens,
            claude_session_id: r.claude_session_id,
            is_session: r.is_session,
        }
    }
}

/// The History tab: sessions and events, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "HistoryView")]
pub struct HistoryView {
    /// EVENT, WHEN, DURATION, TOKENS.
    pub columns: [&'static str; 4],
    pub rows: Vec<HistoryRowView>,
    pub copy_hint: &'static str,
    pub copied: &'static str,
    pub empty: &'static str,
}

pub fn history_of(rows: Vec<HistoryRow>) -> HistoryView {
    HistoryView {
        columns: ["EVENT", "WHEN", "DURATION", "TOKENS"],
        rows: rows.into_iter().map(Into::into).collect(),
        copy_hint: "Copy Claude session ID",
        copied: "Copied",
        empty: "Nothing has happened to this ticket yet.",
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

pub async fn history_view(app: &App, params: Params) -> Result<Option<HistoryView>, RequestError> {
    let k = || KeyParams {
        key: params.key.clone(),
    };
    let events = match app.service.send(Request::TicketHistory(k())).await {
        Ok(Reply::History(events)) => events,
        Ok(other) => return Err(unexpected("ticket.history", &other)),
        Err(e) if e.not_found => return Ok(None),
        Err(e) => return Err(e),
    };
    let sessions = sessions(app, &params.key).await?;
    // Tokens are a nicety: the table still shows without them (USG-01's "—").
    let usage = match app.service.send(Request::UsageSessions(k())).await {
        Ok(Reply::SessionUsage(usage)) => usage,
        _ => Vec::new(),
    };
    Ok(Some(history_of(history::rows(&events, &sessions, &usage))))
}

// ---------------------------------------------------------------------------------------------------------------
// The epic panel (EPC-01, EPC-04; d0059)

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicChildView")]
pub struct ChildView {
    pub key: String,
    pub title: String,
    /// "⛓ SHOP-40.1".
    pub deps: Option<String>,
    pub pill: PillView,
    /// Merged or closed: drawn muted.
    pub finished: bool,
}

/// What the panel shows for an epic, above its tabs; `null` for a ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EpicView")]
pub struct EpicView {
    pub external_id: Option<String>,
    /// "1 / 4 merged".
    pub progress: String,
    pub merged: u32,
    pub total: u32,
    /// "crewboard/SHOP-40 · local".
    pub branch: String,
    /// "TICKETS · 3".
    pub heading: String,
    pub children: Vec<ChildView>,
    pub add: &'static str,
    /// "· new tickets get the next key, SHOP-40.4".
    pub next_key: Option<String>,
    pub note: String,
}

pub fn epic_of(epic: &Ticket, tickets: &[Ticket]) -> Option<EpicView> {
    if !epic.is_epic() {
        return None;
    }
    let header = epic_review::header(epic, tickets);
    let progress = progress_of(&epic.key, tickets);
    let finished = |key: &TicketKey| {
        tickets
            .iter()
            .find(|t| &t.key == key)
            .is_some_and(|t| t.column == crewboard_core::domain::Column::Done)
    };
    Some(EpicView {
        external_id: header.external_id,
        progress: header.progress,
        merged: progress.merged,
        total: progress.total,
        branch: header.branch,
        heading: format!("TICKETS \u{b7} {}", header.children.len()),
        children: header
            .children
            .into_iter()
            .map(|c| ChildView {
                finished: finished(&c.key),
                key: c.key.to_string(),
                title: c.title,
                deps: c.deps,
                pill: PillView::of(c.status),
            })
            .collect(),
        add: "Add ticket to epic",
        next_key: header.next_key,
        note: header.note,
    })
}

pub async fn epic_view(app: &App, params: Params) -> Result<Option<EpicView>, RequestError> {
    Ok(with_project(app, &params.key)
        .await?
        .and_then(|(ticket, all)| epic_of(&ticket, &all)))
}

// ---------------------------------------------------------------------------------------------------------------
// Move to epic… (EPC-07, TKT-07; d0044) and Close epic… (EPC-06; d0033)

/// An epic the ticket can move into, with the dialog's texts for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "MoveChoiceView")]
pub struct MoveChoiceView {
    pub key: String,
    pub title: String,
    pub progress: String,
    pub new_key: String,
    /// "Move SHOP-51 Wishlist share link to an epic".
    pub heading: String,
    /// "· the old key SHOP-51 still works".
    pub alias_note: String,
    /// The base change, when the ticket had another base (*08 → Move to epic · base changes*).
    pub base_note: Option<String>,
    /// "Move to SHOP-40".
    pub confirm: String,
}

/// The Move to epic… dialog: the open epics to pick from (the first picked), or why it can't move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "MoveDialogView")]
pub struct MoveDialogView {
    pub old_key: String,
    pub choices: Vec<MoveChoiceView>,
    pub disk_note: &'static str,
    /// Set when the ticket can't move (it has been to Dev, it is in an epic, or no epic is open).
    pub refusal: Option<String>,
    pub title: String,
}

pub fn move_dialog_of(ticket: &Ticket, tickets: &[Ticket]) -> MoveDialogView {
    let choices: Vec<EpicChoice> = epic_actions::epic_choices(tickets);
    let refusal = if !epic_actions::can_move_to_epic(ticket) {
        Some(format!(
            "{} can\u{2019}t move: only a ticket that has never been to Dev, and isn\u{2019}t in an epic, moves to one.",
            ticket.key
        ))
    } else if choices.is_empty() {
        Some(format!("{} has no open epic to move into.", ticket.key.project))
    } else {
        None
    };
    MoveDialogView {
        old_key: ticket.key.to_string(),
        title: format!("Move {} {} to an epic", ticket.key, ticket.title),
        choices: choices
            .iter()
            .map(|c| {
                let preview = epic_actions::move_preview(ticket, c, tickets);
                MoveChoiceView {
                    key: c.key.to_string(),
                    title: c.title.clone(),
                    progress: c.progress.clone(),
                    new_key: c.new_key.to_string(),
                    heading: preview.title,
                    alias_note: preview.alias_note,
                    base_note: preview.base_note,
                    confirm: preview.confirm,
                }
            })
            .collect(),
        disk_note: "No branch or worktree exists yet, so nothing on disk is renamed.",
        refusal,
    }
}

pub async fn move_dialog(app: &App, params: Params) -> Result<MoveDialogView, RequestError> {
    let (ticket, all) = found(app, &params.key).await?;
    Ok(move_dialog_of(&ticket, &all))
}

/// What `sections.move_to_epic` takes.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "MoveToEpicParams")]
pub struct MoveTo {
    pub key: String,
    pub epic: String,
}

/// Moves the ticket; answers its new key ("SHOP-40.4"). A refusal rejects and stays in the dialog.
pub async fn move_to_epic(app: &App, params: MoveTo) -> Result<String, RequestError> {
    let old = params.key.clone();
    match app
        .service
        .send(Request::EpicMoveTo(MoveToEpicParams {
            key: params.key,
            epic: params.epic,
        }))
        .await?
    {
        Reply::Ticket(t) => {
            let new = t.key.to_string();
            toast(
                app,
                ToastLevel::Success,
                format!("Moved {old} \u{2192} {new}"),
                None,
            );
            Ok(new)
        }
        other => Err(unexpected("epic.move_to", &other)),
    }
}

/// What the Close epic… overlay takes.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "EpicParams")]
pub struct EpicParams {
    pub epic: String,
}

/// The Close epic confirmation (*Epic dialogs → Close epic · confirm*): the agents it stops, the tickets it also
/// closes, the ones that stay merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CloseEpicDialogView")]
pub struct CloseEpicDialogView {
    pub title: String,
    pub agents: Vec<String>,
    pub also: Option<String>,
    pub note: &'static str,
    pub confirm: String,
}

pub async fn close_epic_dialog(
    app: &App,
    params: EpicParams,
) -> Result<CloseEpicDialogView, RequestError> {
    let (epic, all) = found(app, &params.epic).await?;
    let plan = match app
        .service
        .send(Request::EpicClosePlan(KeyParams {
            key: params.epic.clone(),
        }))
        .await?
    {
        Reply::ClosePlan(plan) => plan,
        other => return Err(unexpected("epic.close_plan", &other)),
    };
    let text = epic_actions::close_dialog(&epic, &plan, &all);
    Ok(CloseEpicDialogView {
        title: text.title,
        agents: text.agents,
        also: text.also,
        note: text.note,
        confirm: text.confirm,
    })
}

/// Closes the epic and its unfinished children; a refusal rejects and stays in the dialog.
pub async fn close_epic(app: &App, params: EpicParams) -> Result<(), RequestError> {
    let key = params.epic;
    app.service
        .send(Request::EpicClose(CloseParams {
            key: key.clone(),
            confirm: true,
        }))
        .await?;
    toast(
        app,
        ToastLevel::Success,
        format!("Closed epic {key}"),
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crewboard_core::board::projects::AddProject;
    use crewboard_core::board::tickets::NewTicketInput;
    use crewboard_core::domain::{Kind, Status};
    use crewboard_core::git::testing::TempRepos;
    use crewboard_view::view_models::card_fixtures::ticket as fixture;
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    fn key(text: &str) -> TicketKey {
        text.parse().unwrap()
    }

    #[test]
    fn the_stack_lists_the_chain_and_marks_the_open_ticket() {
        let parent = fixture("SHOP-18", "Checkout API", Status::ReadyForReview);
        let mut child = fixture("SHOP-20", "Checkout page", Status::Queued);
        child.depends_on = vec![key("SHOP-18")];
        let lone = fixture("SHOP-12", "Footer", Status::Backlog);
        let all = vec![parent, child.clone(), lone.clone()];

        let view = stack_of(&child, &all);
        assert_eq!(view.summary, "Depends on SHOP-18");
        assert_eq!(view.count, "2 tickets");
        let rows: Vec<_> = view.rows.iter().map(|r| (r.key.as_str(), r.depth, r.current)).collect();
        assert_eq!(rows, [("SHOP-18", 0, false), ("SHOP-20", 1, true)]);

        let none = stack_of(&lone, &all);
        assert!(none.rows.is_empty());
        assert_eq!(none.summary, "No dependencies \u{b7} nothing waits on SHOP-12");
    }

    #[test]
    fn no_messages_no_section_and_history_has_its_columns() {
        assert_eq!(messages_of(Vec::new()), None);
        let history = history_of(Vec::new());
        assert_eq!(history.columns, ["EVENT", "WHEN", "DURATION", "TOKENS"]);
        assert_eq!(history.copy_hint, "Copy Claude session ID");
    }

    #[test]
    fn the_epic_panel_lists_children_and_move_is_for_never_started_tickets() {
        let mut epic = fixture("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        epic.next_child_number = Some(3);
        let mut api = fixture("SHOP-40.1", "Wishlist API", Status::Merged);
        api.epic = Some(key("SHOP-40"));
        let mut button = fixture("SHOP-40.2", "Wishlist button", Status::Working);
        button.epic = Some(key("SHOP-40"));
        button.depends_on = vec![key("SHOP-40.1")];
        let share = fixture("SHOP-51", "Wishlist share link", Status::Backlog);
        let mut working = fixture("SHOP-19", "Price table", Status::Working);
        working.started_at = Some(1);
        let all = vec![epic.clone(), api, button, share.clone(), working.clone()];

        assert_eq!(epic_of(&share, &all), None, "a ticket has no epic panel");
        let view = epic_of(&epic, &all).unwrap();
        assert_eq!(view.progress, "1 / 2 merged");
        assert_eq!((view.merged, view.total), (1, 2));
        assert_eq!(view.heading, "TICKETS \u{b7} 2");
        assert_eq!(view.children[1].deps.as_deref(), Some("\u{26d3} SHOP-40.1"));
        assert!(view.children[0].finished && !view.children[1].finished);

        let dialog = move_dialog_of(&share, &all);
        assert_eq!(dialog.refusal, None);
        assert_eq!(dialog.choices[0].new_key, "SHOP-40.3");
        assert_eq!(dialog.choices[0].confirm, "Move to SHOP-40");
        assert_eq!(dialog.choices[0].alias_note, "\u{b7} the old key SHOP-51 still works");
        assert!(move_dialog_of(&working, &all).refusal.is_some(), "it has been to Dev");
    }

    async fn create(h: &Harness, project: &str, title: &str, kind: Kind, epic: Option<&str>) -> String {
        match h
            .app
            .service
            .send(Request::TicketCreate(NewTicketInput {
                project: project.to_owned(),
                kind,
                title: title.into(),
                body: String::new(),
                priority: None,
                epic: epic.map(str::to_owned),
                base_branch: None,
                external_id: None,
            }))
            .await
            .unwrap()
        {
            Reply::Ticket(t) => t.key.to_string(),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn the_sections_read_and_change_real_tickets() {
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
            Reply::Project(p) => p.key,
            other => panic!("{other:?}"),
        };
        let epic = create(&h, &project, "Wishlist", Kind::Epic, None).await;
        let child = create(&h, &project, "Wishlist API", Kind::Ticket, Some(&epic)).await;
        let lone = create(&h, &project, "Share link", Kind::Ticket, None).await;

        let view = table(&h.app, "sections.epic", json!({ "key": epic })).await.unwrap();
        assert_eq!(view["children"][0]["key"], child.as_str());
        assert_eq!(view["progress"], "0 / 1 merged");
        let none = table(&h.app, "sections.epic", json!({ "key": lone })).await.unwrap();
        assert_eq!(none, Value::Null);

        let stack = table(&h.app, "sections.stack", json!({ "key": lone })).await.unwrap();
        assert_eq!(stack["rows"], json!([]));
        let messages = table(&h.app, "sections.messages", json!({ "key": lone })).await.unwrap();
        assert_eq!(messages, Value::Null);
        let history = table(&h.app, "sections.history", json!({ "key": lone })).await.unwrap();
        assert_eq!(history["rows"][0]["title"], "Created");

        let dialog = table(&h.app, "sections.move_dialog", json!({ "key": lone })).await.unwrap();
        assert_eq!(dialog["refusal"], Value::Null);
        let new_key = dialog["choices"][0]["new_key"].as_str().unwrap().to_owned();
        let moved = table(
            &h.app,
            "sections.move_to_epic",
            json!({ "key": lone, "epic": epic }),
        )
        .await
        .unwrap();
        assert_eq!(moved, new_key.as_str());
        let view = table(&h.app, "sections.epic", json!({ "key": epic })).await.unwrap();
        assert_eq!(view["children"].as_array().unwrap().len(), 2);

        let close = table(&h.app, "sections.close_epic_dialog", json!({ "epic": epic }))
            .await
            .unwrap();
        assert_eq!(close["confirm"], "Close epic & 2 tickets");
        table(&h.app, "sections.close_epic", json!({ "epic": epic }))
            .await
            .unwrap();
        let gone = table(&h.app, "sections.stack", json!({ "key": "SHOP-99" })).await.unwrap();
        assert_eq!(gone, Value::Null);
        h.service.stop_all().await;
    }
}
