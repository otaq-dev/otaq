//! The queue toast (EPC-15, d0080; *03 · Board & columns → epic flows 1 → 2*, *flows 3 → 9*, *flows 4 → 12*):
//! every epic queue says what it queued and offers **Undo** (no ⌘Z). With the project's Auto-merge Off (d0064) it
//! says each child waits for review and offers **Turn on for epic children** too (short-term, d0080). Undo takes
//! back only what hasn't started; what had started is named in the toast that replaces it.

use otaq_core::board::epic_flight::{EpicQueueOutcome, UndoQueueOutcome};
use otaq_core::domain::TicketKey;

use crate::view_models::toasts::{NewToast, ToastAction, ToastButton, ToastLevel, ToastLink};

pub const UNDO: &str = "Undo";
pub const TURN_ON: &str = "Turn on for epic children";

/// The toast after an epic queue: `epic_title` is the epic's name ("Search"), for the one-child wording.
///
/// - several children: "Queued 2 children of SHOP-45" · Undo (resumed parked children count as queued);
/// - one child: "Queued SHOP-45.1 of Search" · Undo;
/// - Auto-merge Off: "Queued 3 tickets. Auto-merge is off for shop-web, so each one waits for your review." ·
///   Turn on for epic children · Undo.
pub fn queue_toast(outcome: &EpicQueueOutcome, epic_title: &str) -> NewToast {
    let keys: Vec<&TicketKey> = outcome.keys().collect();
    let undo = ToastButton {
        label: UNDO.to_owned(),
        action: ToastAction::UndoEpicQueue {
            epic: outcome.epic.to_string(),
            keys: keys.iter().map(|k| k.to_string()).collect(),
        },
    };
    let (title, actions) = if outcome.auto_merge_off {
        let noun = if keys.len() == 1 { "ticket" } else { "tickets" };
        let title = format!(
            "Queued {} {noun}. Auto-merge is off for {}, so each one waits for your review.",
            keys.len(),
            outcome.project
        );
        let turn_on = ToastButton {
            label: TURN_ON.to_owned(),
            action: ToastAction::EpicChildrenAutoMerge {
                project: outcome.epic.project.to_string(),
                name: outcome.project.clone(),
                undo: Some(Box::new(undo.clone())),
            },
        };
        (title, vec![turn_on, undo])
    } else {
        let title = match keys.as_slice() {
            [one] => format!("Queued {one} of {epic_title}"),
            many => format!("Queued {} children of {}", many.len(), outcome.epic),
        };
        (title, vec![undo])
    };
    NewToast {
        link: Some(ToastLink::Actions { actions }),
        ..NewToast::new(ToastLevel::Success, title, None)
    }
}

/// The toast that replaces the queue toast after Undo, when something had already started: "Took back SHOP-45.2 ·
/// SHOP-45.1 already started", or "SHOP-45.1 already started" alone. `None` when everything was taken back: the
/// board shows it, and the toast just goes.
pub fn undo_toast(outcome: &UndoQueueOutcome) -> Option<NewToast> {
    if outcome.already_started.is_empty() {
        return None;
    }
    let started = format!("{} already started", list(&outcome.already_started));
    let title = if outcome.taken_back.is_empty() {
        started
    } else {
        format!("Took back {} · {started}", list(&outcome.taken_back))
    };
    Some(NewToast::new(ToastLevel::Success, title, None))
}

/// The toast after Turn on for epic children: "Auto-merge: epic children only for shop-web", keeping the queue's
/// Undo.
pub fn auto_merge_on_toast(name: &str, undo: Option<ToastButton>) -> NewToast {
    NewToast {
        link: undo.map(|undo| ToastLink::Actions {
            actions: vec![undo],
        }),
        ..NewToast::new(
            ToastLevel::Success,
            format!("Auto-merge: epic children only for {name}"),
            None,
        )
    }
}

/// The error toast when a toast's button failed.
pub fn action_failed(label: &str, message: String) -> NewToast {
    NewToast::new(ToastLevel::Error, format!("{label} failed"), Some(message))
}

