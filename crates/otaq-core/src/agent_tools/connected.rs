//! Connected tools (ADP-10, d0086): each agent tool's state on Settings → Agent tools, kept in memory by the
//! service, and the tools that count. Only a **Connected** tool counts: for the starter list, for "one tool
//! connected", for avatars and for launching.
//!
//! The states come from each adapter's detect ([`AgentToolState::from_detection`]); T-0348 runs the rechecks. A
//! found tool the user set **Hidden** shows as Hidden whatever detect found; the hidden tools are kept in `settings`
//! ([`AGENT_TOOLS_HIDDEN`]). Every change of what the page would show is sent on a watch channel
//! ([`ConnectedTools::subscribe`]), once per change.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::{Detection, Tool};
use crate::board::{BoardError, Refusal};
use crate::store::settings::AGENT_TOOLS_HIDDEN;
use crate::store::{Store, StoreError};

/// A tool's state (d0086, Q-G).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentToolState {
    /// Installed, runs, signed in (or can't say: Claude Code). Counts.
    Connected {
        version: Option<String>,
        /// How it is signed in, where the tool says ("signed in with ChatGPT").
        account: Option<String>,
    },
    /// Found, but signed out: **Sign in…**.
    FoundNotSignedIn { version: Option<String> },
    /// Found, but older than Otaq can run (Copilot before 1.0.98): never launched.
    UpdateNeeded { version: Option<String> },
    /// Not on PATH: **How to install**.
    NotInstalled,
    /// Found, and set by the user not to count.
    Hidden { version: Option<String> },
}

impl AgentToolState {
    /// The state detect's answer gives, before Hidden: not found → Not installed; update needed; signed in
    /// `Some(false)` → Found · not signed in; signed in or can't say → Connected.
    pub fn from_detection(detection: &Detection) -> Self {
        let version = detection.version.clone();
        if !detection.installed {
            Self::NotInstalled
        } else if detection.update_needed {
            Self::UpdateNeeded { version }
        } else if detection.signed_in == Some(false) {
            Self::FoundNotSignedIn { version }
        } else {
            Self::Connected {
                version,
                account: detection.account.clone(),
            }
        }
    }

    /// The version found, if any.
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Connected { version, .. }
            | Self::FoundNotSignedIn { version }
            | Self::UpdateNeeded { version }
            | Self::Hidden { version } => version.as_deref(),
            Self::NotInstalled => None,
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// Why a tool in this state can't start an agent ("Codex isn't available · logged out"); `None` when
    /// Connected.
    pub fn unavailable_reason(&self) -> Option<&'static str> {
        match self {
            Self::Connected { .. } => None,
            Self::FoundNotSignedIn { .. } => Some("logged out"),
            Self::UpdateNeeded { .. } => Some("update needed"),
            Self::NotInstalled => Some("not installed"),
            Self::Hidden { .. } => Some("hidden in Settings"),
        }
    }
}

/// One tool's row: its state, when it was last checked and the notes its adapter's readers set ("limits not
/// reported", T-0350).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStatus {
    pub tool: Tool,
    #[serde(flatten)]
    pub state: AgentToolState,
    /// Unix milliseconds; `None` before the first check.
    pub checked_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Why a tool can't start an agent now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotAvailable {
    /// "logged out", "not installed", "update needed", "hidden in Settings".
    pub reason: String,
}

impl NotAvailable {
    /// "Codex isn't available · logged out".
    pub fn message(&self, tool: Tool) -> String {
        format!(
            "{} isn\u{2019}t available \u{b7} {}",
            tool.display_name(),
            self.reason
        )
    }
}

/// The answer to `agent_tools.list` and `agent_tools.set_hidden`: every tool, in [`Tool::ALL`] order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentToolsList {
    pub tools: Vec<ToolStatus>,
}

