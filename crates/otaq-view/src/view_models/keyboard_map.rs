//! The keyboard map as the design shows it (*01 · Components → Keyboard map*, d0058): three groups of rows, each row
//! an action's name with its keys on macOS, on Windows and Linux, and on Windows and Linux while a terminal has focus.
//! The keys come from [`KeyAction::keys`], so the map shown is the map bound. Numbered actions show one row
//! ("Project 1–9"); paired actions share one ("Previous / next ticket in panel").

use serde::Serialize;
use ts_rs::TS;

use crate::keymap::{KeyAction, Platform};

/// One row: the action's name and its keys per column, one Kbd each; `None` where there is no shortcut ("—").
/// `KeyMapRow` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "KeyMapRow")]
pub struct KeyRow {
    pub label: String,
    pub mac: Option<String>,
    pub win_linux: Option<String>,
    pub in_terminal: Option<String>,
}

/// A titled group of rows. `KeyMapGroup` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "KeyMapGroup")]
pub struct KeyGroup {
    pub title: String,
    pub rows: Vec<KeyRow>,
}

/// The columns' heading under each group title.
pub const COLUMNS: &str = "macOS · Win/Linux · in terminal";

/// The notes under the map, as in the frame.
pub const NOTES: [&str; 5] = [
    "No shortcut appears twice across the three columns (0058). OS system shortcuts such as ⌘` are off-limits. \
     Find in terminal scrollback: ⌘F / Ctrl+Shift+F.",
    "While the terminal has focus every key goes to the CLI (Esc, Ctrl+C, Ctrl+J, Ctrl+T…). App shortcuts keep \
     working on macOS (⌘ isn’t used by the terminal). On Windows and Linux they move to Ctrl+Shift+…, like VS Code \
     and Windows Terminal: that’s the “in terminal” column. Leave the terminal with ⌘⇧` / Ctrl+Shift+`, a click \
     outside, or the inbox shortcut.",
    "Update V (0087): Terminals and Settings moved to the bottom of the project rail; ⇧⌘T opens the Terminals \
     drawer (Project terminals only), ⇧⌘A an empty Project agent in the pane. ⌘1–9 stay Project 1–9 and ⌘P Switch \
     project; the Board and Files views are ⇧⌘B / ⇧⌘O, Go to file ⌥⌘P, the pane ⌘L (focus) and ⌘\\ (hide / \
     show), Activity ⇧⌘U. Windows/Linux and in-terminal versions follow 0058's table (the in-terminal column is the \
     proposal for 0058).",
    "⌘K in the description: Link. While the description editor has focus, ⌘K / Ctrl+K adds or edits a link; \
     everywhere else it stays Search / filter (0078, 0058 refined 2026-10-15).",
    "⌘⌥C in the description: Code block. While the description editor’s text has focus, ⌘⌥C / Ctrl+Alt+C adds or \
     removes a code block; everywhere else ⌘⌥C does nothing (the webview’s inspector stays closed) (0078, Q51).",
];

