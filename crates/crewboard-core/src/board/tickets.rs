//! Tickets and epics: create, edit, key allocation, fields, actor and the event log (TKT-01, TKT-03; d0019, d0029,
//! d0032, d0051, d0059).
//!
//! New tickets and epics land in Backlog. Numbers increase per project and are never handed out again, even after
//! a deletion (d0029); epic children get the epic's next `.n`, also never reused (d0039). Standalone tickets and
//! epics store their base branch ("Start from and merge into"), the project's at creation unless overridden;
//! epic children have none of their own and use the epic's branch (d0051, d0059). External IDs exist only on
//! epics and standalone tickets. Every change records its actor and appends an `events` row.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, Sqlite, Transaction};

use super::{Board, BoardError, Refusal};
use crate::domain::{Actor, Column, Flags, Kind, ProjectKey, Status, TicketKey, stored_enum};
use crate::store::projects::Project;
use crate::store::tickets::TicketRow;
use crate::store::{new_id, now_ms};

/// One "Done when" criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneWhenItem {
    pub text: String,
    /// Who added it; items added by an agent are marked in the editor (d0019).
    pub added_by: Actor,
}

/// The branch a ticket starts from and merges into (d0051, d0059).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    /// A branch of the repository: standalone tickets and epics.
    Branch(String),
    /// The integration branch of this epic: epic children.
    Epic(TicketKey),
}

impl Base {
    /// How the base reads in messages: `main`, or "the epic's branch (SHOP-40)".
    pub fn describe(&self) -> String {
        match self {
            Self::Branch(branch) => format!("`{branch}`"),
            Self::Epic(epic) => format!("the epic's branch ({epic})"),
        }
    }
}

/// A ticket or epic as the board, the editor and the CLI see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub id: String,
    pub key: TicketKey,
    pub kind: Kind,
    pub title: String,
    /// "What and why" (Markdown).
    pub body: String,
    pub done_when: Vec<DoneWhenItem>,
    /// The epic this ticket belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<TicketKey>,
    pub column: Column,
    pub status: Status,
    pub base: Base,
    /// The base can't change any more (the ticket has been to Dev; for an epic, a child has).
    pub base_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Direct parents, in key order.
    pub depends_on: Vec<TicketKey>,
    pub flags: Flags,
    pub created_by: Actor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<Actor>,
    /// When the ticket first went to Dev; `None` for a ticket that never did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Ticket {
    pub fn is_epic(&self) -> bool {
        self.kind == Kind::Epic
    }

    /// Has this ticket ever been to Dev? Never-started tickets can be deleted and moved into an epic (d0044,
    /// d0060).
    pub fn has_started(&self) -> bool {
        self.started_at.is_some()
    }
}

/// Creates a ticket or epic in Backlog.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTicketInput {
    /// The project key.
    pub project: String,
    #[serde(default)]
    pub kind: Kind,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub done_when: Vec<String>,
    /// Creates an epic child: the epic's key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    /// Overrides the project's base branch (standalone tickets and epics only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

/// One edit of the "Done when" list. Indexes are 0-based and apply to the list as it is after the edits before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DoneWhenEdit {
    Add { text: String },
    Edit { index: usize, text: String },
    Move { from: usize, to: usize },
    Remove { index: usize },
}

/// Changes to a ticket's fields. Absent fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub done_when: Vec<DoneWhenEdit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    /// An empty string removes the External ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

stored_enum! {
    /// A field whose changes are recorded.
    pub enum Field ("field") {
        Title = "title",
        Body = "body",
        DoneWhen = "done_when",
        BaseBranch = "base_branch",
        ExternalId = "external_id",
    }
}

/// One recorded change: the before/after view of agent edits reads these (TKT-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: Field,
    /// For "Done when": the item's position after the change (0-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

stored_enum! {
    /// Kinds of rows in the `events` log.
    pub enum EventKind ("event kind") {
        Created = "created",
        Updated = "updated",
        Moved = "moved",
        Pinned = "pinned",
        Unpinned = "unpinned",
        DependencyAdded = "dependency_added",
        DependencyRemoved = "dependency_removed",
        MovedToEpic = "moved_to_epic",
        Closed = "closed",
        AgentEditsSeen = "agent_edits_seen",
    }
}

/// A `tickets` row with its project's key.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct TicketRecord {
    #[sqlx(flatten)]
    pub row: TicketRow,
    pub project_key: String,
}

pub(crate) const SELECT_TICKETS: &str =
    "SELECT t.*, p.key AS project_key FROM tickets t JOIN projects p ON p.id = t.project_id";

