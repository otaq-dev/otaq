//! `review.*`: Send back, Approve ▾ and Local merge (REV-04, REV-06, REV-08, GIT-03, GIT-04, GIT-05, UI-02; d0015,
//! d0017, d0034, d0038, d0043, d0050; *07 · Diff, review, approve & send back*). The dialogs' views come from
//! `otaq-view`'s `review_page.rs`; the requests behind their buttons land here, so their toasts and refusals stay
//! in Rust. Open PR and Push to PR are T-0227's dialog.

use otaq_core::board::review::ApproveCheck;
use otaq_core::board::tickets::Ticket;
use otaq_core::decisions::{ChildDecisions, Decision};
use otaq_core::domain::{Status, TicketKey};
use otaq_core::protocol::{
    CommentsParams, KeyParams, LocalMergeParams, ProjectParams, Reply, Request,
};
use otaq_core::review::ReturnOutcome;
use otaq_view::view_models::board_page;
use otaq_view::view_models::decision_item::{self as di, DecisionsSection};
use otaq_view::view_models::review_page::{self as page, ApproveFacts};
use otaq_view::view_models::send_back as sb;
use otaq_view::view_models::terminal_strip::{Click, UnsentPrompt};
use otaq_view::view_models::toasts::{NewToast, ToastLevel};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::panel::{Sent, Unsent};
use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "review.available" => reply(&available(app, parse(params)?).await?),
        "review.approve" => reply(&approve(app, parse(params)?).await?),
        "review.local_merge" => reply(&local_merge(app, parse(params)?).await?),
        "review.send_back_view" => reply(&send_back_view(app, parse(params)?).await?),
        "review.send_back" => reply(&send_back(app, parse(params)?).await?),
        "review.decisions" => reply(&decisions(app, parse(params)?).await?),
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

fn toast(app: &App, level: ToastLevel, title: String, text: Option<String>) {
    app.notices.toast(NewToast::new(level, title, text));
}

async fn ticket(app: &App, key: &str) -> Result<Ticket, RequestError> {
    match app
        .service
        .send(Request::TicketGet(KeyParams {
            key: key.to_owned(),
        }))
        .await?
    {
        Reply::Ticket(t) => Ok(*t),
        other => Err(unexpected("ticket.get", &other)),
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

/// The open ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "ReviewParams")]
pub struct Params {
    pub key: String,
}

/// Which review dialogs the ticket takes now (⌘↵ / ⌘⌫ open them only then).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ReviewAvailable")]
pub struct Available {
    pub approve: bool,
    pub send_back: bool,
}

pub async fn available(app: &App, params: Params) -> Result<Available, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    Ok(Available {
        approve: ticket.status == Status::ReadyForReview,
        send_back: sb::available(&ticket),
    })
}

/// The Approve dialogs for one ticket: the choice, the confirmation with the proposed message, the running steps,
/// and the commits since the review (REV-06), gathered at once.
pub async fn approve(app: &App, params: Params) -> Result<page::ApproveView, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let project = ticket.key.project.to_string();
    let k = || KeyParams {
        key: params.key.clone(),
    };
    let (all, proposal, check, provider, projects) = tokio::join!(
        tickets(app, &project),
        app.service.send(Request::TicketMergeProposal(k())),
        app.service.send(Request::TicketApproveCheck(k())),
        app.service.send(Request::ProjectPrProvider(ProjectParams {
            project: project.clone(),
        })),
        app.service.send(Request::ProjectList),
    );
    let all = all?;
    let proposal = match proposal? {
        Reply::MergeProposal(p) => p,
        other => return Err(unexpected("ticket.merge_proposal", &other)),
    };
    let check = match check {
        Ok(Reply::ApproveCheck(c)) => c,
        _ => ApproveCheck::Unchanged,
    };
    let provider = match provider {
        Ok(Reply::PrProvider(p)) => Some(p),
        _ => None,
    };
    let info = match projects {
        Ok(Reply::Projects(list)) => list.into_iter().find(|p| p.key == project),
        _ => None,
    };
    let (repo, project_base) = info.map_or_else(Default::default, |p| (p.repo_path, p.base_branch));
    Ok(page::approve_view(ApproveFacts {
        ticket: &ticket,
        tickets: &all,
        target: &proposal.target,
        into_epic: proposal.into_epic,
        message: &proposal.message,
        repo: &repo,
        project_base: &project_base,
        check: &check,
        menu: board_page::approve_menu(&ticket, &proposal.target, &all, provider.as_ref()),
    }))
}

