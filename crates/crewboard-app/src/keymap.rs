//! The keyboard map of decision 0058 (SDD §21.3): every app action with its shortcut on macOS, on Windows/Linux,
//! and on Windows/Linux while the terminal has focus (there app shortcuts move to Ctrl+Shift because Ctrl+letter
//! belongs to the CLI). Shortcut texts are exactly the decision's.

/// Which column of the keyboard map applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    WinLinux,
    /// Windows/Linux with the terminal focused.
    WinLinuxTerminal,
}

impl Platform {
    /// The platform this build runs on (outside the terminal).
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::WinLinux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    NextInboxItem,
    OpenInbox,
    PreviousTicket,
    NextTicket,
    LeaveTerminal,
    Search,
    SwitchProject,
    /// Project 1–9.
    Project(u8),
    NewTicket,
    ProjectTerminal,
    Settings,
    OpenPanel,
    ClosePanel,
    ExpandPanel,
    /// Switch tab 1–6.
    SwitchTab(u8),
    Approve,
    SendBack,
    StartDev,
    FindInTerminal,
}

impl KeyAction {
    /// Every action, with numbered ones expanded (Project 1–9, tabs 1–6).
    pub fn all() -> Vec<KeyAction> {
        let mut all = vec![
            Self::NextInboxItem,
            Self::OpenInbox,
            Self::PreviousTicket,
            Self::NextTicket,
            Self::LeaveTerminal,
            Self::Search,
            Self::SwitchProject,
            Self::NewTicket,
            Self::ProjectTerminal,
            Self::Settings,
            Self::OpenPanel,
            Self::ClosePanel,
            Self::ExpandPanel,
            Self::Approve,
            Self::SendBack,
            Self::StartDev,
            Self::FindInTerminal,
        ];
        all.extend((1..=9).map(Self::Project));
        all.extend((1..=6).map(Self::SwitchTab));
        all
    }

    /// The shortcut as shown in a Kbd, or `None` where the action has none.
    pub fn keys(self, platform: Platform) -> Option<String> {
        use Platform::*;
        let fixed = |mac: &str, win: &str, term: &str| -> Option<String> {
            let s = match platform {
                Mac => mac,
                WinLinux => win,
                WinLinuxTerminal => term,
            };
            (!s.is_empty()).then(|| s.to_owned())
        };
        match self {
            Self::NextInboxItem => fixed("⌘J", "Ctrl+J", "Ctrl+Shift+J"),
            Self::OpenInbox => fixed("⇧⌘I", "Ctrl+Shift+I", "Ctrl+Shift+I"),
            Self::PreviousTicket => fixed("⌘[", "Ctrl+[", "Ctrl+Shift+["),
            Self::NextTicket => fixed("⌘]", "Ctrl+]", "Ctrl+Shift+]"),
            Self::LeaveTerminal => fixed("⇧⌘`", "Ctrl+Shift+`", "Ctrl+Shift+`"),
            Self::Search => fixed("⌘K", "Ctrl+K", "Ctrl+Shift+K"),
            Self::SwitchProject => fixed("⌘P", "Ctrl+P", "Ctrl+Shift+P"),
            Self::Project(n) => {
                let n = n.clamp(1, 9);
                fixed(
                    &format!("⌘{n}"),
                    &format!("Ctrl+{n}"),
                    &format!("Ctrl+Shift+{n}"),
                )
            }
            Self::NewTicket => fixed("⌘N", "Ctrl+N", "Ctrl+Shift+N"),
            Self::ProjectTerminal => fixed("⇧⌘T", "Ctrl+Shift+T", "Ctrl+Shift+T"),
            Self::Settings => fixed("⌘,", "Ctrl+,", "Ctrl+Shift+,"),
            Self::OpenPanel => fixed("↵", "↵", ""),
            Self::ClosePanel => fixed("Esc", "Esc", ""),
            Self::ExpandPanel => fixed("⇧⌘E", "Ctrl+Shift+E", "Ctrl+Shift+E"),
            Self::SwitchTab(n) => {
                let n = n.clamp(1, 6);
                fixed(
                    &format!("⌘⌥{n}"),
                    &format!("Ctrl+Alt+{n}"),
                    &format!("Ctrl+Shift+Alt+{n}"),
                )
            }
            Self::Approve => fixed("⌘↵", "Ctrl+↵", "Ctrl+Shift+↵"),
            Self::SendBack => fixed("⌘⌫", "Ctrl+⌫", "Ctrl+Shift+⌫"),
            Self::StartDev => fixed("⇧⌘D", "Ctrl+Shift+D", "Ctrl+Shift+D"),
            Self::FindInTerminal => fixed("⌘F", "", "Ctrl+Shift+F"),
        }
    }
}

/// Where a keystroke goes (SES-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRoute {
    /// To the program in the focused terminal (Claude Code): Esc, Ctrl+C, Ctrl+J, arrows, Tab, letters…
    Terminal,
    /// To the app's shortcuts and views.
    App,
}

