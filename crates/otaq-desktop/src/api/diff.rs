//! `diff.*`: the panel's Diff tab (REV-01, REV-02, REV-03; d0017, d0049, d0059, d0075; *07 · Diff, review, approve
//! & send back*). The file list against the ticket's `base_sha`, each file's highlighted lines when it comes into
//! view, the viewed ticks, the folded agent summary and Unified / Side-by-side (kept in [`App`] while the app runs,
//! like the GPUI app's `ReviewMemory`), and the line comments with the pending counter Send back uses. `diff.review`
//! gives the Review entry and the final validation report shown next to the agent's summary (VAL-12; d0063).

use std::time::{SystemTime, UNIX_EPOCH};

use otaq_core::board::comments::NewComment;
use otaq_core::domain::{Status, TicketKey};
use otaq_core::protocol::{
    CommentIdParams, CommentTextParams, CommentsParams, DiffFileParams, KeyParams, Reply, Request,
};
use otaq_core::board::tickets::{EventKind, Ticket};
use otaq_core::domain::Actor;
use otaq_view::view_models::diff_page::{self as page, DiffSide};
use otaq_view::view_models::validation_report::{self, Review};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "diff.view" => reply(&view(app, parse(params)?).await?),
        "diff.review" => reply(&review(app, parse(params)?).await?),
        "diff.file" => reply(&file(app, parse(params)?).await?),
        "diff.comments" => reply(&comments(app, parse(params)?).await?),
        "diff.viewed" => reply(&viewed(app, parse(params)?)?),
        "diff.summary_fold" => reply(&summary_fold(app, parse(params)?)?),
        "diff.mode" => reply(&mode(app, parse(params)?)),
        "diff.comment_add" => reply(&comment_add(app, parse(params)?).await?),
        "diff.comment_reply" => reply(&comment_text(app, parse(params)?, true).await?),
        "diff.comment_edit" => reply(&comment_text(app, parse(params)?, false).await?),
        "diff.comment_delete" => reply(&comment_delete(app, parse(params)?).await?),
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

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The open ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffParams")]
pub struct Params {
    pub key: String,
}

/// What `diff.view` answers: the tab, or why there is no diff ("SHOP-12 has no branch yet: it hasn't started").
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "DiffLoad")]
pub struct Load {
    pub view: Option<page::TabView>,
    pub problem: Option<String>,
}

pub async fn view(app: &App, params: Params) -> Result<Load, RequestError> {
    let reply = app
        .service
        .send(Request::DiffFiles(KeyParams { key: params.key }))
        .await;
    Ok(match reply {
        Ok(Reply::DiffSummary(summary)) => {
            let memory = app.review.lock().unwrap_or_else(|e| e.into_inner());
            Load {
                view: Some(page::tab_view(&summary, &memory)),
                problem: None,
            }
        }
        Ok(other) => return Err(unexpected("diff.files", &other)),
        Err(e) => Load {
            view: None,
            problem: Some(e.message),
        },
    })
}

/// The Review entry ("Validated · round 2") and the final report, or the last findings of a ticket that failed every
/// round and was moved on by the user; `null` without a Review entry.
pub async fn review(app: &App, params: Params) -> Result<Option<Review>, RequestError> {
    let ticket: Ticket = match app
        .service
        .send(Request::TicketGet(KeyParams {
            key: params.key.clone(),
        }))
        .await
    {
        Ok(Reply::Ticket(t)) => *t,
        Ok(other) => return Err(unexpected("ticket.get", &other)),
        Err(e) if e.not_found => return Ok(None),
        Err(e) => return Err(e),
    };
    let Some(flag) = ticket.flags.review_entry else {
        return Ok(None);
    };
    let records = super::sections::validations(app, &params.key).await;
    let rounds = super::panel::validation_rounds(app, &ticket).await;
    // When the user moved it to Review (for "Moved by you at 11:12").
    let moved_at = match app
        .service
        .send(Request::TicketHistory(KeyParams {
            key: params.key.clone(),
        }))
        .await
    {
        Ok(Reply::History(events)) => events
            .iter()
            .filter(|e| e.kind == EventKind::Moved && matches!(e.actor, Actor::User(_)))
            .filter(|e| {
                e.payload
                    .get("to")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Status>().ok())
                    == Some(Status::WaitingForYou)
            })
            .map(|e| e.at)
            .max(),
        _ => None,
    };
    Ok(validation_report::review(
        Some(flag),
        &records,
        rounds,
        moved_at,
    ))
}

/// One file of the open ticket; `since` (a commit) limits it to what changed after the review (REV-06).
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffFileParams")]
pub struct FileParams {
    pub key: String,
    pub path: String,
    #[ts(optional)]
    pub since: Option<String>,
}

/// A file's hunks, highlighted off the runtime's threads (a long file takes a while).
pub async fn file(app: &App, params: FileParams) -> Result<page::FileView, RequestError> {
    let diff = match app
        .service
        .send(Request::DiffFile(DiffFileParams {
            key: params.key,
            path: params.path,
            child: None,
            since: params.since,
        }))
        .await?
    {
        Reply::FileDiff(diff) => diff,
        other => return Err(unexpected("diff.file", &other)),
    };
    tokio::task::spawn_blocking(move || page::file_view(&diff))
        .await
        .map_err(|e| RequestError::plain(e.to_string()))
}

