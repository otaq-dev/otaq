//! The banner area and the toasts (T-0231; UI-07, SVC-03, SVC-04, USG-06, REC-06), as the GPUI app's root kept them:
//! one [`BannerModel`] and one [`ToastModel`] for the window, fed by the link's states (service banners, the
//! after-restart summary, "Reconnected"), the plan limit (`usage.summary` on `usage_changed`, `ticket_changed` and
//! resyncs) and the service events that show a toast ([`EVENT_TOASTS`]). A one-second tick times the success toasts
//! out and counts the usage limit down.
//!
//! The page draws what [`Notices::view`] returns: it loads it with `notices.view` and gets every change as a
//! `notices` event. Nothing here restarts the service or resumes an agent on its own: those are the page's requests,
//! sent on the user's click (d0022, d0020).

use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use crewboard_core::attention::notify::{OPEN_INBOX, open_env_var};
use crewboard_core::client::{Client, Compatibility};
use crewboard_core::domain::TicketKey;
use crewboard_core::protocol::{Event, Reply, Request, UsageParams};
use crewboard_core::store::now_ms;
use crewboard_core::usage::plan::PlanLimits;
use crewboard_view::view_models::after_restart::{OnConnect, on_connect};
use crewboard_view::view_models::banners::{BannerModel, BannerView, Situation, SituationKind};
use crewboard_view::view_models::connection::{ConnectionViewModel, LinkState};
use crewboard_view::view_models::limit_banner::{self, LimitBanner};
use crewboard_view::view_models::restart;
use crewboard_view::view_models::toasts::{
    NewToast, RECONNECTED, ToastLevel, ToastModel, ToastView,
};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use ts_rs::TS;

use crate::events::{EventHub, PageEvent};
use crate::service::ServiceLink;

/// How often success toasts are timed out and the usage limit's countdown is recomputed.
const TICK: Duration = Duration::from_secs(1);
/// A `usage.summary` that takes longer is given up; the next change asks again.
const USAGE_TIMEOUT: Duration = Duration::from_secs(5);
/// The service events after which the plan limit and the agents waiting for it are fetched again.
const USAGE_EVENTS: &[&str] = &["usage_changed", "ticket_changed"];

/// The service events that show a toast: one registration per kind, each turning its event into the toast (with the
/// wording the service sent) or passing.
pub const EVENT_TOASTS: &[fn(&Event) -> Option<NewToast>] = &[
    merged_automatically,
    validation_cancelled,
    crewboard_view::view_models::run_page::stopped_toast,
];

/// "SHOP-40.1 merged automatically into the Wishlist branch" · "Validated · round 1 · local, not pushed · SHOP-40.2
/// can start" (UI-07, d0064; *11 → Success toasts*). A click opens the ticket.
fn merged_automatically(event: &Event) -> Option<NewToast> {
    let Event::MergedAutomatically {
        key,
        title,
        message,
        ..
    } = event
    else {
        return None;
    };
    Some(ticket_toast(key, title, message))
}

/// "Validation cancelled: SHOP-29’s code changed" · "Round not counted · the agent was told to submit again" (UI-07,
/// d0063).
fn validation_cancelled(event: &Event) -> Option<NewToast> {
    let Event::ValidationCancelled {
        key,
        title,
        message,
        ..
    } = event
    else {
        return None;
    };
    Some(ticket_toast(key, title, message))
}

fn ticket_toast(key: &TicketKey, title: &str, message: &str) -> NewToast {
    NewToast {
        ticket: Some(key.to_string()),
        ..NewToast::new(
            ToastLevel::Success,
            title,
            (!message.is_empty()).then(|| message.to_owned()),
        )
    }
}

