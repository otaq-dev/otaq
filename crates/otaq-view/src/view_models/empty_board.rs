//! The empty board of a project with no tickets yet (UI-08; *14 · First run → 4 · Done · empty board*): when it
//! shows, its buttons and what each asks of the shell.

use otaq_core::board::settings::AutoMerge;
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

/// The empty board's centred card (*14 → 4 · Done · empty board*; *03 → Empty column & empty board*): three ways to
/// start (New ticket, a Project agent, the Project terminal; d0068) and the line about Auto-merge (d0064).
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

/// "Project agent" (⇧⌘A): Claude investigates and creates tickets (d0068). Page only; the GPUI app had no Project
/// agents.
fn project_agent_button() -> Button {
    Button {
        label: "Project agent",
        icon: IconName::Bot,
        primary: false,
        action: KeyAction::NewProjectAgent,
    }
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
            "Write a ticket for the first thing an agent should do, open a Project agent to let Claude investigate and create tickets, or open a Project terminal (a shell in {}).",
            tilde(repo_path)
        ),
        buttons: {
            let mut buttons: Vec<Button> = actions()
                .iter()
                .map(|a| Button {
                    label: a.label,
                    icon: a.icon,
                    primary: a.primary,
                    action: action_of(&a.request),
                })
                .collect();
            buttons.insert(1, project_agent_button());
            buttons
        },
        hints: vec![
            Hint {
                keys: KeyAction::NewTicket.keys(platform),
                label: "new ticket",
            },
            Hint {
                keys: KeyAction::ProjectTerminal.keys(platform),
                label: "terminals & agents",
            },
            Hint {
                keys: KeyAction::NewProjectAgent.keys(platform),
                label: "new Project agent",
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

    #[test]
    fn the_page_view_names_the_project_and_runs_the_shortcuts() {
        let v = view("shop-web", "/Users/me/code/shop-web", Platform::Mac);
        assert_eq!(v.title, "shop-web is ready");
        assert!(v.text.contains("(a shell in "), "{}", v.text);
        let actions: Vec<KeyAction> = v.buttons.iter().map(|b| b.action).collect();
        assert_eq!(
            actions,
            [
                KeyAction::NewTicket,
                KeyAction::NewProjectAgent,
                KeyAction::ProjectTerminal
            ]
        );
        assert_eq!(v.buttons[1].label, "Project agent");
        assert_eq!(v.hints[0].keys.as_deref(), Some("⌘N"));
        let hints: Vec<(&str, &str)> = v
            .hints
            .iter()
            .map(|h| (h.keys.as_deref().unwrap(), h.label))
            .collect();
        assert_eq!(
            hints,
            [
                ("⌘N", "new ticket"),
                ("⇧⌘T", "terminals & agents"),
                ("⇧⌘A", "new Project agent")
            ]
        );
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
