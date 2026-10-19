//! The command palette's recent items (KEY-06, d0097): the actions run, tickets opened and projects switched to last,
//! from every project, newest first. One `settings` row (no migration), written by the user's `recent.items`; it is
//! not one of the typed settings, so a write announces nothing.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Store, StoreError};

/// The row the list is kept in.
pub const RECENT_ITEMS: &str = "palette.recent";

/// How many items are kept ("about 20").
pub const RECENT_LIMIT: usize = 20;

/// What was used. An action is kept as the page names it (`{"action": "park"}`, `{"action": "project", "number": 2}`):
/// the core doesn't know the app's actions, and one the app no longer has is left out when the list is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecentUse {
    Action {
        action: Value,
    },
    /// `"NOTE-2"`: its project is the key's.
    Ticket {
        key: String,
    },
    Project {
        project: String,
    },
}

/// One recent item: what, and when (Unix milliseconds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentItem {
    #[serde(flatten)]
    pub used: RecentUse,
    pub at: i64,
}

/// Puts `used` first (once: an earlier use of the same thing goes) and keeps the newest [`RECENT_LIMIT`].
pub fn with_use(mut items: Vec<RecentItem>, used: RecentUse, at: i64) -> Vec<RecentItem> {
    items.retain(|i| i.used != used);
    items.insert(0, RecentItem { used, at });
    items.truncate(RECENT_LIMIT);
    items
}

impl Store {
    /// The recent items, newest first. A row that doesn't read (written by a later version) reads as none.
    pub async fn recent_items(&self) -> Result<Vec<RecentItem>, StoreError> {
        Ok(self
            .setting(RECENT_ITEMS)
            .await?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default())
    }

    /// Records a use at `at` and answers the new list. Read and written in one write transaction, so two uses at
    /// once both stay.
    pub async fn record_recent(
        &self,
        used: RecentUse,
        at: i64,
    ) -> Result<Vec<RecentItem>, StoreError> {
        let mut tx = self.begin_write().await?;
        let stored: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?")
                .bind(RECENT_ITEMS)
                .fetch_optional(&mut *tx)
                .await?;
        let items = with_use(
            stored
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default(),
            used,
            at,
        );
        let json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_owned());
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?)
             ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(RECENT_ITEMS)
        .bind(json)
        .bind(at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::data_dir::DataDir;

    fn ticket(key: &str) -> RecentUse {
        RecentUse::Ticket { key: key.into() }
    }

    #[test]
    fn a_use_goes_first_once_and_the_list_keeps_twenty() {
        let mut items = Vec::new();
        for n in 0..25 {
            items = with_use(items, ticket(&format!("NOTE-{n}")), n);
        }
        assert_eq!(items.len(), RECENT_LIMIT);
        assert_eq!(items[0].used, ticket("NOTE-24"));
        assert_eq!(items[19].used, ticket("NOTE-5"));
        let items = with_use(items, ticket("NOTE-10"), 99);
        assert_eq!(
            items[0],
            RecentItem {
                used: ticket("NOTE-10"),
                at: 99
            }
        );
        assert_eq!(
            items.iter().filter(|i| i.used == ticket("NOTE-10")).count(),
            1
        );
        assert_eq!(items.len(), RECENT_LIMIT);
    }

    #[test]
    fn the_row_names_each_kind() {
        let items = vec![
            RecentItem {
                used: RecentUse::Action {
                    action: json!({"action": "park"}),
                },
                at: 3,
            },
            RecentItem {
                used: ticket("API-1"),
                at: 2,
            },
            RecentItem {
                used: RecentUse::Project {
                    project: "API".into(),
                },
                at: 1,
            },
        ];
        assert_eq!(
            serde_json::to_value(&items).unwrap(),
            json!([
                {"kind": "action", "action": {"action": "park"}, "at": 3},
                {"kind": "ticket", "key": "API-1", "at": 2},
                {"kind": "project", "project": "API", "at": 1},
            ])
        );
    }

    #[tokio::test]
    async fn recents_survive_reopening_the_store() {
        let tmp = tempfile::tempdir().unwrap();
        let data = DataDir::new(tmp.path().join("home"));
        let store = Store::open_in(&data).await.unwrap();
        assert!(store.recent_items().await.unwrap().is_empty());
        store.record_recent(ticket("NOTE-2"), 1).await.unwrap();
        let answer = store.record_recent(ticket("API-1"), 2).await.unwrap();
        assert_eq!(answer[0].used, ticket("API-1"));
        store.close().await;

        let store = Store::open_in(&data).await.unwrap();
        let items = store.recent_items().await.unwrap();
        assert_eq!(
            items.iter().map(|i| i.used.clone()).collect::<Vec<_>>(),
            [ticket("API-1"), ticket("NOTE-2")]
        );
        store.close().await;
    }
}
