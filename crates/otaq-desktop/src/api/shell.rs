//! `shell.*`: the header and the project rail (UI-01, ATT-10, USG-01, USG-02, USG-05, SCH-03, RUN-04, PRJ-01; d0024,
//! d0016, d0020, d0058; *02 · App shell & project switcher*). The header's view gathers the global things every
//! project shares (inbox count, tokens today, plan meter, agents, processes) with the shown project's name; the rail's
//! view is the switcher model's, kept here between calls so a project that newly needs you rings for 10 s. The page
//! refetches both on the events that change their counts, so they update live with no spinner.

use std::time::Instant;

use otaq_core::board::settings::{AgentLimits, GlobalSettings};
use otaq_core::board::snapshot::ProjectInfo;
use otaq_core::protocol::{Reply, Request, UsageParams};
use otaq_core::scheduler::SchedulerStatus;
use otaq_core::usage::UsageSummary;
use otaq_view::keymap::{KeyAction, Platform};
use otaq_view::shell::tilde;
use otaq_view::view_models::inbox::{self, HeaderButton};
use otaq_view::view_models::limits::{self, AgentsChip, Field, Popover};
use otaq_view::view_models::plan_meter::{self, PlanMeter};
use otaq_view::view_models::processes::{self, Chip};
use otaq_view::view_models::search_page;
use otaq_view::view_models::switcher::{self, QuickRow, RailItem, SIDEBAR_FROM_WIDTH};
use otaq_view::view_models::usage::{self, HeaderUsage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::{RequestError, reply, unknown};
use crate::App;

pub async fn request(app: &App, method: &str, params: Value) -> Result<Value, RequestError> {
    match method {
        "shell.header" => reply(&header(app, parse(params)?).await?),
        "shell.rail" => reply(&rail(app, parse(params)?).await?),
        "shell.quick" => reply(&quick(app, parse(params)?)),
        "shell.project_for_digit" => reply(&project_for_digit(app, parse(params)?)),
        "shell.step_limit" => reply(&step_limit(app, parse(params)?).await?),
        "shell.cleanup" => reply(&cleanup(app).await?),
        _ => Err(unknown(method)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, RequestError> {
    serde_json::from_value(params).map_err(|e| RequestError::plain(e.to_string()))
}

fn unexpected(method: &str, other: &Reply) -> RequestError {
    RequestError::plain(format!("unexpected answer to {method}: {other:?}"))
}

/// What `shell.header` and `shell.rail` take: the project on screen (the page's selection), if any.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "ShellParams")]
pub struct Params {
    pub project: Option<String>,
}

/// The shown project at the header's start: "shop-web", the base branch chip, "~/code/shop-web".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "HeaderProject")]
pub struct Project {
    pub key: String,
    pub name: String,
    pub base_branch: String,
    pub path: String,
}

/// A button's tooltip: its name and its shortcut on this platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "HeaderHint")]
pub struct Hint {
    pub text: &'static str,
    pub keys: Option<String>,
}

impl Hint {
    fn new(text: &'static str, action: KeyAction) -> Self {
        Self {
            text,
            keys: action.keys(Platform::current()),
        }
    }
}

/// The header, left to right (*02 → Header states*). Items without data are `None` and hidden: tokens until the
/// first summary, the plan meter without status-line data, processes while nothing runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "HeaderView")]
pub struct Header {
    pub project: Option<Project>,
    /// The search entry's placeholder and ⌘K; it opens the board's filter (T-0221).
    pub search: Hint,
    pub usage: Option<HeaderUsage>,
    pub plan_meter: Option<PlanMeter>,
    pub agents: AgentsChip,
    /// The plain chip's tooltip: title and "shop-web 2 · billing-api 1" (limits off).
    pub agents_tooltip: Option<(String, String)>,
    /// The chip's popover (limits on).
    pub limits: Option<Popover>,
    pub processes: Option<Chip>,
    pub inbox: HeaderButton,
    pub inbox_hint: Hint,
    pub new_ticket: Hint,
    pub settings: Hint,
}

async fn projects(app: &App) -> Result<Vec<ProjectInfo>, RequestError> {
    match app.service.send(Request::ProjectList).await? {
        Reply::Projects(projects) => Ok(projects),
        other => Err(unexpected("project.list", &other)),
    }
}

async fn settings(app: &App) -> Result<GlobalSettings, RequestError> {
    match app.service.send(Request::SettingsGet).await? {
        Reply::GlobalSettings(reply) => Ok(reply.settings),
        other => Err(unexpected("settings.get", &other)),
    }
}

/// The parts the header can draw without: a failure leaves that part out instead of failing the whole header.
async fn optional<T>(app: &App, request: Request, pick: impl Fn(Reply) -> Option<T>) -> Option<T> {
    app.service.send(request).await.ok().and_then(pick)
}

pub async fn header(app: &App, params: Params) -> Result<Header, RequestError> {
    let (projects, settings, scheduler, inbox, usage, runs) = tokio::join!(
        projects(app),
        settings(app),
        optional(app, Request::SchedulerStatus, |r| match r {
            Reply::SchedulerStatus(s) => Some(s),
            _ => None,
        }),
        optional(app, Request::InboxCounts, |r| match r {
            Reply::InboxCounts(c) => Some(c.total),
            _ => None,
        }),
        optional(
            app,
            Request::UsageSummary(UsageParams { project: None }),
            |r| match r {
                Reply::Usage(summary) => Some(*summary),
                _ => None,
            }
        ),
        optional(app, Request::RunList, |r| match r {
            Reply::Runs(runs) => Some(runs),
            _ => None,
        }),
    );
    let (projects, settings) = (projects?, settings?);
    Ok(header_view(
        &projects,
        params.project.as_deref(),
        &settings,
        scheduler.as_ref(),
        inbox.unwrap_or(0),
        usage.as_ref(),
        &runs.unwrap_or_default(),
        otaq_core::store::now_ms(),
    ))
}

#[allow(clippy::too_many_arguments)]
fn header_view(
    projects: &[ProjectInfo],
    shown: Option<&str>,
    settings: &GlobalSettings,
    scheduler: Option<&SchedulerStatus>,
    inbox_total: u32,
    usage: Option<&UsageSummary>,
    runs: &[otaq_core::terminals::run::RunInfo],
    now_ms: i64,
) -> Header {
    let limits = settings.agent_limits;
    let agents = limits::agents_chip(&limits, scheduler, limits::running_total(projects));
    Header {
        project: projects
            .iter()
            .find(|p| Some(p.key.as_str()) == shown)
            .map(|p| Project {
                key: p.key.clone(),
                name: p.name.clone(),
                base_branch: p.base_branch.clone(),
                path: tilde(&p.repo_path),
            }),
        search: Hint::new(search_page::PLACEHOLDER, KeyAction::Search),
        usage: usage.map(|u| usage::header(u, settings.dollar_estimate)),
        plan_meter: usage.and_then(|u| plan_meter::meter(u.plan.as_ref(), now_ms)),
        agents_tooltip: (!agents.control).then(|| {
            (
                limits::RUNNING_TOOLTIP.to_owned(),
                limits::running_by_project(projects),
            )
        }),
        limits: agents.control.then(|| limits::popover(&limits)),
        agents,
        processes: processes::chip(runs),
        inbox: inbox::header_button(inbox_total),
        inbox_hint: Hint::new("Needs you", KeyAction::OpenInbox),
        new_ticket: Hint::new("New ticket", KeyAction::NewTicket),
        settings: Hint::new("Settings", KeyAction::Settings),
    }
}

/// The project rail (tiles below 1600 px, the 232 px sidebar above) in the user's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "RailView")]
pub struct Rail {
    /// The project to show: the page's selection while it exists, else the first one (`None`: no projects).
    pub selected: Option<String>,
    pub items: Vec<RailItem>,
    /// Window widths above this get the sidebar.
    pub sidebar_from_width: u32,
    /// When a ringing tile stops ringing, so the page asks again then.
    pub ring_ends_in_ms: Option<u64>,
    /// The sidebar's "PROJECTS ⌘P".
    pub switch_keys: Option<String>,
}

pub async fn rail(app: &App, params: Params) -> Result<Rail, RequestError> {
    let mut projects = projects(app).await?;
    // Waiting Project agents count in their project's badge too (PAG-07); without the inbox, tickets only.
    if let Some(agents) = optional(app, Request::InboxList, |r| match r {
        Reply::Inbox(list) => Some(list.project_agents),
        _ => None,
    })
    .await
    {
        switcher::add_waiting_agents(&mut projects, &agents);
    }
    let now = Instant::now();
    let mut model = app.switcher.lock().expect("switcher");
    if let Some(shown) = params.project
        && !model.select(&shown)
    {
        // Not in the last list (just added): the new list decides whether it exists.
        model.selected = Some(shown);
    }
    model.set_projects(projects, now);
    model.expire(now);
    Ok(Rail {
        selected: model.selected.clone(),
        items: model.rail(now, Platform::current()),
        sidebar_from_width: SIDEBAR_FROM_WIDTH as u32,
        ring_ends_in_ms: model
            .next_ring_end(now)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
        switch_keys: KeyAction::SwitchProject.keys(Platform::current()),
    })
}

/// What `shell.quick` takes: the typed filter.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(rename = "QuickSwitcherParams")]
pub struct QuickParams {
    pub filter: String,
}

/// A footer hint of the quick switcher: "↑↓ move", "↵ switch", "⌘N add project".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "QuickSwitcherHint")]
pub struct QuickHint {
    pub keys: String,
    pub label: &'static str,
}

/// The quick switcher (⌘P): needs-you first, then working, then idle, filtered by name or key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "QuickSwitcherView")]
pub struct Quick {
    pub placeholder: &'static str,
    pub rows: Vec<QuickRow>,
    pub hints: Vec<QuickHint>,
}

/// From the rail's last list, so typing never waits on the service.
pub fn quick(app: &App, params: QuickParams) -> Quick {
    let model = app.switcher.lock().expect("switcher");
    let platform = Platform::current();
    Quick {
        placeholder: "Switch project…",
        rows: model
            .quick(&params.filter, Instant::now(), platform)
            .into_iter()
            .map(QuickRow::from)
            .collect(),
        hints: vec![
            QuickHint {
                keys: "↑↓".to_owned(),
                label: "move",
            },
            QuickHint {
                keys: "↵".to_owned(),
                label: "switch",
            },
            QuickHint {
                keys: KeyAction::NewTicket.keys(platform).unwrap_or_default(),
                label: "add project",
            },
        ],
    }
}

/// What `shell.project_for_digit` takes: the digit of ⌘1–9.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[ts(rename = "ProjectDigitParams")]
pub struct DigitParams {
    pub digit: u8,
}

/// ⌘1–9: the key of the project at that place in the rail, if there is one.
pub fn project_for_digit(app: &App, params: DigitParams) -> Option<String> {
    let model = app.switcher.lock().expect("switcher");
    model.project_for_digit(params.digit).map(str::to_owned)
}

/// What `shell.step_limit` takes: which popover value, one up or down.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[ts(rename = "LimitStep")]
pub struct StepParams {
    pub field: Field,
    pub up: bool,
}

/// A stepper press in the limits popover: saved at once; it reaches the scheduler without restarting anything, and
/// never stops a running agent (SCH-03). Answers the popover with the new values.
pub async fn step_limit(app: &App, params: StepParams) -> Result<Popover, RequestError> {
    let mut settings = settings(app).await?;
    let limits: AgentLimits = limits::stepped(&settings.agent_limits, params.field, params.up);
    settings.agent_limits = limits;
    match app.service.send(Request::SettingsSet(settings)).await? {
        Reply::GlobalSettings(reply) => Ok(limits::popover(&reply.settings.agent_limits)),
        other => Err(unexpected("settings.set", &other)),
    }
}

/// The broom at the bottom of the rail (*16 → Where Cleanup lives*): a dot only while leftovers wait.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "CleanupBroom")]
pub struct Broom {
    pub count: u32,
    pub tooltip: String,
    pub detail: &'static str,
}

fn broom(count: usize) -> Broom {
    let tooltip = match count {
        0 => "Cleanup".to_owned(),
        1 => "Cleanup · 1 leftover".to_owned(),
        n => format!("Cleanup · {n} leftovers"),
    };
    Broom {
        count: u32::try_from(count).unwrap_or(u32::MAX),
        tooltip,
        detail: "Nothing is deleted without your click",
    }
}

/// A read-only scan of the leftovers; the page asks at start and after a resync, not on every event.
pub async fn cleanup(app: &App) -> Result<Broom, RequestError> {
    match app.service.send(Request::CleanupScan).await? {
        Reply::CleanupScan(items) => Ok(broom(items.len())),
        other => Err(unexpected("cleanup.scan", &other)),
    }
}

#[cfg(test)]
mod tests {
    use otaq_core::board::projects::AddProject;
    use otaq_core::git::testing::TempRepos;
    use otaq_core::usage::Tokens;
    use serde_json::json;

    use super::*;
    use crate::api::request;
    use crate::service::testing::{Harness, connected};

    fn project(key: &str, name: &str, working: u32, needs: u32) -> ProjectInfo {
        ProjectInfo {
            key: key.into(),
            name: name.into(),
            repo_path: format!("/code/{name}"),
            base_branch: "main".into(),
            agents_working: working,
            needs_you: needs,
        }
    }

    #[test]
    fn the_header_shows_the_shown_project_and_the_global_counts() {
        let projects = [project("SHOP", "shop-web", 2, 6), project("BILL", "billing-api", 1, 1)];
        let settings = GlobalSettings::default();
        let summary = UsageSummary {
            today: Tokens {
                output: 1_840_000,
                ..Tokens::default()
            },
            ..UsageSummary::default()
        };
        let view = header_view(&projects, Some("BILL"), &settings, None, 7, Some(&summary), &[], 0);
        let shown = view.project.unwrap();
        assert_eq!((shown.name.as_str(), shown.base_branch.as_str()), ("billing-api", "main"));
        assert_eq!(view.usage.unwrap().number, "1.84M");
        assert_eq!(view.plan_meter, None, "no status-line data: hidden");
        assert_eq!((view.agents.count.as_str(), view.agents.label), ("3", "running"));
        assert_eq!(
            view.agents_tooltip.unwrap().1,
            "shop-web 2 · billing-api 1"
        );
        assert_eq!(view.limits, None);
        assert_eq!(view.processes, None);
        assert_eq!((view.inbox.label, view.inbox.count), ("Needs you", 7));

        // Limits on: the chip becomes the "3 / 5 agents" control with its popover; no project shown.
        let mut on = settings.clone();
        on.agent_limits.enabled = true;
        let view = header_view(&projects, None, &on, None, 0, None, &[], 0);
        assert_eq!(view.project, None);
        assert_eq!(view.usage, None);
        assert_eq!((view.agents.count.as_str(), view.agents.label), ("3 / 5", "agents"));
        assert_eq!(view.agents_tooltip, None);
        assert_eq!(view.limits.unwrap().max_agents.value, 5);
        assert_eq!(view.inbox.label, "Inbox");
    }

    #[test]
    fn the_broom_names_its_leftovers() {
        assert_eq!(broom(0).tooltip, "Cleanup");
        assert_eq!(broom(1).tooltip, "Cleanup · 1 leftover");
        assert_eq!(broom(3).count, 3);
    }

    async fn add(h: &Harness, repos: &TempRepos, name: &str) -> ProjectInfo {
        let path = repos.repo(name, 1);
        match h
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
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_rail_header_and_switcher_come_from_the_real_service() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let repos = TempRepos::new();

        let empty = request(&h.app, "shell.rail", json!({ "project": null }))
            .await
            .unwrap();
        assert_eq!(empty["selected"], Value::Null);
        assert_eq!(empty["items"], json!([]));

        let shop = add(&h, &repos, "shop-web").await;
        let docs = add(&h, &repos, "docs-site").await;

        // No selection yet: the first project is shown.
        let rail = request(&h.app, "shell.rail", json!({ "project": null }))
            .await
            .unwrap();
        assert_eq!(rail["selected"], shop.key);
        assert_eq!(rail["items"][1]["title"], format!("docs-site · {}", docs.key));
        assert_eq!(rail["items"][1]["idle"], true);
        assert_eq!(rail["sidebar_from_width"], 1600);

        // The page's selection is kept; a project that is gone falls back to the first.
        let rail = request(&h.app, "shell.rail", json!({ "project": docs.key }))
            .await
            .unwrap();
        assert_eq!(rail["selected"], docs.key);
        assert_eq!(rail["items"][1]["selected"], true);
        let rail = request(&h.app, "shell.rail", json!({ "project": "GONE" }))
            .await
            .unwrap();
        assert_eq!(rail["selected"], shop.key);

        assert_eq!(
            request(&h.app, "shell.project_for_digit", json!({ "digit": 2 }))
                .await
                .unwrap(),
            json!(docs.key)
        );
        assert_eq!(
            request(&h.app, "shell.project_for_digit", json!({ "digit": 3 }))
                .await
                .unwrap(),
            Value::Null
        );

        let quick = request(&h.app, "shell.quick", json!({ "filter": "docs" }))
            .await
            .unwrap();
        assert_eq!(quick["rows"].as_array().unwrap().len(), 1);
        assert_eq!(quick["rows"][0]["project"]["key"], docs.key);
        assert_eq!(quick["hints"][2]["label"], "add project");

        let header = request(&h.app, "shell.header", json!({ "project": shop.key }))
            .await
            .unwrap();
        assert_eq!(header["project"]["name"], "shop-web");
        assert_eq!(header["agents"]["label"], "running");
        assert_eq!(header["inbox"]["label"], "Inbox");
        assert_eq!(header["processes"], Value::Null);

        let broom = request(&h.app, "shell.cleanup", Value::Null).await.unwrap();
        assert_eq!(broom["count"], 0);
        h.service.stop_all().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_stepper_press_saves_the_limits_at_once() {
        let h = Harness::new(None);
        h.wait_for("connected", connected).await;
        let mut settings = settings(&h.app).await.unwrap();
        settings.agent_limits.enabled = true;
        h.app
            .service
            .send(Request::SettingsSet(settings))
            .await
            .unwrap();

        let popover = request(
            &h.app,
            "shell.step_limit",
            json!({ "field": "max_agents", "up": true }),
        )
        .await
        .unwrap();
        assert_eq!(popover["max_agents"]["value"], 6);
        let popover = request(
            &h.app,
            "shell.step_limit",
            json!({ "field": "back_pressure", "up": false }),
        )
        .await
        .unwrap();
        assert_eq!(popover["back_pressure"]["value"], 2);
        let stored = super::settings(&h.app).await.unwrap().agent_limits;
        assert_eq!(
            stored,
            AgentLimits {
                enabled: true,
                max_concurrency: 6,
                back_pressure: 2
            }
        );
        let header = request(&h.app, "shell.header", json!({ "project": null }))
            .await
            .unwrap();
        assert_eq!(header["agents"]["count"], "0 / 6");
        h.service.stop_all().await;
    }
}
