//! The panel's action bar for each state (UI-04, TKT-08; brief 05, *05 · Ticket side panel → Action bar · every
//! state*; d0015, d0018, d0033, d0052, d0060).
//!
//! The primary button is always the one thing that unblocks the ticket. Actions whose features arrive in later
//! milestones are listed here with the ticket that enables them and stay **hidden** (not disabled) until then.

use otaq_core::board::tickets::Ticket;
use otaq_core::domain::{Column, Kind, Status};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icons::IconName;
use crate::view_models::card_menu;
use crate::view_models::delete;
use crate::view_models::epic_actions::{can_close_epic, can_move_to_epic};
use crate::view_models::menu::MenuItem;

/// A button of the action bar. `PanelAction` on the page, which sends it back to do it (T-0222).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "PanelAction")]
pub enum Action {
    /// Backlog → Dev (⇧⌘D).
    Start,
    Edit,
    /// Shows the Terminal tab.
    OpenTerminal,
    /// Run (M9, T-0134).
    Run,
    /// Needs-you family: the Terminal tab, focused.
    AnswerInTerminal,
    /// Queued and waiting for a parent: show the first parent in the panel.
    ViewParent,
    /// Interrupted (T-0121).
    Resume,
    /// Resume failed: the same resume again (REC-03).
    RetryResume,
    /// Resume failed: a new conversation in the same worktree, after a confirmation (REC-03).
    StartFromScratch,
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
    /// Validating…: the Terminal tab on the validator's side (d0063, T-0237).
    OpenValidator,
    /// Needs you · validator: the turquoise "Answer in terminal · Validator".
    AnswerValidator,
    /// Validation failed N times or ended without a result: Ready for Review without a pass (VAL-07).
    MoveToReviewAnyway,
    /// Validation failed N times: the findings go to the agent with one more round.
    AnotherRound,
    /// Validation failed N times: the user's note goes with the findings (opens the note box).
    AnswerAgent,
    /// Validation ended without a result: a fresh validator.
    RetryValidation,
    /// Auto-merge stopped: the merge again (AMG-03).
    RetryMerge,
    /// Auto-merge stopped: Ready for Review, not merged again (AMG-04).
    MoveToReview,
}

