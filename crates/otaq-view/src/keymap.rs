//! The keyboard map of decision 0058 (SDD §21.3): every app action with its shortcut on macOS, on Windows/Linux,
//! and on Windows/Linux while the terminal has focus (there app shortcuts move to Ctrl+Shift because Ctrl+letter
//! belongs to the CLI). Shortcut texts are exactly the decision's, and each app makes its key bindings from them
//! ([`KeyAction::keys`]), so the table is the one source for what is shown and what is bound.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::view_models::layout::FILES_VIEW;

/// Which column of the keyboard map applies. `KeyPlatform` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "KeyPlatform")]
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

/// An app action of the map. On the page it is `{ action: "open_inbox" }`, or with its number
/// `{ action: "project", number: 3 }`; screens register their handlers by `action` (T-0212).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "action", content = "number", rename_all = "snake_case")]
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
    /// The Board view of the main area (d0087).
    ShowBoard,
    /// The Files view of the main area (d0087).
    ShowFiles,
    /// Go to file in the Files view (d0087).
    GoToFile,
    /// Focus the Project agent pane (d0087).
    FocusPane,
    /// Hide the Project agent pane to its strip, or show it again (d0087).
    TogglePane,
    /// The Activity popover (d0087).
    OpenActivity,
    /// The Terminals drawer on the last-used Project terminal (d0068, d0087).
    ProjectTerminal,
    /// The Project agent pane's empty state: nothing starts until the first message is sent (d0068, d0087, PAG-01).
    NewProjectAgent,
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
    /// Focus the epic of the selected card (EPC-18, d0080): the same as its `epic:` filter.
    FocusEpic,
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
            Self::ShowBoard,
            Self::ShowFiles,
            Self::GoToFile,
            Self::FocusPane,
            Self::TogglePane,
            Self::OpenActivity,
            Self::NewTicket,
            Self::ProjectTerminal,
            Self::NewProjectAgent,
            Self::Settings,
            Self::OpenPanel,
            Self::ClosePanel,
            Self::ExpandPanel,
            Self::Approve,
            Self::SendBack,
            Self::StartDev,
            Self::FindInTerminal,
            Self::FocusEpic,
        ];
        all.extend((1..=9).map(Self::Project));
        all.extend((1..=6).map(Self::SwitchTab));
        all
    }

    /// The Files view's own actions (⇧⌘O, ⌥⌘P): unbound and unlisted while it's parked ([`FILES_VIEW`]).
    pub fn is_files_view(self) -> bool {
        matches!(self, Self::ShowFiles | Self::GoToFile)
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
            Self::ShowBoard => fixed("⇧⌘B", "Ctrl+Shift+B", "Ctrl+Shift+B"),
            Self::ShowFiles => fixed("⇧⌘O", "Ctrl+Shift+O", "Ctrl+Shift+O"),
            Self::GoToFile => fixed("⌥⌘P", "Ctrl+Alt+P", "Ctrl+Shift+Alt+P"),
            Self::FocusPane => fixed("⌘L", "Ctrl+L", "Ctrl+Shift+L"),
            Self::TogglePane => fixed("⌘\\", "Ctrl+\\", "Ctrl+Shift+\\"),
            Self::OpenActivity => fixed("⇧⌘U", "Ctrl+Shift+U", "Ctrl+Shift+U"),
            Self::NewTicket => fixed("⌘N", "Ctrl+N", "Ctrl+Shift+N"),
            Self::ProjectTerminal => fixed("⇧⌘T", "Ctrl+Shift+T", "Ctrl+Shift+T"),
            Self::NewProjectAgent => fixed("⇧⌘A", "Ctrl+Shift+A", "Ctrl+Shift+A"),
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
            Self::FocusEpic => fixed("⇧⌘G", "Ctrl+Shift+G", "Ctrl+Shift+G"),
        }
    }
}

/// The modifier keys held with a keystroke, as the UI toolkit reports them. `KeyModifiers` on the page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(rename = "KeyModifiers")]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    /// ⌘ on macOS, the Windows key on Windows, Super on Linux.
    pub platform: bool,
    pub function: bool,
}

/// Where a keystroke goes (SES-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRoute {
    /// To the program in the focused terminal (Claude Code): Esc, Ctrl+C, Ctrl+J, arrows, Tab, letters…
    Terminal,
    /// To the app's shortcuts and views.
    App,
}