impl Board {
    /// Creates a ticket or epic in Backlog, or an epic child when `epic` is set.
    pub async fn create_ticket(
        &self,
        input: &NewTicketInput,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let project = self.project(&input.project).await?;
        let title = required_title(&input.title)?;
        let epic = match &input.epic {
            Some(epic) => Some(self.epic_for_child(&project, epic, input).await?),
            None => None,
        };
        let base_branch = match (&epic, &input.base_branch) {
            (Some(_), _) => None,
            (None, Some(b)) if !b.trim().is_empty() => Some(b.trim().to_owned()),
            (None, _) => Some(project.base_branch.clone()),
        };
        let external_id = non_empty(input.external_id.as_deref());

        let now = now_ms();
        let id = new_id();
        let mut tx = self.store.pool().begin().await?;
        let (number, child_number) = match &epic {
            Some(epic) => (
                epic.row.number,
                allocate_child_number(&mut tx, &epic.row.id).await?,
            ),
            None => (allocate_number(&mut tx, &project.id).await?, 0),
        };
        sqlx::query(
            "INSERT INTO tickets (id, project_id, number, epic_id, child_number, kind, title, body, external_id,
                                  board_column, state, base_branch, created_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(number)
        .bind(epic.as_ref().map(|e| &e.row.id))
        .bind(child_number)
        .bind(input.kind.as_str())
        .bind(&title)
        .bind(&input.body)
        .bind(&external_id)
        .bind(Column::Backlog.as_str())
        .bind(Status::Backlog.as_str())
        .bind(&base_branch)
        .bind(actor.to_string())
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let items: Vec<DoneWhenItem> = input
            .done_when
            .iter()
            .filter(|t| !t.trim().is_empty())
            .map(|text| DoneWhenItem {
                text: text.trim().to_owned(),
                added_by: actor.clone(),
            })
            .collect();
        write_done_when(&mut tx, &id, &items).await?;
        record_event(
            &mut tx,
            &id,
            EventKind::Created,
            actor,
            json!({"kind": input.kind, "title": title}),
        )
        .await?;
        tx.commit().await?;
        let ticket = self.ticket_by_id(&id).await?;
        tracing::info!(key = %ticket.key, actor = %actor, "ticket created");
        Ok(ticket)
    }

    /// Changes a ticket's fields and records each change with its actor.
    pub async fn update_ticket(
        &self,
        key: &str,
        update: &TicketUpdate,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let ticket = self.ticket(key).await?;
        let mut changes = Vec::new();

        let mut title = ticket.title.clone();
        if let Some(new) = &update.title {
            let new = required_title(new)?;
            if new != title {
                changes.push(change(Field::Title, None, Some(&title), Some(&new)));
                title = new;
            }
        }
        let mut body = ticket.body.clone();
        if let Some(new) = &update.body
            && *new != body
        {
            changes.push(change(Field::Body, None, Some(&body), Some(new)));
            body = new.clone();
        }

        let mut base_branch = match &ticket.base {
            Base::Branch(b) => Some(b.clone()),
            Base::Epic(_) => None,
        };
        if let Some(new) = &update.base_branch {
            let new = new.trim();
            if let Base::Epic(epic) = &ticket.base {
                return Err(BoardError::refused(
                    Refusal::BaseFromEpic,
                    format!("{} starts from the epic's branch ({epic})", ticket.key),
                ));
            }
            if new.is_empty() {
                return Err(BoardError::refused(
                    Refusal::NoBaseBranch,
                    "The base branch can't be empty",
                ));
            }
            if base_branch.as_deref() != Some(new) {
                if ticket.base_locked {
                    return Err(BoardError::refused(
                        Refusal::BaseLocked,
                        format!(
                            "{} has started; its base branch can't change any more",
                            ticket.key
                        ),
                    ));
                }
                self.check_base_change(&ticket, &Base::Branch(new.to_owned()))
                    .await?;
                changes.push(change(
                    Field::BaseBranch,
                    None,
                    base_branch.as_deref(),
                    Some(new),
                ));
                base_branch = Some(new.to_owned());
            }
        }

        let mut external_id = ticket.external_id.clone();
        if let Some(new) = &update.external_id {
            let new = non_empty(Some(new));
            if new != external_id {
                if ticket.epic.is_some() {
                    return Err(BoardError::refused(
                        Refusal::ExternalIdOnChild,
                        "Epic children have no External ID; set it on the epic",
                    ));
                }
                changes.push(change(
                    Field::ExternalId,
                    None,
                    external_id.as_deref(),
                    new.as_deref(),
                ));
                external_id = new;
            }
        }

        let mut done_when = ticket.done_when.clone();
        for edit in &update.done_when {
            changes.push(apply_done_when_edit(&mut done_when, edit, actor)?);
        }

        if changes.is_empty() {
            return Ok(ticket);
        }
        let mut tx = self.store.pool().begin().await?;
        sqlx::query(
            "UPDATE tickets SET title = ?, body = ?, base_branch = ?, external_id = ?, updated_by = ?,
                                updated_at = ?
             WHERE id = ?",
        )
        .bind(&title)
        .bind(&body)
        .bind(&base_branch)
        .bind(&external_id)
        .bind(actor.to_string())
        .bind(now_ms())
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        if !update.done_when.is_empty() {
            write_done_when(&mut tx, &ticket.id, &done_when).await?;
        }
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Updated,
            actor,
            json!({ "changes": changes }),
        )
        .await?;
        tx.commit().await?;
        self.ticket_by_id(&ticket.id).await
    }