/// The banner rows and the toasts as the page draws them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "Notices")]
pub struct NoticesView {
    /// At most two rows, top first.
    pub banners: Vec<BannerView>,
    /// Oldest first; the newest sits at the bottom.
    pub toasts: Vec<ToastView>,
    /// "Restart required": everything else that talks to the service is disabled.
    pub only_restart_allowed: bool,
    /// Whether the board, rail and panel take input: not while connecting, reconnecting or stopped (SVC-04), nor
    /// while only Restart works. The page dims them and swallows their clicks meanwhile.
    pub input_enabled: bool,
    /// The service start the after-restart summary was raised for: the page opens the summary sheet once per
    /// value (*16 → Opening after a reboot*, T-0235).
    #[ts(type = "number | null")]
    pub summary_for: Option<i64>,
}

/// What a notification click opens (`CREWBOARD_OPEN`, or the `open_from_notification` event): a ticket with its
/// terminal focused, or the inbox when both are absent (a grouped notification).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "NotificationTarget")]
pub struct OpenTarget {
    pub project: Option<String>,
    pub ticket: Option<String>,
}

impl OpenTarget {
    /// From a notification's value: a ticket key, or `inbox`. Anything else opens nothing.
    pub fn parse(value: &str) -> Option<Self> {
        if value == OPEN_INBOX {
            return Some(Self::inbox());
        }
        value
            .parse::<TicketKey>()
            .ok()
            .map(|key| Self::ticket(&key))
    }

    pub fn ticket(key: &TicketKey) -> Self {
        Self {
            project: Some(key.project.to_string()),
            ticket: Some(key.to_string()),
        }
    }

    pub fn inbox() -> Self {
        Self {
            project: None,
            ticket: None,
        }
    }
}

pub struct Notices {
    state: Mutex<State>,
    events: Arc<EventHub>,
}

struct State {
    banners: BannerModel,
    toasts: ToastModel,
    link: LinkState,
    /// The plan state and the agents waiting for the reset, as last fetched.
    usage: Option<(Option<PlanLimits>, u32)>,
    limit: Option<LimitBanner>,
    /// When the app opened, the service start a summary was shown for, whether the app has connected yet and
    /// whether it was disconnected at the last state (REC-06).
    opened_ms: i64,
    summary_for: Option<i64>,
    connected_once: bool,
    was_disconnected: bool,
    /// A start or restart the user asked for is running.
    busy: bool,
    /// What a notification click asked this app to open when it started, until the page takes it.
    open_at_start: Option<OpenTarget>,
    published: Option<NoticesView>,
}

impl Notices {
    /// The notices of a window, following `service` from now on. Call inside the app's tokio runtime; the task ends
    /// when the notices are dropped.
    pub fn start(service: &ServiceLink, events: Arc<EventHub>) -> Arc<Self> {
        let open_at_start = std::env::var(open_env_var())
            .ok()
            .and_then(|v| OpenTarget::parse(&v));
        let notices = Arc::new(Self::new(events.clone(), now_ms(), open_at_start));
        let (sender, receiver) = mpsc::unbounded_channel();
        events.subscribe(Box::new(move |event| sender.send(event).is_ok()));
        tokio::spawn(follow(
            Arc::downgrade(&notices),
            service.states(),
            service.clients(),
            receiver,
        ));
        notices
    }

