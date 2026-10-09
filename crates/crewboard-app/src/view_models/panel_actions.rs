//! The panel's action bar for each state (UI-04, TKT-08; brief 05, *05 · Ticket side panel → Action bar · every
//! state*; d0015, d0018, d0033, d0052, d0060).
//!
//! The primary button is always the one thing that unblocks the ticket. Actions whose features arrive in later
//! milestones are listed here with the ticket that enables them and stay **hidden** (not disabled) until then.

use crewboard_core::board::tickets::Ticket;
use crewboard_core::domain::{Column, Kind, Status};

use crate::icons::IconName;
use crate::view_models::card_menu;
use crate::view_models::delete;
use crate::view_models::epic_actions::{can_close_epic, can_move_to_epic};
use crate::view_models::menu::MenuItem;

/// A button of the action bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Backlog → Dev (⇧⌘D).
    Start,
    Edit,
    /// Shows the Terminal tab.
    OpenTerminal,
    /// Run (M9, T-0134).
    Run,
    /// Orange family: the Terminal tab, focused.
    AnswerInTerminal,
    /// Queued and waiting for a parent: show the first parent in the panel.
    ViewParent,
    /// Interrupted (M7, T-0121).
    Resume,
    /// Start interrupted (T-0063).
    RetryStart,
    /// Ready for Review (M6, T-0095).
    Approve,
    /// Ready for Review (M6, T-0094).
    SendBack,
    /// PR open (M10, T-0150).
    OpenPrInBrowser,
    MarkMerged,
    MarkClosed,
    /// Done (M6, T-0091).
    ViewDiff,
    /// Closed (M6, T-0106).
    Reopen,
}

/// How a button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Primary,
    Secondary,
    /// The orange "Answer in terminal".
    Attention,
    /// Mark closed.
    Danger,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start (to Dev)",
            Self::Edit => "Edit",
            Self::OpenTerminal => "Open terminal",
            Self::Run => "Run",
            Self::AnswerInTerminal => "Answer in terminal",
            Self::ViewParent => "View parent",
            Self::Resume => "Resume",
            Self::RetryStart => "Retry start",
            Self::Approve => "Approve · Local merge",
            Self::SendBack => "Send back",
            Self::OpenPrInBrowser => "Open PR in browser",
            Self::MarkMerged => "Mark merged",
            Self::MarkClosed => "Mark closed",
            Self::ViewDiff => "View diff",
            Self::Reopen => "Reopen",
        }
    }

    pub fn icon(self) -> Option<IconName> {
        Some(match self {
            Self::Start => IconName::Play,
            Self::Edit => IconName::Pencil,
            Self::OpenTerminal | Self::AnswerInTerminal => IconName::SquareTerminal,
            Self::Run => IconName::Play,
            Self::ViewParent => IconName::Link2,
            Self::Resume | Self::RetryStart => IconName::RotateCw,
            Self::SendBack => IconName::Undo2,
            Self::OpenPrInBrowser => IconName::ExternalLink,
            Self::MarkMerged => IconName::GitMerge,
            Self::ViewDiff => IconName::FileDiff,
            Self::Reopen => IconName::RotateCcw,
            Self::Approve | Self::MarkClosed => return None,
        })
    }

    /// `None` when it works in this build; otherwise the ticket that wires it (until then it is hidden).
    pub fn arrives_with(self) -> Option<&'static str> {
        match self {
            Self::Start
            | Self::Edit
            | Self::OpenTerminal
            | Self::AnswerInTerminal
            | Self::ViewParent
            | Self::RetryStart => None,
            Self::Run => Some("T-0134"),
            Self::Resume => Some("T-0121"),
            Self::Approve => Some("T-0095"),
            Self::SendBack => Some("T-0094"),
            Self::OpenPrInBrowser | Self::MarkMerged | Self::MarkClosed => Some("T-0150"),
            Self::ViewDiff => Some("T-0091"),
            Self::Reopen => Some("T-0106"),
        }
    }
}

