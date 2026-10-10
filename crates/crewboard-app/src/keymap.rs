//! The keyboard map of decision 0058 (SDD §21.3): every app action with its shortcut on macOS, on Windows/Linux,
//! and on Windows/Linux while the terminal has focus (there app shortcuts move to Ctrl+Shift because Ctrl+letter
//! belongs to the CLI). Shortcut texts are exactly the decision's, and the app's key bindings are made from them
//! ([`KeyAction::keystroke`]), so the table is the one source for what is shown and what is bound.

/// Which column of the keyboard map applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    WinLinux,
    /// Windows/Linux with the terminal focused.
    WinLinuxTerminal,
}

/// The columns bound on a platform, each with whether it applies only while a terminal has focus. On macOS one
/// column serves everywhere (⌘ isn't the terminal's); on Windows and Linux the "in terminal" column is bound in
/// the terminal's key context on top of the Windows/Linux one.
pub fn bound_columns(platform: Platform) -> &'static [(Platform, bool)] {
    match platform {
        Platform::Mac => &[(Platform::Mac, false)],
        Platform::WinLinux | Platform::WinLinuxTerminal => &[
            (Platform::WinLinux, false),
            (Platform::WinLinuxTerminal, true),
        ],
    }
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

    /// The shortcut as a gpui keystroke ("cmd-shift-i", "ctrl-shift-j"), for key bindings.
    pub fn keystroke(self, platform: Platform) -> Option<String> {
        gpui_keystroke(&self.keys(platform)?)
    }
}

/// A d0058 shortcut text as a gpui keystroke: "⇧⌘I" → "cmd-shift-i", "⌘⌥1" → "cmd-alt-1",
/// "Ctrl+Shift+↵" → "ctrl-shift-enter", "Esc" → "escape". `None` for text that isn't one shortcut.
pub fn gpui_keystroke(text: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    let key = if text.contains('+') && text.len() > 1 {
        let mut pieces: Vec<&str> = text.split('+').collect();
        // "Ctrl+Shift++" would end in two empty pieces; no d0058 shortcut uses "+".
        let key = pieces.pop()?;
        for modifier in pieces {
            parts.push(match modifier {
                "Ctrl" => "ctrl",
                "Shift" => "shift",
                "Alt" => "alt",
                _ => return None,
            });
        }
        key
    } else {
        let mut rest = text;
        loop {
            let next = if let Some(r) = rest.strip_prefix('⌘') {
                ("cmd", r)
            } else if let Some(r) = rest.strip_prefix('⇧') {
                ("shift", r)
            } else if let Some(r) = rest.strip_prefix('⌥') {
                ("alt", r)
            } else if let Some(r) = rest.strip_prefix('⌃') {
                ("ctrl", r)
            } else {
                break;
            };
            parts.push(next.0);
            rest = next.1;
        }
        rest
    };
    let key = match key {
        "" => return None,
        "↵" => "enter".to_owned(),
        "⌫" => "backspace".to_owned(),
        "Esc" => "escape".to_owned(),
        k if k.chars().count() == 1 => k.to_lowercase(),
        _ => return None,
    };
    // gpui reads modifiers in any order; keep one order so equal shortcuts compare equal.
    let order = ["ctrl", "alt", "shift", "cmd"];
    parts.sort_by_key(|m| order.iter().position(|o| o == m));
    parts.dedup();
    Some(
        parts
            .into_iter()
            .map(str::to_owned)
            .chain([key])
            .collect::<Vec<_>>()
            .join("-"),
    )
}

