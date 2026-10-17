//! The starter entry (AGT-01, AGT-03; d0086, d0088): when the list is empty (after every check of the agent
//! tools, and when onboarding adds a project) Otaq creates one entry per connected tool, in the order Claude Code,
//! Codex, Copilot, OpenCode ([`crate::agent_tools::Tool::ALL`]'s order), named after the tool, with the model empty
//! (tool default) and the "Use for" note [`STARTER_USE_FOR`], and points all four role defaults at the first.
//! Connecting a tool later adds its entry if none uses that tool and changes nothing else
//! ([`add_for_connected_tool`], called when a tool becomes Connected). The same for everyone: there is no upgrade
//! path (d0088).
//!
//! "Connected" is the tool's state on Settings → Agent tools ([`ConnectedTools`], d0086), which runs the starter
//! after every check. Tools are taken as their stored names so the rules can be tested with a test-only tool list.

use crate::agent_tools::Adapters;
use crate::agent_tools::connected::ConnectedTools;
use crate::store::named_agents::NewNamedAgent;
use crate::store::{Store, StoreError};

use super::RoleDefaults;

/// The starter entry's "Use for" note.
pub const STARTER_USE_FOR: &str = "Everything, until you add more agents";

/// A connected tool: its stored name (`claude`) and the name shown to people, which names its entry ("Claude
/// Code").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedTool {
    pub tool: String,
    pub name: String,
}

/// The connected tools ([`ConnectedTools::connected`]), in [`crate::agent_tools::Tool::ALL`]'s order: Connected only, never a
/// tool that is found but signed out, hidden or needs an update (d0086).
pub fn connected(tools: &ConnectedTools, adapters: &Adapters) -> Vec<ConnectedTool> {
    tools
        .connected()
        .into_iter()
        .map(|tool| ConnectedTool {
            tool: tool.as_str().to_owned(),
            name: adapters.get(tool).display_name().to_owned(),
        })
        .collect()
}

/// Creates the starter entries and the role defaults when the list is empty and a tool is connected. Returns
/// whether it created them. Deleting down to one entry never brings them back (the list is never empty again:
/// the last entry can't be deleted).
pub async fn ensure(store: &Store, tools: &[ConnectedTool]) -> Result<bool, StoreError> {
    if tools.is_empty() {
        return Ok(false);
    }
    let mut tx = store.begin_write().await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM named_agents")
        .fetch_one(&mut *tx)
        .await?;
    if count > 0 {
        return Ok(false);
    }
    let mut first = None;
    for tool in tools {
        let id =
            Store::insert_named_agent_in(&mut tx, &starter_entry(tool, tool.name.clone())).await?;
        first.get_or_insert(id);
    }
    if let Some(first) = first {
        Store::set_agent_defaults_in(&mut tx, &RoleDefaults::all_on(&first)).await?;
    }
    tx.commit().await?;
    tracing::info!(entries = tools.len(), "created the starter named agents");
    Ok(true)
}

/// Adds the entry of a tool connected later, if no entry uses that tool; changes no other entry and no default.
/// Returns the new entry's id. When another entry already has the tool's name, the new one is "<name> 2" (or the
/// next free number).
pub async fn add_for_connected_tool(
    store: &Store,
    tool: &ConnectedTool,
) -> Result<Option<String>, StoreError> {
    let mut tx = store.begin_write().await?;
    let existing: Vec<(String, String)> = sqlx::query_as("SELECT name, tool FROM named_agents")
        .fetch_all(&mut *tx)
        .await?;
    if existing.iter().any(|(_, t)| *t == tool.tool) {
        return Ok(None);
    }
    let taken = |name: &str| {
        let folded = name.to_lowercase();
        existing.iter().any(|(n, _)| n.to_lowercase() == folded)
    };
    let mut name = tool.name.clone();
    let mut n = 2;
    while taken(&name) {
        name = format!("{} {n}", tool.name);
        n += 1;
    }
    let id = Store::insert_named_agent_in(&mut tx, &starter_entry(tool, name)).await?;
    tx.commit().await?;
    Ok(Some(id))
}

fn starter_entry(tool: &ConnectedTool, name: String) -> NewNamedAgent {
    NewNamedAgent {
        name,
        tool: tool.tool.clone(),
        model: None,
        use_for: STARTER_USE_FOR.to_owned(),
        fallback: None,
    }
}
