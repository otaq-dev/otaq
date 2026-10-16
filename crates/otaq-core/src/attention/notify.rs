//! OS notifications (ATT-11; d0016, d0042): only for items that **block an agent** (the needs-you family and
//! Interrupted), never for Ready for Review, a ticket whose terminal has focus or an agent stopped by the plan
//! limit (both already left out of the needs-you items), and **once per item**, not per event. A global setting
//! turns them off (on by default), and it applies at the next derivation.
//!
//! An item is the ticket in that status since that moment: the same question still waiting after the user looked
//! at its terminal and left is not notified again; a new question later is. More than three new items of one
//! project at once become one notification for the project (frame *11 → OS notifications*: "grouped per project
//! after 3").
//!
//! From M12 (d0063, d0064; frame *11 → OS notifications → macOS · M12*): a validator's permission prompt,
//! "Validation failed N times", "Validation ended without a result" and the Auto-merge stops notify too, as turquoise
//! items; failed rounds that go back to the agent and auto-merges raise no item, so they never notify.
//!
//! [`should_notify`] and [`plan`] are the pure decision; [`Notifier`] posts. The macOS notifier runs
//! `osascript`, which works from the plain binary; **clicking a notification can't open a ticket that way** (that
//! needs an app bundle, questions.md Q22). On Linux `notify-send` posts through the desktop's notification
//! service and reports a click; the service then opens the app on [`OsNotification::open`] with its terminal
//! focused (T-0159, Q34). On Windows a toast is shown under Windows PowerShell's identity; Windows hands its
//! click only to a registered app, so there, as on macOS, a click doesn't reach Otaq.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::board::needs_you::{NeedsYouGroup, NeedsYouItem};
use crate::domain::{Status, TicketKey, validation};
use crate::naming;
use crate::signals::hooks::HELPER_PROMPT_PREFIX;

/// More new items than this in one project at once are grouped into one notification.
pub const GROUP_AFTER: usize = 3;

/// One OS notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsNotification {
    /// "shop-web · SHOP-22 needs permission".
    pub title: String,
    /// "Allow rm -rf .next? — Fix flaky login redirect".
    pub body: String,
    /// The ticket a click opens with its terminal focused; `None` for a grouped notification (opens the inbox).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<TicketKey>,
}

/// What a notification says beyond its item: the question, prompt, last line or stop message (for "Validation
/// failed N times", the first finding) and N.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemText {
    pub text: Option<String>,
    /// "Validation failed N times": the rounds that failed.
    pub failed_times: u32,
}

impl From<&str> for ItemText {
    fn from(text: &str) -> Self {
        Self {
            text: Some(text.to_owned()),
            failed_times: 0,
        }
    }
}

/// An item, as far as "once per item" is concerned.
pub type ItemId = (String, Status, i64);

pub fn item_id(item: &NeedsYouItem) -> ItemId {
    (item.id.clone(), item.status, item.since)
}

/// Whether an item blocks an agent: turquoise, or Interrupted (d0016).
pub fn should_notify(item: &NeedsYouItem) -> bool {
    // An agent's question to a finished ticket blocks no agent (the ask is non-blocking): no notification.
    item.message.is_none()
        && (item.group == NeedsYouGroup::Family || item.status == Status::Interrupted)
}

/// The notifications for `items` (the current needs-you items), given the items notified before; `enabled` is the
/// setting. Every eligible item is remembered in `seen`, notified or not, so turning the setting on later doesn't
/// bring old items up.
pub fn plan(
    items: &[NeedsYouItem],
    texts: &HashMap<String, ItemText>,
    seen: &mut HashSet<ItemId>,
    enabled: bool,
) -> Vec<OsNotification> {
    let fresh: Vec<&NeedsYouItem> = items
        .iter()
        .filter(|i| should_notify(i))
        .filter(|i| seen.insert(item_id(i)))
        .collect();
    if !enabled {
        return Vec::new();
    }
    let mut by_project: Vec<(&str, Vec<&NeedsYouItem>)> = Vec::new();
    for item in fresh {
        match by_project.iter_mut().find(|(p, _)| *p == item.project) {
            Some((_, list)) => list.push(item),
            None => by_project.push((&item.project, vec![item])),
        }
    }
    by_project
        .into_iter()
        .flat_map(|(_, list)| {
            if list.len() > GROUP_AFTER {
                vec![grouped(&list)]
            } else {
                list.iter()
                    .map(|i| one(i, texts.get(&i.id).cloned().unwrap_or_default()))
                    .collect()
            }
        })
        .collect()
}

