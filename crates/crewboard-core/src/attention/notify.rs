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
//! needs an app bundle, questions.md Q22), so [`OsNotification::open`] carries the ticket for the day it can.

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
    item.group == NeedsYouGroup::Orange || item.status == Status::Interrupted
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

/// Posts OS notifications.
pub trait Notifier: Send + Sync {
    fn post(&self, notification: &OsNotification);
}

/// Posts nothing (tests, and platforms without a notifier yet).
pub struct NoNotifier;

impl Notifier for NoNotifier {
    fn post(&self, _notification: &OsNotification) {}
}

/// macOS: `osascript`'s `display notification`, which works without an app bundle. The texts go in as
/// arguments, never into the script. A click shows the notification's sender, not the ticket (Q22).
pub struct OsaScriptNotifier;

impl Notifier for OsaScriptNotifier {
    fn post(&self, n: &OsNotification) {
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
            reason: String::new(),
            since,
            age_ms: 0,
            blocked: 0,
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
}
