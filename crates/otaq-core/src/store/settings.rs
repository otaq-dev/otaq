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

    pub async fn project_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(self.pool())
            .await?)
    }
}
