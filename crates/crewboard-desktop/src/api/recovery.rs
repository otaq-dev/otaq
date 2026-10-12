//! `recovery.*`: Recovery and Cleanup (T-0235; REC-02, REC-03, REC-04, REC-06, SES-02; d0018, d0022, d0045; *16 ·
//! Recovery & cleanup*). `recovery.summary` draws the after-restart sheet from the service's startup report, and
//! `recovery.resume` / `recovery.retry` are its buttons; `recovery.scratch` words Start from scratch's confirmation
//! and `recovery.start_from_scratch` is its confirm. The Cleanup overlay keeps its scan and selection here between
//! requests (`cleanup.rs`'s model): `recovery.cleanup` scans or redraws, the toggles tick, and only
//! `recovery.cleanup_delete`, the confirmation's button, deletes. Every text is `crewboard-view`'s.

use std::collections::{BTreeMap, BTreeSet};

use crewboard_core::board::tickets::Ticket;
use crewboard_core::protocol::{CleanupDeleteParams, CloseParams, KeyParams, Reply, Request};
use crewboard_core::recovery::StartupReport;
use crewboard_view::view_models::after_restart::{self, SummaryPage};
use crewboard_view::view_models::cleanup::{self, Page as CleanupPage, TargetPage};
use crewboard_view::view_models::recovery::{self, ScratchConfirm};
use crewboard_view::view_models::toasts::{NewToast, ToastLevel};
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "recovery.summary" => reply(&summary(app, parse(params)?).await?),
        "recovery.resume" => reply(&resume(app, parse(params)?).await?),
        "recovery.retry" => reply(&retry(app, parse(params)?).await?),
        "recovery.scratch" => reply(&scratch(app, parse(params)?).await?),
        "recovery.start_from_scratch" => reply(&start_from_scratch(app, parse(params)?).await?),
        "recovery.cleanup" => reply(&cleanup(app, parse(params)?).await?),
        "recovery.cleanup_toggle" => reply(&cleanup_toggle(app, parse(params)?)?),
        "recovery.cleanup_toggle_group" => reply(&cleanup_toggle_group(app, parse(params)?)?),
        "recovery.cleanup_delete" => reply(&cleanup_delete(app).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(what: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {what}: {other:?}"))
}

/// `{"unchecked": ["SHOP-19"], "unfolded": false}`: what the user unticked and whether "+3" was opened.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "AfterRestartParams")]
pub struct SummaryParams {
    #[serde(default)]
    pub unchecked: Vec<String>,
    #[serde(default)]
    pub unfolded: bool,
}

/// `{"keys": ["SHOP-25", "SHOP-19"]}`: the tickets to resume, in order.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "RecoveryResumeParams")]
pub struct ResumeParams {
    pub keys: Vec<String>,
}

/// `{"scan": true}` scans again; otherwise the last scan is drawn.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "CleanupParams")]
pub struct CleanupParams {
    #[serde(default)]
    pub scan: bool,
}

/// `{"target": {"group": "worktrees", "id": "…"}}`.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "CleanupToggleParams")]
pub struct ToggleParams {
    pub target: TargetPage,
}

/// `{"group": "worktrees"}`.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(rename = "CleanupGroupParams")]
pub struct GroupParams {
    pub group: String,
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

/// The after-restart sheet, or `null` when this service start interrupted nothing.
pub async fn summary(app: &App, p: SummaryParams) -> Result<Option<SummaryPage>, RequestError> {
    let report = match app.service.send(Request::StartupReport).await? {
        Reply::StartupReport(r) => *r,
        other => return Err(unexpected("service.startup_report", &other)),
    };
    let StartupReport {
        interrupted,
        session_ended,
        ..
    } = &report;
    let mut tickets = Vec::new();
    for key in interrupted.iter().chain(session_ended) {
        if let Some(t) = ticket(app, &key.to_string()).await? {
            tickets.push(t);
        }
    }
    let projects: BTreeMap<String, String> = match app.service.send(Request::ProjectList).await? {
        Reply::Projects(list) => list.into_iter().map(|p| (p.key, p.name)).collect(),
        other => return Err(unexpected("project.list", &other)),
    };
    let unchecked: BTreeSet<String> = p.unchecked.into_iter().collect();
    Ok(after_restart::summary(
        &report,
        &tickets,
        &projects,
        &unchecked,
        p.unfolded,
    ))
}

/// Resume, and Resume selected: each ticket in turn (`ticket.resume`, the user's click, d0045). One that can't be
/// resumed any more is skipped with an error toast; the rest still go.
pub async fn resume(app: &App, p: ResumeParams) -> Result<u32, RequestError> {
    let mut resumed = 0;
    for key in p.keys {
        match app
            .service
            .send(Request::TicketResume(KeyParams { key: key.clone() }))
            .await
        {
            Ok(_) => resumed += 1,
            Err(e) => {
                app.notices.toast(NewToast::new(
                    ToastLevel::Error,
                    format!("Couldn’t resume {key}"),
                    Some(e.message),
                ));
            }
        }
    }
    Ok(resumed)
}

/// Retry after Resume failed: the same resume again (REC-03). A refusal is an error toast.
pub async fn retry(app: &App, p: KeyParams) -> Result<(), RequestError> {
    let key = p.key.clone();
    if let Err(e) = app.service.send(Request::TicketRetryResume(p)).await {
        app.notices.toast(NewToast::new(
            ToastLevel::Error,
            format!("Couldn’t resume {key}"),
            Some(e.message),
        ));
    }
    Ok(())
}

/// Start from scratch's confirmation: what is kept and what is lost.
pub async fn scratch(app: &App, p: KeyParams) -> Result<ScratchConfirm, RequestError> {
    match ticket(app, &p.key).await? {
        Some(t) => Ok(recovery::scratch_confirm(&t)),
        None => Err(RequestError::plain(format!("{} no longer exists", p.key))),
    }
}

/// The confirmation's button: a new conversation in the same worktree (REC-03).
pub async fn start_from_scratch(app: &App, p: KeyParams) -> Result<(), RequestError> {
    app.service
        .send(Request::TicketStartFromScratch(CloseParams {
            key: p.key,
            confirm: true,
        }))
        .await?;
    Ok(())
}

async fn scan(app: &App) -> Result<(), RequestError> {
    let items = match app.service.send(Request::CleanupScan).await? {
        Reply::CleanupScan(items) => items,
        other => return Err(unexpected("cleanup.scan", &other)),
    };
    app.cleanup.lock().expect("cleanup").set_items(items);
    Ok(())
}

/// The Cleanup overlay; a scan first when asked (the overlay opens with one; nothing is ticked, d0018).
pub async fn cleanup(app: &App, p: CleanupParams) -> Result<CleanupPage, RequestError> {
    if p.scan {
        scan(app).await?;
    }
    Ok(app.cleanup.lock().expect("cleanup").page())
}

fn cleanup_toggle(app: &App, p: ToggleParams) -> Result<CleanupPage, RequestError> {
    let target = p
        .target
        .target()
        .ok_or_else(|| RequestError::plain(format!("no such group: {}", p.target.group)))?;
    let mut model = app.cleanup.lock().expect("cleanup");
    model.toggle(&target);
    Ok(model.page())
}

fn cleanup_toggle_group(app: &App, p: GroupParams) -> Result<CleanupPage, RequestError> {
    let target = TargetPage {
        group: p.group.clone(),
        id: String::new(),
    }
    .target()
    .ok_or_else(|| RequestError::plain(format!("no such group: {}", p.group)))?;
    let mut model = app.cleanup.lock().expect("cleanup");
    model.toggle_group(target.group);
    Ok(model.page())
}

/// The confirmation's Delete: exactly what is ticked, then the success toast and a new scan.
pub async fn cleanup_delete(app: &App) -> Result<CleanupPage, RequestError> {
    let (items, bytes, before) = {
        let model = app.cleanup.lock().expect("cleanup");
        (model.selected(), model.selected_bytes(), model.len())
    };
    if items.is_empty() {
        return Ok(app.cleanup.lock().expect("cleanup").page());
    }
    let outcome = match app
        .service
        .send(Request::CleanupDelete(CleanupDeleteParams {
            items,
            confirm: true,
        }))
        .await?
    {
        Reply::CleanupOutcome(outcome) => outcome,
        other => return Err(unexpected("cleanup.delete", &other)),
    };
    let left = before.saturating_sub(outcome.deleted.len());
    let (title, text) = cleanup::success(&outcome, bytes, left);
    app.notices.toast(NewToast::new(ToastLevel::Success, title, text));
    scan(app).await?;
    Ok(app.cleanup.lock().expect("cleanup").page())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::api::request as table;
    use crate::service::testing::{Harness, connected};

    #[tokio::test(flavor = "multi_thread")]
    async fn cleanup_lists_leftovers_and_deletes_only_what_is_ticked() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let data = h.app.service.data().unwrap();
        let leftover = data.worktrees_dir().join("shop-web").join("SHOP-99");
        std::fs::create_dir_all(&leftover).unwrap();
        std::fs::write(leftover.join("notes.txt"), "left behind").unwrap();

        let page = table(&h.app, "recovery.cleanup", json!({ "scan": true }))
            .await
            .unwrap();
        assert_eq!(page["count"], "1 item");
        assert_eq!(page["groups"][0]["group"], "worktrees");
        let row = &page["groups"][0]["rows"][0];
        assert_eq!(row["checked"], false, "nothing is ticked at first");
        assert_eq!(page["confirm"], Value::Null);

        // Delete with nothing ticked deletes nothing.
        table(&h.app, "recovery.cleanup_delete", Value::Null)
            .await
            .unwrap();
        assert!(leftover.exists());

        let page = table(
            &h.app,
            "recovery.cleanup_toggle",
            json!({ "target": row["target"] }),
        )
        .await
        .unwrap();
        assert_eq!(page["groups"][0]["rows"][0]["checked"], true);
        assert_eq!(page["confirm"]["title"], "Delete 1 item?");
        let page = table(
            &h.app,
            "recovery.cleanup_toggle_group",
            json!({ "group": "worktrees" }),
        )
        .await
        .unwrap();
        assert_eq!(page["groups"][0]["all_checked"], false);
        table(
            &h.app,
            "recovery.cleanup_toggle_group",
            json!({ "group": "worktrees" }),
        )
        .await
        .unwrap();

        let page = table(&h.app, "recovery.cleanup_delete", Value::Null)
            .await
            .unwrap();
        assert!(!leftover.exists());
        assert_eq!(page["empty"]["title"], "Nothing to clean up");
        let toasts = h.app.notices.view().toasts;
        assert_eq!(toasts.last().unwrap().title, "Cleaned up 1 item");

        assert!(
            table(
                &h.app,
                "recovery.cleanup_toggle_group",
                json!({ "group": "nonsense" })
            )
            .await
            .is_err()
        );
        h.service.stop_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_start_that_interrupted_nothing_has_no_sheet() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        assert_eq!(
            table(&h.app, "recovery.summary", json!({})).await.unwrap(),
            Value::Null
        );
        let missing = table(&h.app, "recovery.scratch", json!({ "key": "SHOP-1" })).await;
        assert!(missing.is_err());
        // Resuming a ticket that doesn't exist says so in a toast; nothing else happens.
        assert_eq!(
            table(&h.app, "recovery.resume", json!({ "keys": ["SHOP-1"] }))
                .await
                .unwrap(),
            0
        );
        assert!(
            h.app
                .notices
                .view()
                .toasts
                .iter()
                .any(|t| t.title == "Couldn’t resume SHOP-1")
        );
        h.service.stop_all().await;
    }
}
