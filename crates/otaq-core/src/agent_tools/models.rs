//! The tools' model lists (AGT-02; d0086, d0088): what each tool itself says it can run, read through its adapter
//! ([`super::AgentTool::list_models`]), and Otaq's copy of it.
//!
//! Only what a tool's list contains can be picked for a named agent (d0086). The copy is kept per tool in the
//! `settings` table (one key per tool, JSON: [`settings_key`]) with when it was read, and refreshed at service
//! start, on `agents.models_refresh` (Settings → Agents opening) and once a day ([`ModelLists::run`]). A failed
//! read keeps the old copy and is logged. Readers ask the in-memory copy, synchronously
//! ([`ModelLists::copy`]): "is `opus` in Claude Code's list, and does it follow the newest?" is
//! `lists.copy(Tool::Claude).and_then(|c| c.list.entry("opus").map(|e| e.kind == ModelKind::Follow))`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::{Adapters, Tool};
use crate::store::{Store, now_ms};

/// How often the copies are read again while the service runs (d0086: "once a day").
pub const REFRESH_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// What a list entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// The tool's own default (Claude Code's `default`): shown as "Tool default" and passed as nothing (d0088).
    ToolDefault,
    /// A name the tool resolves to its newest model of a family (Claude Code's `opus`, `sonnet`, `haiku`,
    /// `fable`): "always the newest", passed as it is (d0086).
    Follow,
    /// One exact model.
    Exact,
}

/// One model a tool can run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntry {
    /// What is passed to the tool (`opus`); the tool default's id is the tool's own name for it (`default`).
    pub id: String,
    /// The tool's name for it: "Opus".
    pub display_name: String,
    /// The tool's description, when it gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The exact model it runs today: `claude-opus-5-5`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolves_to: Option<String>,
    pub kind: ModelKind,
}

impl ModelEntry {
    /// The model option the tool gets: nothing for the tool default, else the id unchanged (d0088).
    pub fn passed(&self) -> Option<&str> {
        match self.kind {
            ModelKind::ToolDefault => None,
            ModelKind::Follow | ModelKind::Exact => Some(&self.id),
        }
    }
}

/// A tool's model list, in the tool's own order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelList {
    pub entries: Vec<ModelEntry>,
}

impl ModelList {
    /// The entry passed as `id` (`opus`, `claude-opus-5-5`).
    pub fn entry(&self, id: &str) -> Option<&ModelEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Whether `id` can be picked.
    pub fn contains(&self, id: &str) -> bool {
        self.entry(id).is_some()
    }

    /// The tool default's entry: what an empty model runs on today.
    pub fn tool_default(&self) -> Option<&ModelEntry> {
        self.entries
            .iter()
            .find(|e| e.kind == ModelKind::ToolDefault)
    }
}

/// Why a tool's list couldn't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelListError {
    /// The sentence a person reads: "Claude Code didn't list its models".
    pub message: String,
    /// What went wrong, for the log.
    pub reason: String,
}

impl std::fmt::Display for ModelListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.reason)
    }
}

impl std::error::Error for ModelListError {}

/// Otaq's copy of one tool's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCopy {
    pub tool: Tool,
    pub list: ModelList,
    /// When the tool was read, in Unix milliseconds.
    pub read_at: i64,
}

/// The `settings` key of `tool`'s copy: `agent_tools.models.claude`.
pub fn settings_key(tool: Tool) -> String {
    format!("agent_tools.models.{}", tool.as_str())
}

/// The copies of every tool's list, held by the service. Cheap to clone.
#[derive(Clone)]
pub struct ModelLists {
    store: Store,
    adapters: Adapters,
    /// Whether the tools are read at all: only the daemon's service reads them, so no test starts a tool.
    read_tools: bool,
    copies: Arc<RwLock<HashMap<Tool, ModelCopy>>>,
}