    fn new(events: Arc<EventHub>, opened_ms: i64, open_at_start: Option<OpenTarget>) -> Self {
        Self {
            state: Mutex::new(State {
                banners: BannerModel::default(),
                toasts: ToastModel::default(),
                link: LinkState::Connecting,
                usage: None,
                limit: None,
                opened_ms,
                summary_for: None,
                connected_once: false,
                was_disconnected: false,
                busy: false,
                open_at_start,
                published: None,
            }),
            events,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("notices")
    }

    pub fn view(&self) -> NoticesView {
        self.lock().view()
    }

    /// Tells the page when what it draws changed.
    pub fn publish(&self) {
        let view = {
            let mut state = self.lock();
            let view = state.view();
            if state.published.as_ref() == Some(&view) {
                return;
            }
            state.published = Some(view.clone());
            view
        };
        self.events.publish(&PageEvent::Notices { notices: view });
    }

    /// The link's state changed: raises and clears the service banners, the after-restart summary and the
    /// "Reconnected" toast. Returns whether the summary was raised.
    pub fn on_link(&self, link: &LinkState) -> bool {
        let summarized = self.lock().on_link(link, Instant::now());
        self.publish();
        summarized
    }

    pub fn toast(&self, toast: NewToast) -> u64 {
        let id = self.lock().toasts.push_toast(toast, Instant::now());
        self.publish();
        id
    }

    pub fn dismiss_toast(&self, id: u64) {
        self.lock().toasts.dismiss(id);
        self.publish();
    }

    pub fn hold_toast(&self, id: u64, held: bool) {
        self.lock().toasts.hold(id, held, Instant::now());
        self.publish();
    }

    /// The user closed a banner. Banners that can't be closed stay.
    pub fn dismiss_banner(&self, kind: SituationKind) {
        self.lock().banners.dismiss(kind);
        self.publish();
    }

    pub fn raise(&self, situation: Situation) {
        self.lock().banners.raise(situation);
        self.publish();
    }

    pub fn clear(&self, kind: SituationKind) {
        self.lock().banners.clear(kind);
        self.publish();
    }

    /// The agent counts for the restart confirmation (0, 0 while not connected).
    pub fn agent_counts(&self) -> (u32, u32) {
        match &self.lock().link {
            LinkState::Connected(info) => (info.status.agents_working, info.status.agents_waiting),
            _ => (0, 0),
        }
    }

    /// A start or restart begins: its button says so and can't be pressed again. Returns the service start the
    /// summary was last shown for, to tell afterwards whether the restart raised one.
    pub fn begin_restart(&self) -> Option<i64> {
        let summary_for = {
            let mut state = self.lock();
            state.busy = true;
            state.summary_for
        };
        self.publish();
        summary_for
    }

    /// The start or restart ended. `link` is the state now (the link may not have reported it yet): a summary banner
    /// says what was interrupted; otherwise a toast says it worked, or why not.
    pub fn end_restart(
        &self,
        outcome: Result<(), String>,
        summary_before: Option<i64>,
        link: &LinkState,
    ) {
        {
            let mut state = self.lock();
            let now = Instant::now();
            state.on_link(link, now);
            state.busy = false;
            match outcome {
                Ok(()) if state.summary_for != summary_before => {}
                Ok(()) => {
                    state.toasts.push(
                        ToastLevel::Success,
                        restart::RESTARTED,
                        Some(restart::RESTARTED_TEXT.to_owned()),
                        now,
                    );
                }
                Err(reason) => {
                    state.toasts.push(
                        ToastLevel::Error,
                        restart::RESTART_FAILED,
                        Some(reason),
                        now,
                    );
                }
            }
        }
        self.publish();
    }

    /// The plan state and the agents waiting for its reset, freshly fetched.
    pub fn on_usage(&self, plan: Option<PlanLimits>, waiting: u32) {
        {
            let mut state = self.lock();
            state.usage = Some((plan, waiting));
            state.sync_limit(now_ms());
        }
        self.publish();
    }

    /// The target `CREWBOARD_OPEN` named when the app started, once.
    pub fn take_open_at_start(&self) -> Option<OpenTarget> {
        self.lock().open_at_start.take()
    }

    fn tick(&self) {
        {
            let mut state = self.lock();
            state.toasts.expire(Instant::now());
            state.sync_limit(now_ms());
        }
        self.publish();
    }
}

impl State {
    fn view(&self) -> NoticesView {
        NoticesView {
            banners: self
                .banners
                .rows()
                .iter()
                .map(|row| row.view(self.busy))
                .collect(),
            toasts: self.toasts.items().iter().map(|t| t.view()).collect(),
            only_restart_allowed: self.banners.only_restart_allowed(),
            input_enabled: ConnectionViewModel::new(&self.link).input_enabled
                && !self.banners.only_restart_allowed(),
            summary_for: self.summary_for,
        }
    }

