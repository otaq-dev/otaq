//! Agent activity as it happens (TKT-03, TKT-05, SES-08, UI-07; d0019, d0048): the "created by agent" origin
//! label on new Backlog cards, one success toast per batch of agent-created tickets (*11 · Banners, toasts & OS
//! notifications → Success toasts*), and the editor's note that the agent will be told what changed (*08 ·
//! Ticket editor → Editing while the agent works*).

use std::time::{Duration, Instant};

use crewboard_core::board::change_note::change_parts;
use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, TicketKey};
use crewboard_core::signals::LIVE;

/// Tickets an agent creates within this quiet time make one toast (a `create -f` batch arrives as one burst).
pub const BATCH_QUIET: Duration = Duration::from_millis(800);

/// The title every agent-creation toast ends with; the toast host gives it the sparkles icon.
pub const CREATED_BY_AGENT: &str = "created by agent";

/// The card's origin label for a ticket an agent created, while it waits in Backlog.
pub fn origin_label(ticket: &Ticket) -> Option<&'static str> {
    (ticket.created_by.is_agent() && ticket.column == Column::Backlog).then_some(CREATED_BY_AGENT)
}

/// Collects agent-created tickets until a quiet moment, then gives one toast for them all.
#[derive(Debug, Default)]
pub struct CreationBatch {
    items: Vec<(TicketKey, String)>,
    last: Option<Instant>,
}

impl CreationBatch {
    /// Notes a ticket an agent created (seen for the first time).
    pub fn add(&mut self, ticket: &Ticket, now: Instant) {
        if ticket.created_by.is_agent() && !self.items.iter().any(|(k, _)| k == &ticket.key) {
            self.items.push((ticket.key.clone(), ticket.title.clone()));
            self.last = Some(now);
        }
    }

    /// The toast (title, text) once nothing new arrived for [`BATCH_QUIET`]; the batch is then empty again.
    pub fn take_if_quiet(&mut self, now: Instant) -> Option<(String, String)> {
        let last = self.last?;
        if now.duration_since(last) < BATCH_QUIET || self.items.is_empty() {
            return None;
        }
        let items = std::mem::take(&mut self.items);
        self.last = None;
        Some(creation_toast(&items))
    }
}

/// "Ticket created by agent" / "SHOP-29 Add cart analytics → Backlog", or for a batch "3 tickets created by agent"
/// / "SHOP-29, SHOP-30, SHOP-31 → Backlog".
pub fn creation_toast(items: &[(TicketKey, String)]) -> (String, String) {
    match items {
        [(key, title)] => (
            format!("Ticket {CREATED_BY_AGENT}"),
            format!("{key} {title} → Backlog"),
        ),
        _ => {
            let keys: Vec<String> = items.iter().map(|(k, _)| k.to_string()).collect();
            (
                format!("{} tickets {CREATED_BY_AGENT}", items.len()),
                format!("{} → Backlog", keys.join(", ")),
            )
        }
    }
}

/// Whether saving `ticket` sends its agent a change note: its agent is at work and has a live session.
pub fn agent_is_told(ticket: &Ticket, live_agent_session: bool) -> bool {
    live_agent_session && LIVE.contains(&ticket.status)
}

/// The line under Save while editing a working ticket: "SHOP-19’s agent will be told what changed."
pub fn save_hint(ticket: &Ticket, live_agent_session: bool) -> Option<String> {
    agent_is_told(ticket, live_agent_session)
        .then(|| format!("{}’s agent will be told what changed.", ticket.key))
}

