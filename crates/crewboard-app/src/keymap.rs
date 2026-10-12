//! The keyboard map's GPUI glue: d0058's shortcut texts as gpui keystrokes for the key bindings, and gpui's
//! modifiers routed by the table in [`crewboard_view::keymap`] (re-exported here).

pub use crewboard_view::keymap::*;

/// The shortcut as a gpui keystroke ("cmd-shift-i", "ctrl-shift-j"), for key bindings.
pub trait GpuiKeystroke {
    fn keystroke(self, platform: Platform) -> Option<String>;
}

impl GpuiKeystroke for KeyAction {
    fn keystroke(self, platform: Platform) -> Option<String> {
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

/// Routes one gpui keystroke with [`crewboard_view::keymap::route`].
pub fn route(platform: Platform, terminal_focused: bool, modifiers: &gpui::Modifiers) -> KeyRoute {
    crewboard_view::keymap::route(platform, terminal_focused, &plain(modifiers))
}

fn plain(m: &gpui::Modifiers) -> Modifiers {
    Modifiers {
        control: m.control,
        alt: m.alt,
        shift: m.shift,
        platform: m.platform,
        function: m.function,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

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

    /// gpui's modifiers reach the table unchanged: the Ctrl+Shift column is the app's in a focused terminal.
    #[test]
    fn gpui_modifiers_are_routed_by_the_table() {
        for action in KeyAction::all() {
            let Some(stroke) = action.keystroke(Platform::WinLinuxTerminal) else {
                continue;
            };
            let k = gpui::Keystroke::parse(&stroke).unwrap();
            assert_eq!(
                route(Platform::WinLinux, true, &k.modifiers),
                KeyRoute::App,
                "{stroke}"
            );
        }
        let k = gpui::Keystroke::parse("ctrl-j").unwrap();
        assert_eq!(
            route(Platform::WinLinux, true, &k.modifiers),
            KeyRoute::Terminal
        );
        let k = gpui::Keystroke::parse("cmd-k").unwrap();
        assert_eq!(route(Platform::Mac, true, &k.modifiers), KeyRoute::App);
    }
}