    /// A ticket by key (`SHOP-18`, `SHOP-40.1`), or by an old key kept after Move to epic (d0044).
    pub async fn ticket(&self, key: &str) -> Result<Ticket, BoardError> {
        let id = self.ticket_id(key).await?;
        self.ticket_by_id(&id).await
    }

    /// The id of the ticket a key (or an alias) names.
    pub async fn ticket_id(&self, key: &str) -> Result<String, BoardError> {
        let unknown = || BoardError::NotFound(format!("unknown ticket {key}"));
        let parsed: TicketKey = key
            .parse()
            .map_err(|e: crate::domain::KeyError| BoardError::NotFound(e.to_string()))?;
        let found: Option<String> = sqlx::query_scalar(
            "SELECT t.id FROM tickets t JOIN projects p ON p.id = t.project_id
             WHERE p.key = ? AND t.number = ? AND t.child_number = ?",
        )
        .bind(parsed.project.as_str())
        .bind(i64::from(parsed.number))
        .bind(i64::from(parsed.child.unwrap_or(0)))
        .fetch_optional(self.store.pool())
        .await?;
        if let Some(id) = found {
            return Ok(id);
        }
        sqlx::query_scalar("SELECT ticket_id FROM ticket_key_aliases WHERE old_key = ?")
            .bind(parsed.to_string())
            .fetch_optional(self.store.pool())
            .await?
            .ok_or_else(unknown)
    }