impl Modifiers {
    const NONE: Self = Self {
        control: false,
        alt: false,
        shift: false,
        platform: false,
        function: false,
    };

    /// Whether every modifier of `other` is held here (others may be held too).
    pub fn holds(&self, other: &Modifiers) -> bool {
        (!other.control || self.control)
            && (!other.alt || self.alt)
            && (!other.shift || self.shift)
            && (!other.platform || self.platform)
            && (!other.function || self.function)
    }
}

/// The modifier sets that take a key to the app while a terminal has focus: a key goes to the app when it holds
/// every modifier of one of them. On macOS ⌘ (the terminal doesn't use ⌘, d0058); on Windows and Linux Ctrl+Shift
/// (the "in terminal" column), or the Windows / Super key.
pub fn terminal_app_modifiers(platform: Platform) -> &'static [Modifiers] {
    const PLATFORM: Modifiers = Modifiers {
        platform: true,
        ..Modifiers::NONE
    };
    const CONTROL_SHIFT: Modifiers = Modifiers {
        control: true,
        shift: true,
        ..Modifiers::NONE
    };
    match platform {
        Platform::Mac => &[PLATFORM],
        Platform::WinLinux | Platform::WinLinuxTerminal => &[CONTROL_SHIFT, PLATFORM],
    }
}

/// Routes one keystroke. While a terminal has focus **every key goes to Claude Code, Esc included**, except
/// the app's own shortcuts ([`terminal_app_modifiers`]). Without terminal focus everything is the app's.
pub fn route(platform: Platform, terminal_focused: bool, modifiers: &Modifiers) -> KeyRoute {
    if !terminal_focused {
        return KeyRoute::App;
    }
    if terminal_app_modifiers(platform)
        .iter()
        .any(|m| modifiers.holds(m))
    {
        KeyRoute::App
    } else {
        KeyRoute::Terminal
    }
}

/// One shortcut as the page matches it against a key event (T-0215). `Keystroke` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct Keystroke {
    /// The key as the web's `KeyboardEvent.key` names it: a lowercase letter, a digit or a sign (`k`, `1`, `[`), or a
    /// named key (`Enter`, `Backspace`, `Escape`).
    pub key: String,
    /// The physical key as `KeyboardEvent.code` names it (`KeyK`, `Digit1`, `BracketLeft`, `Enter`), for keys whose
    /// character depends on the keyboard layout or on the modifiers held (⌥1 types `¡` on a Mac).
    pub code: String,
    pub modifiers: Modifiers,
}

impl Keystroke {
    /// A d0058 shortcut text as a keystroke: "⇧⌘I", "⌘⌥1", "Ctrl+Shift+↵", "Esc". `None` for text that isn't one
    /// shortcut.
    pub fn parse(text: &str) -> Option<Self> {
        let mut modifiers = Modifiers::default();
        let key = if text.contains('+') && text.len() > 1 {
            let mut pieces: Vec<&str> = text.split('+').collect();
            let key = pieces.pop()?;
            for modifier in pieces {
                match modifier {
                    "Ctrl" => modifiers.control = true,
                    "Shift" => modifiers.shift = true,
                    "Alt" => modifiers.alt = true,
                    _ => return None,
                }
            }
            key
        } else {
            let mut rest = text;
            loop {
                let mut chars = rest.chars();
                match chars.next() {
                    Some('⌘') => modifiers.platform = true,
                    Some('⇧') => modifiers.shift = true,
                    Some('⌥') => modifiers.alt = true,
                    Some('⌃') => modifiers.control = true,
                    _ => break,
                }
                rest = chars.as_str();
            }
            rest
        };
        let (key, code) = match key {
            "↵" => ("Enter".to_owned(), "Enter".to_owned()),
            "⌫" => ("Backspace".to_owned(), "Backspace".to_owned()),
            "Esc" => ("Escape".to_owned(), "Escape".to_owned()),
            k => {
                let mut chars = k.chars();
                let (Some(c), None) = (chars.next(), chars.next()) else {
                    return None;
                };
                let code = match c {
                    'a'..='z' | 'A'..='Z' => format!("Key{}", c.to_ascii_uppercase()),
                    '0'..='9' => format!("Digit{c}"),
                    '[' => "BracketLeft".to_owned(),
                    ']' => "BracketRight".to_owned(),
                    '`' => "Backquote".to_owned(),
                    '\\' => "Backslash".to_owned(),
                    ',' => "Comma".to_owned(),
                    '=' => "Equal".to_owned(),
                    '-' => "Minus".to_owned(),
                    _ => return None,
                };
                (c.to_ascii_lowercase().to_string(), code)
            }
        };
        Some(Self {
            key,
            code,
            modifiers,
        })
    }
}