    fn on_link(&mut self, link: &LinkState, now: Instant) -> bool {
        let situations = match link {
            LinkState::Connecting => Vec::new(),
            LinkState::Connected(info) => {
                let (working, waiting) = (info.status.agents_working, info.status.agents_waiting);
                match info.compatibility {
                    Compatibility::Same => Vec::new(),
                    Compatibility::ServiceOlder => {
                        vec![Situation::ServiceUpdateReady { working, waiting }]
                    }
                    Compatibility::Incompatible => {
                        vec![Situation::RestartRequired { working, waiting }]
                    }
                }
            }
            LinkState::Reconnecting { .. } => vec![Situation::Reconnecting],
            LinkState::Stopped => vec![Situation::ServiceStopped],
            // Q40: the stopped banner says why the service couldn't start.
            LinkState::Failed(reason) => vec![Situation::ServiceCouldNotStart {
                reason: reason.clone(),
            }],
        };
        for kind in [
            SituationKind::ServiceStopped,
            SituationKind::RestartRequired,
            SituationKind::ServiceUpdateReady,
            SituationKind::Reconnecting,
        ] {
            if !situations.iter().any(|s| s.kind() == kind) {
                self.banners.clear(kind);
            }
        }
        for situation in situations {
            self.banners.raise(situation);
        }
        let disconnected = matches!(
            link,
            LinkState::Reconnecting { .. } | LinkState::Stopped | LinkState::Failed(_)
        );
        // After a reboot, crash or restart: the summary of what was interrupted; after a normal reopen a quiet
        // "Reconnected" (REC-06).
        let mut summarized = false;
        if let LinkState::Connected(info) = link {
            match on_connect(
                &info.status,
                self.opened_ms,
                !self.connected_once,
                self.summary_for,
            ) {
                OnConnect::Summary(situation) => {
                    self.summary_for = Some(info.status.started_at);
                    self.banners.raise(situation);
                    summarized = true;
                }
                OnConnect::Reconnected => {
                    self.toasts
                        .push(ToastLevel::Success, RECONNECTED, None, now);
                }
                OnConnect::Nothing => {}
            }
            self.connected_once = true;
            if self.was_disconnected && !self.busy && !summarized {
                self.toasts
                    .push(ToastLevel::Success, RECONNECTED, None, now);
            }
        }
        self.was_disconnected = disconnected;
        self.link = link.clone();
        summarized
    }