/// The frame's texts: `text` is the item's question, prompt or last line.
fn one(item: &NeedsYouItem, facts: ItemText) -> OsNotification {
    let text = facts
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let who = format!("{} · {}", item.project_name, item.key);
    let decide = format!("{} · decide in {}", item.title, naming::PRODUCT_NAME);
    let failed;
    let (what, body) = match item.status {
        Status::AsksYou => ("asks a question", text.unwrap_or(&item.title).to_owned()),
        Status::NeedsYou => (
            "needs permission",
            match text {
                Some(t) if t.starts_with(HELPER_PROMPT_PREFIX) => t.to_owned(),
                Some(t) => format!("{t} — {}", item.title),
                None => item.title.clone(),
            },
        ),
        Status::Idle => (
            "ended its turn",
            match text {
                Some(t) => format!("{t} — {}", item.title),
                None => item.title.clone(),
            },
        ),
        // macOS · M12: "shop-web · SHOP-33 validator permission" · "validator: Allow Bash(pnpm test)? — Shipping
        // estimate".
        Status::ValidatorNeedsYou => (
            "validator permission",
            match text {
                Some(t) => format!("validator: {t} — {}", item.title),
                None => item.title.clone(),
            },
        ),
        // "shop-web · SHOP-32 Validation failed 3 times" · "Arrow keys do nothing — Address autocomplete · decide in
        // Otaq".
        Status::ValidationFailed => {
            failed = validation::failed_times(facts.failed_times.max(1));
            (
                failed.as_str(),
                match text {
                    Some(finding) => format!("{finding} — {decide}"),
                    None => decide,
                },
            )
        }
        // The frame's note names these two without texts; they read like the two above.
        Status::ValidationNoResult => (
            text.filter(|t| *t == validation::DISCARDED)
                .unwrap_or(validation::NO_RESULT),
            decide,
        ),
        Status::AutoMergeStopped => (
            "Auto-merge stopped",
            match text {
                Some(t) => format!("{} — {}", t.replace('`', ""), item.title),
                None => item.title.clone(),
            },
        ),
        _ => (
            "session interrupted",
            format!("{} · Resume from {}", item.title, naming::PRODUCT_NAME),
        ),
    };
    OsNotification {
        title: format!("{who} {what}"),
        body,
        open: Some(item.key.clone()),
    }
}

/// The notifications for waiting Project agents (PAG-07; frame *11 → OS notifications → macOS · M12*): "shop-web ·
/// Project agent asks a question" / "needs permission", the question or prompt and the agent's name as the body.
/// Once per wait, like tickets' items; focused agents raise no item, so they never notify.
pub fn plan_agents(
    items: &[crate::project_agents::AgentNeedsYou],
    seen: &mut HashSet<ItemId>,
    enabled: bool,
) -> Vec<OsNotification> {
    let fresh: Vec<_> = items
        .iter()
        .filter(|i| seen.insert((format!("agent:{}", i.agent), i.status, i.since)))
        .collect();
    if !enabled {
        return Vec::new();
    }
    fresh
        .into_iter()
        .map(|i| {
            let (what, text) = match &i.question {
                Some(q) => ("asks a question", q.text.trim().to_owned()),
                None => (
                    "needs permission",
                    i.reason
                        .strip_prefix("permission: ")
                        .unwrap_or(&i.reason)
                        .to_owned(),
                ),
            };
            OsNotification {
                title: format!("{} · Project agent {what}", i.project_name),
                body: format!("{text} — {}", i.name),
                open: None,
            }
        })
        .collect()
}

fn grouped(list: &[&NeedsYouItem]) -> OsNotification {
    let keys: Vec<String> = list.iter().map(|i| i.key.to_string()).collect();
    OsNotification {
        title: format!("{} · {} agents need you", list[0].project_name, list.len()),
        body: keys.join(", "),
        open: None,
    }
}

/// What a click on a notification does. Called at most once, from any thread.
pub type OnClick = Box<dyn FnOnce() + Send>;

/// Posts OS notifications.
pub trait Notifier: Send + Sync {
    fn post(&self, notification: &OsNotification, on_click: OnClick);
}