/// One action's shortcut as the page binds it. `KeyBinding` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "KeyBinding")]
pub struct Binding {
    pub action: KeyAction,
    /// The shortcut as shown in a Kbd (`⌘K`, `Ctrl+Shift+K`).
    pub text: String,
    pub keystroke: Keystroke,
    /// Bound only while a terminal has focus (the Windows/Linux "in terminal" column).
    pub in_terminal: bool,
}

/// Every shortcut bound on `platform`, from [`KeyAction::keys`] over [`bound_columns`]: the page's whole map.
pub fn bindings(platform: Platform) -> Vec<Binding> {
    bindings_for(platform, FILES_VIEW)
}

/// [`bindings`] with the Files view on or off: off, its actions aren't bound.
pub fn bindings_for(platform: Platform, files_view: bool) -> Vec<Binding> {
    let mut all = Vec::new();
    for &(column, in_terminal) in bound_columns(platform) {
        for action in KeyAction::all() {
            if !files_view && action.is_files_view() {
                continue;
            }
            let Some(text) = action.keys(column) else {
                continue;
            };
            // Every d0058 text parses (`every_shortcut_is_one_keystroke`).
            let Some(keystroke) = Keystroke::parse(&text) else {
                continue;
            };
            all.push(Binding {
                action,
                text,
                keystroke,
                in_terminal,
            });
        }
    }
    all
}

/// A focused terminal's own keys, outside the d0058 table (as the GPUI terminal bound them): copy and paste, and ⌘+ /
/// ⌘− for the font size. On Windows and Linux Ctrl+letter belongs to the program in the terminal, so they are on
/// Ctrl+Shift there, as in Windows Terminal and Linux terminals. ⇧↵ is a new line in the prompt: xterm.js sends the
/// same `\r` as ↵, so the terminal sends ESC + CR instead (OT-5). `TerminalKey` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKey {
    Copy,
    Paste,
    FontBigger,
    FontSmaller,
    /// A new line in the program's prompt instead of sending it: ESC + CR (⌥↵, what Claude Code's `/terminal-setup`
    /// makes ⇧↵ send). Claude Code, Codex, Copilot CLI and OpenCode all take it as a new line; zsh adds a line to
    /// the command and bash ignores it, so a Project terminal gets it too.
    NewLine,
}

impl TerminalKey {
    pub const ALL: [Self; 5] = [
        Self::Copy,
        Self::Paste,
        Self::FontBigger,
        Self::FontSmaller,
        Self::NewLine,
    ];

    /// Its shortcuts on `platform` (Windows/Linux in or out of the terminal alike). On macOS copy and paste have
    /// none here: the Edit menu's ⌘C / ⌘V reach the terminal's own copy and paste.
    pub fn keys(self, platform: Platform) -> &'static [&'static str] {
        match (self, platform) {
            (Self::Copy | Self::Paste, Platform::Mac) => &[],
            (Self::Copy, _) => &["Ctrl+Shift+C"],
            (Self::Paste, _) => &["Ctrl+Shift+V"],
            (Self::FontBigger, Platform::Mac) => &["⌘=", "⇧⌘="],
            (Self::FontBigger, _) => &["Ctrl+Shift+="],
            (Self::FontSmaller, Platform::Mac) => &["⌘-"],
            (Self::FontSmaller, _) => &["Ctrl+Shift+-"],
            (Self::NewLine, Platform::Mac) => &["⇧↵"],
            (Self::NewLine, _) => &["Shift+↵"],
        }
    }
}

/// One of a terminal's own shortcuts as the page matches it. `TerminalKeyBinding` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "TerminalKeyBinding")]
pub struct TerminalBinding {
    pub key: TerminalKey,
    pub keystroke: Keystroke,
}