/// Shortcuts the operating system or desktop keeps for itself on Windows and Linux (Windows, GNOME, KDE): no
/// app shortcut may use one (d0058). macOS keeps ⌘Q, ⌘H, ⌘M, ⌘Tab, ⌘Space and ⇧⌘3/4/5; ⇧⌘` (previous window)
/// is d0058's Leave terminal and stays, see `tickets/questions.md` (T-0058 note).
#[cfg(test)]
const SYSTEM_SHORTCUTS: &[&str] = &[
    // Windows
    "alt-f4",
    "alt-tab",
    "alt-space",
    "alt-escape",
    "ctrl-alt-delete",
    "ctrl-shift-escape",
    "ctrl-escape",
    // Linux desktops (GNOME, KDE, X11)
    "ctrl-alt-t",
    "ctrl-alt-l",
    "ctrl-alt-d",
    "ctrl-alt-tab",
    "ctrl-alt-backspace",
    "ctrl-alt-left",
    "ctrl-alt-right",
    "ctrl-alt-up",
    "ctrl-alt-down",
    "alt-f2",
    "ctrl-alt-shift-r",
    "ctrl-alt-f1",
    "ctrl-alt-f2",
    "ctrl-alt-f3",
    "ctrl-alt-f4",
    "ctrl-alt-f5",
    "ctrl-alt-f6",
    "ctrl-alt-f7",
    // macOS
    "cmd-q",
    "cmd-h",
    "cmd-m",
    "cmd-tab",
    "cmd-space",
    "shift-cmd-3",
    "shift-cmd-4",
    "shift-cmd-5",
];

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
/// Ctrl+Shift combinations (the "in terminal" column). Without terminal focus everything is the app's.
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

    /// d0058 across the columns: on Windows and Linux both columns are bound while the terminal has focus, so a
    /// shortcut must mean the same action wherever it appears. Every shortcut is also a valid gpui keystroke,
    /// and none is a system shortcut.
    #[test]
    fn no_shortcut_means_two_things_across_the_columns() {
        let mut seen: HashMap<String, KeyAction> = HashMap::new();
        for platform in [
            Platform::Mac,
            Platform::WinLinux,
            Platform::WinLinuxTerminal,
        ] {
            for action in KeyAction::all() {
                let Some(keys) = action.keys(platform) else {
                    continue;
                };
                let stroke = action
                    .keystroke(platform)
                    .unwrap_or_else(|| panic!("{keys} isn't a keystroke"));
                assert!(gpui::Keystroke::parse(&stroke).is_ok(), "{stroke}");
                assert!(
                    !SYSTEM_SHORTCUTS.contains(&stroke.as_str()),
                    "{keys} ({action:?}) is a system shortcut"
                );
                if let Some(other) = seen.insert(stroke.clone(), action) {
                    assert_eq!(other, action, "{keys} means {other:?} and {action:?}");
                }
            }
        }
    }

    #[test]
    fn shortcut_texts_become_gpui_keystrokes() {
        for (text, stroke) in [
            ("⌘J", "cmd-j"),
            ("⇧⌘I", "shift-cmd-i"),
            ("⌘⌥3", "alt-cmd-3"),
            ("⌘↵", "cmd-enter"),
            ("⌘⌫", "cmd-backspace"),
            ("⇧⌘`", "shift-cmd-`"),
            ("Ctrl+J", "ctrl-j"),
            ("Ctrl+Shift+[", "ctrl-shift-["),
            ("Ctrl+Shift+Alt+4", "ctrl-alt-shift-4"),
            ("Ctrl+↵", "ctrl-enter"),
            ("Ctrl+Shift+,", "ctrl-shift-,"),
            ("Esc", "escape"),
            ("↵", "enter"),
        ] {
            assert_eq!(gpui_keystroke(text).as_deref(), Some(stroke), "{text}");
        }
        assert_eq!(gpui_keystroke(""), None);
        assert_eq!(gpui_keystroke("Meta+J"), None);
    }

    /// With the terminal focused on Windows/Linux, the CLI's Ctrl keys reach the PTY and the Ctrl+Shift column
    /// reaches the app (SES-10).
    #[test]
    fn the_terminal_keeps_ctrl_keys_on_windows_and_linux() {
        for key in [
            "ctrl-j",
            "ctrl-c",
            "ctrl-t",
            "ctrl-k",
            "ctrl-p",
            "ctrl-[",
            "escape",
            "alt-enter",
        ] {
            let k = keystroke(key);
            assert_eq!(
                route(Platform::WinLinux, true, &k.modifiers),
                KeyRoute::Terminal,
                "{key}"
            );
        }
        for action in KeyAction::all() {
            let Some(stroke) = action.keystroke(Platform::WinLinuxTerminal) else {
                continue;
            };
            let k = keystroke(&stroke);
            assert_eq!(
                route(Platform::WinLinux, true, &k.modifiers),
                KeyRoute::App,
                "{stroke}"
            );
        }
        assert_eq!(
            bound_columns(Platform::WinLinux),
            [
                (Platform::WinLinux, false),
                (Platform::WinLinuxTerminal, true)
            ]
        );
        assert_eq!(bound_columns(Platform::Mac), [(Platform::Mac, false)]);
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