/// Posts nothing (tests).
pub struct NoNotifier;

impl Notifier for NoNotifier {
    fn post(&self, _notification: &OsNotification, _on_click: OnClick) {}
}

/// The longest a notification is watched for a click; after that a click does nothing.
const CLICK_WATCH: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// macOS: `osascript`'s `display notification`, which works without an app bundle. The texts go in as
/// arguments, never into the script. A click shows the notification's sender, not the ticket (Q22).
pub struct OsaScriptNotifier;

impl Notifier for OsaScriptNotifier {
    fn post(&self, n: &OsNotification, _on_click: OnClick) {
        let result = std::process::Command::new("osascript")
            .args([
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                &n.title,
                &n.body,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Err(e) = result {
            tracing::warn!(error = %e, "can't post a notification");
        }
    }
}

/// Linux: `notify-send` (libnotify) through the desktop's notification service (freedesktop). It waits for the
/// notification to close and prints `default` when it was clicked; nothing is installed or configured.
pub struct NotifySendNotifier;

impl Notifier for NotifySendNotifier {
    fn post(&self, n: &OsNotification, on_click: OnClick) {
        let mut command = std::process::Command::new("notify-send");
        command
            .args([
                format!("--app-name={}", naming::PRODUCT_NAME),
                "--action=default=Open".to_owned(),
                "--wait".to_owned(),
                "--".to_owned(),
                n.title.clone(),
                n.body.clone(),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        watch_for_click(command, |out| out.trim() == "default", on_click);
    }
}

/// Windows: a toast through the Windows notification platform, attributed to Windows PowerShell's app identity,
/// which ships with every Windows (no identity of Otaq's own is registered, Q34). Windows hands a toast's
/// click only to an app registered for its identity, so a click can't reach Otaq: like macOS (Q22), the toast
/// tells, the app opens the ticket. It therefore carries no "Open terminal" button.
pub struct ToastNotifier;

/// The AppUserModelID of Windows PowerShell, registered by Windows itself.
pub const POWERSHELL_APP_ID: &str =
    r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

/// The toast's XML: the title and the text (frame *11 → OS notifications → Windows 11*).
pub fn toast_xml(title: &str, body: &str) -> String {
    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual>\
         </toast>",
        escape(title),
        escape(body)
    )
}

impl Notifier for ToastNotifier {
    fn post(&self, n: &OsNotification, _on_click: OnClick) {
        let xml = toast_xml(&n.title, &n.body);
        #[cfg(windows)]
        toast::post(xml);
        #[cfg(not(windows))]
        let _ = xml;
    }
}

#[cfg(windows)]
mod toast {
    use std::sync::OnceLock;
    use std::sync::mpsc::{Sender, channel};

    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
    use windows::core::HSTRING;

    use super::POWERSHELL_APP_ID;

    /// Hands the toast to the one notification thread, started on the first toast. Every toast goes through that
    /// thread, which initialises the Windows Runtime once and never ends: with a thread per toast, the second toast
    /// of a service crashed it (an access violation), as the runtime objects the first thread set up went away with
    /// that thread.
    pub fn post(xml: String) {
        static THREAD: OnceLock<Option<Sender<String>>> = OnceLock::new();
        let thread = THREAD.get_or_init(|| {
            let (sender, toasts) = channel::<String>();
            let spawned = std::thread::Builder::new()
                .name("notifications".into())
                .spawn(move || {
                    // SAFETY: initialises the Windows Runtime for this thread, which lives as long as the process.
                    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
                    for xml in toasts {
                        if let Err(e) = show(&xml) {
                            tracing::warn!(error = %e, "can't post a notification");
                        }
                    }
                });
            match spawned {
                Ok(_) => Some(sender),
                Err(e) => {
                    tracing::warn!(error = %e, "can't post a notification");
                    None
                }
            }
        });
        if let Some(sender) = thread {
            let _ = sender.send(xml);
        }
    }

    fn show(xml: &str) -> windows::core::Result<()> {
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(POWERSHELL_APP_ID))?
            .Show(&toast)
    }
}

/// Starts `command` and, on a thread of its own, waits up to [`CLICK_WATCH`] for it to end; calls `on_click` when
/// its output says the notification was clicked.
fn watch_for_click(
    mut command: std::process::Command,
    clicked: fn(&str) -> bool,
    on_click: OnClick,
) {
    let child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(program = ?command.get_program(), error = %e, "can't post a notification");
            return;
        }
    };
    let spawned = std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || {
            let mut child = child;
            let deadline = std::time::Instant::now() + CLICK_WATCH;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
            let mut out = String::new();
            if let Some(mut stdout) = child.stdout.take() {
                let _ = std::io::Read::read_to_string(&mut stdout, &mut out);
            }
            if clicked(&out) {
                on_click();
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "can't watch a notification");
    }
}