    pub async fn ticket_by_id(&self, id: &str) -> Result<Ticket, BoardError> {
        let sql = format!("{SELECT_TICKETS} WHERE t.id = ?");
        let record: TicketRecord = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(self.store.pool())
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("unknown ticket id {id}")))?;
        let project_id = record.row.project_id.clone();
        let mut tickets = self.assemble(vec![record], &project_id).await?;
        Ok(tickets.remove(0))
    }

    /// Every ticket and epic of a project, by key.
    pub async fn tickets(&self, project_key: &str) -> Result<Vec<Ticket>, BoardError> {
        let project = self.project(project_key).await?;
        let sql =
            format!("{SELECT_TICKETS} WHERE t.project_id = ? ORDER BY t.number, t.child_number");
        let records: Vec<TicketRecord> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(&project.id)
            .fetch_all(self.store.pool())
            .await?;
        self.assemble(records, &project.id).await
    }

    /// Turns rows of one project into tickets, with their Done-when items and dependencies.
    async fn assemble(
        &self,
        records: Vec<TicketRecord>,
        project_id: &str,
    ) -> Result<Vec<Ticket>, BoardError> {
        let pool = self.store.pool();
        let items: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT d.ticket_id, d.text, d.added_by FROM ticket_done_when d
             JOIN tickets t ON t.id = d.ticket_id WHERE t.project_id = ? ORDER BY d.ticket_id, d.position",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        // Every ticket of the project by id, for epic and dependency keys (links stay within a project).
        let keys: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT t.id, p.key, t.number, t.child_number FROM tickets t JOIN projects p ON p.id = t.project_id
             WHERE t.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        let deps: Vec<(String, String)> = sqlx::query_as(
            "SELECT d.ticket_id, d.depends_on_id FROM ticket_deps d
             JOIN tickets t ON t.id = d.ticket_id WHERE t.project_id = ?",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        let agent_edited: std::collections::HashSet<String> = self
            .tickets_updated_by_agent(project_id)
            .await?
            .into_iter()
            .collect();
        let key_of: std::collections::HashMap<&str, TicketKey> = keys
            .iter()
            .filter_map(|(id, project, number, child)| {
                Some((id.as_str(), key_from_parts(project, *number, *child)?))
            })
            .collect();

        records
            .into_iter()
            .map(|record| {
                let row = record.row;
                let key = key_from_parts(&record.project_key, row.number, row.child_number)
                    .ok_or_else(|| BoardError::NotFound(format!("bad key in ticket {}", row.id)))?;
                let epic = row
                    .epic_id
                    .as_deref()
                    .and_then(|id| key_of.get(id).cloned());
                let base = match (&epic, &row.base_branch) {
                    (Some(epic), _) => Base::Epic(epic.clone()),
                    (None, Some(branch)) => Base::Branch(branch.clone()),
                    (None, None) => Base::Branch(String::new()),
                };
                let mut depends_on: Vec<TicketKey> = deps
                    .iter()
                    .filter(|(child, _)| *child == row.id)
                    .filter_map(|(_, parent)| key_of.get(parent.as_str()).cloned())
                    .collect();
                depends_on.sort();
                let done_when = items
                    .iter()
                    .filter(|(id, _, _)| *id == row.id)
                    .map(|(_, text, added_by)| DoneWhenItem {
                        text: text.clone(),
                        added_by: added_by.parse().expect("infallible"),
                    })
                    .collect();
                let status: Status =
                    row.state
                        .parse()
                        .map_err(|e: crate::domain::UnknownValue| {
                            BoardError::NotFound(e.to_string())
                        })?;
                let kind: Kind = row.kind.parse().map_err(|e: crate::domain::UnknownValue| {
                    BoardError::NotFound(e.to_string())
                })?;
                let flags = Flags {
                    pinned_at: row.pinned_at,
                    pr_number: row.pr_number.and_then(|n| u32::try_from(n).ok()),
                    updated_by_agent: agent_edited.contains(&row.id),
                    ..Flags::default()
                };
                Ok(Ticket {
                    key,
                    kind,
                    title: row.title,
                    body: row.body,
                    done_when,
                    epic,
                    column: status.column(),
                    status,
                    base,
                    base_locked: row.branch_locked,
                    external_id: row.external_id,
                    depends_on,
                    flags,
                    created_by: row.created_by.parse().expect("infallible"),
                    updated_by: row.updated_by.map(|a| a.parse().expect("infallible")),
                    started_at: row.started_at,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    id: row.id,
                })
            })
            .collect()
    }

    /// Checks that `epic` can take a new child created from `input`.
    async fn epic_for_child(
        &self,
        project: &Project,
        epic: &str,
        input: &NewTicketInput,
    ) -> Result<TicketRecord, BoardError> {
        let epic_ticket = self.ticket(epic).await?;
        if epic_ticket.key.project.as_str() != project.key {
            return Err(BoardError::refused(
                Refusal::OtherProject,
                format!("{} belongs to another project", epic_ticket.key),
            ));
        }
        if !epic_ticket.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAnEpic,
                format!("{} isn't an epic", epic_ticket.key),
            ));
        }
        if input.kind == Kind::Epic {
            return Err(BoardError::refused(
                Refusal::EpicInEpic,
                "An epic can't be inside another epic",
            ));
        }
        if input
            .base_branch
            .as_deref()
            .is_some_and(|b| !b.trim().is_empty())
        {
            return Err(BoardError::refused(
                Refusal::BaseFromEpic,
                format!(
                    "Epic children start from the epic's branch ({})",
                    epic_ticket.key
                ),
            ));
        }
        if non_empty(input.external_id.as_deref()).is_some() {
            return Err(BoardError::refused(
                Refusal::ExternalIdOnChild,
                "Epic children have no External ID; set it on the epic",
            ));
        }
        let sql = format!("{SELECT_TICKETS} WHERE t.id = ?");
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(&epic_ticket.id)
            .fetch_one(self.store.pool())
            .await?)
    }
}

/// `SHOP` + 40 + 1 → `SHOP-40.1`; child number 0 means no child.
pub(crate) fn key_from_parts(project: &str, number: i64, child: i64) -> Option<TicketKey> {
    let project = ProjectKey::new(project).ok()?;
    let number = u32::try_from(number).ok()?;
    Some(match child {
        0 => TicketKey::new(project, number),
        n => TicketKey::child(project, number, u32::try_from(n).ok()?),
    })
}