/// The bar's buttons for a ticket, in order, with how each looks. Includes the ones that arrive later.
pub fn all_buttons(ticket: &Ticket) -> Vec<(Action, Look)> {
    use Action as A;
    use Look::*;
    let s = ticket.status;
    if ticket.kind == Kind::Epic {
        // No Start on an epic: its children start (d0059). Its review flow is T-0109.
        return match ticket.column {
            Column::Backlog => vec![(A::Edit, Secondary)],
            Column::Done if s == Status::ClosedNotMerged => vec![(A::Reopen, Secondary)],
            _ => Vec::new(),
        };
    }
    match s {
        Status::Backlog => vec![(A::Start, Primary), (A::Edit, Secondary)],
        Status::Parked => vec![(A::Start, Primary), (A::Edit, Secondary)],
        Status::EpicInProgress => Vec::new(),
        // Held because the base diverged: once pushed or pulled, Retry start (d0059).
        Status::Held
            if ticket
                .start_problem
                .as_ref()
                .is_some_and(|p| p.step == crewboard_core::domain::StartStep::SyncBase) =>
        {
            vec![(A::RetryStart, Primary), (A::Edit, Secondary)]
        }
        Status::Queued | Status::WaitingForSlot | Status::Held | Status::ParentClosed => {
            let mut bar = vec![(A::Edit, Secondary)];
            if !ticket.depends_on.is_empty() {
                bar.push((A::ViewParent, Secondary));
            }
            bar
        }
        Status::StartInterrupted => vec![(A::RetryStart, Primary)],
        Status::Working | Status::WorkingInBackground | Status::WaitingForLimitReset => {
            vec![(A::OpenTerminal, Secondary), (A::Run, Secondary)]
        }
        Status::AsksYou | Status::NeedsYou | Status::Idle => vec![(A::AnswerInTerminal, Attention)],
        Status::Interrupted | Status::ResumeFailed => vec![(A::Resume, Primary)],
        Status::ReadyForReview => vec![(A::Approve, Primary), (A::SendBack, Secondary)],
        Status::PrOpen => {
            // Epic children never offer the PR actions; they merge locally (d0059).
            if ticket.epic.is_some() {
                Vec::new()
            } else {
                vec![
                    (A::OpenPrInBrowser, Secondary),
                    (A::MarkMerged, Primary),
                    (A::MarkClosed, Danger),
                ]
            }
        }
        Status::Merged => vec![(A::ViewDiff, Secondary)],
        Status::ClosedNotMerged => vec![(A::Reopen, Secondary)],
    }
}

/// The buttons shown now: the ones whose features exist in this build.
pub fn buttons(ticket: &Ticket) -> Vec<(Action, Look)> {
    all_buttons(ticket)
        .into_iter()
        .filter(|(a, _)| a.arrives_with().is_none())
        .collect()
}

pub const EDIT_TICKET: &str = "edit-ticket";
pub const COPY_BRANCH: &str = "copy-branch";
pub const COPY_ATTACH: &str = "copy-attach";