/// How a row's actions are shown.
enum Keys {
    /// One action's keys.
    One(KeyAction),
    /// Two actions' keys in one Kbd, joined as in the frame: `⌘[  ⌘]`, `↵ / Esc`.
    Pair(KeyAction, KeyAction, &'static str),
    /// A numbered action's first number, shown as a range: `⌘1` → `⌘1–9`.
    Range(KeyAction, &'static str),
}

const GROUPS: [(&str, &[(&str, Keys)]); 3] = [
    (
        "Attention",
        &[
            (
                "Next inbox item (terminal focused)",
                Keys::One(KeyAction::NextInboxItem),
            ),
            ("Open inbox", Keys::One(KeyAction::OpenInbox)),
            (
                "Previous / next ticket in panel",
                Keys::Pair(KeyAction::PreviousTicket, KeyAction::NextTicket, "  "),
            ),
            ("Leave terminal focus", Keys::One(KeyAction::LeaveTerminal)),
        ],
    ),
    (
        "Board & navigation",
        &[
            ("Search / filter", Keys::One(KeyAction::Search)),
            (
                "Focus the selected card's epic",
                Keys::One(KeyAction::FocusEpic),
            ),
            ("Switch project", Keys::One(KeyAction::SwitchProject)),
            ("Project 1–9", Keys::Range(KeyAction::Project(1), "1–9")),
            ("Board view", Keys::One(KeyAction::ShowBoard)),
            ("Files view", Keys::One(KeyAction::ShowFiles)),
            ("Go to file (Files view)", Keys::One(KeyAction::GoToFile)),
            (
                "Focus the Project agent pane",
                Keys::One(KeyAction::FocusPane),
            ),
            ("Hide / show the pane", Keys::One(KeyAction::TogglePane)),
            ("Activity popover", Keys::One(KeyAction::OpenActivity)),
            ("New ticket", Keys::One(KeyAction::NewTicket)),
            (
                "Terminals drawer (last-used Project terminal)",
                Keys::One(KeyAction::ProjectTerminal),
            ),
            (
                "New Project agent (empty pane, starts on the first message)",
                Keys::One(KeyAction::NewProjectAgent),
            ),
            ("Settings", Keys::One(KeyAction::Settings)),
        ],
    ),
    (
        "Ticket panel",
        &[
            (
                "Open / close panel",
                Keys::Pair(KeyAction::OpenPanel, KeyAction::ClosePanel, " / "),
            ),
            ("Expand panel", Keys::One(KeyAction::ExpandPanel)),
            ("Switch tab", Keys::Range(KeyAction::SwitchTab(1), "1–6")),
            ("Approve ▾", Keys::One(KeyAction::Approve)),
            ("Send back", Keys::One(KeyAction::SendBack)),
            ("Start (to Ready for Dev)", Keys::One(KeyAction::StartDev)),
        ],
    ),
];

fn column(keys: &Keys, platform: Platform) -> Option<String> {
    match keys {
        Keys::One(action) => action.keys(platform),
        Keys::Pair(first, second, joiner) => {
            let both: Vec<String> = [first, second]
                .iter()
                .filter_map(|a| a.keys(platform))
                .collect();
            (!both.is_empty()).then(|| both.join(joiner))
        }
        Keys::Range(action, range) => action
            .keys(platform)
            .map(|k| format!("{}{range}", k.trim_end_matches('1'))),
    }
}

/// The three groups.
pub fn groups() -> Vec<KeyGroup> {
    GROUPS
        .iter()
        .map(|(title, rows)| KeyGroup {
            title: (*title).to_owned(),
            rows: rows
                .iter()
                .map(|(label, keys)| KeyRow {
                    label: (*label).to_owned(),
                    mac: column(keys, Platform::Mac),
                    win_linux: column(keys, Platform::WinLinux),
                    in_terminal: column(keys, Platform::WinLinuxTerminal),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &str) -> KeyRow {
        groups()
            .into_iter()
            .flat_map(|g| g.rows)
            .find(|r| r.label == label)
            .unwrap()
    }

    #[test]
    fn rows_show_the_bound_keys() {
        let some = |s: &str| Some(s.to_owned());
        let inbox = row("Next inbox item (terminal focused)");
        assert_eq!(
            (inbox.mac, inbox.win_linux, inbox.in_terminal),
            (some("⌘J"), some("Ctrl+J"), some("Ctrl+Shift+J"))
        );
        let project = row("Project 1–9");
        assert_eq!(project.mac, some("⌘1–9"));
        assert_eq!(project.in_terminal, some("Ctrl+Shift+1–9"));
        assert_eq!(row("Switch tab").win_linux, some("Ctrl+Alt+1–6"));
        let panel = row("Open / close panel");
        assert_eq!(panel.mac, some("↵ / Esc"));
        assert_eq!(panel.in_terminal, None, "no shortcut in a terminal");
        assert_eq!(
            row("Previous / next ticket in panel").in_terminal,
            some("Ctrl+Shift+[  Ctrl+Shift+]")
        );
        let focus = row("Focus the selected card's epic");
        assert_eq!(
            (focus.mac, focus.win_linux, focus.in_terminal),
            (some("⇧⌘G"), some("Ctrl+Shift+G"), some("Ctrl+Shift+G"))
        );
        let agent = row("New Project agent (empty pane, starts on the first message)");
        assert_eq!(
            (agent.mac, agent.win_linux, agent.in_terminal),
            (some("⇧⌘A"), some("Ctrl+Shift+A"), some("Ctrl+Shift+A"))
        );
    }

    /// The frame's Board & navigation group (`SteSX`), in its order, with the 0087 rows.
    #[test]
    fn board_and_navigation_is_the_frames() {
        let group = groups().into_iter().nth(1).unwrap();
        assert_eq!(group.title, "Board & navigation");
        type Shown<'a> = (&'a str, Option<&'a str>, Option<&'a str>, Option<&'a str>);
        let shown: Vec<Shown> = group
            .rows
            .iter()
            .map(|r| {
                (
                    r.label.as_str(),
                    r.mac.as_deref(),
                    r.win_linux.as_deref(),
                    r.in_terminal.as_deref(),
                )
            })
            .collect();
        let some = Some;
        assert_eq!(
            shown,
            [
                (
                    "Search / filter",
                    some("⌘K"),
                    some("Ctrl+K"),
                    some("Ctrl+Shift+K")
                ),
                (
                    "Focus the selected card's epic",
                    some("⇧⌘G"),
                    some("Ctrl+Shift+G"),
                    some("Ctrl+Shift+G")
                ),
                (
                    "Switch project",
                    some("⌘P"),
                    some("Ctrl+P"),
                    some("Ctrl+Shift+P")
                ),
                (
                    "Project 1–9",
                    some("⌘1–9"),
                    some("Ctrl+1–9"),
                    some("Ctrl+Shift+1–9")
                ),
                (
                    "Board view",
                    some("⇧⌘B"),
                    some("Ctrl+Shift+B"),
                    some("Ctrl+Shift+B")
                ),
                (
                    "Files view",
                    some("⇧⌘O"),
                    some("Ctrl+Shift+O"),
                    some("Ctrl+Shift+O")
                ),
                (
                    "Go to file (Files view)",
                    some("⌥⌘P"),
                    some("Ctrl+Alt+P"),
                    some("Ctrl+Shift+Alt+P")
                ),
                (
                    "Focus the Project agent pane",
                    some("⌘L"),
                    some("Ctrl+L"),
                    some("Ctrl+Shift+L")
                ),
                (
                    "Hide / show the pane",
                    some("⌘\\"),
                    some("Ctrl+\\"),
                    some("Ctrl+Shift+\\")
                ),
                (
                    "Activity popover",
                    some("⇧⌘U"),
                    some("Ctrl+Shift+U"),
                    some("Ctrl+Shift+U")
                ),
                (
                    "New ticket",
                    some("⌘N"),
                    some("Ctrl+N"),
                    some("Ctrl+Shift+N")
                ),
                (
                    "Terminals drawer (last-used Project terminal)",
                    some("⇧⌘T"),
                    some("Ctrl+Shift+T"),
                    some("Ctrl+Shift+T")
                ),
                (
                    "New Project agent (empty pane, starts on the first message)",
                    some("⇧⌘A"),
                    some("Ctrl+Shift+A"),
                    some("Ctrl+Shift+A")
                ),
                ("Settings", some("⌘,"), some("Ctrl+,"), some("Ctrl+Shift+,")),
            ]
        );
        assert!(NOTES[2].starts_with("Update V (0087)"));
    }

    /// Every action of the keymap has a row, except Find in terminal, which the notes name.
    #[test]
    fn every_action_is_shown() {
        let shown: Vec<KeyAction> = GROUPS
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .flat_map(|(_, keys)| match keys {
                Keys::One(action) => vec![*action],
                Keys::Pair(first, second, _) => vec![*first, *second],
                Keys::Range(KeyAction::Project(_), _) => (1..=9).map(KeyAction::Project).collect(),
                Keys::Range(KeyAction::SwitchTab(_), _) => {
                    (1..=6).map(KeyAction::SwitchTab).collect()
                }
                Keys::Range(other, _) => vec![*other],
            })
            .collect();
        for action in KeyAction::all() {
            assert_eq!(
                shown.contains(&action),
                action != KeyAction::FindInTerminal,
                "{action:?}"
            );
        }
        assert!(NOTES[0].contains("⌘F / Ctrl+Shift+F"));
        assert!(NOTES[3].starts_with("⌘K in the description: Link"));
        assert!(NOTES[4].starts_with("⌘⌥C in the description: Code block"));
        assert_eq!(groups().len(), 3);
    }
}