/// Squash & merge (and Retry): the Local merge with the confirmed message; `approve_anyway` after "changed since you
/// reviewed it". A merge that went through shows its toasts here.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "LocalMergeParams")]
pub struct MergeParams {
    pub key: String,
    pub message: String,
    #[serde(default)]
    pub approve_anyway: bool,
}

pub async fn local_merge(app: &App, params: MergeParams) -> Result<page::MergeResult, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let key = ticket.key.clone();
    let outcome = match app
        .service
        .send(Request::TicketLocalMerge(LocalMergeParams {
            key: params.key,
            message: params.message,
            approve_anyway: params.approve_anyway,
        }))
        .await
    {
        Ok(Reply::LocalMerge(outcome)) => outcome,
        Ok(other) => return Err(unexpected("ticket.local_merge", &other)),
        Err(e) => {
            toast(
                app,
                ToastLevel::Error,
                format!("Local merge of {key} failed"),
                Some(e.message.clone()),
            );
            return Err(e);
        }
    };
    let target = match &outcome {
        otaq_core::review::LocalMergeOutcome::Merged { target, .. }
        | otaq_core::review::LocalMergeOutcome::Conflict { target, .. } => target.clone(),
        _ => match app
            .service
            .send(Request::TicketMergeProposal(KeyParams {
                key: key.to_string(),
            }))
            .await
        {
            Ok(Reply::MergeProposal(p)) => p.target,
            _ => String::new(),
        },
    };
    let all = tickets(app, &key.project.to_string())
        .await
        .unwrap_or_default();
    let result = page::merge_result(&ticket, &target, &outcome, &all);
    if let page::MergeResult::Merged { toasts } = &result {
        for t in toasts {
            toast(app, ToastLevel::Success, t.title.clone(), t.text.clone());
        }
    }
    Ok(result)
}

/// The Send back dialog: every pending comment, the note field, who keeps waiting.
pub async fn send_back_view(app: &App, params: Params) -> Result<page::SendBackView, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let project = ticket.key.project.to_string();
    let (all, comments) = tokio::join!(
        tickets(app, &project),
        app.service.send(Request::ReviewComments(CommentsParams {
            key: params.key.clone(),
            round: None,
        })),
    );
    let comments = match comments? {
        Reply::ReviewComments(list) => list.comments,
        other => return Err(unexpected("review.comments", &other)),
    };
    Ok(page::send_back_view(&ticket, &comments, &all?))
}

/// Send back: the pending comments and the note go to the agent; `clear` is "Clear it and send" (d0048).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "SendBackParams")]
pub struct SendBackParams {
    pub key: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub clear: bool,
    /// How many comments it sends, for the unsent-text question's wording.
    #[serde(default)]
    pub comments: u32,
}

pub async fn send_back(app: &App, params: SendBackParams) -> Result<Sent, RequestError> {
    let key = key_of(&params.key)?;
    match app
        .service
        .send(sb::request(&key, &params.note, params.clear))
        .await
    {
        Ok(Reply::Returned(ReturnOutcome::NeedsConfirmation { .. })) => {
            Ok(Sent::Ask(Unsent::of(&UnsentPrompt::for_click(
                key,
                None,
                sb::sending(params.comments as usize),
                Click::SendBack { note: params.note },
            ))))
        }
        Ok(_) => Ok(Sent::Done),
        Err(e) => {
            toast(
                app,
                ToastLevel::Error,
                format!("{key} wasn\u{2019}t sent back"),
                Some(e.message.clone()),
            );
            Err(e)
        }
    }
}

async fn decision_list(app: &App, key: &str) -> Result<Vec<Decision>, RequestError> {
    match app
        .service
        .send(Request::DecisionList(KeyParams {
            key: key.to_owned(),
        }))
        .await?
    {
        Reply::Decisions(list) => Ok(list),
        other => Err(unexpected("decision.list", &other)),
    }
}