/// After a save: "Saved. SHOP-19’s agent will be told what changed: Added to 'Done when': 'works on mobile
/// widths'." `None` when no note goes out.
pub fn saved_note(before: &Ticket, after: &Ticket, live_agent_session: bool) -> Option<String> {
    if !agent_is_told(after, live_agent_session) {
        return None;
    }
    let parts = change_parts(before, after);
    (!parts.is_empty()).then(|| {
        format!(
            "Saved. {}’s agent will be told what changed: {}",
            after.key,
            parts.join(" ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use crewboard_core::board::tickets::DoneWhenItem;
    use crewboard_core::domain::{Actor, Status};

    fn by_agent(key: &str, title: &str) -> Ticket {
        let mut t = ticket(key, title, Status::Backlog);
        t.created_by = Actor::Agent("SHOP-19".parse().unwrap());
        t
    }

    #[test]
    fn the_origin_label_is_for_agent_tickets_in_backlog() {
        assert_eq!(
            origin_label(&by_agent("SHOP-29", "Add cart analytics")),
            Some("created by agent")
        );
        assert_eq!(
            origin_label(&ticket("SHOP-24", "Footer", Status::Backlog)),
            None
        );
        let mut started = by_agent("SHOP-29", "Add cart analytics");
        started.status = Status::Working;
        started.column = Column::Dev;
        assert_eq!(origin_label(&started), None);
    }

    #[test]
    fn the_card_carries_the_origin_label() {
        use crate::view_models::card::{CardExtras, CardViewModel};
        let vm = CardViewModel::from(
            &by_agent("SHOP-29", "Add cart analytics"),
            &CardExtras::default(),
            0,
        );
        assert!(vm.meta.iter().any(|m| m.text == "created by agent" && m.icon == crate::icons::IconName::Sparkles));
        let mine = CardViewModel::from(
            &ticket("SHOP-24", "Footer", Status::Backlog),
            &CardExtras::default(),
            0,
        );
        assert!(!mine.meta.iter().any(|m| m.text == "created by agent"));
    }

    #[test]
    fn one_toast_per_burst() {
        let t0 = Instant::now();
        let mut batch = CreationBatch::default();
        batch.add(&by_agent("SHOP-29", "Add cart analytics"), t0);
        assert_eq!(batch.take_if_quiet(t0 + Duration::from_millis(100)), None);
        assert_eq!(
            batch.take_if_quiet(t0 + BATCH_QUIET),
            Some((
                "Ticket created by agent".into(),
                "SHOP-29 Add cart analytics → Backlog".into()
            ))
        );
        assert_eq!(batch.take_if_quiet(t0 + BATCH_QUIET * 3), None, "emptied");

        for (n, key) in ["SHOP-30", "SHOP-31", "SHOP-32"].iter().enumerate() {
            batch.add(
                &by_agent(key, "x"),
                t0 + Duration::from_millis(100 * n as u64),
            );
        }
        batch.add(&ticket("SHOP-33", "mine", Status::Backlog), t0);
        let (title, text) = batch.take_if_quiet(t0 + Duration::from_secs(5)).unwrap();
        assert_eq!(title, "3 tickets created by agent");
        assert_eq!(text, "SHOP-30, SHOP-31, SHOP-32 → Backlog");
    }

    #[test]
    fn the_editor_note_only_for_a_working_ticket_with_a_live_agent() {
        let item = |t: &str| DoneWhenItem {
            text: t.into(),
            added_by: Actor::User("me".into()),
        };
        let mut before = ticket("SHOP-19", "Price table", Status::Working);
        before.done_when = vec![item("Monthly / yearly toggle")];
        let mut after = before.clone();
        after.done_when.push(item("Works on mobile widths"));
        assert_eq!(
            save_hint(&before, true).as_deref(),
            Some("SHOP-19’s agent will be told what changed.")
        );
        assert_eq!(
            saved_note(&before, &after, true).as_deref(),
            Some(
                "Saved. SHOP-19’s agent will be told what changed: Added to 'Done when': 'Works on mobile widths'."
            )
        );
        assert_eq!(saved_note(&before, &after, false), None, "no live session");
        assert_eq!(saved_note(&before, &before, true), None, "nothing changed");
        for status in [
            Status::Queued,
            Status::Parked,
            Status::Backlog,
            Status::ReadyForReview,
        ] {
            let mut t = after.clone();
            t.status = status;
            assert_eq!(saved_note(&before, &t, true), None, "{status}");
            assert_eq!(save_hint(&t, true), None, "{status}");
        }
    }
}