/// How a button looks. `PanelButtonLook` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "PanelButtonLook")]
pub enum Look {
    Primary,
    Secondary,
    /// The turquoise "Answer in terminal".
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
            Self::RetryResume => "Retry",
            Self::StartFromScratch => "Start from scratch",
            Self::RetryStart => "Retry start",
            Self::Approve => "Approve · Local merge",
            Self::SendBack => "Send back",
            Self::OpenPrInBrowser => "Open PR in browser",
            Self::MarkMerged => "Mark merged",
            Self::MarkClosed => "Mark closed",
            Self::ViewDiff => "View diff",
            Self::Reopen => "Reopen",
            Self::OpenValidator => "Open terminal \u{b7} Validator",
            Self::AnswerValidator => "Answer in terminal \u{b7} Validator",
            Self::MoveToReviewAnyway => "Move to Review anyway",
            Self::AnotherRound => "Give it another round",
            Self::AnswerAgent => "Answer the agent\u{2026}",
            Self::RetryValidation => "Retry validation",
            Self::RetryMerge => "Retry merge",
            Self::MoveToReview => "Move to Review",
        }
    }

    pub fn icon(self) -> Option<IconName> {
        Some(match self {
            Self::Start => IconName::Play,
            Self::Edit => IconName::Pencil,
            Self::OpenTerminal
            | Self::AnswerInTerminal
            | Self::OpenValidator
            | Self::AnswerValidator => IconName::SquareTerminal,
            // *Action bar · every state → Validation failed 3 times, … ended without a result, Auto-merge stopped*.
            Self::MoveToReviewAnyway | Self::MoveToReview => IconName::Eye,
            Self::AnotherRound | Self::RetryValidation | Self::RetryMerge => IconName::RotateCcw,
            Self::AnswerAgent => IconName::MessageSquare,
            Self::Run => IconName::Play,
            Self::ViewParent => IconName::Link2,
            Self::Resume => IconName::Play,
            // Resume failed: Retry and Start from scratch (*16 → Resume · in progress → resumed → failed*).
            Self::RetryResume => IconName::RefreshCw,
            Self::RetryStart | Self::StartFromScratch => IconName::RotateCcw,
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
            | Self::RetryStart
            | Self::ViewDiff
            | Self::SendBack
            | Self::Approve => None,
            Self::Run => Some("T-0134"),
            Self::Resume | Self::RetryResume | Self::StartFromScratch => None,
            Self::OpenPrInBrowser | Self::MarkMerged | Self::MarkClosed => None,
            Self::Reopen => None,
            Self::OpenValidator
            | Self::AnswerValidator
            | Self::MoveToReviewAnyway
            | Self::AnotherRound
            | Self::AnswerAgent
            | Self::RetryValidation
            | Self::RetryMerge
            | Self::MoveToReview => None,
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
            // A delivery unit in Review: Approve, never Send back (EPC-05).
            Column::ReadyForReview => vec![(A::Approve, Primary)],
            // The epic's PR (EPC-08): the same manual outcomes as a ticket's (REV-13).
            Column::Approved if s == Status::PrOpen => vec![
                (A::OpenPrInBrowser, Secondary),
                (A::MarkMerged, Primary),
                (A::MarkClosed, Danger),
            ],
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
                .is_some_and(|p| p.step == otaq_core::domain::StartStep::SyncBase) =>
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
        Status::Interrupted => vec![(A::Resume, Primary)],
        // *16 → Resume · in progress → resumed → failed*: Start from scratch is the primary, Retry beside it.
        Status::ResumeFailed => vec![(A::RetryResume, Secondary), (A::StartFromScratch, Primary)],
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
        // The M12 rows (d0063, d0064): Stop validation and Skip to Review are in the "…" menu.
        Status::Validating => vec![(A::OpenValidator, Secondary), (A::Run, Secondary)],
        Status::ValidatorNeedsYou => vec![(A::AnswerValidator, Attention)],
        Status::ValidationFailed => vec![
            (A::MoveToReviewAnyway, Primary),
            (A::AnotherRound, Secondary),
            (A::AnswerAgent, Secondary),
        ],
        Status::ValidationNoResult => vec![
            (A::RetryValidation, Primary),
            (A::MoveToReviewAnyway, Secondary),
        ],
        Status::AutoMergeStopped => vec![(A::RetryMerge, Primary), (A::MoveToReview, Secondary)],
        // The agent fixes the Auto-merge conflict in its own terminal.
        Status::MergeConflictFixing => vec![(A::OpenTerminal, Secondary)],
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
pub const STOP_VALIDATION: &str = "stop-validation";
pub const SKIP_TO_REVIEW: &str = "skip-to-review";

/// Validating… and Needs you · validator: the validator runs (VAL-06).
pub fn validating(ticket: &Ticket) -> bool {
    matches!(
        ticket.status,
        Status::Validating | Status::ValidatorNeedsYou
    )
}

/// The "…" menu: Stop validation and Skip to Review while it validates (d0063), Edit ticket, Pin / Unpin, Copy
/// branch name, Copy attach command (with a session), Submit for review (skips validation) and Validate and submit
/// (both hidden while it validates), Move to epic…, Close epic…, Close ticket…, and Delete… (enabled only for
/// never-started tickets, d0060).
pub fn menu(
    ticket: &Ticket,
    has_epics: bool,
    has_branch: bool,
    has_session: bool,
) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if validating(ticket) {
        items.push(
            MenuItem::new(STOP_VALIDATION, "Stop validation")
                .icon(IconName::Square)
                .hint("round not counted"),
        );
        items.push(
            MenuItem::new(SKIP_TO_REVIEW, "Skip to Review")
                .icon(IconName::SkipForward)
                .hint("never auto-merged"),
        );
    }
    let edit = MenuItem::new(EDIT_TICKET, "Edit ticket").icon(IconName::Pencil);
    items.push(if items.is_empty() {
        edit
    } else {
        edit.separated()
    });
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
    if let Some(submit) = crate::view_models::close::submit_item(ticket) {
        let validate = crate::view_models::close::validate_item(&submit);
        items.push(MenuItem {
            label: "Submit for review\u{2026} (skips validation)".to_owned(),
            ..submit
        });
        items.push(validate);
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
    items.extend(crate::view_models::close::close_item(ticket));
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
        assert_eq!(shown(Status::ReadyForReview), [A::Approve, A::SendBack]);
        assert_eq!(
            shown(Status::PrOpen),
            [A::OpenPrInBrowser, A::MarkMerged, A::MarkClosed],
            "M10"
        );
        assert_eq!(shown(Status::Interrupted), [A::Resume]);
        assert_eq!(
            shown(Status::ResumeFailed),
            [A::RetryResume, A::StartFromScratch]
        );
        assert_eq!(shown(Status::NeedsYou), [A::AnswerInTerminal]);
        assert_eq!(shown(Status::StartInterrupted), [A::RetryStart]);
        // *16 → Resume · in progress → resumed → failed*; Retry start as on the 05 action bar.
        assert_eq!(A::RetryResume.icon(), Some(IconName::RefreshCw));
        assert_eq!(
            buttons(&ticket(
                "BILL-4",
                "Invoice PDF export",
                Status::ResumeFailed
            )),
            [
                (A::RetryResume, Look::Secondary),
                (A::StartFromScratch, Look::Primary)
            ],
            "Start from scratch is the primary"
        );
        assert_eq!(A::StartFromScratch.icon(), Some(IconName::RotateCcw));
        assert_eq!(A::RetryStart.icon(), Some(IconName::RotateCcw));
    }

    #[test]
    fn a_diverged_hold_offers_retry_start() {
        let mut held = ticket("BILL-6", "Invoices", Status::Held);
        assert_eq!(buttons(&held), [(A::Edit, Look::Secondary)]);
        held.start_problem = Some(otaq_core::board::tickets::StartProblem {
            step: otaq_core::domain::StartStep::SyncBase,
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
                "Submit for review… (skips validation)",
                "Validate and submit",
                "Close ticket…",
                "Delete…"
            ]
        );
        let validate = &items[5];
        assert_eq!(
            (validate.id.as_str(), validate.disabled, validate.icon),
            (
                crate::view_models::close::VALIDATE_AND_SUBMIT,
                true,
                Some(IconName::ShieldCheck)
            ),
            "mid-turn, like Submit for review"
        );
    }

    /// The M12 rows of *Action bar · every state* (d0063, d0064).
    #[test]
    fn the_validation_rows() {
        use Look::*;
        assert_eq!(
            all(Status::Validating),
            [(A::OpenValidator, Secondary), (A::Run, Secondary)]
        );
        assert_eq!(shown(Status::Validating), [A::OpenValidator]);
        assert_eq!(
            all(Status::ValidatorNeedsYou),
            [(A::AnswerValidator, Attention)]
        );
        assert_eq!(
            all(Status::ValidationFailed),
            [
                (A::MoveToReviewAnyway, Primary),
                (A::AnotherRound, Secondary),
                (A::AnswerAgent, Secondary)
            ]
        );
        assert_eq!(
            all(Status::ValidationNoResult),
            [
                (A::RetryValidation, Primary),
                (A::MoveToReviewAnyway, Secondary)
            ]
        );
        assert_eq!(
            all(Status::AutoMergeStopped),
            [(A::RetryMerge, Primary), (A::MoveToReview, Secondary)]
        );
        assert_eq!(A::OpenValidator.label(), "Open terminal \u{b7} Validator");
        assert_eq!(A::AnswerAgent.label(), "Answer the agent\u{2026}");
        assert_eq!(A::MoveToReviewAnyway.icon(), Some(IconName::Eye));
    }

    /// While it validates: Stop validation and Skip to Review first; both submit items hidden.
    #[test]
    fn the_menu_while_validating() {
        let mut t = ticket("SHOP-29", "Coupon field", Status::Validating);
        t.started_at = Some(1);
        let items = menu(&t, false, true, false);
        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Stop validation",
                "Skip to Review",
                "Edit ticket",
                "Pin to top",
                "Copy branch name",
                "Close ticket…",
                "Delete…"
            ]
        );
        assert_eq!(items[0].hint.as_deref(), Some("round not counted"));
        assert_eq!(items[1].hint.as_deref(), Some("never auto-merged"));
        assert!(items[2].separated);
        let mut idle = ticket("SHOP-19", "Price table", Status::Idle);
        idle.started_at = Some(1);
        let items = menu(&idle, false, false, false);
        assert_eq!(items[0].label, "Edit ticket");
        assert!(!items[0].separated);
        assert!(
            items
                .iter()
                .any(|i| i.label == "Validate and submit" && !i.disabled)
        );
        t.status = Status::ValidatorNeedsYou;
        assert_eq!(menu(&t, false, true, false)[0].id, STOP_VALIDATION);
    }
}