#[derive(Debug, Default)]
struct Found {
    /// What the last detect gave (never `Hidden`), and when.
    states: HashMap<Tool, (AgentToolState, i64)>,
    hidden: BTreeSet<Tool>,
    notes: HashMap<Tool, Vec<String>>,
}

impl Found {
    fn status(&self, tool: Tool) -> ToolStatus {
        let (found, checked_at) = match self.states.get(&tool) {
            Some((state, at)) => (state.clone(), Some(*at)),
            None => (AgentToolState::NotInstalled, None),
        };
        let state = match found {
            AgentToolState::NotInstalled => AgentToolState::NotInstalled,
            found if self.hidden.contains(&tool) => AgentToolState::Hidden {
                version: found.version().map(str::to_owned),
            },
            found => found,
        };
        ToolStatus {
            tool,
            state,
            checked_at,
            notes: self.notes.get(&tool).cloned().unwrap_or_default(),
        }
    }

    fn statuses(&self) -> Vec<ToolStatus> {
        Tool::ALL.iter().map(|&tool| self.status(tool)).collect()
    }
}

/// The tools' states in memory, shared by the service. Cheap to clone.
#[derive(Clone)]
pub struct ConnectedTools {
    found: Arc<Mutex<Found>>,
    changes: Arc<watch::Sender<Vec<ToolStatus>>>,
}

impl ConnectedTools {
    /// Nothing checked yet (every tool Not installed), with `hidden` set Hidden.
    pub fn new(hidden: impl IntoIterator<Item = Tool>) -> Self {
        let found = Found {
            hidden: hidden.into_iter().collect(),
            ..Found::default()
        };
        let (changes, _) = watch::channel(found.statuses());
        Self {
            found: Arc::new(Mutex::new(found)),
            changes: Arc::new(changes),
        }
    }

    /// Nothing checked yet, with the hidden tools stored in `settings`.
    pub async fn load(store: &Store) -> Result<Self, StoreError> {
        Ok(Self::new(hidden_tools(store).await?))
    }

    /// Every tool's row, in [`Tool::ALL`] order.
    pub fn list(&self) -> AgentToolsList {
        AgentToolsList {
            tools: self.lock().statuses(),
        }
    }

    pub fn status(&self, tool: Tool) -> ToolStatus {
        self.lock().status(tool)
    }

    /// The tools that count, in [`Tool::ALL`] order.
    pub fn connected(&self) -> Vec<Tool> {
        let found = self.lock();
        Tool::ALL
            .iter()
            .copied()
            .filter(|&tool| found.status(tool).state.is_connected())
            .collect()
    }

    /// Whether `tool` can start an agent now, or why not.
    pub fn is_available(&self, tool: Tool) -> Result<(), NotAvailable> {
        match self.status(tool).state.unavailable_reason() {
            None => Ok(()),
            Some(reason) => Err(NotAvailable {
                reason: reason.to_owned(),
            }),
        }
    }

    /// Records what detect found for `tool` at `at` (Unix milliseconds). A `Hidden` state is taken as the found
    /// tool it hides; hiding is [`Self::set_hidden`]'s.
    pub fn set_detected(&self, tool: Tool, state: AgentToolState, at: i64) {
        let state = match state {
            AgentToolState::Hidden { version } => AgentToolState::Connected {
                version,
                account: None,
            },
            state => state,
        };
        self.change(|found| {
            found.states.insert(tool, (state, at));
        });
    }

    /// Sets `tool` Hidden or not, in memory only.
    pub fn hide(&self, tool: Tool, hidden: bool) {
        self.change(|found| {
            if hidden {
                found.hidden.insert(tool);
            } else {
                found.hidden.remove(&tool);
            }
        });
    }

