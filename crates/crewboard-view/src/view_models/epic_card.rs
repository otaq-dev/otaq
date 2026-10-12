//! What an Epic Card shows (*01 · Components → Card component → Epic Card*; *04 → Row · epics*, *Row · epics as
//! delivery units (0059)*): key, External ID chip, the epic pill, title, progress ("1 / 3 merged", one segment
//! per child), its branch, the child list with statuses, "Add ticket to epic", and the actions of its later
//! states (Review feature in M6, Mark merged / Mark closed in M10). EPC-01, d0039, d0059.

use crewboard_core::board::epics::progress_of;
use crewboard_core::board::tickets::{Base, Ticket};
use crewboard_core::domain::{Column, Status, TicketKey};

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpicViewModel {
    pub key: TicketKey,
    pub pinned: bool,
    /// Hidden when empty.
    pub external_id: Option<String>,
    /// The Priority chip after the External ID; `None` for Normal (d0065).
    pub priority: Option<crewboard_core::domain::Priority>,
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
    /// branch name once it has one (the temporary `crewboard/SHOP-40` until the first push, d0050).
    pub fn from(epic: &Ticket, tickets: &[Ticket], branch: Option<&str>) -> Self {
        let mut children: Vec<&Ticket> = tickets
            .iter()
            .filter(|t| t.epic.as_ref() == Some(&epic.key))
            .collect();
        children.sort_by(|a, b| a.key.cmp(&b.key));
        let progress = progress_of(&epic.key, tickets);
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
            .unwrap_or_else(|| crewboard_core::naming::branch(&epic.key.to_string()));
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
    use crewboard_core::domain::Kind;

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
            format!("{} · on main", crewboard_core::naming::branch("SHOP-40"))
        );
        assert!(vm.can_add);
        assert!(vm.actions.is_empty());
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