/// The "…" menu: Edit ticket, Pin / Unpin, Copy branch name, Copy attach command (with a session), Move to
/// epic…, Close epic…, and Delete… (enabled only for never-started tickets, d0060). Close ticket… and Submit for
/// review… join with T-0106.
pub fn menu(
    ticket: &Ticket,
    has_epics: bool,
    has_branch: bool,
    has_session: bool,
) -> Vec<MenuItem> {
    let mut items = vec![MenuItem::new(EDIT_TICKET, "Edit ticket").icon(IconName::Pencil)];
    let pin = if ticket.flags.is_pinned() {
        MenuItem::new(card_menu::UNPIN, "Unpin").icon(IconName::PinOff)
    } else {
        MenuItem::new(card_menu::PIN, "Pin to top").icon(IconName::Pin)
    };
    items.push(pin);
    if has_branch {
        items.push(MenuItem::new(COPY_BRANCH, "Copy branch name").icon(IconName::GitBranch));
    }
    if has_session {
        items.push(MenuItem::new(COPY_ATTACH, "Copy attach command").icon(IconName::Copy));
    }
    if has_epics && can_move_to_epic(ticket) {
        items.push(MenuItem::new(card_menu::MOVE_TO_EPIC, "Move to epic…").icon(IconName::Layers));
    }
    if can_close_epic(ticket) {
        items.push(
            MenuItem::new(card_menu::CLOSE_EPIC, "Close epic…")
                .icon(IconName::CircleSlash)
                .danger()
                .separated(),
        );
    }
    items.push(delete::menu_item(ticket));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::card_fixtures::ticket;
    use Action as A;

    fn shown(status: Status) -> Vec<Action> {
        buttons(&ticket("SHOP-1", "t", status))
            .into_iter()
            .map(|(a, _)| a)
            .collect()
    }

    fn all(status: Status) -> Vec<(Action, Look)> {
        all_buttons(&ticket("SHOP-1", "t", status))
    }

    /// Every row of *Action bar · every state*, as designed (later features included).
    #[test]
    fn every_row_of_the_frame() {
        use Look::*;
        assert_eq!(
            all(Status::Backlog),
            [(A::Start, Primary), (A::Edit, Secondary)]
        );
        assert_eq!(
            all(Status::Working),
            [(A::OpenTerminal, Secondary), (A::Run, Secondary)]
        );
        for s in [Status::NeedsYou, Status::AsksYou, Status::Idle] {
            assert_eq!(all(s), [(A::AnswerInTerminal, Attention)]);
        }
        let mut waiting = ticket("SHOP-20", "t", Status::Queued);
        waiting.depends_on = vec!["SHOP-18".parse().unwrap()];
        assert_eq!(
            all_buttons(&waiting),
            [(A::Edit, Secondary), (A::ViewParent, Secondary)]
        );
        assert_eq!(all(Status::Interrupted), [(A::Resume, Primary)]);
        assert_eq!(all(Status::StartInterrupted), [(A::RetryStart, Primary)]);
        assert_eq!(
            all(Status::ReadyForReview),
            [(A::Approve, Primary), (A::SendBack, Secondary)]
        );
        assert_eq!(
            all(Status::PrOpen),
            [
                (A::OpenPrInBrowser, Secondary),
                (A::MarkMerged, Primary),
                (A::MarkClosed, Danger)
            ]
        );
        assert_eq!(all(Status::Merged), [(A::ViewDiff, Secondary)]);
        assert_eq!(all(Status::ClosedNotMerged), [(A::Reopen, Secondary)]);
    }

    #[test]
    fn later_features_are_hidden_not_disabled() {
        assert_eq!(shown(Status::Backlog), [A::Start, A::Edit]);
        assert_eq!(
            shown(Status::Working),
            [A::OpenTerminal],
            "Run comes with M9"
        );
        assert_eq!(shown(Status::ReadyForReview), Vec::<Action>::new());
        assert_eq!(shown(Status::PrOpen), Vec::<Action>::new());
        assert_eq!(shown(Status::Interrupted), Vec::<Action>::new());
        assert_eq!(shown(Status::NeedsYou), [A::AnswerInTerminal]);
        assert_eq!(shown(Status::StartInterrupted), [A::RetryStart]);
    }

    #[test]
    fn a_diverged_hold_offers_retry_start() {
        let mut held = ticket("BILL-6", "Invoices", Status::Held);
        assert_eq!(buttons(&held), [(A::Edit, Look::Secondary)]);
        held.start_problem = Some(crewboard_core::board::tickets::StartProblem {
            step: crewboard_core::domain::StartStep::SyncBase,
            message: "`develop` and `origin/develop` have diverged. Push or pull, then retry"
                .into(),
        });
        assert_eq!(buttons(&held)[0], (A::RetryStart, Look::Primary));
    }

    #[test]
    fn epics_and_children_follow_their_rules() {
        let mut epic = ticket("SHOP-40", "Wishlist", Status::Backlog);
        epic.kind = Kind::Epic;
        assert_eq!(
            all_buttons(&epic),
            [(A::Edit, Look::Secondary)],
            "no Start on an epic"
        );
        let mut child = ticket("SHOP-40.2", "Wishlist page", Status::PrOpen);
        child.epic = Some("SHOP-40".parse().unwrap());
        assert!(
            all_buttons(&child).is_empty(),
            "children never offer PR actions"
        );
        let child = ticket("SHOP-40.1", "Wishlist API", Status::Backlog);
        assert_eq!(buttons(&child)[0].0, A::Start);
    }

    #[test]
    fn menu_delete_only_for_never_started() {
        let fresh = ticket("SHOP-21", "Order confirmation email", Status::Backlog);
        let items = menu(&fresh, false, false, false);
        let delete = items.last().unwrap();
        assert_eq!(delete.label, "Delete…");
        assert!(!delete.disabled);
        assert_eq!(
            delete.hint.as_deref(),
            Some("Never started · nothing on disk")
        );

        let mut started = ticket("SHOP-19", "Price table", Status::Working);
        started.started_at = Some(1);
        let items = menu(&started, false, true, true);
        let delete = items.last().unwrap();
        assert!(delete.disabled);
        assert_eq!(
            delete.hint.as_deref(),
            Some("Started tickets can only be closed.")
        );
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Edit ticket",
                "Pin to top",
                "Copy branch name",
                "Copy attach command",
                "Delete…"
            ]
        );
    }
}
