//! The queue toast (EPC-15, d0080; *03 · Board & columns → epic flows 1 → 2*, *flows 3 → 9*, *flows 4 → 12*):
//! every epic queue says what it queued and offers **Undo** (no ⌘Z). Undo takes back only what hasn't started; what
//! had started is named in the toast that replaces it.

use otaq_core::board::epic_flight::{EpicQueueOutcome, UndoQueueOutcome};
use otaq_core::domain::TicketKey;

use crate::view_models::toasts::{NewToast, ToastAction, ToastButton, ToastLevel, ToastLink};

pub const UNDO: &str = "Undo";
/// Added when the queue started the epic's lead (LEAD-02, d0083; frame 17).
pub const LEAD_STARTED: &str = " \u{b7} its lead started";

/// The toast after an epic queue: `epic_title` is the epic's name ("Search"), for the one-child wording.
///
/// - several children: "Moved 2 children of SHOP-45 to Ready for Dev" · Undo (resumed parked children count as queued);
/// - one child: "Moved SHOP-45.1 of Search to Ready for Dev" · Undo.
///
/// When the queue started the epic's lead, it says so: "Moved 2 children of SHOP-45 to Ready for Dev · its lead
/// started" (frame 17).
pub fn queue_toast(outcome: &EpicQueueOutcome, epic_title: &str) -> NewToast {
    let keys: Vec<&TicketKey> = outcome.keys().collect();
    let lead = if outcome.lead_started {
        LEAD_STARTED
    } else {
        ""
    };
    let undo = ToastButton {
        label: UNDO.to_owned(),
        action: ToastAction::UndoEpicQueue {
            epic: outcome.epic.to_string(),
            keys: keys.iter().map(|k| k.to_string()).collect(),
        },
    };
    let title = match keys.as_slice() {
        [one] => format!("Moved {one} of {epic_title} to Ready for Dev{lead}"),
        many => format!(
            "Moved {} children of {} to Ready for Dev{lead}",
            many.len(),
            outcome.epic
        ),
    };
    let actions = vec![undo];
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

    fn outcome(queued: &[&str], resumed: &[&str]) -> EpicQueueOutcome {
        EpicQueueOutcome {
            epic: key("SHOP-45"),
            queued: queued.iter().map(|k| key(k)).collect(),
            resumed: resumed.iter().map(|k| key(k)).collect(),
            project: "shop-web".into(),
            lead_started: false,
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
        let toast = queue_toast(&outcome(&["SHOP-45.1", "SHOP-45.2"], &[]), "Search");
        assert_eq!(toast.title, "Moved 2 children of SHOP-45 to Ready for Dev");
        assert_eq!(toast.level, ToastLevel::Success);
        assert_eq!(toast.text, None);
        assert_eq!(
            actions(&toast),
            [("Undo", &undo_of(&["SHOP-45.1", "SHOP-45.2"]))]
        );

        let toast = queue_toast(&outcome(&["SHOP-45.1"], &[]), "Search");
        assert_eq!(toast.title, "Moved SHOP-45.1 of Search to Ready for Dev");
        assert_eq!(actions(&toast), [("Undo", &undo_of(&["SHOP-45.1"]))]);

        // Resumed parked children count as queued, and Undo takes them back too.
        let toast = queue_toast(&outcome(&["SHOP-45.2"], &["SHOP-45.1"]), "Search");
        assert_eq!(toast.title, "Moved 2 children of SHOP-45 to Ready for Dev");
        assert_eq!(
            actions(&toast),
            [("Undo", &undo_of(&["SHOP-45.2", "SHOP-45.1"]))]
        );
    }

    /// Frame 17: the drag started SHOP-45's lead; frame 18: SHOP-40's lead was already open, nothing added.
    #[test]
    fn the_toast_says_when_the_queue_started_the_lead() {
        let mut started = outcome(&["SHOP-45.1", "SHOP-45.2"], &[]);
        started.lead_started = true;
        let toast = queue_toast(&started, "Search");
        assert_eq!(
            toast.title,
            "Moved 2 children of SHOP-45 to Ready for Dev · its lead started"
        );
        assert_eq!(
            actions(&toast),
            [("Undo", &undo_of(&["SHOP-45.1", "SHOP-45.2"]))]
        );
        let mut one = outcome(&["SHOP-45.1"], &[]);
        one.lead_started = true;
        assert_eq!(
            queue_toast(&one, "Search").title,
            "Moved SHOP-45.1 of Search to Ready for Dev · its lead started"
        );
        let without = queue_toast(&outcome(&["SHOP-45.1", "SHOP-45.2"], &[]), "Search");
        assert_eq!(
            without.title,
            "Moved 2 children of SHOP-45 to Ready for Dev"
        );
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
        let toast = queue_toast(&outcome(&["SHOP-45.1"], &[]), "Search");
        let json = serde_json::to_value(&toast.link).unwrap();
        assert_eq!(json["kind"], "actions");
        assert_eq!(json["actions"][0]["action"]["kind"], "undo_epic_queue");
        assert_eq!(json["actions"][0]["action"]["keys"][0], "SHOP-45.1");
        let back: ToastLink = serde_json::from_value(json).unwrap();
        assert_eq!(Some(back), toast.link);
    }
}