    /// Raises, updates or clears the one usage-limit banner (USG-06, P6).
    fn sync_limit(&mut self, now_ms: i64) {
        let banner = self
            .usage
            .as_ref()
            .and_then(|(plan, waiting)| limit_banner::banner(plan.as_ref(), *waiting, now_ms));
        if banner == self.limit {
            return;
        }
        match &banner {
            Some(b) => self.banners.raise(Situation::UsageLimit {
                title: b.title.clone(),
                text: b.text.clone(),
                resume: Some(b.resume.clone()),
            }),
            None => self.banners.clear(SituationKind::UsageLimit),
        }
        self.limit = banner;
    }
}

/// Follows the link's states, the page's event stream and the tick until the notices are dropped.
async fn follow(
    weak: Weak<Notices>,
    mut states: watch::Receiver<LinkState>,
    clients: watch::Receiver<Option<Client>>,
    mut events: mpsc::UnboundedReceiver<Value>,
) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut usage_due = true;
    let first = states.borrow_and_update().clone();
    if let Some(notices) = weak.upgrade() {
        notices.on_link(&first);
    }
    loop {
        tokio::select! {
            changed = states.changed() => {
                if changed.is_err() {
                    return;
                }
                let state = states.borrow_and_update().clone();
                let Some(notices) = weak.upgrade() else { return };
                if matches!(state, LinkState::Connected(_)) {
                    usage_due = true;
                }
                notices.on_link(&state);
            }
            event = events.recv() => {
                let Some(event) = event else { return };
                let Some(notices) = weak.upgrade() else { return };
                match event["kind"].as_str() {
                    Some("resync") => usage_due = true,
                    Some("service") => {
                        let name = event["name"].as_str().unwrap_or_default();
                        usage_due |= USAGE_EVENTS.contains(&name);
                        if let Ok(event) = serde_json::from_value::<Event>(event["event"].clone()) {
                            for toast in EVENT_TOASTS.iter().filter_map(|to_toast| to_toast(&event)) {
                                notices.toast(toast);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = tick.tick() => {
                let Some(notices) = weak.upgrade() else { return };
                let client = clients.borrow().clone();
                if usage_due && let Some(client) = client {
                    usage_due = false;
                    let summary = tokio::time::timeout(
                        USAGE_TIMEOUT,
                        client.request(Request::UsageSummary(UsageParams { project: None })),
                    )
                    .await;
                    // Not connected or too slow: the next change or resync asks again.
                    if let Ok(Ok(Reply::Usage(summary))) = summary {
                        notices.on_usage(summary.plan, summary.limit_waiting);
                    }
                }
                notices.tick();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::client::ServiceInfo;
    use crewboard_core::protocol::{PROTOCOL_VERSION, ServiceStatus};
    use crewboard_core::usage::limit::LimitKind;
    use crewboard_core::usage::plan::{ActiveLimit, RateLimits};

    use super::*;

    const OPENED: i64 = 1_791_600_000_000;

    fn notices() -> Notices {
        Notices::new(Arc::new(EventHub::default()), OPENED, None)
    }

    fn connected(compatibility: Compatibility, status: ServiceStatus) -> LinkState {
        LinkState::Connected(ServiceInfo {
            status: ServiceStatus {
                protocol_version: PROTOCOL_VERSION,
                ..status
            },
            compatibility,
        })
    }

    fn running(working: u32, waiting: u32) -> ServiceStatus {
        ServiceStatus {
            started_at: OPENED - 3_600_000,
            agents_working: working,
            agents_waiting: waiting,
            ..Default::default()
        }
    }

    fn titles(view: &NoticesView) -> Vec<(&str, &str)> {
        view.banners
            .iter()
            .map(|b| (b.title.as_str(), b.text.as_str()))
            .collect()
    }

    fn toasts(view: &NoticesView) -> Vec<&str> {
        view.toasts.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn the_service_banners_follow_the_link() {
        let n = notices();
        n.on_link(&connected(Compatibility::ServiceOlder, running(3, 2)));
        assert_eq!(
            titles(&n.view()),
            [(
                "Service update ready",
                "Restart now (3 agents working, 2 waiting) · or later, when it suits you"
            )]
        );
        assert_eq!(n.agent_counts(), (3, 2));
        n.on_link(&connected(Compatibility::Incompatible, running(3, 2)));
        let view = n.view();
        assert_eq!(view.banners.len(), 1, "one service banner at a time");
        assert_eq!(view.banners[0].title, "Restart required");
        assert!(view.only_restart_allowed);
        assert!(!view.input_enabled);
        n.on_link(&LinkState::Reconnecting { attempt: 1 });
        assert_eq!(titles(&n.view())[0].0, "Reconnecting…");
        assert!(!n.view().input_enabled, "SVC-04");
        n.on_link(&LinkState::Stopped);
        assert_eq!(titles(&n.view())[0].0, "Background service stopped");
        n.on_link(&connected(Compatibility::Same, running(0, 0)));
        assert!(n.view().banners.is_empty());
        assert!(!n.view().only_restart_allowed);
        assert!(n.view().input_enabled);
    }

    #[test]
    fn a_failed_start_says_why_in_the_stopped_banner() {
        let n = notices();
        n.on_link(&LinkState::Failed(
            "Couldn't start the background service: the data folder's path is too long".into(),
        ));
        let view = n.view();
        assert_eq!(view.banners.len(), 1);
        assert_eq!(view.banners[0].title, "Background service couldn’t start");
        assert!(view.banners[0].text.contains("path is too long"));
        assert_eq!(view.banners[0].buttons[0].label, "Start service");
    }

    #[test]
    fn a_normal_reopen_and_a_reconnect_are_a_quiet_toast() {
        let n = notices();
        n.on_link(&connected(Compatibility::Same, running(0, 0)));
        assert_eq!(toasts(&n.view()), [RECONNECTED]);
        n.dismiss_toast(n.view().toasts[0].id);
        n.on_link(&LinkState::Reconnecting { attempt: 1 });
        assert!(toasts(&n.view()).is_empty());
        n.on_link(&connected(Compatibility::Same, running(0, 0)));
        assert_eq!(toasts(&n.view()), [RECONNECTED]);
        assert!(n.view().banners.is_empty(), "reconnecting cleared");
    }

    #[test]
    fn after_a_reboot_the_summary_shows_once() {
        let n = notices();
        let rebooted = ServiceStatus {
            started_at: OPENED + 1_000,
            interrupted_at_start: 4,
            unclean_start: true,
            ..Default::default()
        };
        assert!(n.on_link(&connected(Compatibility::Same, rebooted.clone())));
        let view = n.view();
        assert_eq!(view.banners[0].title, "4 sessions were interrupted");
        assert!(toasts(&view).is_empty(), "the summary says it, no toast");
        assert_eq!(
            view.summary_for,
            Some(OPENED + 1_000),
            "the page opens the sheet"
        );
        assert!(!n.on_link(&connected(Compatibility::Same, rebooted)));
        n.dismiss_banner(SituationKind::SessionsInterrupted);
        assert!(n.view().banners.is_empty());
    }

    #[test]
    fn a_restart_ends_with_a_toast_or_the_summary() {
        let n = notices();
        n.on_link(&connected(Compatibility::ServiceOlder, running(3, 2)));
        n.dismiss_toast(n.view().toasts[0].id);
        let before = n.begin_restart();
        let view = n.view();
        assert_eq!(view.banners[0].buttons[0].label, "Restarting…");
        assert!(!view.banners[0].buttons[0].enabled);
        n.on_link(&LinkState::Reconnecting { attempt: 1 });
        // The new service interrupted nobody: a toast says it worked, not "Reconnected".
        let fresh = ServiceStatus {
            started_at: OPENED + 600_000,
            ..Default::default()
        };
        n.end_restart(Ok(()), before, &connected(Compatibility::Same, fresh));
        let view = n.view();
        assert!(view.banners.is_empty());
        assert_eq!(toasts(&view), [restart::RESTARTED]);

        // Interrupted sessions: the summary banner instead of the toast.
        let n = notices();
        n.on_link(&connected(Compatibility::ServiceOlder, running(3, 2)));
        let before = n.begin_restart();
        let restarted = ServiceStatus {
            started_at: OPENED + 600_000,
            interrupted_at_start: 3,
            ..Default::default()
        };
        n.end_restart(Ok(()), before, &connected(Compatibility::Same, restarted));
        let view = n.view();
        assert_eq!(view.banners[0].title, "Service restarted");
        assert_eq!(toasts(&view), [RECONNECTED], "only the first connect's");

        n.begin_restart();
        n.end_restart(Err("timed out".into()), None, &LinkState::Stopped);
        let view = n.view();
        let failed = view.toasts.last().unwrap();
        assert_eq!(failed.title, restart::RESTART_FAILED);
        assert_eq!(failed.text.as_deref(), Some("timed out"));
        assert_eq!(failed.level, ToastLevel::Error);
    }

    #[test]
    fn the_usage_limit_is_one_banner_that_follows_the_plan() {
        let n = notices();
        let limited = |resets_at: Option<i64>| PlanLimits {
            windows: RateLimits::default(),
            limit: Some(ActiveLimit {
                kind: LimitKind::Session,
                text: String::new(),
                hit_at: now_ms() - 60_000,
                resets_at,
                reset_source: None,
                reset_seen_at: None,
            }),
            observed_at: now_ms(),
        };
        n.on_usage(Some(limited(Some(now_ms() + 3 * 3_600_000))), 8);
        let view = n.view();
        assert_eq!(view.banners.len(), 1);
        assert_eq!(view.banners[0].title, "Session limit reached");
        assert!(
            !view.banners[0].buttons[0].enabled,
            "Resume all waits for the reset"
        );
        n.on_usage(Some(limited(None)), 2);
        let view = n.view();
        assert_eq!(view.banners.len(), 1, "updated, never a second banner");
        assert_eq!(view.banners[0].title, "Usage limit reached");
        assert!(view.banners[0].buttons[0].enabled);
        n.on_usage(None, 0);
        assert!(n.view().banners.is_empty());
    }

    #[test]
    fn changes_reach_the_page_once() {
        let hub = Arc::new(EventHub::default());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        hub.subscribe(Box::new(move |event| {
            record.lock().unwrap().push(event);
            true
        }));
        let n = Notices::new(hub, OPENED, None);
        n.raise(Situation::PermissionWarning);
        n.publish();
        n.tick();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "nothing changed after the raise: {seen:?}");
        assert_eq!(seen[0]["kind"], "notices");
        assert_eq!(
            seen[0]["notices"]["banners"][0]["title"],
            "Permission prompts are on"
        );
    }

    #[test]
    fn notification_targets() {
        assert_eq!(
            OpenTarget::parse("SHOP-22"),
            Some(OpenTarget {
                project: Some("SHOP".into()),
                ticket: Some("SHOP-22".into())
            })
        );
        assert_eq!(
            OpenTarget::parse("SHOP-40.1").unwrap().ticket.as_deref(),
            Some("SHOP-40.1")
        );
        assert_eq!(OpenTarget::parse(OPEN_INBOX), Some(OpenTarget::inbox()));
        assert_eq!(OpenTarget::parse("not a key"), None);
        let n = Notices::new(
            Arc::new(EventHub::default()),
            OPENED,
            OpenTarget::parse("SHOP-22"),
        );
        assert!(n.take_open_at_start().is_some());
        assert_eq!(n.take_open_at_start(), None, "only once");
    }

    #[test]
    fn auto_merge_and_a_cancelled_validation_show_their_toasts() {
        use crewboard_core::domain::validation::{CancelledBy, cancelled_toast, merged_toast};
        let n = notices();
        let key: TicketKey = "SHOP-40.1".parse().unwrap();
        let (title, message) =
            merged_toast("SHOP-40.1", "the Wishlist branch", 1, &["SHOP-40.2".into()]);
        let merged = Event::MergedAutomatically {
            project: "SHOP".into(),
            key: key.clone(),
            title,
            message,
        };
        let (title, message) = cancelled_toast("SHOP-29", CancelledBy::Code);
        let cancelled = Event::ValidationCancelled {
            project: "SHOP".into(),
            key: "SHOP-29".parse().unwrap(),
            title,
            message,
        };
        let other = Event::InboxChanged {
            counts: Default::default(),
        };
        for event in [&merged, &cancelled, &other] {
            for toast in EVENT_TOASTS.iter().filter_map(|to_toast| to_toast(event)) {
                n.toast(toast);
            }
        }
        let view = n.view();
        assert_eq!(
            toasts(&view),
            [
                "SHOP-40.1 merged automatically into the Wishlist branch",
                "Validation cancelled: SHOP-29\u{2019}s code changed"
            ]
        );
        assert_eq!(
            view.toasts[0].text.as_deref(),
            Some("Validated · round 1 · local, not pushed · SHOP-40.2 can start")
        );
        assert_eq!(view.toasts[0].ticket.as_deref(), Some("SHOP-40.1"));
        assert_eq!(
            view.toasts[0].icon,
            Some(crewboard_view::icons::IconName::GitMerge)
        );
        assert_eq!(
            view.toasts[1].text.as_deref(),
            Some("Round not counted · the agent was told to submit again")
        );
        assert_eq!(
            view.toasts[1].icon,
            Some(crewboard_view::icons::IconName::Ban)
        );
        assert_eq!(view.toasts[1].level, ToastLevel::Success);
    }
}