/// Routes one keystroke. While a terminal has focus **every key goes to Claude Code, Esc included**, except
/// the app's own shortcuts: on macOS everything with ⌘ (the terminal doesn't use ⌘, d0058); on Windows and Linux
/// Ctrl+Shift combinations (wired fully in T-0158). Without terminal focus everything is the app's.
pub fn route(platform: Platform, terminal_focused: bool, modifiers: &gpui::Modifiers) -> KeyRoute {
    if !terminal_focused {
        return KeyRoute::App;
    }
    let app = match platform {
        Platform::Mac => modifiers.platform,
        Platform::WinLinux | Platform::WinLinuxTerminal => {
            modifiers.control && modifiers.shift || modifiers.platform
        }
    };
    if app {
        KeyRoute::App
    } else {
        KeyRoute::Terminal
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn same_action_differs_by_platform() {
        assert_eq!(
            KeyAction::NextInboxItem.keys(Platform::Mac).as_deref(),
            Some("⌘J")
        );
        assert_eq!(
            KeyAction::NextInboxItem.keys(Platform::WinLinux).as_deref(),
            Some("Ctrl+J")
        );
        assert_eq!(
            KeyAction::NextInboxItem
                .keys(Platform::WinLinuxTerminal)
                .as_deref(),
            Some("Ctrl+Shift+J")
        );
        assert_eq!(
            KeyAction::Project(3).keys(Platform::Mac).as_deref(),
            Some("⌘3")
        );
        assert_eq!(KeyAction::OpenPanel.keys(Platform::WinLinuxTerminal), None);
    }

    /// d0058: no shortcut may stand for two different actions within a column.
    #[test]
    fn no_duplicates_in_a_column() {
        for columns in [
            vec![Platform::Mac],
            vec![Platform::WinLinux],
            vec![Platform::WinLinuxTerminal],
        ] {
            let mut seen: HashMap<String, KeyAction> = HashMap::new();
            for action in KeyAction::all() {
                for platform in &columns {
                    if let Some(keys) = action.keys(*platform)
                        && let Some(other) = seen.insert(keys.clone(), action)
                    {
                        panic!("{keys} used by {other:?} and {action:?} on {platform:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn decision_0058_collisions_stay_fixed() {
        // Project terminal and Expand panel moved off Ctrl+Shift+P / Ctrl+Shift+Enter.
        assert_eq!(
            KeyAction::ProjectTerminal
                .keys(Platform::WinLinux)
                .as_deref(),
            Some("Ctrl+Shift+T")
        );
        assert_eq!(
            KeyAction::ExpandPanel.keys(Platform::WinLinux).as_deref(),
            Some("Ctrl+Shift+E")
        );
        assert_ne!(
            KeyAction::SwitchProject.keys(Platform::WinLinuxTerminal),
            KeyAction::ProjectTerminal.keys(Platform::WinLinuxTerminal)
        );
    }

    fn keystroke(s: &str) -> gpui::Keystroke {
        gpui::Keystroke::parse(s).unwrap()
    }

    #[test]
    fn a_focused_terminal_takes_every_key_but_command_shortcuts() {
        for key in [
            "escape",
            "ctrl-c",
            "ctrl-j",
            "up",
            "down",
            "left",
            "right",
            "tab",
            "shift-tab",
            "enter",
            "a",
            "shift-a",
            "alt-enter",
            "ctrl-t",
            "backspace",
            "ctrl-[",
        ] {
            let k = keystroke(key);
            assert_eq!(
                route(Platform::Mac, true, &k.modifiers),
                KeyRoute::Terminal,
                "{key}"
            );
        }
        // d0058's ⌘ shortcuts keep working while focused.
        for key in [
            "cmd-j",
            "cmd-shift-i",
            "cmd-[",
            "cmd-]",
            "cmd-k",
            "cmd-p",
            "cmd-n",
            "cmd-shift-t",
            "cmd-,",
            "cmd-shift-e",
            "cmd-alt-1",
            "cmd-enter",
            "cmd-backspace",
            "cmd-shift-d",
            "cmd-f",
            "cmd-shift-`",
        ] {
            let k = keystroke(key);
            assert_eq!(
                route(Platform::Mac, true, &k.modifiers),
                KeyRoute::App,
                "{key}"
            );
        }
    }

    #[test]
    fn without_terminal_focus_everything_is_the_apps() {
        for key in ["escape", "enter", "a", "cmd-k"] {
            let k = keystroke(key);
            assert_eq!(route(Platform::Mac, false, &k.modifiers), KeyRoute::App);
        }
    }

    #[test]
    fn windows_and_linux_use_ctrl_shift_in_the_terminal() {
        let k = keystroke("ctrl-shift-k");
        assert_eq!(route(Platform::WinLinux, true, &k.modifiers), KeyRoute::App);
        let k = keystroke("ctrl-k");
        assert_eq!(
            route(Platform::WinLinux, true, &k.modifiers),
            KeyRoute::Terminal
        );
    }
}