/// The notifier of this platform: `osascript` on macOS, `notify-send` on Linux, a PowerShell toast on Windows.
pub fn platform_notifier() -> std::sync::Arc<dyn Notifier> {
    if cfg!(target_os = "macos") {
        std::sync::Arc::new(OsaScriptNotifier)
    } else if cfg!(windows) {
        std::sync::Arc::new(ToastNotifier)
    } else {
        std::sync::Arc::new(NotifySendNotifier)
    }
}

/// What a click on a notification opens. An app connected to the service (one that subscribed to events) is
/// told to come forward on the ticket; with none, the app next to the service's binary is started on it (the
/// ticket in the [`open_env_var`] variable).
#[derive(Clone)]
pub struct Opener {
    events: crate::service::EventBus,
    subscribers: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    app: Option<std::path::PathBuf>,
}

/// Counts a subscribed client while it lives.
pub struct Subscribed(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl Drop for Subscribed {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Posting notifications and opening what a click asks for, together.
#[derive(Clone)]
pub struct Notifications {
    pub notifier: std::sync::Arc<dyn Notifier>,
    pub opener: Opener,
}

/// The variable that tells a starting app what to open: `OTAQ_OPEN` = a ticket key, or `inbox`.
pub fn open_env_var() -> String {
    naming::env_var("OPEN")
}

/// The value of [`open_env_var`] that opens the inbox.
pub const OPEN_INBOX: &str = "inbox";

impl Opener {
    pub fn new(events: crate::service::EventBus, app: Option<std::path::PathBuf>) -> Self {
        Self {
            events,
            subscribers: Default::default(),
            app,
        }
    }

    /// A client subscribed to events; it counts until the guard is dropped.
    pub fn subscribed(&self) -> Subscribed {
        self.subscribers
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Subscribed(self.subscribers.clone())
    }

    pub fn open(&self, key: Option<TicketKey>) {
        if self.subscribers.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            self.events
                .publish(crate::protocol::Event::OpenFromNotification { key });
            return;
        }
        let Some(app) = &self.app else {
            return;
        };
        let target = key.map_or_else(|| OPEN_INBOX.to_owned(), |k| k.to_string());
        let mut command = std::process::Command::new(app);
        command
            .env(open_env_var(), target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(
                crate::process::DETACHED_PROCESS | crate::process::CREATE_NEW_PROCESS_GROUP,
            );
        }
        if let Err(e) = command.spawn() {
            tracing::warn!(app = %app.display(), error = %e, "can't open the app from a notification");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-0201 (PAG-07, ATT-11): a waiting Project agent notifies once per wait, with its name; the setting off
    /// posts nothing.
    #[test]
    fn project_agents_notify_once_per_wait() {
        use crate::project_agents::AgentNeedsYou;
        let wait = |agent: &str, status: Status, since: i64| AgentNeedsYou {
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            agent: agent.into(),
            name: "Checkout slowness".into(),
            epic: None,
            status,
            reason: "permission: Allow pnpm test?".into(),
            question: (status == Status::AsksYou).then(|| crate::board::questions::HumanQuestion {
                id: "q".into(),
                text: "Profile in production too?".into(),
                options: Vec::new(),
                recommended: None,
                asked_at: since,
            }),
            since,
        };
        let mut seen = HashSet::new();
        let items = [
            wait("a1", Status::NeedsYou, 5),
            wait("a2", Status::AsksYou, 6),
        ];
        let out = plan_agents(&items, &mut seen, true);
        let texts: Vec<(&str, &str)> = out
            .iter()
            .map(|n| (n.title.as_str(), n.body.as_str()))
            .collect();
        assert_eq!(
            texts,
            [
                (
                    "shop-web · Project agent needs permission",
                    "Allow pnpm test? — Checkout slowness"
                ),
                (
                    "shop-web · Project agent asks a question",
                    "Profile in production too? — Checkout slowness"
                ),
            ]
        );
        assert!(out.iter().all(|n| n.open.is_none()));
        assert!(plan_agents(&items, &mut seen, true).is_empty(), "once");
        // A new wait later notifies again; with the setting off it is only remembered.
        let later = [wait("a1", Status::NeedsYou, 9)];
        assert!(plan_agents(&later, &mut seen, false).is_empty());
        assert!(plan_agents(&later, &mut seen, true).is_empty());
        assert_eq!(
            plan_agents(&[wait("a1", Status::NeedsYou, 12)], &mut seen, true).len(),
            1
        );
    }

    fn item(key: &str, status: Status, group: NeedsYouGroup, since: i64) -> NeedsYouItem {
        NeedsYouItem {
            group,
            project: "SHOP".into(),
            project_name: "shop-web".into(),
            id: key.into(),
            key: key.parse().unwrap(),
            title: match key {
                "SHOP-22" => "Fix flaky login redirect".into(),
                "SHOP-25" => "Upgrade to Next.js 15".into(),
                _ => "Split user full name".into(),
            },
            status,
            interrupted_from: None,
            question: None,
            reason: String::new(),
            since,
            age_ms: 0,
            blocked: 0,
            message: None,
        }
    }

    fn texts() -> HashMap<String, ItemText> {
        HashMap::from([
            ("SHOP-22".into(), "Allow rm -rf .next?".into()),
            (
                "SHOP-23".into(),
                "Should fullName be split everywhere, or only in the profile form?".into(),
            ),
        ])
    }

    #[test]
    fn only_blocking_items_notify() {
        use NeedsYouGroup::*;
        assert!(should_notify(&item("SHOP-22", Status::NeedsYou, Family, 1)));
        assert!(should_notify(&item("SHOP-23", Status::AsksYou, Family, 1)));
        assert!(should_notify(&item("SHOP-19", Status::Idle, Family, 1)));
        assert!(should_notify(&item(
            "SHOP-25",
            Status::Interrupted,
            Stopped,
            1
        )));
        assert!(!should_notify(&item(
            "SHOP-28",
            Status::StartInterrupted,
            Stopped,
            1
        )));
        assert!(!should_notify(&item(
            "SHOP-30",
            Status::ParentClosed,
            Stopped,
            1
        )));
        assert!(!should_notify(&item(
            "SHOP-18",
            Status::ReadyForReview,
            Review,
            1
        )));
    }

    #[test]
    fn the_frame_texts_once_per_item() {
        use NeedsYouGroup::*;
        let items = vec![
            item("SHOP-22", Status::NeedsYou, Family, 10),
            item("SHOP-23", Status::AsksYou, Family, 20),
            item("SHOP-25", Status::Interrupted, Stopped, 30),
            item("SHOP-18", Status::ReadyForReview, Review, 40),
        ];
        let mut seen = HashSet::new();
        let out = plan(&items, &texts(), &mut seen, true);
        let resume = format!(
            "Upgrade to Next.js 15 · Resume from {}",
            naming::PRODUCT_NAME
        );
        let shown: Vec<(&str, &str)> = out
            .iter()
            .map(|n| (n.title.as_str(), n.body.as_str()))
            .collect();
        assert_eq!(
            shown,
            [
                (
                    "shop-web · SHOP-22 needs permission",
                    "Allow rm -rf .next? — Fix flaky login redirect"
                ),
                (
                    "shop-web · SHOP-23 asks a question",
                    "Should fullName be split everywhere, or only in the profile form?"
                ),
                ("shop-web · SHOP-25 session interrupted", resume.as_str()),
            ]
        );
        assert_eq!(out[0].open, Some("SHOP-22".parse().unwrap()));
        // Nothing again for the same items (another event, a focus round trip).
        assert!(plan(&items, &texts(), &mut seen, true).is_empty());
        // A new question from the same ticket is a new item.
        let again = vec![item("SHOP-23", Status::AsksYou, Family, 99)];
        assert_eq!(plan(&again, &texts(), &mut seen, true).len(), 1);
    }

    /// T-0196 (ATT-11, d0063, d0064): the M12 items notify with frame *macOS · M12*'s texts; failed rounds that
    /// go back to the agent (Working) and auto-merges (Done) are no items, so they never notify.
    #[test]
    fn the_m12_texts() {
        use NeedsYouGroup::*;
        let titled = |key: &str, title: &str, status: Status| NeedsYouItem {
            title: title.into(),
            ..item(key, status, Family, 1)
        };
        let items = vec![
            titled("SHOP-33", "Shipping estimate", Status::ValidatorNeedsYou),
            titled("SHOP-32", "Address autocomplete", Status::ValidationFailed),
            titled("SHOP-34", "Tax display", Status::ValidationNoResult),
            titled("SHOP-35", "Wishlist share link", Status::AutoMergeStopped),
        ];
        assert!(items.iter().all(should_notify));
        let texts = HashMap::from([
            (
                "SHOP-33".to_owned(),
                ItemText::from("Allow Bash(pnpm test)?"),
            ),
            (
                "SHOP-32".to_owned(),
                ItemText {
                    text: Some("Arrow keys do nothing".into()),
                    failed_times: 3,
                },
            ),
            (
                "SHOP-35".to_owned(),
                ItemText::from("Keeps conflicting with `wishlist`"),
            ),
        ]);
        // Two at a time: more than three at once would be grouped.
        let mut seen = HashSet::new();
        let mut out = plan(&items[..2], &texts, &mut seen, true);
        out.extend(plan(&items, &texts, &mut seen, true));
        let shown: Vec<(&str, &str)> = out
            .iter()
            .map(|n| (n.title.as_str(), n.body.as_str()))
            .collect();
        let decide = |title: &str| format!("{title} · decide in {}", naming::PRODUCT_NAME);
        let failed = format!("Arrow keys do nothing — {}", decide("Address autocomplete"));
        let no_result = decide("Tax display");
        assert_eq!(
            shown,
            [
                (
                    "shop-web · SHOP-33 validator permission",
                    "validator: Allow Bash(pnpm test)? — Shipping estimate"
                ),
                (
                    "shop-web · SHOP-32 Validation failed 3 times",
                    failed.as_str()
                ),
                (
                    "shop-web · SHOP-34 Validation ended without a result",
                    no_result.as_str()
                ),
                (
                    "shop-web · SHOP-35 Auto-merge stopped",
                    "Keeps conflicting with wishlist — Wishlist share link"
                ),
            ]
        );
        for quiet in [
            Status::Working,
            Status::Validating,
            Status::MergeConflictFixing,
            Status::Merged,
        ] {
            assert!(
                crate::board::needs_you::group_of(quiet).is_none(),
                "{quiet:?} raises no item, so no notification"
            );
        }
    }

    #[test]
    fn off_means_off_and_old_items_stay_quiet() {
        let items = vec![item("SHOP-22", Status::NeedsYou, NeedsYouGroup::Family, 10)];
        let mut seen = HashSet::new();
        assert!(plan(&items, &texts(), &mut seen, false).is_empty());
        assert!(plan(&items, &texts(), &mut seen, true).is_empty());
    }

    #[test]
    fn more_than_three_at_once_are_grouped_per_project() {
        let items: Vec<NeedsYouItem> = (1..=4)
            .map(|n| {
                item(
                    &format!("SHOP-{n}"),
                    Status::Interrupted,
                    NeedsYouGroup::Stopped,
                    5,
                )
            })
            .collect();
        let out = plan(&items, &HashMap::new(), &mut HashSet::new(), true);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "shop-web · 4 agents need you");
        assert_eq!(out[0].body, "SHOP-1, SHOP-2, SHOP-3, SHOP-4");
        assert_eq!(out[0].open, None);
        let three = &items[..3];
        assert_eq!(
            plan(three, &HashMap::new(), &mut HashSet::new(), true).len(),
            3
        );
    }

    #[test]
    fn the_toast_carries_the_texts_escaped() {
        let xml = toast_xml(
            "shop-web · SHOP-22 needs permission",
            "Allow `rm -rf .next`? <a & b>",
        );
        assert!(
            xml.contains("<text>shop-web · SHOP-22 needs permission</text>"),
            "{xml}"
        );
        assert!(
            xml.contains("<text>Allow `rm -rf .next`? &lt;a &amp; b&gt;</text>"),
            "{xml}"
        );
        assert!(
            !xml.contains("<action"),
            "no button that couldn't act: {xml}"
        );
    }
}
