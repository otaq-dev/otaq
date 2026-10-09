//! Keystrokes to the bytes a terminal sends (xterm conventions, as alacritty and Zed's terminal send them).
//!
//! ⌘ (the platform key) never produces bytes: on macOS app shortcuts keep working while a terminal has focus,
//! because the terminal doesn't use ⌘ (d0058). Everything else, Esc and Ctrl+C included, goes to the program.

use std::borrow::Cow;

use alacritty_terminal::term::TermMode;
use gpui::Keystroke;

/// The bytes for `keystroke`, or `None` when the terminal doesn't take it (⌘ shortcuts, bare modifiers).
pub fn to_bytes(keystroke: &Keystroke, mode: TermMode) -> Option<Cow<'static, [u8]>> {
    let m = &keystroke.modifiers;
    if m.platform || m.function {
        return None;
    }
    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let key = keystroke.key.as_str();
    let (ctrl, alt, shift) = (m.control, m.alt, m.shift);
    let plain = !ctrl && !alt && !shift;

    let fixed: Option<&'static [u8]> = match key {
        "enter" if plain => Some(b"\r"),
        "enter" if shift && !ctrl && !alt => Some(b"\n"),
        "enter" if alt && !ctrl && !shift => Some(b"\x1b\r"),
        "tab" if plain => Some(b"\t"),
        "tab" if shift && !ctrl && !alt => Some(b"\x1b[Z"),
        "escape" if plain => Some(b"\x1b"),
        "backspace" if plain || (shift && !ctrl && !alt) => Some(b"\x7f"),
        "backspace" if ctrl && !alt => Some(b"\x08"),
        "backspace" if alt && !ctrl => Some(b"\x1b\x7f"),
        "space" if ctrl && !alt => Some(b"\x00"),
        "insert" if plain => Some(b"\x1b[2~"),
        "delete" if plain => Some(b"\x1b[3~"),
        "pageup" if plain => Some(b"\x1b[5~"),
        "pagedown" if plain => Some(b"\x1b[6~"),
        "up" if plain => Some(if app_cursor { b"\x1bOA" } else { b"\x1b[A" }),
        "down" if plain => Some(if app_cursor { b"\x1bOB" } else { b"\x1b[B" }),
        "right" if plain => Some(if app_cursor { b"\x1bOC" } else { b"\x1b[C" }),
        "left" if plain => Some(if app_cursor { b"\x1bOD" } else { b"\x1b[D" }),
        "home" if plain => Some(if app_cursor { b"\x1bOH" } else { b"\x1b[H" }),
        "end" if plain => Some(if app_cursor { b"\x1bOF" } else { b"\x1b[F" }),
        "f1" if plain => Some(b"\x1bOP"),
        "f2" if plain => Some(b"\x1bOQ"),
        "f3" if plain => Some(b"\x1bOR"),
        "f4" if plain => Some(b"\x1bOS"),
        _ => None,
    };
    if let Some(bytes) = fixed {
        return Some(Cow::Borrowed(bytes));
    }

    // Arrows and Home/End with modifiers: CSI 1 ; <mod> X.
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let final_byte = match key {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(f) = final_byte {
        return Some(Cow::Owned(format!("\x1b[1;{modifier}{f}").into_bytes()));
    }

    // Ctrl+letter and friends: caret notation.
    if ctrl && !alt {
        let mut chars = key.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            let code = match c.to_ascii_lowercase() {
                c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
                '@' | '2' => Some(0),
                '[' | '3' => Some(0x1b),
                '\\' | '4' => Some(0x1c),
                ']' | '5' => Some(0x1d),
                '^' | '6' => Some(0x1e),
                '_' | '7' | '-' => Some(0x1f),
                '?' | '8' => Some(0x7f),
                _ => None,
            };
            if let Some(code) = code {
                return Some(Cow::Owned(vec![code]));
            }
        }
        return None;
    }

    // Text: what the key types (Option on macOS types its own characters).
    let text = keystroke.key_char.as_deref().or_else(|| {
        let mut chars = key.chars();
        match (chars.next(), chars.next()) {
            (Some(_), None) if !ctrl => Some(key),
            _ => None,
        }
    })?;
    if key == "space" && text.is_empty() {
        return Some(Cow::Borrowed(b" "));
    }
    Some(Cow::Owned(text.as_bytes().to_vec()))
}

/// Pasted text: inside bracketed-paste markers when the program asked for them, with line breaks as Enter.
pub fn paste(text: &str, mode: TermMode) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if mode.contains(TermMode::BRACKETED_PASTE) {
        // A paste must not end the bracket early.
        let text = text.replace("\x1b[201~", "");
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> Keystroke {
        Keystroke::parse(s).unwrap()
    }

    fn bytes(s: &str) -> Option<Vec<u8>> {
        to_bytes(&key(s), TermMode::empty()).map(|b| b.into_owned())
    }

    #[test]
    fn claude_code_keys_reach_the_program() {
        assert_eq!(bytes("escape"), Some(b"\x1b".to_vec()));
        assert_eq!(bytes("ctrl-c"), Some(vec![3]));
        assert_eq!(bytes("ctrl-j"), Some(vec![10]));
        assert_eq!(bytes("ctrl-t"), Some(vec![20]));
        assert_eq!(bytes("enter"), Some(b"\r".to_vec()));
        assert_eq!(bytes("shift-enter"), Some(b"\n".to_vec()));
        assert_eq!(bytes("alt-enter"), Some(b"\x1b\r".to_vec()));
        assert_eq!(bytes("tab"), Some(b"\t".to_vec()));
        assert_eq!(bytes("shift-tab"), Some(b"\x1b[Z".to_vec()));
        assert_eq!(bytes("backspace"), Some(vec![0x7f]));
        assert_eq!(bytes("up"), Some(b"\x1b[A".to_vec()));
        assert_eq!(bytes("ctrl-]"), Some(vec![0x1d]));
    }

    #[test]
    fn app_cursor_mode_changes_arrows() {
        let up = to_bytes(&key("up"), TermMode::APP_CURSOR).unwrap();
        assert_eq!(&*up, b"\x1bOA");
    }

    #[test]
    fn modified_arrows() {
        assert_eq!(bytes("shift-up"), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(bytes("alt-left"), Some(b"\x1b[1;3D".to_vec()));
        assert_eq!(bytes("ctrl-right"), Some(b"\x1b[1;5C".to_vec()));
    }

    #[test]
    fn command_shortcuts_are_left_to_the_app() {
        for s in [
            "cmd-k",
            "cmd-j",
            "cmd-shift-e",
            "cmd-c",
            "cmd-v",
            "cmd-[",
            "cmd-f",
        ] {
            assert_eq!(bytes(s), None, "{s}");
        }
    }

    #[test]
    fn typed_text() {
        let mut k = key("a");
        k.key_char = Some("a".into());
        assert_eq!(to_bytes(&k, TermMode::empty()).as_deref(), Some(&b"a"[..]));
        let mut k = key("shift-a");
        k.key_char = Some("A".into());
        assert_eq!(to_bytes(&k, TermMode::empty()).as_deref(), Some(&b"A"[..]));
        let mut k = key("alt-e");
        k.key_char = Some("é".into());
        assert_eq!(
            to_bytes(&k, TermMode::empty()).as_deref(),
            Some("é".as_bytes())
        );
    }

    #[test]
    fn pasting() {
        assert_eq!(paste("a\nb", TermMode::empty()), b"a\rb");
        assert_eq!(
            paste("a\r\nb", TermMode::BRACKETED_PASTE),
            b"\x1b[200~a\rb\x1b[201~"
        );
    }
}
