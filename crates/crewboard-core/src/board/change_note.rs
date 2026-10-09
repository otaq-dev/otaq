//! The ticket-change note (SES-08, d0048): when the **user** saves a change to a ticket whose agent is running,
//! the agent gets a short note with only what changed, for example *"The ticket was updated by the user. Added to
//! 'Done when': 'works on mobile widths'. Run `crewboard prime` for the full ticket."*
//!
//! The note goes through the automatic-delivery queue (T-0054): it waits until the agent is idle between turns and
//! its input line is empty. Agent edits send nothing; Queued, Parked and Backlog tickets get nothing.
//!
//! Several saves before a delivery queue **one note each, in order** (implementation choice): each says exactly
//! what that save changed, and they are typed one after the other when the agent is idle.

use super::tickets::Ticket;
use crate::naming;

/// The note for a change from `before` to `after`, or `None` when nothing the agent works from changed (pins,
/// External ID, status). The base can't change once a ticket has started (d0051), so it never appears.
pub fn change_note(before: &Ticket, after: &Ticket) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if before.title != after.title {
        parts.push(format!("The title is now '{}'.", after.title));
    }
    if before.body.trim() != after.body.trim() {
        parts.push(if after.body.trim().is_empty() {
            "'What and why' was cleared.".to_owned()
        } else {
            "'What and why' changed.".to_owned()
        });
    }
    parts.extend(done_when_parts(before, after));
    let added: Vec<String> = after
        .depends_on
        .iter()
        .filter(|k| !before.depends_on.contains(k))
        .map(ToString::to_string)
        .collect();
    let removed: Vec<String> = before
        .depends_on
        .iter()
        .filter(|k| !after.depends_on.contains(k))
        .map(ToString::to_string)
        .collect();
    if !added.is_empty() {
        parts.push(format!("It now depends on {}.", added.join(", ")));
    }
    if !removed.is_empty() {
        parts.push(format!("It no longer depends on {}.", removed.join(", ")));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!(
        "The ticket was updated by the user. {} Run `{} prime` for the full ticket.",
        parts.join(" "),
        naming::COMMAND_NAME
    ))
}

fn quoted(items: &[&str]) -> String {
    items
        .iter()
        .map(|i| format!("'{i}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Added, removed, edited (same position, new text) and reordered "Done when" items.
fn done_when_parts(before: &Ticket, after: &Ticket) -> Vec<String> {
    let old: Vec<&str> = before.done_when.iter().map(|i| i.text.as_str()).collect();
    let new: Vec<&str> = after.done_when.iter().map(|i| i.text.as_str()).collect();
    let mut added: Vec<&str> = new.iter().copied().filter(|t| !old.contains(t)).collect();
    let mut removed: Vec<&str> = old.iter().copied().filter(|t| !new.contains(t)).collect();
    let mut edited: Vec<(&str, &str)> = Vec::new();
    for (i, was) in old.iter().enumerate() {
        if let Some(now) = new.get(i)
            && removed.contains(was)
            && added.contains(now)
        {
            edited.push((was, now));
            removed.retain(|r| r != was);
            added.retain(|a| a != now);
        }
    }
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("Added to 'Done when': {}.", quoted(&added)));
    }
    for (was, now) in &edited {
        parts.push(format!("Changed in 'Done when': '{was}' is now '{now}'."));
    }
    if !removed.is_empty() {
        parts.push(format!("Removed from 'Done when': {}.", quoted(&removed)));
    }
    if parts.is_empty() && old != new {
        parts.push("The 'Done when' items were reordered.".to_owned());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::tickets::{Base, DoneWhenItem};
    use crate::domain::{Actor, Column, Flags, Kind, Status};

    fn ticket(done_when: &[&str]) -> Ticket {
        Ticket {
            id: "t19".into(),
            key: "SHOP-19".parse().unwrap(),
            kind: Kind::Ticket,
            title: "Price table".into(),
            body: "Show the prices.".into(),
            done_when: done_when
                .iter()
                .map(|t| DoneWhenItem {
                    text: (*t).into(),
                    added_by: Actor::User("me".into()),
                })
                .collect(),
            epic: None,
            column: Column::Dev,
            status: Status::Working,
            base: Base::Branch("main".into()),
            base_locked: true,
            external_id: None,
            depends_on: vec![],
            aliases: vec![],
            next_child_number: None,
            flags: Flags::default(),
            created_by: Actor::User("me".into()),
            updated_by: None,
            started_at: Some(1),
            created_at: 0,
            updated_at: 0,
            work: None,
            start_problem: None,
            signal: None,
        }
    }

    fn note(before: &Ticket, after: &Ticket) -> String {
        change_note(before, after).unwrap()
    }

    #[test]
    fn the_sdd_sample() {
        let before = ticket(&["sorted by price"]);
        let after = ticket(&["sorted by price", "works on mobile widths"]);
        assert_eq!(
            note(&before, &after),
            format!(
                "The ticket was updated by the user. Added to 'Done when': 'works on mobile widths'. Run \
                 `{} prime` for the full ticket.",
                naming::COMMAND_NAME
            )
        );
    }

    #[test]
    fn each_kind_of_change() {
        let before = ticket(&["a", "b", "c"]);
        let mut after = ticket(&["a", "B!", "d"]);
        after.title = "Price table v2".into();
        after.body = "Show the prices, sorted.".into();
        after.depends_on = vec!["SHOP-18".parse().unwrap()];
        let text = note(&before, &after);
        assert_eq!(
            text,
            format!(
                "The ticket was updated by the user. The title is now 'Price table v2'. 'What and why' changed. \
                 Changed in 'Done when': 'b' is now 'B!'. Changed in 'Done when': 'c' is now 'd'. It now depends \
                 on SHOP-18. Run `{} prime` for the full ticket.",
                naming::COMMAND_NAME
            )
        );

        let removed = note(&ticket(&["a", "b"]), &ticket(&["a"]));
        assert!(
            removed.contains("Removed from 'Done when': 'b'."),
            "{removed}"
        );
        let reordered = note(&ticket(&["a", "b"]), &ticket(&["b", "a"]));
        assert!(
            reordered.contains("The 'Done when' items were reordered."),
            "{reordered}"
        );
        let mut unlinked = ticket(&[]);
        unlinked.depends_on = vec!["SHOP-18".parse().unwrap()];
        assert!(note(&unlinked, &ticket(&[])).contains("It no longer depends on SHOP-18."));
        let mut cleared = ticket(&[]);
        cleared.body.clear();
        assert!(note(&ticket(&[]), &cleared).contains("'What and why' was cleared."));
    }

    #[test]
    fn nothing_the_agent_works_from_changed() {
        let before = ticket(&["a"]);
        let mut after = before.clone();
        after.external_id = Some("PAY-123".into());
        after.flags.pinned_at = Some(5);
        assert_eq!(change_note(&before, &after), None);
    }
}
