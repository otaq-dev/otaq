//! The empty board of a project with no tickets yet (UI-08; *14 · First run → 4 · Done · empty board*): when it
//! shows, its buttons and what each asks of the shell.

use crate::icons::IconName;
use crate::shell::ShellRequest;

/// Whether the project's empty board shows instead of the board. New ticket's title field sits at the top of
/// Backlog (*08 · Ticket editor → Quick-create*), so the board shows while it is open; Esc with no ticket brings
/// the empty board back.
pub fn shows(tickets: usize, quick_creating: bool) -> bool {
    tickets == 0 && !quick_creating
}

/// One of the empty board's buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: IconName,
    pub primary: bool,
    pub request: ShellRequest,
}

/// "New ticket" (⌘N) and "Project terminal" (⇧⌘T, RUN-06): the same requests as the header's buttons.
pub fn actions() -> [Action; 2] {
    [
        Action {
            id: "empty-new-ticket",
            label: "New ticket",
            icon: IconName::Plus,
            primary: true,
            request: ShellRequest::NewTicket,
        },
        Action {
            id: "empty-project-terminal",
            label: "Project terminal",
            icon: IconName::SquareTerminal,
            primary: false,
            request: ShellRequest::ProjectTerminal,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ticket_leaves_the_empty_board_while_its_title_field_is_open() {
        assert!(shows(0, false));
        assert!(!shows(0, true), "the title field is on the board");
        assert!(!shows(1, false));
        assert!(!shows(1, true));
    }

    #[test]
    fn the_buttons_ask_for_a_ticket_and_the_project_terminal() {
        let requests: Vec<ShellRequest> = actions().into_iter().map(|a| a.request).collect();
        assert_eq!(
            requests,
            [ShellRequest::NewTicket, ShellRequest::ProjectTerminal]
        );
    }
}