/// The recorded decisions the review lists (DEC-04): the ticket's own and, for an epic, each child's with any, in
/// child order.
pub async fn decisions_of(
    app: &App,
    ticket: &Ticket,
    tickets: &[Ticket],
) -> Result<(Vec<Decision>, Vec<ChildDecisions>), RequestError> {
    let own = decision_list(app, &ticket.key.to_string()).await?;
    let mut children = Vec::new();
    if ticket.is_epic() {
        let mut kids: Vec<&Ticket> = tickets
            .iter()
            .filter(|t| t.epic.as_ref() == Some(&ticket.key))
            .collect();
        kids.sort_by(|a, b| a.key.cmp(&b.key));
        for kid in kids {
            let decisions = decision_list(app, &kid.key.to_string()).await?;
            if !decisions.is_empty() {
                children.push(ChildDecisions {
                    key: kid.key.clone(),
                    title: kid.title.clone(),
                    decisions,
                });
            }
        }
    }
    Ok((own, children))
}

/// How many open decisions Approve keeps now: "Approve · keeps N decisions" (DEC-05); 0 outside Ready for Review.
pub async fn open_decisions(app: &App, ticket: &Ticket, tickets: &[Ticket]) -> u32 {
    if !di::in_review(ticket) {
        return 0;
    }
    decisions_of(app, ticket, tickets)
        .await
        .map_or(0, |(own, children)| di::open_count(&own, &children))
}

/// The Decisions section on the Diff tab, under the agent summary; `null` when there is nothing to list.
pub async fn decisions(app: &App, params: Params) -> Result<Option<DecisionsSection>, RequestError> {
    let ticket = ticket(app, &params.key).await?;
    let all = tickets(app, &ticket.key.project.to_string()).await?;
    let (own, children) = decisions_of(app, &ticket, &all).await?;
    Ok(di::section(&ticket, &own, &children, &all))
}

#[cfg(test)]
mod tests {
    use otaq_core::board::projects::AddProject;
    use otaq_core::board::tickets::NewTicketInput;
    use otaq_core::git::testing::TempRepos;
    use serde_json::json;

    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn review_dialogs_refuse_a_ticket_not_in_review() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let Reply::Project(project) = h
            .app
            .service
            .send(Request::ProjectAdd(AddProject {
                path: path.display().to_string(),
                key: None,
                base_branch: None,
            }))
            .await
            .unwrap()
        else {
            panic!("no project");
        };
        let Reply::Ticket(created) = h
            .app
            .service
            .send(Request::TicketCreate(NewTicketInput {
                project: project.key.clone(),
                kind: Default::default(),
                title: "Checkout API".into(),
                body: String::new(),
                priority: None,
                epic: None,
                base_branch: None,
                external_id: None,
            }))
            .await
            .unwrap()
        else {
            panic!("no ticket");
        };
        let key = created.key.to_string();

        let available = table(&h.app, "review.available", json!({ "key": key }))
            .await
            .unwrap();
        assert_eq!(available, json!({ "approve": false, "send_back": false }));

        let view = table(&h.app, "review.send_back_view", json!({ "key": key }))
            .await
            .unwrap();
        assert_eq!(view["title"], format!("Send {key} Checkout API back"));
        assert_eq!(view["comments"], json!([]));

        let refused = table(
            &h.app,
            "review.send_back",
            json!({ "key": key, "note": "again" }),
        )
        .await
        .unwrap_err();
        assert!(!refused.message.is_empty());
        let merge = table(
            &h.app,
            "review.local_merge",
            json!({ "key": key, "message": "x" }),
        )
        .await;
        assert!(merge.is_err(), "{merge:?}");

        // A recorded decision is read back, but a ticket that hasn't reached its review lists none (DEC-04).
        let recorded = h
            .app
            .service
            .send(Request::DecisionRecord(
                otaq_core::protocol::DecisionRecordParams {
                    key: key.clone(),
                    decision: otaq_core::decisions::NewDecision {
                        question: "Split fullName everywhere, or only in the profile form?".into(),
                        built: "only in the profile form".into(),
                        other_options: vec!["everywhere".into()],
                        why: "the description only mentions the profile form".into(),
                    },
                },
            ))
            .await;
        assert!(matches!(recorded, Ok(Reply::Decision(_))), "{recorded:?}");
        let (own, children) = decisions_of(&h.app, &created, &[]).await.unwrap();
        assert_eq!((own.len(), children.len()), (1, 0));
        assert_eq!(open_decisions(&h.app, &created, &[]).await, 0, "not in review");
        let section = table(&h.app, "review.decisions", json!({ "key": key }))
            .await
            .unwrap();
        assert_eq!(section, Value::Null);
        h.service.stop_all().await;
    }
}
