//! The **tool avatar** (SHL-08, d0087; *01 · Components · Agent-first shell & agent tools (0086 · V) → Provider Mark
//! (tool avatar)*): Otaq's own 16 px circle with two letters on one theme colour per agent tool, never the tool's
//! logo and never a status colour. Hovering it shows the tool's name, and the model when the session stored one.
//!
//! The view covers the four tools of d0087 (CC Claude Code, CX Codex, CP GitHub Copilot, OC OpenCode). Avatars are
//! hidden everywhere while only one tool is connected (d0086) and shown everywhere once two are (SHL-08): the
//! connected tools come from the service (`ConnectedTools` on `board.agents`'s answer, T-0362), which the desktop
//! puts here with [`set_connected`] each time it reads them, so every screen that asks [`shown`] follows. Board cards
//! take the board's own list instead (`CardExtras::several_tools`).

use std::sync::RwLock;

use otaq_core::agent_tools::Tool;
use serde::Serialize;
use ts_rs::TS;

/// The agent tools an avatar can stand for (d0087). `AvatarTool` on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename = "AvatarTool")]
pub enum AvatarTool {
    ClaudeCode,
    Codex,
    Copilot,
    OpenCode,
}

impl AvatarTool {
    /// The four tools, in the order of d0087 and the frame.
    pub const ALL: [AvatarTool; 4] = [Self::ClaudeCode, Self::Codex, Self::Copilot, Self::OpenCode];

    /// The two letters: three of the tools start with C.
    pub fn letters(self) -> &'static str {
        match self {
            Self::ClaudeCode => "CC",
            Self::Codex => "CX",
            Self::Copilot => "CP",
            Self::OpenCode => "OC",
        }
    }

    /// The name people see, as each adapter's `display_name` says it.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Copilot => "GitHub Copilot",
            Self::OpenCode => "OpenCode",
        }
    }

    /// The background's colour token (`tool-cc`); the letters use the same name with `-fg`.
    pub fn token(self) -> String {
        format!("tool-{}", self.letters().to_ascii_lowercase())
    }
}

impl From<Tool> for AvatarTool {
    fn from(tool: Tool) -> Self {
        match tool {
            Tool::Claude => Self::ClaudeCode,
            Tool::Codex => Self::Codex,
            Tool::Copilot => Self::Copilot,
            Tool::OpenCode => Self::OpenCode,
        }
    }
}

/// One avatar. `ToolAvatarView` on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ToolAvatarView")]
pub struct ToolAvatarView {
    pub tool: AvatarTool,
    /// "CC".
    pub letters: String,
    /// The circle's colour token, `tool-cc`.
    pub background: String,
    /// The letters' colour token, `tool-cc-fg`.
    pub foreground: String,
    /// "Claude Code", or "Claude Code · <model>" when the session stored its model.
    pub hover: String,
}

impl ToolAvatarView {
    pub fn of(tool: impl Into<AvatarTool>, model: Option<&str>) -> Self {
        let tool = tool.into();
        let background = tool.token();
        let hover = match model.filter(|m| !m.is_empty()) {
            Some(model) => format!("{} \u{b7} {model}", tool.name()),
            None => tool.name().to_owned(),
        };
        Self {
            tool,
            letters: tool.letters().to_owned(),
            foreground: format!("{background}-fg"),
            background,
            hover,
        }
    }
}

/// The connected tools as the service last said them; `None` until the desktop read them.
static CONNECTED: RwLock<Option<Vec<AvatarTool>>> = RwLock::new(None);

/// Puts the service's connected tools here (`ConnectedTools::connected`, in d0087's order).
pub fn set_connected(tools: &[Tool]) {
    let tools = tools.iter().copied().map(AvatarTool::from).collect();
    *CONNECTED.write().unwrap_or_else(|e| e.into_inner()) = Some(tools);
}

/// The tools connected now. Before the service said which, only the first tool counts, so the app looks like M22
/// (d0086) until it knows better.
pub fn connected() -> Vec<AvatarTool> {
    CONNECTED
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| {
            Tool::ALL[..1]
                .iter()
                .copied()
                .map(AvatarTool::from)
                .collect()
        })
}

/// Whether avatars are drawn at all: only once two or more tools are connected. Every screen asks this.
pub fn shown() -> bool {
    shown_with(&connected())
}

/// [`shown`] for a given list of connected tools.
pub fn shown_with(connected: &[AvatarTool]) -> bool {
    connected.len() > 1
}

/// The gallery's line under the avatars.
pub const HIDDEN_NOTE: &str =
    "Hidden in the app while only one tool is connected; shown everywhere once a second one is.";

/// *01 · Components → Provider Mark (tool avatar)* in the gallery: the four avatars and the note. `ToolAvatarGallery`
/// on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "ToolAvatarGallery")]
pub struct Gallery {
    pub avatars: Vec<ToolAvatarView>,
    /// Whether the app draws avatars now (false while one tool is connected).
    pub shown: bool,
    pub note: String,
}

pub fn gallery() -> Gallery {
    Gallery {
        avatars: AvatarTool::ALL
            .into_iter()
            .map(|tool| ToolAvatarView::of(tool, None))
            .collect(),
        shown: shown(),
        note: HIDDEN_NOTE.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Colors, ThemeId};

    #[test]
    fn each_tool_has_its_letters_tokens_and_hover() {
        let want = [
            (AvatarTool::ClaudeCode, "CC", "tool-cc", "Claude Code"),
            (AvatarTool::Codex, "CX", "tool-cx", "Codex"),
            (AvatarTool::Copilot, "CP", "tool-cp", "GitHub Copilot"),
            (AvatarTool::OpenCode, "OC", "tool-oc", "OpenCode"),
        ];
        for (tool, letters, token, name) in want {
            let view = ToolAvatarView::of(tool, None);
            assert_eq!(view.letters, letters);
            assert_eq!(view.background, token);
            assert_eq!(view.foreground, format!("{token}-fg"));
            assert_eq!(view.hover, name);
            // Both tokens exist in every theme.
            for id in ThemeId::ALL {
                let colors = Colors::new(id);
                assert!(colors.get(&view.background).is_some(), "{token} {id:?}");
                assert!(colors.get(&view.foreground).is_some(), "{token}-fg {id:?}");
            }
        }
        assert_eq!(
            ToolAvatarView::of(AvatarTool::Codex, Some("gpt-5.4")).hover,
            "Codex \u{b7} gpt-5.4"
        );
        assert_eq!(
            ToolAvatarView::of(AvatarTool::Codex, Some("")).hover,
            "Codex"
        );
    }

    #[test]
    fn the_stored_tool_gives_its_avatar() {
        // Only Claude Code exists in core data until M23 (no model stored yet).
        let view = ToolAvatarView::of(Tool::Claude, None);
        assert_eq!(view.tool, AvatarTool::ClaudeCode);
        assert_eq!(view.hover, "Claude Code");
    }

    #[test]
    fn avatars_show_only_with_two_tools_connected() {
        assert!(!shown_with(&[AvatarTool::ClaudeCode]));
        assert!(shown_with(&[AvatarTool::ClaudeCode, AvatarTool::Codex]));
        // Before the service said which tools are connected: one, so avatars are hidden.
        assert!(!shown_with(
            &Tool::ALL[..1]
                .iter()
                .copied()
                .map(AvatarTool::from)
                .collect::<Vec<_>>()
        ));
        let gallery = gallery();
        let letters: Vec<&str> = gallery.avatars.iter().map(|a| a.letters.as_str()).collect();
        assert_eq!(letters, ["CC", "CX", "CP", "OC"]);
    }
}