    /// Sets `tool` Hidden or not and stores it in `settings` (`agent_tools.set_hidden`). Writes and removes
    /// nothing in the tool's own config (d0086).
    pub async fn set_hidden(
        &self,
        store: &Store,
        tool: Tool,
        hidden: bool,
    ) -> Result<(), StoreError> {
        let mut tools: BTreeSet<Tool> = self.lock().hidden.clone();
        if hidden {
            tools.insert(tool);
        } else {
            tools.remove(&tool);
        }
        let names: Vec<&str> = tools.iter().map(|t| t.as_str()).collect();
        let json = serde_json::to_string(&names).expect("names serialise");
        store.set_setting(AGENT_TOOLS_HIDDEN, &json).await?;
        self.hide(tool, hidden);
        Ok(())
    }

    /// Adds (`on`) or removes a note on `tool`'s row ("limits not reported").
    pub fn set_note(&self, tool: Tool, note: &str, on: bool) {
        self.change(|found| {
            let notes = found.notes.entry(tool).or_default();
            let has = notes.iter().any(|n| n == note);
            if on && !has {
                notes.push(note.to_owned());
            } else if !on && has {
                notes.retain(|n| n != note);
            }
        });
    }

    /// `agent_tools.recheck`: detect `tool` (or every tool) again. T-0348.
    pub async fn recheck(&self, _tool: Option<Tool>) -> Result<(), BoardError> {
        Err(not_built("agent_tools.recheck"))
    }

    /// `agent_tools.sign_in`: the tool's own login in a terminal, then a recheck. T-0348.
    pub async fn sign_in(&self, _tool: Tool) -> Result<(), BoardError> {
        Err(not_built("agent_tools.sign_in"))
    }

    /// `agent_tools.disconnect`: Otaq's own entries removed, the tool set Hidden (Q-F). T-0348.
    pub async fn disconnect(&self, _tool: Tool) -> Result<(), BoardError> {
        Err(not_built("agent_tools.disconnect"))
    }

