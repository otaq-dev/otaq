//! What an Epic Card shows (*01 · Components → Card component → Epic Card*; *04 → Row · epics*, *Row · epics as
//! delivery units (0059)*): key, External ID chip, the epic pill, title, progress ("1 / 3 merged", one segment
//! per child), its branch, the child list with statuses, "Add ticket to epic", and the actions of its later
//! states (Review feature in M6, Mark merged / Mark closed in M10). EPC-01, d0039, d0059.
//!
//! From M16 (d0080) the card is the epic in its **Card** place: its children are rows inside it, never separate
//! Backlog cards. A row is draggable while its child can be queued from there (never started, or Parked after
//! Park all, its status "Parked · has work", EPC-14).

use otaq_core::board::epic_flight::{self, EpicPlace};
use otaq_core::board::epics::progress_of;
use otaq_core::board::tickets::{Base, Ticket};
use otaq_core::domain::{Column, Status, TicketKey};

use crate::icons::IconName;
use crate::theme::Status as Legend;
use crate::view_models::card::{CardAction, CardActionKind, Pill};

/// How a child shows in the list and in the progress bar. `EpicChildState` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "EpicChildState")]
pub enum ChildState {
    Merged,
    Closed,
    Working,
    NotStarted,
}

impl ChildState {
    pub fn of(status: Status) -> Self {
        match status {
            Status::Merged => Self::Merged,
            Status::ClosedNotMerged => Self::Closed,
            s if s.column() == Column::Backlog || s.is_queued() => Self::NotStarted,
            _ => Self::Working,
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Self::Merged => IconName::CircleCheck,
            Self::Closed => IconName::CircleSlash,
            Self::Working => IconName::Activity,
            Self::NotStarted => IconName::CircleDashed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRow {
    pub key: TicketKey,
    pub title: String,
    pub state: ChildState,
    pub status_label: &'static str,
    /// The row can be dragged out of the card to queue only this child (EPC-12, EPC-14): a never-started or
    /// Parked child of an epic in its Card place.
    pub draggable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpicViewModel {
    pub key: TicketKey,
    pub pinned: bool,
    /// Hidden when empty.
    pub external_id: Option<String>,
    /// The Priority chip after the External ID; `None` for Normal (d0065).
    pub priority: Option<otaq_core::domain::Priority>,
    pub pill: Pill,
    pub title: String,
    /// One per child, in key order.
    pub segments: Vec<ChildState>,
    pub progress: String,
    pub branch: String,
    pub children: Vec<ChildRow>,
    /// "Add ticket to epic" (not once the epic is done or closed).
    pub can_add: bool,
    pub actions: Vec<CardAction>,
    pub faded: Option<f32>,
}

impl EpicViewModel {
    /// `tickets` is the epic's project (its children are found by their epic key). `branch` is the epic's
    /// branch name once it has one (the temporary `otaq/SHOP-40` until the first push, d0050).
    pub fn from(epic: &Ticket, tickets: &[Ticket], branch: Option<&str>) -> Self {
        let mut children: Vec<&Ticket> = tickets
            .iter()
            .filter(|t| t.epic.as_ref() == Some(&epic.key))
            .collect();
        children.sort_by(|a, b| a.key.cmp(&b.key));
        let progress = progress_of(&epic.key, tickets);
        let card_place = epic_flight::epic_place(epic, tickets) == EpicPlace::Card;
        let pill = match epic.status {
            Status::ReadyForReview => Pill {
                legend: Legend::ReadyForReview,
                label: "Ready for Review".into(),
            },
            Status::PrOpen => Pill {
                legend: Legend::PrOpen,
                label: epic
                    .flags
                    .pr_number
                    .map_or_else(|| "PR open".into(), |n| format!("PR #{n}")),
            },
            Status::Merged => Pill {
                legend: Legend::Done,
                label: "Epic · Done".into(),
            },
            Status::ClosedNotMerged => Pill {
                legend: Legend::ClosedNotMerged,
                label: "Closed · not merged".into(),
            },
            _ => Pill {
                legend: Legend::Epic,
                label: "Epic".into(),
            },
        };
        let base = match &epic.base {
            Base::Branch(b) => b.clone(),
            Base::Epic(e) => e.to_string(),
        };
        let branch_name = branch
            .map(str::to_owned)
            .unwrap_or_else(|| otaq_core::naming::branch(&epic.key.to_string()));
        let branch = match epic.status {
            Status::Merged | Status::PrOpen => branch_name,
            Status::ClosedNotMerged => format!("{branch_name} · kept"),
            _ => format!("{branch_name} · on {base}"),
        };
        let actions = match epic.status {
            Status::ReadyForReview => vec![CardAction {
                kind: CardActionKind::Review,
                label: "Review feature",
                icon: Some(IconName::Eye),
                primary: true,
            }],
            Status::PrOpen => vec![
                CardAction {
                    kind: CardActionKind::MarkMerged,
                    label: "Mark merged",
                    icon: None,
                    primary: false,
                },
                CardAction {
                    kind: CardActionKind::MarkClosed,
                    label: "Mark closed",
                    icon: None,
                    primary: false,
                },
            ],
            _ => Vec::new(),
        };
        let done = matches!(epic.status, Status::Merged | Status::ClosedNotMerged);
        Self {
            key: epic.key.clone(),
            pinned: epic.flags.is_pinned(),
            external_id: epic.external_id.clone().filter(|e| !e.is_empty()),
            priority: crate::view_models::priority::on_card(epic.priority),
            pill,
            title: epic.title.clone(),
            segments: children.iter().map(|c| ChildState::of(c.status)).collect(),
            progress: format!("{} / {} merged", progress.merged, progress.total),
            branch,
            children: children
                .iter()
                .map(|c| ChildRow {
                    key: c.key.clone(),
                    title: c.title.clone(),
                    state: ChildState::of(c.status),
                    status_label: c.status.label(),
                    draggable: card_place
                        && (epic_flight::is_never_started(c) || epic_flight::is_parked(c)),
                })
                .collect(),
            can_add: !done && epic.status != Status::PrOpen,
            actions,
            faded: done.then_some(0.55),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use otaq_core::domain::Kind;

    fn wishlist() -> Vec<Ticket> {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        epic.external_id = Some("PAY-123".into());
        let child = |key: &str, title: &str, status| {
            let mut t = ticket(key, title, status);
            t.epic = Some("SHOP-40".parse().unwrap());
            t
        };
        vec![
            epic,
            child("SHOP-40.2", "Wishlist button", Status::Working),
            child("SHOP-40.1", "Wishlist API", Status::Merged),
            child("SHOP-40.3", "Wishlist page", Status::Backlog),
        ]
    }

    #[test]
    fn shop_40_wishlist() {
        let tickets = wishlist();
        let vm = EpicViewModel::from(&tickets[0], &tickets, None);
        assert_eq!(vm.progress, "1 / 3 merged");
        assert_eq!(vm.external_id.as_deref(), Some("PAY-123"));
        assert_eq!(vm.pill.label, "Epic");
        assert_eq!(
            vm.segments,
            [
                ChildState::Merged,
                ChildState::Working,
                ChildState::NotStarted
            ]
        );
        let keys: Vec<String> = vm.children.iter().map(|c| c.key.to_string()).collect();
        assert_eq!(keys, ["SHOP-40.1", "SHOP-40.2", "SHOP-40.3"]);
        assert_eq!(vm.children[1].state.icon(), IconName::Activity);
        assert_eq!(
            vm.branch,
            format!("{} · on main", otaq_core::naming::branch("SHOP-40"))
        );
        assert!(vm.can_add);
        assert!(vm.actions.is_empty());
        assert!(
            vm.children.iter().all(|c| !c.draggable),
            "SHOP-40.2 is working: a track, no rows to drag"
        );
    }

    /// EPC-11, EPC-14: before anything starts every row can be dragged out; after Park all the rows say
    /// "Parked · has work" and stay draggable (dragging one resumes it); a merged child's row never is.
    #[test]
    fn rows_know_which_can_be_dragged() {
        let mut tickets = wishlist();
        for t in &mut tickets[1..] {
            t.status = Status::Backlog;
            t.column = Column::Backlog;
        }
        let vm = EpicViewModel::from(&tickets[0], &tickets, None);
        assert!(vm.children.iter().all(|c| c.draggable));
        assert!(vm.children.iter().all(|c| c.status_label == "Backlog"));

        tickets[1].status = Status::Parked;
        tickets[2].status = Status::Merged;
        tickets[2].column = Column::Done;
        tickets[3].status = Status::Parked;
        let vm = EpicViewModel::from(&tickets[0], &tickets, None);
        let rows: Vec<(String, &str, bool)> = vm
            .children
            .iter()
            .map(|c| (c.key.to_string(), c.status_label, c.draggable))
            .collect();
        assert_eq!(
            rows,
            [
                ("SHOP-40.1".to_owned(), "Merged", false),
                ("SHOP-40.2".to_owned(), "Parked · has work", true),
                ("SHOP-40.3".to_owned(), "Parked · has work", true),
            ]
        );

        tickets[0].status = Status::ReadyForReview;
        tickets[0].column = Column::ReadyForReview;
        let vm = EpicViewModel::from(&tickets[0], &tickets, None);
        assert!(
            vm.children.iter().all(|c| !c.draggable),
            "a delivery card's rows stay put"
        );
    }

    #[test]
    fn external_id_hidden_when_empty_and_later_states() {
        let mut tickets = wishlist();
        tickets[0].external_id = Some(String::new());
        tickets[0].status = Status::ReadyForReview;
        let vm = EpicViewModel::from(&tickets[0], &tickets, None);
        assert_eq!(vm.external_id, None);
        assert_eq!(vm.actions[0].label, "Review feature");

        tickets[0].status = Status::PrOpen;
        tickets[0].flags.pr_number = Some(490);
        let vm = EpicViewModel::from(&tickets[0], &tickets, Some("feature/PAY-123-wishlist"));
        assert_eq!(vm.pill.label, "PR #490");
        assert_eq!(vm.branch, "feature/PAY-123-wishlist");
        assert_eq!(vm.actions.len(), 2);
        assert!(!vm.can_add);

        tickets[0].status = Status::ClosedNotMerged;
        let vm = EpicViewModel::from(&tickets[0], &tickets, None);
        assert_eq!(vm.pill.label, "Closed · not merged");
        assert_eq!(vm.faded, Some(0.55));
        assert!(vm.branch.ends_with(" · kept"));
    }
}