/// The ticket's comments; they can be added only while it is in Ready for Review (epics take none, EPC-05).
pub async fn comments(app: &App, params: Params) -> Result<page::CommentsView, RequestError> {
    let ticket = match app
        .service
        .send(Request::TicketGet(KeyParams {
            key: params.key.clone(),
        }))
        .await?
    {
        Reply::Ticket(t) => t,
        other => return Err(unexpected("ticket.get", &other)),
    };
    let list = match app
        .service
        .send(Request::ReviewComments(CommentsParams {
            key: params.key.clone(),
            round: None,
        }))
        .await?
    {
        Reply::ReviewComments(list) => list,
        other => return Err(unexpected("review.comments", &other)),
    };
    let can_comment = ticket.status == Status::WaitingForYou && !ticket.is_epic();
    Ok(page::comments_view(
        &params.key,
        &list.comments,
        can_comment,
        now_ms(),
    ))
}

/// A file's "viewed" tick in this round.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffViewedParams")]
pub struct ViewedParams {
    pub key: String,
    pub round: u32,
    pub path: String,
}

/// Ticks or unticks; answers whether the file is viewed now.
pub fn viewed(app: &App, params: ViewedParams) -> Result<bool, RequestError> {
    let key = key_of(&params.key)?;
    let mut memory = app.review.lock().unwrap_or_else(|e| e.into_inner());
    Ok(memory.toggle_viewed(&key, params.round, &params.path))
}

/// Folds or unfolds the agent's summary for this ticket.
pub fn summary_fold(app: &App, params: Params) -> Result<(), RequestError> {
    let key = key_of(&params.key)?;
    app.review
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .toggle_summary(&key);
    Ok(())
}

/// Unified or Side-by-side, for every ticket.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffModeParams")]
pub struct ModeParams {
    pub side_by_side: bool,
}

pub fn mode(app: &App, params: ModeParams) {
    app.review
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .side_by_side = params.side_by_side;
}

/// A new comment on a line.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffCommentAdd")]
pub struct AddParams {
    pub key: String,
    pub file: String,
    pub line: u32,
    pub side: DiffSide,
    pub body: String,
}

fn body_of(text: &str) -> Result<String, RequestError> {
    let body = text.trim();
    if body.is_empty() {
        return Err(RequestError::plain("Write the comment first."));
    }
    Ok(body.to_owned())
}

/// Adds a comment; the list comes back through the ticket's change event.
pub async fn comment_add(app: &App, params: AddParams) -> Result<(), RequestError> {
    let body = body_of(&params.body)?;
    match app
        .service
        .send(Request::ReviewCommentAdd(NewComment {
            key: params.key,
            file: params.file,
            line: params.line,
            side: params.side.into(),
            body,
        }))
        .await?
    {
        Reply::ReviewComment(_) => Ok(()),
        other => Err(unexpected("review.comment_add", &other)),
    }
}

/// A reply to, or the new text of, a comment.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffCommentText")]
pub struct TextParams {
    pub id: String,
    pub body: String,
}

pub async fn comment_text(app: &App, params: TextParams, reply: bool) -> Result<(), RequestError> {
    let p = CommentTextParams {
        id: params.id,
        body: body_of(&params.body)?,
    };
    let request = if reply {
        Request::ReviewCommentReply(p)
    } else {
        Request::ReviewCommentEdit(p)
    };
    match app.service.send(request).await? {
        Reply::ReviewComment(_) => Ok(()),
        other => Err(unexpected("review.comment", &other)),
    }
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "DiffCommentId")]
pub struct IdParams {
    pub id: String,
}

/// Deletes a pending comment.
pub async fn comment_delete(app: &App, params: IdParams) -> Result<(), RequestError> {
    app.service
        .send(Request::ReviewCommentDelete(CommentIdParams { id: params.id }))
        .await
        .map(|_| ())
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
    async fn the_tab_explains_a_ticket_without_a_branch_and_keeps_its_choices() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();
        let path = repos.repo("shop-web", 1);
        let Reply::ProjectAdded { project, .. } = h
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

        let load = table(&h.app, "diff.view", json!({ "key": key })).await.unwrap();
        // Not in Review through a validation or a submit: no Review entry.
        let review = table(&h.app, "diff.review", json!({ "key": key })).await.unwrap();
        assert_eq!(review, Value::Null);
        assert!(load["view"].is_null());
        assert!(load["problem"].as_str().unwrap().contains(&key), "{load}");

        let comments = table(&h.app, "diff.comments", json!({ "key": key }))
            .await
            .unwrap();
        assert_eq!(comments["can_comment"], false, "a Backlog ticket takes no comments");
        assert_eq!(comments["send_back_label"], "Send back");
        let refused = table(
            &h.app,
            "diff.comment_add",
            json!({ "key": key, "file": "a.ts", "line": 1, "side": "new", "body": "x" }),
        )
        .await
        .unwrap_err();
        assert!(!refused.message.is_empty());
        let empty = table(
            &h.app,
            "diff.comment_add",
            json!({ "key": key, "file": "a.ts", "line": 1, "side": "new", "body": "  " }),
        )
        .await
        .unwrap_err();
        assert_eq!(empty.message, "Write the comment first.");

        let ticked = table(
            &h.app,
            "diff.viewed",
            json!({ "key": key, "round": 1, "path": "a.ts" }),
        )
        .await
        .unwrap();
        assert_eq!(ticked, true);
        table(&h.app, "diff.mode", json!({ "side_by_side": true }))
            .await
            .unwrap();
        table(&h.app, "diff.summary_fold", json!({ "key": key }))
            .await
            .unwrap();
        {
            let memory = h.app.review.lock().unwrap();
            let k: TicketKey = key.parse().unwrap();
            assert!(memory.side_by_side);
            assert!(memory.is_viewed(&k, 1, "a.ts"));
            assert!(!memory.summary_open(&k));
        }
        h.service.stop_all().await;
    }
}