    /// Every row, sent again whenever one changes (once per change).
    pub fn subscribe(&self) -> watch::Receiver<Vec<ToolStatus>> {
        self.changes.subscribe()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Found> {
        self.found.lock().expect("connected tools")
    }

    fn change(&self, edit: impl FnOnce(&mut Found)) {
        let statuses = {
            let mut found = self.lock();
            edit(&mut found);
            found.statuses()
        };
        self.changes.send_if_modified(|sent| {
            let changed = *sent != statuses;
            if changed {
                *sent = statuses;
            }
            changed
        });
    }
}

fn not_built(request: &str) -> BoardError {
    BoardError::refused(
        Refusal::NotAvailableYet,
        format!("{request} isn't built yet"),
    )
}

/// The hidden tools stored in `settings`; names this build doesn't know are left out.
pub async fn hidden_tools(store: &Store) -> Result<Vec<Tool>, StoreError> {
    let names: Vec<String> = store
        .setting(AGENT_TOOLS_HIDDEN)
        .await?
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    Ok(names.iter().filter_map(|name| name.parse().ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected(version: &str) -> AgentToolState {
        AgentToolState::Connected {
            version: Some(version.into()),
            account: None,
        }
    }

    #[test]
    fn states_from_detection() {
        let found = |signed_in, update_needed| Detection {
            installed: true,
            signed_in,
            version: Some("0.157.1".into()),
            update_needed,
            account: Some("signed in with ChatGPT".into()),
        };
        assert_eq!(
            AgentToolState::from_detection(&Detection::default()),
            AgentToolState::NotInstalled
        );
        assert_eq!(
            AgentToolState::from_detection(&found(Some(true), true)),
            AgentToolState::UpdateNeeded {
                version: Some("0.157.1".into())
            }
        );
        assert_eq!(
            AgentToolState::from_detection(&found(Some(false), false)),
            AgentToolState::FoundNotSignedIn {
                version: Some("0.157.1".into())
            }
        );
        for signed_in in [Some(true), None] {
            assert_eq!(
                AgentToolState::from_detection(&found(signed_in, false)),
                AgentToolState::Connected {
                    version: Some("0.157.1".into()),
                    account: Some("signed in with ChatGPT".into())
                }
            );
        }
    }

    #[test]
    fn hidden_wins_over_a_found_tool_and_is_cleared() {
        let tools = ConnectedTools::new([Tool::Copilot]);
        for tool in Tool::ALL.iter().copied() {
            tools.set_detected(tool, connected("1.0.98"), 10);
        }
        assert_eq!(
            tools.connected(),
            [Tool::Claude, Tool::Codex, Tool::OpenCode]
        );
        assert_eq!(
            tools.status(Tool::Copilot).state,
            AgentToolState::Hidden {
                version: Some("1.0.98".into())
            }
        );
        let refused = tools.is_available(Tool::Copilot).unwrap_err();
        assert_eq!(
            refused.message(Tool::Copilot),
            "GitHub Copilot isn\u{2019}t available \u{b7} hidden in Settings"
        );

        tools.hide(Tool::Copilot, false);
        assert_eq!(tools.connected(), Tool::ALL);
        assert_eq!(tools.is_available(Tool::Copilot), Ok(()));
        tools.hide(Tool::Codex, true);
        assert_eq!(
            tools.connected(),
            [Tool::Claude, Tool::Copilot, Tool::OpenCode]
        );

        // A hidden tool that isn't installed reads as not installed.
        tools.set_detected(Tool::Codex, AgentToolState::NotInstalled, 11);
        assert_eq!(
            tools.status(Tool::Codex).state,
            AgentToolState::NotInstalled
        );
        assert_eq!(
            tools
                .is_available(Tool::Codex)
                .unwrap_err()
                .message(Tool::Codex),
            "Codex isn\u{2019}t available \u{b7} not installed"
        );
    }

    #[test]
    fn connected_keeps_the_tools_order_whatever_the_order_found() {
        let tools = ConnectedTools::new([]);
        assert!(tools.connected().is_empty(), "nothing checked yet");
        for tool in [Tool::OpenCode, Tool::Claude, Tool::Codex] {
            tools.set_detected(tool, connected("1"), 1);
        }
        assert_eq!(
            tools.connected(),
            [Tool::Claude, Tool::Codex, Tool::OpenCode]
        );
        let list = tools.list();
        let order: Vec<Tool> = list.tools.iter().map(|s| s.tool).collect();
        assert_eq!(order, Tool::ALL);
        assert_eq!(list.tools[2].checked_at, None, "Copilot not checked");
    }

    #[test]
    fn the_watch_fires_once_per_change() {
        let tools = ConnectedTools::new([]);
        let mut changes = tools.subscribe();
        assert!(!changes.has_changed().unwrap());

        tools.set_detected(Tool::Codex, connected("0.157.1"), 5);
        assert!(changes.has_changed().unwrap());
        let rows = changes.borrow_and_update().clone();
        assert!(rows[1].state.is_connected());
        assert!(!changes.has_changed().unwrap());

        tools.hide(Tool::Codex, true);
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();
        // The same again changes nothing, so nothing is sent.
        tools.hide(Tool::Codex, true);
        tools.set_detected(Tool::Codex, connected("0.157.1"), 5);
        assert!(!changes.has_changed().unwrap());

        tools.hide(Tool::Codex, false);
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();
        tools.set_note(Tool::Codex, "limits not reported", true);
        assert!(changes.has_changed().unwrap());
        assert_eq!(
            changes.borrow_and_update()[1].notes,
            ["limits not reported"]
        );
        tools.set_note(Tool::Codex, "limits not reported", true);
        assert!(!changes.has_changed().unwrap());
    }

    #[test]
    fn rows_serialise_with_their_state() {
        let tools = ConnectedTools::new([]);
        tools.set_detected(Tool::Codex, connected("0.157.1"), 5);
        let json = serde_json::to_value(tools.status(Tool::Codex)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"tool": "codex", "state": "connected", "version": "0.157.1", "account": null,
                               "checked_at": 5})
        );
    }
}
