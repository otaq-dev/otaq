//! The empty board of a project with no tickets yet (UI-08; *14 · First run → 4 · Done · empty board*): its
//! buttons and what each asks of the shell.

use crate::icons::IconName;
use crate::shell::ShellRequest;

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
    fn the_buttons_ask_for_a_ticket_and_the_project_terminal() {
        let requests: Vec<ShellRequest> = actions().into_iter().map(|a| a.request).collect();
        assert_eq!(
            requests,
            [ShellRequest::NewTicket, ShellRequest::ProjectTerminal]
        );
    }
}
