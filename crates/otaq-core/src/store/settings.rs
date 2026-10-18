//! Raw access to the global `settings` table (key → JSON value). Typed settings with defaults and validation
//! come in T-0028; until then only the onboarding's flags live here.

use super::{Store, StoreError, now_ms};

/// Key of the remembered dismissal of the first-run permission-mode warning (d0011, d0016).
pub const PERMISSION_WARNING_DISMISSED: &str = "onboarding.permission_warning_dismissed";

/// Set by a startup pass that recorded Claude Code's trust for a project added earlier: the one-time note shows
/// (PRJ-08, d0085).
pub const TRUST_NOTE_PENDING: &str = "trust.note_pending";

/// The user dismissed the folder trust note: it never comes back (PRJ-08).
pub const TRUST_NOTE_DISMISSED: &str = "trust.note_dismissed";

/// The app-wide role defaults (AGT-03, d0088): `{"developer": id, "validator": id, "epic_lead": id, "project_agent":
/// id}`, each a named agent's id. Written by the starter entry and `agents.set_default`; a project's overrides are
/// in its own settings ([`super::projects::PROJECT_AGENT_DEFAULTS`]).
pub const AGENT_DEFAULTS: &str = "agents.defaults";

/// The agent tools the user set Hidden on Settings → Agent tools (d0086): a list of stored tool names
/// (`["copilot"]`). A hidden tool doesn't count as connected.
pub const AGENT_TOOLS_HIDDEN: &str = "agent_tools.hidden";

/// The agent tools that counted as connected at the last check (d0086): a list of stored tool names. A tool that
/// counts and isn't listed has just become Connected: it is set up, gets its entry and its folder trust, once.
pub const AGENT_TOOLS_CONNECTED: &str = "agent_tools.connected";

/// The tools the pending folder trust note names (`["codex"]`), cleared with the note (PRJ-08, d0085, d0086).
pub const TRUST_NOTE_TOOLS: &str = "trust.note_tools";

/// The tools whose connecting raised the folder trust note once already: it is raised once per tool (d0086).
pub const TRUST_NOTE_RAISED: &str = "trust.note_raised";

/// "Use fallback agents automatically" (AGT-08, d0086): `true` or `false`; absent = off.
pub const AUTO_FALLBACK: &str = "agents.auto_fallback";

/// Otaq's own "Allow notifications?" line is up (ATT-13, d0090): `true`, set when the first agent session starts while
/// macOS hasn't been asked; removed once answered.
pub const NOTIFICATIONS_ASK_PENDING: &str = "notifications.ask_pending";

/// The user answered the ask (Allow, Not now, or Settings' Allow notifications): `true`. It is never set pending again.
pub const NOTIFICATIONS_ASK_ANSWERED: &str = "notifications.ask_answered";

/// The user closed the "macOS is blocking Otaq's notifications" banner (ATT-14): the permission it was closed for
/// (`"turned_off"`). Removed when the permission becomes allowed, so a later turn-off can show it once more.
pub const NOTIFICATIONS_BLOCKED_DISMISSED: &str = "notifications.blocked_dismissed";

impl Store {
    /// The stored JSON value of a global setting.
    pub async fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(self.pool())
                .await?,
        )
    }

    /// Stores a global setting's JSON value, replacing an earlier one.
    pub async fn set_setting(&self, key: &str, value_json: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?)
             ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value_json)
        .bind(now_ms())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Removes a global setting; nothing happens when it isn't stored.
    pub async fn remove_setting(&self, key: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn project_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(self.pool())
            .await?)
    }
}