/// Takes the project's next ticket number. Numbers are never handed out twice (d0029).
async fn allocate_number(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: &str,
) -> Result<i64, BoardError> {
    Ok(sqlx::query_scalar(
        "UPDATE projects SET next_ticket_number = next_ticket_number + 1 WHERE id = ?
         RETURNING next_ticket_number - 1",
    )
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?)
}

/// Takes the epic's next child number, never handed out twice within the epic (d0039).
pub(crate) async fn allocate_child_number(
    tx: &mut Transaction<'_, Sqlite>,
    epic_id: &str,
) -> Result<i64, BoardError> {
    Ok(sqlx::query_scalar(
        "UPDATE tickets SET next_child_number = next_child_number + 1 WHERE id = ?
         RETURNING next_child_number - 1",
    )
    .bind(epic_id)
    .fetch_one(&mut **tx)
    .await?)
}

async fn write_done_when(
    tx: &mut Transaction<'_, Sqlite>,
    ticket_id: &str,
    items: &[DoneWhenItem],
) -> Result<(), BoardError> {
    sqlx::query("DELETE FROM ticket_done_when WHERE ticket_id = ?")
        .bind(ticket_id)
        .execute(&mut **tx)
        .await?;
    for (position, item) in items.iter().enumerate() {
        sqlx::query(
            "INSERT INTO ticket_done_when (ticket_id, position, text, added_by) VALUES (?, ?, ?, ?)",
        )
        .bind(ticket_id)
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(&item.text)
        .bind(item.added_by.to_string())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// Appends a row to the `events` log.
pub(crate) async fn record_event(
    tx: &mut Transaction<'_, Sqlite>,
    ticket_id: &str,
    kind: EventKind,
    actor: &Actor,
    payload: Value,
) -> Result<(), BoardError> {
    sqlx::query(
        "INSERT INTO events (id, ticket_id, kind, actor, payload_json, at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(ticket_id)
    .bind(kind.as_str())
    .bind(actor.to_string())
    .bind(payload.to_string())
    .bind(now_ms())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn apply_done_when_edit(
    items: &mut Vec<DoneWhenItem>,
    edit: &DoneWhenEdit,
    actor: &Actor,
) -> Result<FieldChange, BoardError> {
    let missing = |index: usize| {
        BoardError::refused(
            Refusal::NoSuchItem,
            format!("\"Done when\" has no item {}", index + 1),
        )
    };
    let text_of = |text: &str| -> Result<String, BoardError> {
        let text = text.trim();
        if text.is_empty() {
            Err(BoardError::refused(
                Refusal::EmptyField,
                "A \"Done when\" item can't be empty",
            ))
        } else {
            Ok(text.to_owned())
        }
    };
    Ok(match edit {
        DoneWhenEdit::Add { text } => {
            let text = text_of(text)?;
            items.push(DoneWhenItem {
                text: text.clone(),
                added_by: actor.clone(),
            });
            change(Field::DoneWhen, Some(items.len() - 1), None, Some(&text))
        }
        DoneWhenEdit::Edit { index, text } => {
            let text = text_of(text)?;
            let item = items.get_mut(*index).ok_or_else(|| missing(*index))?;
            let before = std::mem::replace(&mut item.text, text.clone());
            change(Field::DoneWhen, Some(*index), Some(&before), Some(&text))
        }
        DoneWhenEdit::Move { from, to } => {
            if *from >= items.len() {
                return Err(missing(*from));
            }
            if *to >= items.len() {
                return Err(missing(*to));
            }
            let item = items.remove(*from);
            let text = item.text.clone();
            items.insert(*to, item);
            FieldChange {
                field: Field::DoneWhen,
                item: Some(*to),
                before: Some(format!("#{} {text}", from + 1)),
                after: Some(format!("#{} {text}", to + 1)),
            }
        }
        DoneWhenEdit::Remove { index } => {
            if *index >= items.len() {
                return Err(missing(*index));
            }
            let item = items.remove(*index);
            change(Field::DoneWhen, Some(*index), Some(&item.text), None)
        }
    })
}

fn change(
    field: Field,
    item: Option<usize>,
    before: Option<&str>,
    after: Option<&str>,
) -> FieldChange {
    FieldChange {
        field,
        item,
        before: before.map(str::to_owned),
        after: after.map(str::to_owned),
    }
}

fn required_title(title: &str) -> Result<String, BoardError> {
    let title = title.trim();
    if title.is_empty() {
        Err(BoardError::refused(
            Refusal::EmptyField,
            "A ticket needs a title",
        ))
    } else {
        Ok(title.to_owned())
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}
