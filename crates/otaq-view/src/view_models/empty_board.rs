//! The empty board of a project with no tickets yet (UI-08, SHL-06; *14 · First run → 4 · Done · empty board*,
//! *03 → Empty column & empty board*): when it shows, its buttons and what each asks of the shell. Since d0087 the
//! Project agent pane is always beside the board, so the card points at it instead of offering a button for it.

use otaq_core::board::settings::AutoMerge;
use serde::Serialize;
use ts_rs::TS;

use crate::icons::IconName;
use crate::keymap::{KeyAction, Platform};
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

/// The empty board's centred card (*14 → 4 · Done · empty board*; *03 → Empty column & empty board*): New ticket and
/// the Project terminal, the text pointing at the pane's Project agent (d0068, d0087), and the line about Auto-merge
/// (d0064).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "EmptyBoardView")]
pub struct View {
    pub title: String,
    pub text: String,
    pub buttons: Vec<Button>,
    pub hints: Vec<Hint>,
    /// "Auto-merge is off. …", once the project's settings are read (`auto_merge_line`).
    pub auto_merge: Option<String>,
}

/// The empty board's line about Auto-merge for the project's effective setting (`inactive`: set, but validation is
/// off, so it does nothing).
pub fn auto_merge_line(value: AutoMerge, inactive: bool) -> String {
    match (value, inactive) {
        (AutoMerge::Off, _) => {
            "Auto-merge is off. Validated tickets wait for your review; turn it on in project settings."
        }
        (_, true) => {
            "Auto-merge does nothing while Validation is off; turn Validation on in project settings."
        }
        (AutoMerge::EpicChildren, false) => {
            "Auto-merge is on for epic children: validated ones merge into their epic branch, locally."
        }
        (AutoMerge::All, false) => {
            "Auto-merge is on: validated tickets merge into their base branch, locally, never pushed."
        }
    }
    .to_owned()
}

fn action_of(request: &ShellRequest) -> KeyAction {
    match request {
        ShellRequest::ProjectTerminal => KeyAction::ProjectTerminal,
        _ => KeyAction::NewTicket,
    }
}

/// "shop-web is ready" for the project `name`.
pub fn view(name: &str, platform: Platform) -> View {
    let new_ticket = KeyAction::NewTicket.keys(platform);
    View {
        title: format!("{name} is ready"),
        text: match &new_ticket {
            Some(keys) => format!(
                "Write the first ticket (+ in Backlog or {keys}), or ask the Project agent on the left to investigate and create tickets."
            ),
            None => "Write the first ticket (+ in Backlog), or ask the Project agent on the left to investigate and create tickets.".to_owned(),
        },
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
                keys: new_ticket,
                label: "new ticket",
            },
            Hint {
                keys: KeyAction::ProjectTerminal.keys(platform),
                label: "project terminal",
            },
        ],
        auto_merge: None,
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

    /// *03 → Empty column & empty board* (`kItEl`), *14 → 4 · Done · empty board* (`oVlcH`), d0087: the text points
    /// at the pane; New ticket and Project terminal, no Project agent button.
    #[test]
    fn the_page_view_names_the_project_and_runs_the_shortcuts() {
        let v = view("shop-web", Platform::Mac);
        assert_eq!(v.title, "shop-web is ready");
        assert_eq!(
            v.text,
            "Write the first ticket (+ in Backlog or ⌘N), or ask the Project agent on the left to investigate and create tickets."
        );
        let actions: Vec<KeyAction> = v.buttons.iter().map(|b| b.action).collect();
        assert_eq!(actions, [KeyAction::NewTicket, KeyAction::ProjectTerminal]);
        let labels: Vec<&str> = v.buttons.iter().map(|b| b.label).collect();
        assert_eq!(labels, ["New ticket", "Project terminal"]);
        assert_eq!(v.hints[0].keys.as_deref(), Some("⌘N"));
        let hints: Vec<(&str, &str)> = v
            .hints
            .iter()
            .map(|h| (h.keys.as_deref().unwrap(), h.label))
            .collect();
        assert_eq!(hints, [("⌘N", "new ticket"), ("⇧⌘T", "project terminal")]);
        assert!(v.buttons[0].primary && !v.buttons[1].primary);
    }

    #[test]
    fn the_auto_merge_line_says_what_happens() {
        assert_eq!(
            auto_merge_line(AutoMerge::Off, false),
            "Auto-merge is off. Validated tickets wait for your review; turn it on in project settings."
        );
        assert!(auto_merge_line(AutoMerge::All, true).starts_with("Auto-merge does nothing"));
        assert!(auto_merge_line(AutoMerge::EpicChildren, false).contains("epic children"));
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