impl ModelLists {
    /// The copies stored earlier. With `read_tools` off, [`Self::refresh`] reads nothing and keeps them.
    pub async fn load(store: Store, adapters: Adapters, read_tools: bool) -> Self {
        let mut copies = HashMap::new();
        for &tool in Tool::ALL {
            match stored(&store, tool).await {
                Ok(Some(copy)) => {
                    copies.insert(tool, copy);
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(%tool, error = %e, "can't read the stored model list"),
            }
        }
        Self {
            store,
            adapters,
            read_tools,
            copies: Arc::new(RwLock::new(copies)),
        }
    }

    /// `tool`'s copy, if it was ever read.
    pub fn copy(&self, tool: Tool) -> Option<ModelCopy> {
        self.copies.read().expect("model lists").get(&tool).cloned()
    }

    /// Reads `tool`'s list again through its adapter and answers the copy as it is then.
    pub async fn refresh(&self, tool: Tool) -> Option<ModelCopy> {
        if !self.read_tools {
            tracing::debug!(%tool, "model lists aren't read by this service");
            return self.copy(tool);
        }
        let read = self.adapters.get(tool).list_models().await;
        self.record(tool, read, now_ms()).await
    }

    /// Reads every tool's list again.
    pub async fn refresh_all(&self) {
        for &tool in Tool::ALL {
            self.refresh(tool).await;
        }
    }

    /// Keeps what a read of `tool`'s list gave at `now`: a list replaces the copy (stored, then kept in memory);
    /// a failure keeps the old copy and is logged. Answers the copy as it is then.
    pub async fn record(
        &self,
        tool: Tool,
        read: Result<ModelList, ModelListError>,
        now: i64,
    ) -> Option<ModelCopy> {
        match read {
            Ok(list) => {
                let copy = ModelCopy {
                    tool,
                    list,
                    read_at: now,
                };
                tracing::info!(%tool, models = copy.list.entries.len(), "model list read");
                let json = serde_json::to_string(&copy).expect("a model list serialises");
                if let Err(e) = self.store.set_setting(&settings_key(tool), &json).await {
                    tracing::warn!(%tool, error = %e, "can't store the model list");
                }
                self.copies
                    .write()
                    .expect("model lists")
                    .insert(tool, copy.clone());
                Some(copy)
            }
            Err(e) => {
                tracing::warn!(%tool, error = %e, "model list not read; the last copy stays");
                self.copy(tool)
            }
        }
    }

    /// Reads every tool's list at once (the service's start), then once a day, until stopped.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut tick = tokio::time::interval(REFRESH_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = stop.wait_for(|s| *s) => break,
                _ = tick.tick() => {}
            }
            tokio::select! {
                _ = stop.wait_for(|s| *s) => break,
                () = self.refresh_all() => {}
            }
        }
    }
}

/// `tool`'s stored copy. A value that no longer reads as a copy counts as none (logged).
async fn stored(store: &Store, tool: Tool) -> Result<Option<ModelCopy>, crate::store::StoreError> {
    let Some(json) = store.setting(&settings_key(tool)).await? else {
        return Ok(None);
    };
    Ok(serde_json::from_str(&json)
        .inspect_err(|e| tracing::warn!(%tool, error = %e, "the stored model list doesn't read"))
        .ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_dir::DataDir;

    fn entry(id: &str, kind: ModelKind) -> ModelEntry {
        ModelEntry {
            id: id.into(),
            display_name: id.into(),
            description: None,
            resolves_to: Some(format!("claude-{id}-5-5")),
            kind,
        }
    }

    fn list(ids: &[(&str, ModelKind)]) -> ModelList {
        ModelList {
            entries: ids.iter().map(|&(id, kind)| entry(id, kind)).collect(),
        }
    }

    async fn lists(home: &std::path::Path) -> ModelLists {
        let data = DataDir::new(home);
        let store = Store::open_in(&data).await.unwrap();
        ModelLists::load(store, Adapters::launching_only(None, data), false).await
    }

    #[test]
    fn entries_say_what_is_passed() {
        let l = list(&[
            ("default", ModelKind::ToolDefault),
            ("opus", ModelKind::Follow),
        ]);
        assert!(l.contains("opus") && !l.contains("foo-bar-9"));
        assert_eq!(l.entry("opus").unwrap().passed(), Some("opus"));
        assert_eq!(l.tool_default().unwrap().passed(), None);
        assert_eq!(settings_key(Tool::Claude), "agent_tools.models.claude");
    }

    /// The copy is stored, read back by the next service, and replaced by a later read; a failed read keeps it.
    #[tokio::test]
    async fn the_copy_is_stored_read_back_replaced_and_kept_on_failure() {
        let home = tempfile::tempdir().unwrap();
        let first = lists(home.path()).await;
        assert_eq!(first.copy(Tool::Claude), None);
        let one = list(&[("default", ModelKind::ToolDefault)]);
        let copy = first.record(Tool::Claude, Ok(one.clone()), 1_000).await;
        assert_eq!(copy.as_ref().map(|c| c.read_at), Some(1_000));

        // The next service reads it back from the settings table.
        let again = lists(home.path()).await;
        assert_eq!(again.copy(Tool::Claude), copy);

        // A later read replaces it.
        let two = list(&[
            ("default", ModelKind::ToolDefault),
            ("opus", ModelKind::Follow),
        ]);
        let replaced = again.record(Tool::Claude, Ok(two.clone()), 2_000).await;
        assert_eq!(
            replaced,
            Some(ModelCopy {
                tool: Tool::Claude,
                list: two.clone(),
                read_at: 2_000
            })
        );

        // A failed read keeps the old copy, in memory and stored.
        let failed = ModelListError {
            message: "Claude Code didn't list its models".into(),
            reason: "timed out".into(),
        };
        assert_eq!(
            again.record(Tool::Claude, Err(failed), 3_000).await,
            replaced
        );
        assert_eq!(lists(home.path()).await.copy(Tool::Claude), replaced);
    }

    /// A service that doesn't read tools (every test's) refreshes nothing and answers the copy it has.
    #[tokio::test]
    async fn a_service_that_reads_no_tools_keeps_its_copy() {
        let home = tempfile::tempdir().unwrap();
        let l = lists(home.path()).await;
        assert_eq!(l.refresh(Tool::Claude).await, None);
        let copy = l
            .record(
                Tool::Claude,
                Ok(list(&[("opus", ModelKind::Follow)])),
                1_000,
            )
            .await;
        assert_eq!(l.refresh(Tool::Claude).await, copy);
    }
}
