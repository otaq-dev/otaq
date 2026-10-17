//! `inbox.*`: the Needs-you inbox and ⌘J (UI-06, ATT-06, ATT-07, ATT-08, ATT-09, ATT-10, ATT-12, USG-07, MSG-05, CLS-04;
//! d0016, d0024, d0042, d0045, d0061, d0030; *10 · Needs-you inbox*). The inbox model (resolved rows fading out) and
//! the ⌘J run live here, between requests, so the wording and the run's rules stay in `otaq-view`: `inbox.view`
//! draws the overlay, `inbox.answer` types an answer (or answers an agent's question sent to the user), `inbox.act`
//! runs a row's button, `inbox.next` / `inbox.run_bar` / `inbox.exit_run` drive the run, and
//! `inbox.resume_all_confirm` words Resume all's confirmation. Project agents waiting for the user are rows of group 1
//! (T-0241; PAG-07): their answer goes to `project_agent.answer`. An epic lead waiting for the user is one of them,
//! named after its epic and walked by ⌘J (LEAD-11); a lead's suggestion is confirmed or dismissed with `inbox.act`
//! (LEAD-08; the close / delete confirmation sends the confirm).

use std::time::Instant;

use otaq_core::attention::inbox::InboxList;
use otaq_core::board::needs_you::NeedsYouItem;
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Status, TicketKey};
use otaq_core::board::questions::{Choice, HumanQuestion};
use otaq_core::protocol::{
    AnswerParams, DepParams, IdParams, KeyParams, ProjectAgentAnswerParams, Reply, Request,
    UsageParams, ValidationActionParams,
};
use otaq_core::session::deliveries::Delivery;
use otaq_view::keymap::{KeyAction, Platform};
use otaq_view::view_models::answer::{self, AnswerOutcome};
use otaq_view::view_models::answer_options::AnswerOptions;
use otaq_view::view_models::inbox::{FADE, run_items, run_tab};
use otaq_view::view_models::inbox_page::{
    self as page, Action, AllClear, ResumeAllDialog, RunBarView,
};
use otaq_view::view_models::inbox_run::{InboxRun, NextStep};
use otaq_view::view_models::limit_banner;
use otaq_view::view_models::limits::running_total;
use otaq_view::view_models::toasts::{NewToast, ToastLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::panel::{Sent, Unsent};
use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "inbox.view" => reply(&view(app).await?),
        "inbox.answer" => reply(&answer(app, parse(params)?).await?),
        "inbox.act" => reply(&act(app, parse(params)?).await?),
        "inbox.resume_all_confirm" => reply(&resume_all_confirm(app).await?),
        "inbox.next" => reply(&next(app).await?),
        "inbox.run_bar" => reply(&run_bar(app, parse(params)?).await?),
        "inbox.exit_run" => {
            *app.inbox_run.lock().expect("inbox run") = None;
            Ok(Value::Null)
        }
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

fn key_of(text: &str) -> Result<TicketKey, RequestError> {
    text.parse()
        .map_err(|_| RequestError::plain(format!("not a ticket key: {text}")))
}

fn next_keys() -> String {
    KeyAction::NextInboxItem
        .keys(Platform::current())
        .unwrap_or_default()
}

fn toast_error(app: &App, title: String, error: &RequestError) {
    app.notices.toast(NewToast::new(
        ToastLevel::Error,
        title,
        Some(error.message.clone()),
    ));
}

async fn list(app: &App) -> Result<InboxList, RequestError> {
    match app.service.send(Request::InboxList).await? {
        Reply::Inbox(list) => Ok(*list),
        other => Err(unexpected("inbox.list", &other)),
    }
}

async fn ticket(app: &App, key: &str) -> Result<Option<Ticket>, RequestError> {
    match app
        .service
        .send(Request::TicketGet(KeyParams {
            key: key.to_owned(),
        }))
        .await
    {
        Ok(Reply::Ticket(t)) => Ok(Some(*t)),
        Ok(other) => Err(unexpected("ticket.get", &other)),
        Err(e) if e.not_found => Ok(None),
        Err(e) => Err(e),
    }
}

/// The closed parent of each Parent closed item, for Reopen parent / Remove dependency (CLS-04).
async fn closed_parents(
    app: &App,
    items: &[NeedsYouItem],
) -> Result<Vec<(TicketKey, TicketKey)>, RequestError> {
    let mut found = Vec::new();
    for item in items.iter().filter(|i| i.status == Status::ParentClosed) {
        let Some(child) = ticket(app, &item.key.to_string()).await? else {
            continue;
        };
        for parent in &child.depends_on {
            if ticket(app, &parent.to_string())
                .await?
                .is_some_and(|p| p.status == Status::ClosedNotMerged)
            {
                found.push((item.key.clone(), parent.clone()));
                break;
            }
        }
    }
    Ok(found)
}

async fn running(app: &App) -> u32 {
    match app.service.send(Request::ProjectList).await {
        Ok(Reply::Projects(list)) => running_total(&list as &[ProjectInfo]),
        _ => 0,
    }
}

/// What `inbox.view` answers: the inbox, and when to ask again while a resolved row fades out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "InboxLoad")]
pub struct Load {
    pub view: page::View,
    pub refresh_ms: Option<u64>,
}

pub async fn view(app: &App) -> Result<Load, RequestError> {
    let list = list(app).await?;
    let parents = closed_parents(app, &list.items).await?;
    let agents = list.project_agents.clone();
    let usage = match app
        .service
        .send(Request::UsageSummary(UsageParams { project: None }))
        .await
    {
        Ok(Reply::Usage(summary)) => Some(*summary),
        _ => None,
    };
    let running = running(app).await;
    // An epic lead's row names its epic: "Wishlist lead · SHOP-40 Wishlist".
    let mut epics: Vec<(TicketKey, String)> = Vec::new();
    for epic in agents.iter().filter_map(|a| a.epic.as_ref()) {
        if let Some(t) = ticket(app, &epic.to_string()).await? {
            epics.push((epic.clone(), t.title));
        }
    }
    let banner = usage.as_ref().and_then(|u| {
        limit_banner::banner(
            u.plan.as_ref(),
            u.limit_waiting,
            otaq_core::store::now_ms(),
        )
    });
    let (groups, total) = {
        let mut model = app.inbox.lock().expect("inbox");
        let now = Instant::now();
        model.update(list, now);
        model.expire(now);
        let groups = model.groups(&|k| {
            parents
                .iter()
                .find(|(child, _)| child == k)
                .map(|(_, p)| p.clone())
        });
        (groups, model.total() as u32)
    };
    let fading = groups.iter().any(|g| g.rows.iter().any(|r| r.resolved));
    Ok(Load {
        view: page::view(
            &groups,
            &agents,
            &|k| epics.iter().find(|(e, _)| e == k).map(|(_, t)| t.clone()),
            otaq_core::store::now_ms(),
            total,
            banner.as_ref(),
            running,
            &next_keys(),
        ),
        refresh_ms: fading.then_some(FADE.as_millis() as u64),
    })
}

/// What `inbox.answer` takes: the row's ticket (or its Project agent), its open question (or the agent's question
/// sent to the user), the picked option number (Other… is the last) or the typed text, and whether the terminal's
/// line is cleared first.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "InboxAnswerParams")]
pub struct AnswerRequest {
    pub key: String,
    /// A Project agent's row: the agent's id (`key` is ignored).
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub pick: Option<u32>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub clear: bool,
}

/// The answer an item takes: the picked option by the options' rules, or the typed text.
pub fn choice_of(item: &NeedsYouItem, params: &AnswerRequest) -> Option<Choice> {
    choice_for(item.question.as_ref(), params)
}

/// The answer to `question` (a ticket's or a Project agent's), `None` when the question has changed.
fn choice_for(question: Option<&HumanQuestion>, params: &AnswerRequest) -> Option<Choice> {
    if params.question.as_deref() != question.map(|q| q.id.as_str()) {
        return None;
    }
    match question.and_then(AnswerOptions::of) {
        Some(mut options) => {
            options.press(params.pick?).then_some(())?;
            options.choice(&params.text)
        }
        None => answer::free_text(&params.text),
    }
}

/// Send answer (ATT-08, ATT-12): typed into the session, or resuming it with the answer (d0045); with unsent text
/// the user is asked first (d0048). An agent's question sent to the user goes back to the asker (MSG-05).
pub async fn answer(app: &App, params: AnswerRequest) -> Result<Sent, RequestError> {
    if let Some(agent) = params.agent.clone() {
        return answer_agent(app, &agent, &params).await;
    }
    let key = key_of(&params.key)?;
    if let Some(id) = params.message.clone() {
        let text = params.text.trim().to_owned();
        if text.is_empty() {
            return Err(RequestError::plain("The answer is empty"));
        }
        return match app
            .service
            .send(Request::MessageAnswer(AnswerParams { id, answer: text }))
            .await
        {
            Ok(_) => Ok(Sent::Done),
            Err(e) => {
                toast_error(app, format!("The answer to {key} wasn\u{2019}t sent"), &e);
                Err(e)
            }
        };
    }
    let list = list(app).await?;
    let choice = list
        .items
        .iter()
        .find(|i| i.key == key && i.message.is_none())
        .and_then(|item| choice_of(item, &params))
        .ok_or_else(|| RequestError::plain(format!("{key}\u{2019}s question has changed")))?;
    match app
        .service
        .send(answer::request(&key, &choice, params.clear))
        .await
    {
        Ok(Reply::Delivery(delivery)) => Ok(match answer::outcome(&key, &choice, &delivery) {
            AnswerOutcome::Sent => Sent::Done,
            AnswerOutcome::Ask(prompt) => Sent::Ask(Unsent::of(&prompt)),
        }),
        Ok(_) => Ok(Sent::Done),
        Err(e) => {
            toast_error(app, format!("The answer to {key} wasn\u{2019}t sent"), &e);
            Err(e)
        }
    }
}

/// A Project agent's question answered in its row (PAG-07): typed into the agent's session; with unsent text the
/// user is asked first (d0048).
async fn answer_agent(
    app: &App,
    agent: &str,
    params: &AnswerRequest,
) -> Result<Sent, RequestError> {
    let list = list(app).await?;
    let waiting = list.project_agents.iter().find(|a| a.agent == agent);
    let title = waiting.map_or_else(|| page::agent_title(agent), page::waiting_title);
    let choice = waiting
        .and_then(|a| a.question.as_ref())
        .and_then(|q| choice_for(Some(q), params))
        .ok_or_else(|| RequestError::plain(format!("{title}\u{2019}s question has changed")))?;
    let request = Request::ProjectAgentAnswer(ProjectAgentAnswerParams {
        id: agent.to_owned(),
        choice: choice.clone(),
        clear: params.clear,
    });
    match app.service.send(request).await {
        Ok(Reply::Delivery(Delivery::NeedsConfirmation { .. })) => {
            let ask = page::agent_unsent(&title, &choice);
            Ok(Sent::Ask(Unsent {
                title: ask.title,
                body: ask.body,
                clear: "Clear it and send",
            }))
        }
        Ok(_) => Ok(Sent::Done),
        Err(e) => {
            toast_error(app, format!("The answer to {title} wasn\u{2019}t sent"), &e);
            Err(e)
        }
    }
}

/// What `inbox.act` takes: a row's button.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "InboxActParams")]
pub struct ActRequest {
    pub key: String,
    pub action: Action,
}

/// A row's button: Retry start, Reopen parent, Remove dependency, "This is the usage limit", Resume SHOP-17, and the
/// validation and Auto-merge items' actions (VAL-07, VAL-09, AMG-03, AMG-04; Answer the agent… with its note).
pub async fn act(app: &App, params: ActRequest) -> Result<(), RequestError> {
    let key = params.key;
    if let Some((action, note)) = params.action.validation() {
        let request = Request::TicketValidationAction(ValidationActionParams {
            key: key.clone(),
            action,
            note,
        });
        return match app.service.send(request).await {
            Ok(_) => Ok(()),
            Err(e) => {
                toast_error(app, format!("{key}: that didn\u{2019}t work"), &e);
                Err(e)
            }
        };
    }
    let request = match params.action {
        Action::RetryStart => Request::TicketRetryStart(KeyParams { key: key.clone() }),
        Action::ReopenParent { parent } => Request::TicketReopen(KeyParams { key: parent }),
        Action::RemoveDependency { parent } => Request::DepRemove(DepParams {
            child: key.clone(),
            parent,
        }),
        Action::UsageLimit => Request::UsageMarkLimit,
        Action::ResumeTarget { message } => Request::MessageResume(IdParams { id: message }),
        // A lead's suggestion (LEAD-08): the confirmation's confirm, or Dismiss.
        Action::ConfirmSuggestion { id, .. } => Request::SuggestionConfirm(IdParams { id }),
        Action::DismissSuggestion { id } => Request::SuggestionDismiss(IdParams { id }),
        // The page's: it opens the panel with the row's picker open (T-0345).
        Action::ChangeAgent { .. } => return Ok(()),
        // Sent above.
        Action::MoveToReviewAnyway
        | Action::AnotherRound
        | Action::AnswerAgent { .. }
        | Action::RetryValidation
        | Action::RetryMerge
        | Action::MoveToReview => return Ok(()),
    };
    match app.service.send(request).await {
        Ok(_) => Ok(()),
        Err(e) => {
            toast_error(app, format!("{key}: that didn\u{2019}t work"), &e);
            Err(e)
        }
    }
}

/// Resume all's confirmation (*10 → Resume all · confirm*), from the inbox as it is now; `null` when nothing is left
/// to resume. Its confirm sends `notices.resume_all`.
pub async fn resume_all_confirm(app: &App) -> Result<Option<ResumeAllDialog>, RequestError> {
    let list = list(app).await?;
    let mut model = app.inbox.lock().expect("inbox");
    model.update(list, Instant::now());
    Ok(model
        .resume_all_confirm()
        .as_ref()
        .map(page::resume_all_dialog))
}

/// What ⌘J does: open this ticket with its terminal focused, or show "All clear".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "InboxNext")]
pub enum Next {
    Open {
        project: String,
        key: String,
        tab: otaq_view::view_models::panel::Tab,
    },
    AllClear(AllClear),
}

/// ⌘J (ATT-07): the next item after the open one; the run starts at the first press and ends with "All clear".
pub async fn next(app: &App) -> Result<Next, RequestError> {
    let list = list(app).await?;
    let now = Instant::now();
    let agents = list.project_agents.clone();
    let items = {
        let mut model = app.inbox.lock().expect("inbox");
        model.update(list, now);
        run_items(model.items(), &agents)
    };
    let step = {
        let mut run = app.inbox_run.lock().expect("inbox run");
        let run = run.get_or_insert_with(|| InboxRun::new(now));
        match run.next(&items, now) {
            NextStep::Open(item) => Ok(item),
            NextStep::AllClear => Err(run.bar(&items, now)),
        }
    };
    match step {
        Ok(item) => Ok(Next::Open {
            tab: run_tab(&item),
            project: item.project,
            key: item.key.to_string(),
        }),
        Err(bar) => {
            // The run ends with "All clear"; the next ⌘J starts a new one.
            *app.inbox_run.lock().expect("inbox run") = None;
            let (handled, minutes) = match bar {
                otaq_view::view_models::inbox_run::RunBar::AllClear { handled, minutes } => {
                    (handled, minutes)
                }
                _ => (0, 0),
            };
            Ok(Next::AllClear(page::all_clear(
                handled,
                minutes,
                running(app).await,
            )))
        }
    }
}

/// What `inbox.run_bar` takes: the ticket open in the panel.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "InboxRunBarParams")]
pub struct RunBarRequest {
    pub key: String,
}

/// The run bar for the panel showing `key`: only during a run whose open item it is.
pub async fn run_bar(app: &App, params: RunBarRequest) -> Result<Option<RunBarView>, RequestError> {
    let key = key_of(&params.key)?;
    if app
        .inbox_run
        .lock()
        .expect("inbox run")
        .as_ref()
        .is_none_or(|r| r.current() != Some(&key))
    {
        return Ok(None);
    }
    let list = list(app).await?;
    let agents = list.project_agents.clone();
    let items = {
        let mut model = app.inbox.lock().expect("inbox");
        model.update(list, Instant::now());
        run_items(model.items(), &agents)
    };
    let mut run = app.inbox_run.lock().expect("inbox run");
    let Some(run) = run.as_mut().filter(|r| r.current() == Some(&key)) else {
        return Ok(None);
    };
    Ok(page::run_bar(&run.bar(&items, Instant::now()), &next_keys()))
}

#[cfg(test)]
mod tests {
    use otaq_core::board::needs_you::NeedsYouGroup;
    use otaq_core::board::questions::{Choice, HumanQuestion};
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    fn item(question: Option<HumanQuestion>) -> NeedsYouItem {
        NeedsYouItem {
            group: NeedsYouGroup::Family,
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            id: "t".into(),
            key: "SHOP-21".parse().unwrap(),
            title: "Order email".into(),
            status: Status::AsksYou,
            interrupted_from: None,
            question,
            reason: String::new(),
            since: 0,
            age_ms: 0,
            blocked: 0,
            message: None,
            suggestion: None,
            lead_note: None,
            change_agent: None,
        }
    }

    fn params(question: Option<&str>, pick: Option<u32>, text: &str) -> AnswerRequest {
        AnswerRequest {
            key: "SHOP-21".into(),
            agent: None,
            question: question.map(str::to_owned),
            message: None,
            pick,
            text: text.into(),
            clear: false,
        }
    }

    #[test]
    fn an_answer_follows_the_options_rules() {
        let q = HumanQuestion {
            id: "q1".into(),
            text: "Which?".into(),
            options: vec!["Postmark".into(), "SES".into()],
            recommended: Some(1),
            asked_at: 0,
            via_lead: None,
        };
        let with = item(Some(q));
        assert_eq!(
            choice_of(&with, &params(Some("q1"), Some(2), "")),
            Some(Choice::Option(2))
        );
        assert_eq!(
            choice_of(&with, &params(Some("q1"), Some(3), " Mailgun ")),
            Some(Choice::Other("Mailgun".into())),
            "Other… with its text"
        );
        assert_eq!(choice_of(&with, &params(Some("q1"), Some(3), " ")), None);
        assert_eq!(choice_of(&with, &params(Some("q1"), None, "")), None);
        assert_eq!(
            choice_of(&with, &params(Some("q0"), Some(1), "")),
            None,
            "the question changed"
        );
        let free = item(None);
        assert_eq!(
            choice_of(&free, &params(None, None, "Only the form")),
            Some(Choice::Other("Only the form".into()))
        );
        assert_eq!(choice_of(&free, &params(None, None, "  ")), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_inbox_and_the_run_with_a_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let load = table(&h.app, "inbox.view", Value::Null).await.unwrap();
        assert_eq!(load["view"]["title"], "Needs you");
        assert_eq!(load["view"]["total"], 0);
        assert_eq!(load["view"]["empty"]["title"], "Nothing needs you");
        assert_eq!(load["refresh_ms"], Value::Null);
        // Nothing waits: ⌘J says "All clear" and the run ends.
        let next = table(&h.app, "inbox.next", Value::Null).await.unwrap();
        assert_eq!(next["kind"], "all_clear");
        assert_eq!(next["title"], "All clear");
        assert!(h.app.inbox_run.lock().unwrap().is_none());
        let bar = table(&h.app, "inbox.run_bar", json!({ "key": "SHOP-1" }))
            .await
            .unwrap();
        assert_eq!(bar, Value::Null);
        let confirm = table(&h.app, "inbox.resume_all_confirm", Value::Null)
            .await
            .unwrap();
        assert_eq!(confirm, Value::Null);
        table(&h.app, "inbox.exit_run", Value::Null).await.unwrap();
        // A Project agent that doesn't wait (any more) takes no answer.
        let refused = table(
            &h.app,
            "inbox.answer",
            json!({ "key": "a1", "agent": "a1", "question": "q1", "pick": 1 }),
        )
        .await
        .unwrap_err();
        assert_eq!(refused.message, "Project agent “a1”’s question has changed");
        h.service.stop_all().await;
    }
}
