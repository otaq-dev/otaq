//! The empty board of a project with no tickets yet (UI-08; *14 · First run → 4 · Done · empty board*): when it
//! shows, its buttons and what each asks of the shell.

use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::keymap::{KeyAction, Platform};
use crate::shell::{ShellRequest, tilde};

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

/// A button of the empty board as the page draws it: its shortcut action is what a click runs, so the button and
/// the key do the same thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EmptyBoardButton")]
pub struct Button {
    pub label: &'static str,
    pub icon: IconName,
    pub primary: bool,
    pub action: KeyAction,
}

/// "⌘N new ticket" under the buttons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EmptyBoardHint")]
pub struct Hint {
    pub keys: Option<String>,
    pub label: &'static str,
}

/// The empty board's centred card (*14 → 4 · Done · empty board*; *03 → Empty column & empty board*). The M12 parts
/// (the Project agent button and hint, the Auto-merge line) are T-0240's and T-0243's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EmptyBoardView")]
pub struct View {
    pub title: String,
    pub text: String,
    pub buttons: Vec<Button>,
    pub hints: Vec<Hint>,
}

fn action_of(request: &ShellRequest) -> KeyAction {
    match request {
        ShellRequest::ProjectTerminal => KeyAction::ProjectTerminal,
        _ => KeyAction::NewTicket,
    }
}

/// "shop-web is ready" for the project `name` in `repo_path`.
pub fn view(name: &str, repo_path: &str, platform: Platform) -> View {
    View {
        title: format!("{name} is ready"),
        text: format!(
            "Write a ticket for the first thing an agent should do \u{2014} or open the Project terminal (a shell in {}) and run claude to split a feature into tickets.",
            tilde(repo_path)
        ),
        buttons: actions()
            .iter()
            .map(|a| Button {
                label: a.label,
                icon: a.icon,
                primary: a.primary,
                action: action_of(&a.request),
            })
            .collect(),
        hints: vec![
            Hint {
                keys: KeyAction::NewTicket.keys(platform),
                label: "new ticket",
            },
            Hint {
                keys: KeyAction::ProjectTerminal.keys(platform),
                label: "project terminal",
            },
        ],
    }
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
    fn the_page_view_names_the_project_and_runs_the_shortcuts() {
        let v = view("shop-web", "/Users/me/code/shop-web", Platform::Mac);
        assert_eq!(v.title, "shop-web is ready");
        assert!(v.text.contains("(a shell in "), "{}", v.text);
        let actions: Vec<KeyAction> = v.buttons.iter().map(|b| b.action).collect();
        assert_eq!(actions, [KeyAction::NewTicket, KeyAction::ProjectTerminal]);
        assert_eq!(v.hints[0].keys.as_deref(), Some("⌘N"));
        assert!(v.buttons[0].primary && !v.buttons[1].primary);
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
