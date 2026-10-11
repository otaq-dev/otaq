//! OS notifications (ATT-11; d0016, d0042): only for items that **block an agent** (the orange family and
//! Interrupted), never for Ready for Review, a ticket whose terminal has focus or an agent stopped by the plan
//! limit (both already left out of the needs-you items), and **once per item**, not per event. A global setting
//! turns them off (on by default), and it applies at the next derivation.
//!
//! An item is the ticket in that status since that moment: the same question still waiting after the user looked
//! at its terminal and left is not notified again; a new question later is. More than three new items of one
//! project at once become one notification for the project (frame *11 → OS notifications*: "grouped per project
//! after 3").
//!
//! [`should_notify`] and [`plan`] are the pure decision; [`Notifier`] posts. The macOS notifier runs
//! `osascript`, which works from the plain binary; **clicking a notification can't open a ticket that way** (that
//! needs an app bundle, questions.md Q22). On Linux `notify-send` posts through the desktop's notification
//! service and reports a click; the service then opens the app on [`OsNotification::open`] with its terminal
//! focused (T-0159, Q34). On Windows a toast is shown under Windows PowerShell's identity; Windows hands its
//! click only to a registered app, so there, as on macOS, a click doesn't reach Crewboard.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::board::needs_you::{NeedsYouGroup, NeedsYouItem};
use crate::domain::{Status, TicketKey};
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

/// An item, as far as "once per item" is concerned.
pub type ItemId = (String, Status, i64);

pub fn item_id(item: &NeedsYouItem) -> ItemId {
    (item.id.clone(), item.status, item.since)
}

/// Whether an item blocks an agent: orange, or Interrupted (d0016).
pub fn should_notify(item: &NeedsYouItem) -> bool {
    // An agent's question to a finished ticket blocks no agent (the ask is non-blocking): no notification.
    item.message.is_none()
        && (item.group == NeedsYouGroup::Orange || item.status == Status::Interrupted)
}

/// The notifications for `items` (the current needs-you items), given the items notified before; `enabled` is the
/// setting. Every eligible item is remembered in `seen`, notified or not, so turning the setting on later doesn't
/// bring old items up.
pub fn plan(
    items: &[NeedsYouItem],
    texts: &HashMap<String, String>,
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
                    .map(|i| one(i, texts.get(&i.id).map(String::as_str)))
                    .collect()
            }
        })
        .collect()
}

/// The frame's texts: `text` is the item's question, prompt or last line.
fn one(item: &NeedsYouItem, text: Option<&str>) -> OsNotification {
    let text = text.map(str::trim).filter(|t| !t.is_empty());
    let who = format!("{} · {}", item.project_name, item.key);
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
/// which ships with every Windows (no identity of Crewboard's own is registered, Q34). Windows hands a toast's
/// click only to an app registered for its identity, so a click can't reach Crewboard: like macOS (Q22), the toast
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
        // On a thread of its own: it initialises the Windows Runtime for itself, not for the service's workers.
        let spawned = std::thread::Builder::new()
            .name("notification".into())
            .spawn(move || {
                #[cfg(windows)]
                if let Err(e) = toast::show(&xml) {
                    tracing::warn!(error = %e, "can't post a notification");
                }
                #[cfg(not(windows))]
                let _ = xml;
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "can't post a notification");
        }
    }
}

#[cfg(windows)]
mod toast {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
    use windows::core::HSTRING;

    use super::POWERSHELL_APP_ID;

    pub fn show(xml: &str) -> windows::core::Result<()> {
        // SAFETY: initialises the Windows Runtime for this thread; a second call only reports it was done.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
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

/// The variable that tells a starting app what to open: `CREWBOARD_OPEN` = a ticket key, or `inbox`.
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

    fn texts() -> HashMap<String, String> {
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
        assert!(should_notify(&item("SHOP-22", Status::NeedsYou, Orange, 1)));
        assert!(should_notify(&item("SHOP-23", Status::AsksYou, Orange, 1)));
        assert!(should_notify(&item("SHOP-19", Status::Idle, Orange, 1)));
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
            item("SHOP-22", Status::NeedsYou, Orange, 10),
            item("SHOP-23", Status::AsksYou, Orange, 20),
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
        let again = vec![item("SHOP-23", Status::AsksYou, Orange, 99)];
        assert_eq!(plan(&again, &texts(), &mut seen, true).len(), 1);
    }

    #[test]
    fn off_means_off_and_old_items_stay_quiet() {
        let items = vec![item("SHOP-22", Status::NeedsYou, NeedsYouGroup::Orange, 10)];
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