/// "SHOP-45.1", "SHOP-45.1 and SHOP-45.2", "SHOP-45.1, SHOP-45.2 and SHOP-45.3".
fn list(keys: &[TicketKey]) -> String {
    let names: Vec<String> = keys.iter().map(TicketKey::to_string).collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> TicketKey {
        s.parse().unwrap()
    }

    fn outcome(queued: &[&str], resumed: &[&str], auto_merge_off: bool) -> EpicQueueOutcome {
        EpicQueueOutcome {
            epic: key("SHOP-45"),
            queued: queued.iter().map(|k| key(k)).collect(),
            resumed: resumed.iter().map(|k| key(k)).collect(),
            auto_merge_off,
            project: "shop-web".into(),
        }
    }

    fn actions(toast: &NewToast) -> Vec<(&str, &ToastAction)> {
        match &toast.link {
            Some(ToastLink::Actions { actions }) => actions
                .iter()
                .map(|b| (b.label.as_str(), &b.action))
                .collect(),
            other => panic!("{other:?}"),
        }
    }

    fn undo_of(keys: &[&str]) -> ToastAction {
        ToastAction::UndoEpicQueue {
            epic: "SHOP-45".into(),
            keys: keys.iter().map(|k| (*k).to_owned()).collect(),
        }
    }

    #[test]
    fn several_children_one_child_and_resumed_ones() {
        let toast = queue_toast(&outcome(&["SHOP-45.1", "SHOP-45.2"], &[], false), "Search");
        assert_eq!(toast.title, "Queued 2 children of SHOP-45");
        assert_eq!(toast.level, ToastLevel::Success);
        assert_eq!(toast.text, None);
        assert_eq!(
            actions(&toast),
            [("Undo", &undo_of(&["SHOP-45.1", "SHOP-45.2"]))]
        );

        let toast = queue_toast(&outcome(&["SHOP-45.1"], &[], false), "Search");
        assert_eq!(toast.title, "Queued SHOP-45.1 of Search");
        assert_eq!(actions(&toast), [("Undo", &undo_of(&["SHOP-45.1"]))]);

        // Resumed parked children count as queued, and Undo takes them back too.
        let toast = queue_toast(&outcome(&["SHOP-45.2"], &["SHOP-45.1"], false), "Search");
        assert_eq!(toast.title, "Queued 2 children of SHOP-45");
        assert_eq!(
            actions(&toast),
            [("Undo", &undo_of(&["SHOP-45.2", "SHOP-45.1"]))]
        );
    }

    #[test]
    fn auto_merge_off_offers_turn_on_next_to_undo() {
        let toast = queue_toast(
            &outcome(&["SHOP-45.1", "SHOP-45.2"], &["SHOP-45.3"], true),
            "Search",
        );
        assert_eq!(
            toast.title,
            "Queued 3 tickets. Auto-merge is off for shop-web, so each one waits for your review."
        );
        let undo = undo_of(&["SHOP-45.1", "SHOP-45.2", "SHOP-45.3"]);
        let turn_on = ToastAction::EpicChildrenAutoMerge {
            project: "SHOP".into(),
            name: "shop-web".into(),
            undo: Some(Box::new(ToastButton {
                label: "Undo".into(),
                action: undo.clone(),
            })),
        };
        assert_eq!(
            actions(&toast),
            [("Turn on for epic children", &turn_on), ("Undo", &undo)]
        );
        let one = queue_toast(&outcome(&["SHOP-45.1"], &[], true), "Search");
        assert!(one.title.starts_with("Queued 1 ticket. "), "{}", one.title);

        let on = auto_merge_on_toast(
            "shop-web",
            Some(ToastButton {
                label: "Undo".into(),
                action: undo.clone(),
            }),
        );
        assert_eq!(on.title, "Auto-merge: epic children only for shop-web");
        assert_eq!(actions(&on), [("Undo", &undo)]);
    }

    #[test]
    fn undo_names_what_had_already_started() {
        let reply = |taken_back: &[&str], already_started: &[&str]| UndoQueueOutcome {
            epic: key("SHOP-45"),
            taken_back: taken_back.iter().map(|k| key(k)).collect(),
            already_started: already_started.iter().map(|k| key(k)).collect(),
        };
        assert_eq!(undo_toast(&reply(&["SHOP-45.1", "SHOP-45.2"], &[])), None);
        let toast = undo_toast(&reply(&["SHOP-45.2"], &["SHOP-45.1"])).unwrap();
        assert_eq!(
            toast.title,
            "Took back SHOP-45.2 · SHOP-45.1 already started"
        );
        assert_eq!(toast.level, ToastLevel::Success);
        assert_eq!(toast.link, None);
        assert_eq!(
            undo_toast(&reply(&[], &["SHOP-40.1"])).unwrap().title,
            "SHOP-40.1 already started"
        );
        assert_eq!(
            undo_toast(&reply(&["SHOP-40.3"], &["SHOP-40.1", "SHOP-40.2"]))
                .unwrap()
                .title,
            "Took back SHOP-40.3 · SHOP-40.1 and SHOP-40.2 already started"
        );
    }

    #[test]
    fn the_buttons_travel_as_json() {
        let toast = queue_toast(&outcome(&["SHOP-45.1"], &[], true), "Search");
        let json = serde_json::to_value(&toast.link).unwrap();
        assert_eq!(json["kind"], "actions");
        assert_eq!(
            json["actions"][0]["action"]["kind"],
            "epic_children_auto_merge"
        );
        assert_eq!(json["actions"][1]["action"]["kind"], "undo_epic_queue");
        assert_eq!(json["actions"][1]["action"]["keys"][0], "SHOP-45.1");
        let back: ToastLink = serde_json::from_value(json).unwrap();
        assert_eq!(Some(back), toast.link);
    }
}