/// Every terminal shortcut on `platform`.
pub fn terminal_bindings(platform: Platform) -> Vec<TerminalBinding> {
    TerminalKey::ALL
        .into_iter()
        .flat_map(|key| {
            key.keys(platform).iter().filter_map(move |text| {
                Some(TerminalBinding {
                    key,
                    keystroke: Keystroke::parse(text)?,
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// Decision 0058's table, one row per action (Previous / next ticket and Open / close panel split, Project 1–9
    /// and Switch tab 1–6 expanded, Find in terminal from its note): action, macOS, Windows/Linux, in terminal; ""
    /// where the decision has no shortcut ("—").
    fn table_0058() -> Vec<(KeyAction, String, String, String)> {
        let row = |a, mac: &str, win: &str, term: &str| {
            (a, mac.to_owned(), win.to_owned(), term.to_owned())
        };
        let mut table = vec![
            row(KeyAction::NextInboxItem, "⌘J", "Ctrl+J", "Ctrl+Shift+J"),
            row(KeyAction::OpenInbox, "⇧⌘I", "Ctrl+Shift+I", "Ctrl+Shift+I"),
            row(KeyAction::PreviousTicket, "⌘[", "Ctrl+[", "Ctrl+Shift+["),
            row(KeyAction::NextTicket, "⌘]", "Ctrl+]", "Ctrl+Shift+]"),
            row(
                KeyAction::LeaveTerminal,
                "⇧⌘`",
                "Ctrl+Shift+`",
                "Ctrl+Shift+`",
            ),
            row(KeyAction::Search, "⌘K", "Ctrl+K", "Ctrl+Shift+K"),
            row(KeyAction::SwitchProject, "⌘P", "Ctrl+P", "Ctrl+Shift+P"),
            row(KeyAction::NewTicket, "⌘N", "Ctrl+N", "Ctrl+Shift+N"),
            row(
                KeyAction::ProjectTerminal,
                "⇧⌘T",
                "Ctrl+Shift+T",
                "Ctrl+Shift+T",
            ),
            row(
                KeyAction::NewProjectAgent,
                "⇧⌘A",
                "Ctrl+Shift+A",
                "Ctrl+Shift+A",
            ),
            row(KeyAction::ShowBoard, "⇧⌘B", "Ctrl+Shift+B", "Ctrl+Shift+B"),
            row(KeyAction::ShowFiles, "⇧⌘O", "Ctrl+Shift+O", "Ctrl+Shift+O"),
            row(KeyAction::GoToFile, "⌥⌘P", "Ctrl+Alt+P", "Ctrl+Shift+Alt+P"),
            row(KeyAction::FocusPane, "⌘L", "Ctrl+L", "Ctrl+Shift+L"),
            row(KeyAction::TogglePane, "⌘\\", "Ctrl+\\", "Ctrl+Shift+\\"),
            row(
                KeyAction::OpenActivity,
                "⇧⌘U",
                "Ctrl+Shift+U",
                "Ctrl+Shift+U",
            ),
            row(KeyAction::Settings, "⌘,", "Ctrl+,", "Ctrl+Shift+,"),
            row(KeyAction::OpenPanel, "↵", "↵", ""),
            row(KeyAction::ClosePanel, "Esc", "Esc", ""),
            row(
                KeyAction::ExpandPanel,
                "⇧⌘E",
                "Ctrl+Shift+E",
                "Ctrl+Shift+E",
            ),
            row(KeyAction::Approve, "⌘↵", "Ctrl+↵", "Ctrl+Shift+↵"),
            row(KeyAction::SendBack, "⌘⌫", "Ctrl+⌫", "Ctrl+Shift+⌫"),
            row(KeyAction::StartDev, "⇧⌘D", "Ctrl+Shift+D", "Ctrl+Shift+D"),
            row(KeyAction::FocusEpic, "⇧⌘G", "Ctrl+Shift+G", "Ctrl+Shift+G"),
            row(KeyAction::FindInTerminal, "⌘F", "", "Ctrl+Shift+F"),
        ];
        for n in 1..=9 {
            table.push(row(
                KeyAction::Project(n),
                &format!("⌘{n}"),
                &format!("Ctrl+{n}"),
                &format!("Ctrl+Shift+{n}"),
            ));
        }
        for n in 1..=6 {
            table.push(row(
                KeyAction::SwitchTab(n),
                &format!("⌘⌥{n}"),
                &format!("Ctrl+Alt+{n}"),
                &format!("Ctrl+Shift+Alt+{n}"),
            ));
        }
        table
    }

    const COLUMNS: [Platform; 3] = [
        Platform::Mac,
        Platform::WinLinux,
        Platform::WinLinuxTerminal,
    ];

    fn text(row: &(KeyAction, String, String, String), column: Platform) -> &str {
        match column {
            Platform::Mac => &row.1,
            Platform::WinLinux => &row.2,
            Platform::WinLinuxTerminal => &row.3,
        }
    }

    /// The action a keystroke runs, as the page's dispatcher finds it (`ui/src/app/keys.ts`): a focused terminal
    /// keeps what [`route`] gives it; otherwise the first binding of the keystroke, the "in terminal" column only
    /// counting in a terminal.
    fn action_for(platform: Platform, in_terminal: bool, stroke: &Keystroke) -> Option<KeyAction> {
        if in_terminal && route(platform, true, &stroke.modifiers) == KeyRoute::Terminal {
            return None;
        }
        bindings_for(platform, true)
            .into_iter()
            .find(|b| (in_terminal || !b.in_terminal) && &b.keystroke == stroke)
            .map(|b| b.action)
    }

    /// Every action has d0058's three texts, and `all()` lists exactly the table's actions.
    #[test]
    fn every_action_has_its_0058_texts() {
        let table = table_0058();
        for row in &table {
            for column in COLUMNS {
                let want = text(row, column);
                assert_eq!(
                    row.0.keys(column).as_deref(),
                    (!want.is_empty()).then_some(want),
                    "{:?} on {column:?}",
                    row.0
                );
            }
        }
        let mut listed = KeyAction::all();
        let mut tabled: Vec<KeyAction> = table.iter().map(|r| r.0).collect();
        let order = |a: &KeyAction| format!("{a:?}");
        listed.sort_by_key(order);
        tabled.sort_by_key(order);
        assert_eq!(listed, tabled);
    }

    /// Each text of the table, pressed where its column applies, runs its action: macOS on the board and in a
    /// terminal; Windows/Linux on the board; the "in terminal" column in a Windows/Linux terminal, where the plain
    /// Windows/Linux keys that lack Shift stay the CLI's.
    #[test]
    fn each_0058_keystroke_runs_its_action() {
        for row in table_0058() {
            let (action, mac, win, term) = (row.0, &row.1, &row.2, &row.3);
            if !mac.is_empty() {
                let stroke = Keystroke::parse(mac).unwrap();
                assert_eq!(
                    action_for(Platform::Mac, false, &stroke),
                    Some(action),
                    "{mac}"
                );
                if stroke.modifiers.platform {
                    assert_eq!(
                        action_for(Platform::Mac, true, &stroke),
                        Some(action),
                        "{mac}"
                    );
                }
            }
            if !win.is_empty() {
                let stroke = Keystroke::parse(win).unwrap();
                assert_eq!(
                    action_for(Platform::WinLinux, false, &stroke),
                    Some(action),
                    "{win}"
                );
                if !(stroke.modifiers.control && stroke.modifiers.shift) {
                    assert_eq!(action_for(Platform::WinLinux, true, &stroke), None, "{win}");
                }
            }
            if !term.is_empty() {
                let stroke = Keystroke::parse(term).unwrap();
                assert_eq!(
                    action_for(Platform::WinLinux, true, &stroke),
                    Some(action),
                    "{term}"
                );
            }
        }
    }

    /// The 0087 rows parse into the web's key names, ⌘\ by its physical key.
    #[test]
    fn the_0087_keys_parse() {
        let parse = |text: &str| Keystroke::parse(text).unwrap();
        let hide = parse("⌘\\");
        assert_eq!((hide.key.as_str(), hide.code.as_str()), ("\\", "Backslash"));
        assert_eq!(parse("Ctrl+Shift+\\").code, "Backslash");
        let go = parse("Ctrl+Shift+Alt+P");
        assert_eq!(go.code, "KeyP");
        assert!(go.modifiers.control && go.modifiers.shift && go.modifiers.alt);
        let go_mac = parse("⌥⌘P");
        assert!(go_mac.modifiers.alt && go_mac.modifiers.platform && !go_mac.modifiers.shift);
        assert_eq!(parse("⌘L").code, "KeyL");
    }

    /// d0058: no shortcut stands for two actions anywhere in the table, across its three columns.
    #[test]
    fn no_shortcut_appears_twice_across_the_table() {
        let mut seen: HashMap<String, KeyAction> = HashMap::new();
        for row in table_0058() {
            for column in COLUMNS {
                let t = text(&row, column);
                if t.is_empty() {
                    continue;
                }
                if let Some(other) = seen.insert(t.to_owned(), row.0) {
                    assert_eq!(other, row.0, "{t} used by {other:?} and {:?}", row.0);
                }
            }
        }
    }

    /// ⌘1–9 stay Project 1–9 and ⌘P Switch project (d0087: design V's ⌘1 / ⌘2 / ⌘P aren't used).
    #[test]
    fn project_keys_keep_their_meaning() {
        assert_eq!(
            KeyAction::Project(1).keys(Platform::Mac).as_deref(),
            Some("⌘1")
        );
        assert_eq!(
            KeyAction::Project(2).keys(Platform::Mac).as_deref(),
            Some("⌘2")
        );
        assert_eq!(
            KeyAction::SwitchProject.keys(Platform::Mac).as_deref(),
            Some("⌘P")
        );
    }

    #[test]
    fn a_terminals_own_keys_parse_and_stay_off_the_app_map() {
        for platform in [Platform::Mac, Platform::WinLinux] {
            let own = terminal_bindings(platform);
            let texts: usize = TerminalKey::ALL
                .iter()
                .map(|k| k.keys(platform).len())
                .sum();
            assert_eq!(own.len(), texts, "every text parses on {platform:?}");
            for b in &own {
                // Taken from the program in the terminal, as the app's own keys are (SES-10); ⇧↵ stays the
                // program's, only its bytes change.
                let to = if b.key == TerminalKey::NewLine {
                    KeyRoute::Terminal
                } else {
                    KeyRoute::App
                };
                assert_eq!(route(platform, true, &b.keystroke.modifiers), to);
                assert!(
                    !bindings_for(platform, true)
                        .iter()
                        .any(|a| a.keystroke == b.keystroke),
                    "{:?} clashes with the map on {platform:?}",
                    b.key
                );
            }
        }
        let mac = terminal_bindings(Platform::Mac);
        assert!(
            mac.iter()
                .all(|b| !matches!(b.key, TerminalKey::Copy | TerminalKey::Paste))
        );
        assert_eq!(mac[0].keystroke.code, "Equal");
        let win = terminal_bindings(Platform::WinLinux);
        assert_eq!(win[0].key, TerminalKey::Copy);
        assert_eq!(win[0].keystroke.code, "KeyC");
        assert!(win[0].keystroke.modifiers.control && win[0].keystroke.modifiers.shift);
        for own in [&mac, &win] {
            let new_line = own.iter().find(|b| b.key == TerminalKey::NewLine).unwrap();
            assert_eq!(new_line.keystroke.code, "Enter");
            assert_eq!(
                new_line.keystroke.modifiers,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                }
            );
        }
    }

    #[test]
    fn the_page_reads_an_action_by_its_name_and_number() {
        let json = |a: KeyAction| serde_json::to_value(a).unwrap();
        assert_eq!(
            json(KeyAction::OpenInbox),
            serde_json::json!({ "action": "open_inbox" })
        );
        assert_eq!(
            json(KeyAction::SwitchTab(2)),
            serde_json::json!({ "action": "switch_tab", "number": 2 })
        );
        for action in KeyAction::all() {
            assert_eq!(
                serde_json::from_value::<KeyAction>(json(action)).unwrap(),
                action
            );
        }
    }

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
            let Some(stroke) = action.keys(Platform::WinLinuxTerminal) else {
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

    /// d0080: ⇧⌘G / Ctrl+Shift+G focuses the epic on all three rows, the terminal's included, and is no other
    /// action's.
    #[test]
    fn focus_epic_is_on_every_row() {
        assert_eq!(
            [
                Platform::Mac,
                Platform::WinLinux,
                Platform::WinLinuxTerminal
            ]
            .map(|p| KeyAction::FocusEpic.keys(p)),
            [
                Some("⇧⌘G".to_owned()),
                Some("Ctrl+Shift+G".to_owned()),
                Some("Ctrl+Shift+G".to_owned())
            ]
        );
        for platform in [Platform::Mac, Platform::WinLinux] {
            let focus: Vec<_> = bindings_for(platform, true)
                .into_iter()
                .filter(|b| b.keystroke.key == "g" && b.keystroke.modifiers.shift)
                .collect();
            assert!(focus.iter().all(|b| b.action == KeyAction::FocusEpic));
            assert_eq!(focus.len(), if platform == Platform::Mac { 1 } else { 2 });
        }
        let in_terminal = Keystroke::parse("Ctrl+Shift+G").unwrap();
        assert_eq!(
            route(Platform::WinLinux, true, &in_terminal.modifiers),
            KeyRoute::App
        );
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

    struct Stroke {
        modifiers: Modifiers,
    }

    /// A keystroke's modifiers, from gpui-style text ("ctrl-shift-k") or a d0058 text ("Ctrl+Shift+K").
    fn keystroke(s: &str) -> Stroke {
        let parts: Vec<&str> = if s.contains('+') && s.len() > 1 {
            s.split('+').collect()
        } else {
            s.split('-').collect()
        };
        let mut modifiers = Modifiers::default();
        for part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" => modifiers.control = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "cmd" => modifiers.platform = true,
                other => panic!("{other} in {s}"),
            }
        }
        Stroke { modifiers }
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

    #[test]
    fn every_shortcut_is_one_keystroke() {
        for platform in [
            Platform::Mac,
            Platform::WinLinux,
            Platform::WinLinuxTerminal,
        ] {
            for action in KeyAction::all() {
                if let Some(text) = action.keys(platform) {
                    assert!(Keystroke::parse(&text).is_some(), "{text} ({action:?})");
                }
            }
        }
    }

    #[test]
    fn shortcut_texts_parse_into_the_web_key_names() {
        let parse = |text: &str| Keystroke::parse(text).unwrap();
        let mods = |control, alt, shift, platform| Modifiers {
            control,
            alt,
            shift,
            platform,
            function: false,
        };
        let open_inbox = parse("⇧⌘I");
        assert_eq!(
            (open_inbox.key.as_str(), open_inbox.code.as_str()),
            ("i", "KeyI")
        );
        assert_eq!(open_inbox.modifiers, mods(false, false, true, true));
        let tab = parse("⌘⌥1");
        assert_eq!((tab.key.as_str(), tab.code.as_str()), ("1", "Digit1"));
        assert_eq!(tab.modifiers, mods(false, true, false, true));
        let approve = parse("Ctrl+Shift+↵");
        assert_eq!(
            (approve.key.as_str(), approve.code.as_str()),
            ("Enter", "Enter")
        );
        assert_eq!(approve.modifiers, mods(true, false, true, false));
        assert_eq!(parse("Ctrl+⌫").key, "Backspace");
        assert_eq!(parse("Esc").modifiers, Modifiers::default());
        assert_eq!(parse("Esc").code, "Escape");
        assert_eq!(parse("Ctrl+Shift+`").code, "Backquote");
        assert_eq!(parse("⌘,").code, "Comma");
        assert_eq!(parse("Ctrl+Shift+[").code, "BracketLeft");
        assert_eq!(parse("⌘]").code, "BracketRight");
        assert_eq!(parse("↵").key, "Enter");
        for wrong in ["", "Ctrl+", "Meta+K", "⌘", "⌘KK", "Ctrl+Shift+F5"] {
            assert_eq!(Keystroke::parse(wrong), None, "{wrong}");
        }
    }

    /// The page's map (T-0215): each action once per column, and the "in terminal" column only on Windows/Linux.
    #[test]
    fn the_bindings_are_the_table() {
        let mac = bindings_for(Platform::Mac, true);
        let with_keys = |p| {
            KeyAction::all()
                .into_iter()
                .filter(|a| a.keys(p).is_some())
                .count()
        };
        assert_eq!(mac.len(), with_keys(Platform::Mac));
        assert!(mac.iter().all(|b| !b.in_terminal));
        let search = mac.iter().find(|b| b.action == KeyAction::Search).unwrap();
        assert_eq!(search.text, "⌘K");
        assert_eq!(search.keystroke, Keystroke::parse("⌘K").unwrap());

        let win = bindings_for(Platform::WinLinux, true);
        assert_eq!(
            win.len(),
            with_keys(Platform::WinLinux) + with_keys(Platform::WinLinuxTerminal)
        );
        let find: Vec<_> = win
            .iter()
            .filter(|b| b.action == KeyAction::FindInTerminal)
            .map(|b| (b.text.as_str(), b.in_terminal))
            .collect();
        assert_eq!(find, [("Ctrl+Shift+F", true)]);
        assert_eq!(bindings_for(Platform::WinLinuxTerminal, true), win);
    }

    /// Where the page looks a key up, one keystroke means one action: outside a terminal among the bindings that
    /// aren't terminal-only, inside one among every binding the router sends to the app.
    #[test]
    fn a_keystroke_finds_at_most_one_action() {
        for platform in [Platform::Mac, Platform::WinLinux] {
            let all = bindings_for(platform, true);
            for in_terminal in [false, true] {
                let mut seen: HashMap<String, KeyAction> = HashMap::new();
                for b in all.iter().filter(|b| {
                    if in_terminal {
                        route(platform, true, &b.keystroke.modifiers) == KeyRoute::App
                    } else {
                        !b.in_terminal
                    }
                }) {
                    let id = format!("{:?}", b.keystroke);
                    if let Some(other) = seen.insert(id, b.action) {
                        assert_eq!(other, b.action, "{} on {platform:?}", b.text);
                    }
                }
            }
            for b in all.iter().filter(|b| b.in_terminal) {
                assert_eq!(
                    route(platform, true, &b.keystroke.modifiers),
                    KeyRoute::App,
                    "{}",
                    b.text
                );
            }
        }
    }

    /// Tauri's default macOS menu keeps these keys (its app, File, Edit, View and Window menus); none may be a
    /// §21.3 shortcut, or the menu would take it first.
    #[test]
    fn the_default_mac_menu_keys_are_not_in_the_map() {
        let menu = [
            "⌘Q", "⌘W", "⌘H", "⌥⌘H", "⌘M", "⌃⌘F", "⌘Z", "⇧⌘Z", "⌘X", "⌘C", "⌘V", "⌘A",
        ];
        let menu: Vec<Keystroke> = menu.iter().map(|t| Keystroke::parse(t).unwrap()).collect();
        for b in bindings_for(Platform::Mac, true) {
            assert!(!menu.contains(&b.keystroke), "{} is a menu key", b.text);
        }
    }

    #[test]
    fn the_terminal_app_modifiers_are_what_route_uses() {
        assert_eq!(terminal_app_modifiers(Platform::Mac).len(), 1);
        assert!(terminal_app_modifiers(Platform::Mac)[0].platform);
        let held = Modifiers {
            control: true,
            shift: true,
            alt: true,
            ..Modifiers::default()
        };
        assert!(held.holds(&terminal_app_modifiers(Platform::WinLinux)[0]));
        assert!(!Modifiers::default().holds(&terminal_app_modifiers(Platform::WinLinux)[0]));
    }

    /// d0087 (parked 2026-10-18): with the Files view off, ⇧⌘O and ⌥⌘P aren't bound in any column, ⇧⌘B stays, and
    /// the rest of the map is as with it on.
    #[test]
    fn a_parked_files_view_binds_neither_of_its_keys() {
        for platform in [Platform::Mac, Platform::WinLinux] {
            let on = bindings_for(platform, true);
            let off = bindings_for(platform, false);
            assert_eq!(off, bindings(platform));
            assert!(
                off.iter().all(|b| !b.action.is_files_view()),
                "{platform:?}"
            );
            assert!(off.iter().any(|b| b.action == KeyAction::ShowBoard));
            let rest: Vec<_> = on
                .into_iter()
                .filter(|b| !b.action.is_files_view())
                .collect();
            assert_eq!(off, rest, "{platform:?}");
        }
        for text in ["⇧⌘O", "⌥⌘P"] {
            let stroke = Keystroke::parse(text).unwrap();
            assert!(
                !bindings_for(Platform::Mac, false)
                    .iter()
                    .any(|b| b.keystroke == stroke),
                "{text}"
            );
        }
    }
}
