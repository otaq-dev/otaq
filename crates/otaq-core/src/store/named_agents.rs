//! Thin access to the `named_agents` and `ticket_agent_rows` tables and the role defaults (AGT-01, AGT-03, AGT-04;
//! d0086, d0088). The rules (unique names, models from the tool's list, delete while in use, locked rows) live in
//! [`crate::named_agents`].
//!
//! Tools are their stored names here (`claude`), so the starter can be tested with a test-only tool list; a row of
//! a tool this build doesn't know is skipped when read as a [`NamedAgent`].

use sqlx::{FromRow, Sqlite, Transaction};

use super::settings::AGENT_DEFAULTS;
use super::{Store, StoreError, new_id, now_ms};
use crate::agent_tools::Tool;
use crate::domain::{Column, ProjectKey, TicketKey};
use crate::named_agents::{
    AgentHold, AgentRow, HoldReason, Mark, NamedAgent, Role, RoleDefaults, RowMove, RowSetBy,
};
use crate::session::END_MODEL_REFUSED;

/// The query's session (its columns prefixed with the literal, `"s."`) started: it didn't end "not started", that is
/// refused on the model (T-0337) or before its tool reported the conversation's id (a failed start, nothing to resume:
/// T-0349, Q130). Bind [`END_MODEL_REFUSED`] for its `?`.
macro_rules! started {
    ($t:literal) => {
        concat!(
            "COALESCE(",
            $t,
            "end_reason, '') <> ? AND NOT (",
            $t,
            "ended_at IS NOT NULL AND COALESCE(",
            $t,
            "tool_session_id, '') = '')"
        )
    };
}

/// The columns where a ticket counts as not finished for "in use": Backlog through Ready for Review (d0086). Approved
/// and Done don't count.
const UNFINISHED: [Column; 4] = [
    Column::Backlog,
    Column::ReadyForDev,
    Column::Dev,
    Column::ReadyForReview,
];

#[derive(Debug, Clone, FromRow)]
struct NamedAgentRecord {
    id: String,
    name: String,
    tool: String,
    model: Option<String>,
    use_for: String,
    fallback_id: Option<String>,
    mark_text: Option<String>,
    mark_model: Option<String>,
    mark_ticket: Option<String>,
    mark_message: Option<String>,
    marked_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl NamedAgentRecord {
    fn into_agent(self) -> Option<NamedAgent> {
        let Ok(tool) = self.tool.parse() else {
            tracing::warn!(
                id = self.id,
                tool = self.tool,
                "a named agent of an unknown tool"
            );
            return None;
        };
        let mark = match (self.mark_text, self.marked_at) {
            (Some(text), Some(at)) => Some(Mark {
                text,
                model: self.mark_model,
                ticket: self.mark_ticket,
                message: self.mark_message,
                at,
            }),
            _ => None,
        };
        Some(NamedAgent {
            id: self.id,
            name: self.name,
            tool,
            model: self.model,
            use_for: self.use_for,
            fallback: self.fallback_id,
            mark,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// A new entry: `tool` as stored, `model` as passed to the tool (`None` = tool default), `fallback` another entry's
/// id (checked by the rules).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNamedAgent {
    pub name: String,
    pub tool: String,
    pub model: Option<String>,
    pub use_for: String,
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct RowRecord {
    role: String,
    named_agent_id: Option<String>,
    set_by: String,
    picked_by: Option<String>,
    reason: Option<String>,
    set_at: i64,
}

#[derive(Debug, Clone, FromRow)]
struct ProjectRowRecord {
    ticket_id: String,
    #[sqlx(flatten)]
    row: RowRecord,
}

impl RowRecord {
    fn into_row(self) -> Option<AgentRow> {
        let role = self.role.parse().ok()?;
        let set_by = match self.set_by.as_str() {
            "user" => RowSetBy::User,
            _ => RowSetBy::Agent {
                picked_by: self.picked_by.unwrap_or_default(),
                reason: self.reason.unwrap_or_default(),
            },
        };
        Some(AgentRow {
            role,
            agent: self.named_agent_id,
            set_by,
            set_at: self.set_at,
        })
    }
}

impl Store {
    /// Every named agent, oldest first (the starter entry first).
    pub async fn named_agents(&self) -> Result<Vec<NamedAgent>, StoreError> {
        let rows: Vec<NamedAgentRecord> =
            sqlx::query_as("SELECT * FROM named_agents ORDER BY created_at, id")
                .fetch_all(self.pool())
                .await?;
        Ok(rows
            .into_iter()
            .filter_map(NamedAgentRecord::into_agent)
            .collect())
    }

    pub async fn named_agent(&self, id: &str) -> Result<Option<NamedAgent>, StoreError> {
        let row: Option<NamedAgentRecord> =
            sqlx::query_as("SELECT * FROM named_agents WHERE id = ?")
                .bind(id)
                .fetch_optional(self.pool())
                .await?;
        Ok(row.and_then(NamedAgentRecord::into_agent))
    }

    /// How many entries there are, whatever their tool.
    pub async fn named_agent_count(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM named_agents")
            .fetch_one(self.pool())
            .await?)
    }

    /// Every entry's id, name and stored tool, whatever the tool (the starter's view).
    pub async fn named_agent_tools(&self) -> Result<Vec<(String, String, String)>, StoreError> {
        Ok(
            sqlx::query_as("SELECT id, name, tool FROM named_agents ORDER BY created_at, id")
                .fetch_all(self.pool())
                .await?,
        )
    }

    /// Inserts an entry in `tx` and returns its id. The caller checked the name.
    pub(crate) async fn insert_named_agent_in(
        tx: &mut Transaction<'_, Sqlite>,
        new: &NewNamedAgent,
    ) -> Result<String, StoreError> {
        let id = new_id();
        let now = now_ms();
        sqlx::query(
            "INSERT INTO named_agents (id, name, tool, model, use_for, fallback_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&new.name)
        .bind(&new.tool)
        .bind(&new.model)
        .bind(&new.use_for)
        .bind(&new.fallback)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await?;
        Ok(id)
    }

    /// Changes an entry's fields; `clear_mark` removes its mark (a saved model change, AGT-05).
    pub async fn update_named_agent(
        &self,
        id: &str,
        fields: &NewNamedAgent,
        clear_mark: bool,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE named_agents SET name = ?, tool = ?, model = ?, use_for = ?, fallback_id = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&fields.name)
        .bind(&fields.tool)
        .bind(&fields.model)
        .bind(&fields.use_for)
        .bind(&fields.fallback)
        .bind(now_ms())
        .bind(id)
        .execute(self.pool())
        .await?;
        if clear_mark {
            sqlx::query(
                "UPDATE named_agents SET mark_text = NULL, mark_model = NULL, mark_ticket = NULL, mark_message = NULL,
                                         marked_at = NULL
                 WHERE id = ?",
            )
            .bind(id)
            .execute(self.pool())
            .await?;
        }
        Ok(())
    }

    /// Deletes an entry. Rows pointing at it become empty (`ON DELETE SET NULL`); sessions keep its name.
    pub async fn delete_named_agent(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM named_agents WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The app-wide role defaults; all empty before the starter entry exists.
    pub async fn agent_defaults(&self) -> Result<RoleDefaults, StoreError> {
        Ok(self
            .setting(AGENT_DEFAULTS)
            .await?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default())
    }

    pub async fn set_agent_defaults(&self, defaults: &RoleDefaults) -> Result<(), StoreError> {
        let json = serde_json::to_string(defaults).expect("defaults serialize");
        self.set_setting(AGENT_DEFAULTS, &json).await
    }

    /// The app-wide role defaults written in `tx` (the starter's).
    pub(crate) async fn set_agent_defaults_in(
        tx: &mut Transaction<'_, Sqlite>,
        defaults: &RoleDefaults,
    ) -> Result<(), StoreError> {
        let json = serde_json::to_string(defaults).expect("defaults serialize");
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?, ?, ?)
             ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(AGENT_DEFAULTS)
        .bind(json)
        .bind(now_ms())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// The ticket's stored rows (Developer and Validator, or an epic's Epic lead). No row = empty.
    pub async fn ticket_agent_rows(&self, ticket_id: &str) -> Result<Vec<AgentRow>, StoreError> {
        let rows: Vec<RowRecord> = sqlx::query_as(
            "SELECT role, named_agent_id, set_by, picked_by, reason, set_at FROM ticket_agent_rows
             WHERE ticket_id = ? ORDER BY role",
        )
        .bind(ticket_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().filter_map(RowRecord::into_row).collect())
    }

    /// Every stored row of a project's tickets, by ticket id (the board's cards, T-0345).
    pub async fn project_agent_rows(
        &self,
        project_id: &str,
    ) -> Result<Vec<(String, AgentRow)>, StoreError> {
        let rows: Vec<ProjectRowRecord> = sqlx::query_as(
            "SELECT r.ticket_id, r.role, r.named_agent_id, r.set_by, r.picked_by, r.reason, r.set_at
             FROM ticket_agent_rows r JOIN tickets t ON t.id = r.ticket_id
             WHERE t.project_id = ? ORDER BY r.ticket_id, r.role",
        )
        .bind(project_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| Some((r.ticket_id, r.row.into_row()?)))
            .collect())
    }

    /// The named agent of each running session of a project's tickets, by ticket id: the developer's side (agent,
    /// answer, delivery) or the validator's (the board's footer names whoever has the turn, T-0345), with the tool it
    /// runs on and its model (the one the tool reported, else the one passed; the card's avatar, T-0362).
    pub async fn running_agent_names(
        &self,
        project_id: &str,
    ) -> Result<Vec<RunningAgent>, StoreError> {
        let rows: Vec<RunningRecord> = sqlx::query_as(
            "SELECT s.ticket_id, s.kind, s.named_agent_name AS name, s.tool,
                    COALESCE(s.model_ran, s.model) AS model
             FROM sessions s JOIN tickets t ON t.id = s.ticket_id
             WHERE t.project_id = ? AND s.state = 'running' AND s.named_agent_name IS NOT NULL
               AND s.kind IN ('agent', 'answer', 'delivery', 'validator')
             ORDER BY s.started_at, s.id",
        )
        .bind(project_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| RunningAgent {
                role: match r.kind.as_str() {
                    "validator" => Role::Validator,
                    _ => Role::Developer,
                },
                ticket_id: r.ticket_id,
                name: r.name,
                tool: r.tool.and_then(|t| t.parse().ok()),
                model: r.model,
            })
            .collect())
    }

    /// Every row move of a project's tickets, by ticket id and role (the board's "Moved from Strong · Claude Code
    /// limit", T-0362).
    pub async fn project_row_moves(
        &self,
        project_id: &str,
    ) -> Result<Vec<(String, Role, RowMove)>, StoreError> {
        let rows: Vec<ProjectMoveRecord> = sqlx::query_as(
            "SELECT r.ticket_id, r.role, r.moved_from_agent_id, r.moved_from_name, r.moved_reason, r.moved_tool,
                    r.moved_at
             FROM ticket_agent_rows r JOIN tickets t ON t.id = r.ticket_id
             WHERE t.project_id = ? AND r.moved_reason IS NOT NULL ORDER BY r.ticket_id, r.role",
        )
        .bind(project_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| Some((r.ticket_id, r.role.parse().ok()?, r.moved.into_move()?)))
            .collect())
    }

    /// Writes one row (replacing the role's earlier one and its move), or empties it with `None`.
    pub async fn set_ticket_agent_row(
        &self,
        ticket_id: &str,
        role: Role,
        value: Option<(&str, &RowSetBy)>,
    ) -> Result<(), StoreError> {
        self.write_ticket_agent_row(ticket_id, role, value, None)
            .await
    }

    /// Moves a Developer row to `agent` (AGT-08): the row as [`Self::set_ticket_agent_row`] writes it, with where
    /// it moved from and why.
    pub async fn move_ticket_agent_row(
        &self,
        ticket_id: &str,
        role: Role,
        agent: &str,
        set_by: &RowSetBy,
        moved: &RowMove,
    ) -> Result<(), StoreError> {
        self.write_ticket_agent_row(ticket_id, role, Some((agent, set_by)), Some(moved))
            .await
    }

    /// Where a row moved from, and why; `None` when it didn't move (or was changed since).
    pub async fn ticket_row_move(
        &self,
        ticket_id: &str,
        role: Role,
    ) -> Result<Option<RowMove>, StoreError> {
        let row: Option<RowMoveRecord> = sqlx::query_as(
            "SELECT moved_from_agent_id, moved_from_name, moved_reason, moved_tool, moved_at
             FROM ticket_agent_rows WHERE ticket_id = ? AND role = ? AND moved_reason IS NOT NULL",
        )
        .bind(ticket_id)
        .bind(role.as_str())
        .fetch_optional(self.pool())
        .await?;
        Ok(row.and_then(RowMoveRecord::into_move))
    }

    async fn write_ticket_agent_row(
        &self,
        ticket_id: &str,
        role: Role,
        value: Option<(&str, &RowSetBy)>,
        moved: Option<&RowMove>,
    ) -> Result<(), StoreError> {
        let Some((agent, set_by)) = value else {
            sqlx::query("DELETE FROM ticket_agent_rows WHERE ticket_id = ? AND role = ?")
                .bind(ticket_id)
                .bind(role.as_str())
                .execute(self.pool())
                .await?;
            return Ok(());
        };
        let (by, picked_by, reason) = match set_by {
            RowSetBy::User => ("user", None, None),
            RowSetBy::Agent { picked_by, reason } => {
                ("agent", Some(picked_by.as_str()), Some(reason.as_str()))
            }
        };
        sqlx::query(
            "INSERT INTO ticket_agent_rows (ticket_id, role, named_agent_id, set_by, picked_by, reason, set_at,
                                            moved_from_agent_id, moved_from_name, moved_reason, moved_tool, moved_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (ticket_id, role) DO UPDATE SET named_agent_id = excluded.named_agent_id,
                 set_by = excluded.set_by, picked_by = excluded.picked_by, reason = excluded.reason,
                 set_at = excluded.set_at, moved_from_agent_id = excluded.moved_from_agent_id,
                 moved_from_name = excluded.moved_from_name, moved_reason = excluded.moved_reason,
                 moved_tool = excluded.moved_tool, moved_at = excluded.moved_at",
        )
        .bind(ticket_id)
        .bind(role.as_str())
        .bind(agent)
        .bind(by)
        .bind(picked_by)
        .bind(reason)
        .bind(now_ms())
        .bind(moved.and_then(|m| m.from_id.as_deref()))
        .bind(moved.map(|m| m.from_name.as_str()))
        .bind(moved.map(|m| m.reason.as_str()))
        .bind(moved.and_then(|m| m.tool).map(Tool::as_str))
        .bind(moved.map(|m| m.at))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// The tickets from Backlog through Ready for Review with a row on this agent, by key (in use, d0086).
    pub async fn unfinished_tickets_on_agent(
        &self,
        id: &str,
    ) -> Result<Vec<TicketKey>, StoreError> {
        let columns = UNFINISHED.map(Column::as_str);
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT DISTINCT p.key, t.number, t.child_number FROM ticket_agent_rows r
             JOIN tickets t ON t.id = r.ticket_id JOIN projects p ON p.id = t.project_id
             WHERE r.named_agent_id = ? AND t.board_column IN (?, ?, ?, ?)
             ORDER BY p.key, t.number, t.child_number",
        )
        .bind(id)
        .bind(columns[0])
        .bind(columns[1])
        .bind(columns[2])
        .bind(columns[3])
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(project, number, child)| {
                let project = ProjectKey::new(&project).ok()?;
                let number = u32::try_from(number).ok()?;
                Some(match u32::try_from(child).ok().filter(|c| *c > 0) {
                    Some(child) => TicketKey::child(project, number, child),
                    None => TicketKey::new(project, number),
                })
            })
            .collect())
    }

    /// The names of the entries whose fallback is this one (M23's column; in use already).
    pub async fn fallback_of(&self, id: &str) -> Result<Vec<String>, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT name FROM named_agents WHERE fallback_id = ? ORDER BY created_at, id",
        )
        .bind(id)
        .fetch_all(self.pool())
        .await?)
    }

    /// Whether the ticket's agent has started (the Developer row locks): it has a developer session that didn't end
    /// in "not started" (its first turn refused on the model, T-0337, or the tool ended before its conversation's id
    /// arrived, T-0349), when only Retry start or another agent starts it again.
    pub async fn developer_started(&self, ticket_id: &str) -> Result<bool, StoreError> {
        Ok(sqlx::query_scalar(concat!(
            "SELECT EXISTS (SELECT 1 FROM sessions WHERE ticket_id = ? AND kind = 'agent' AND ",
            started!(""),
            ")"
        ))
        .bind(ticket_id)
        .bind(END_MODEL_REFUSED)
        .fetch_one(self.pool())
        .await?)
    }

    /// Whether a validation round of the ticket is running (the Validator row locks): its validator started and the
    /// round hasn't ended. A round still waiting for the usage limit, or whose validator didn't start, has no
    /// validator yet: the row resolves when it starts.
    pub async fn validation_running(&self, ticket_id: &str) -> Result<bool, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM validations
                            WHERE ticket_id = ? AND ended_at IS NULL AND session_id IS NOT NULL)",
        )
        .bind(ticket_id)
        .fetch_one(self.pool())
        .await?)
    }

    /// Whether the epic's lead has started (opened, or the epic dragged; d0083): the Epic lead row locks. A lead
    /// whose only sessions ended in "not started" (refused on the model, Q-C, or before its conversation's id) hasn't.
    pub async fn lead_started(&self, epic_id: &str) -> Result<bool, StoreError> {
        Ok(sqlx::query_scalar(concat!(
            "SELECT EXISTS (SELECT 1 FROM project_agents a JOIN sessions s ON s.project_agent_id = a.id
                            WHERE a.epic_id = ? AND ",
            started!("s."),
            ")"
        ))
        .bind(epic_id)
        .bind(END_MODEL_REFUSED)
        .fetch_one(self.pool())
        .await?)
    }

    /// The ticket's agent sessions with the named agent each ran on (T-0341's rows and History; the columns T-0336
    /// writes), with what `SessionInfo` doesn't carry: the role, each session's tokens and round outcome, and an
    /// epic's lead sessions: the developer's (agent, answer, PR), the validators', and for an epic its lead's
    /// (the named agent kept on the lead's `project_agents` row when the session has none). Oldest first.
    pub async fn session_agents(
        &self,
        ticket_id: &str,
    ) -> Result<Vec<SessionAgentRun>, StoreError> {
        let rows: Vec<SessionAgentRunRecord> = sqlx::query_as(
            "SELECT s.id, s.kind, s.state, s.started_at, s.ended_at, COALESCE(s.tool, pa.tool) AS tool,
                    COALESCE(s.named_agent_id, pa.named_agent_id) AS named_agent_id,
                    COALESCE(s.named_agent_name, pa.named_agent_name) AS named_agent_name,
                    COALESCE(s.model, pa.model) AS model,
                    COALESCE(s.model_ran, pa.model_ran) AS model_ran,
                    (SELECT SUM(u.input + u.output + COALESCE(u.reasoning, 0)) FROM usage u
                     WHERE u.session_id = s.id) AS tokens,
                    (SELECT v.outcome FROM validations v WHERE v.session_id = s.id
                     ORDER BY v.started_at DESC LIMIT 1) AS round_outcome
             FROM sessions s LEFT JOIN project_agents pa ON pa.id = s.project_agent_id
             WHERE (s.ticket_id = ? OR pa.epic_id = ?)
               AND s.kind IN ('agent', 'answer', 'delivery', 'validator', 'project_agent')
             ORDER BY s.started_at, s.id",
        )
        .bind(ticket_id)
        .bind(ticket_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(SessionAgentRunRecord::into_session)
            .collect())
    }
}

/// Where an agent hold belongs: a ticket's (or epic's) role, or a project's Project agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldScope<'a> {
    Ticket(&'a str),
    Project(&'a str),
}

impl Store {
    /// Holds `hold`'s role (SCH-04 `[M23]`): one row per ticket (or project) and role, replacing an earlier hold of
    /// it. A hold that stays for the same reason keeps its `since`.
    pub async fn set_agent_hold(&self, hold: &AgentHold) -> Result<(), StoreError> {
        let conflict = if hold.ticket_id.is_some() {
            "(ticket_id, role) WHERE ticket_id IS NOT NULL"
        } else {
            "(project_id, role) WHERE project_id IS NOT NULL"
        };
        let sql = format!(
            "INSERT INTO agent_holds (id, ticket_id, project_id, role, reason, tool, model, named_agent_id,
                                      fallback_agent_id, fallback_held, since)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT {conflict} DO UPDATE SET
                 since = CASE WHEN agent_holds.reason = excluded.reason THEN agent_holds.since
                              ELSE excluded.since END,
                 reason = excluded.reason, tool = excluded.tool, model = excluded.model,
                 named_agent_id = excluded.named_agent_id, fallback_agent_id = excluded.fallback_agent_id,
                 fallback_held = excluded.fallback_held"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&hold.id)
            .bind(&hold.ticket_id)
            .bind(&hold.project_id)
            .bind(hold.role.as_str())
            .bind(hold.reason.as_str())
            .bind(hold.tool.as_str())
            .bind(&hold.model)
            .bind(&hold.named_agent_id)
            .bind(&hold.fallback_agent_id)
            .bind(hold.fallback_held)
            .bind(hold.since)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Lifts the hold of `role` in `scope`. Returns whether there was one.
    pub async fn clear_agent_hold(
        &self,
        scope: HoldScope<'_>,
        role: Role,
    ) -> Result<bool, StoreError> {
        let (column, id) = match scope {
            HoldScope::Ticket(id) => ("ticket_id", id),
            HoldScope::Project(id) => ("project_id", id),
        };
        let sql = format!("DELETE FROM agent_holds WHERE {column} = ? AND role = ?");
        let done = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(role.as_str())
            .execute(self.pool())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// The holds of a ticket or an epic, by role.
    pub async fn ticket_holds(&self, ticket_id: &str) -> Result<Vec<AgentHold>, StoreError> {
        self.holds_where("ticket_id = ?", ticket_id).await
    }

    /// The holds of a project's Project agent (its first message).
    pub async fn project_holds(&self, project_id: &str) -> Result<Vec<AgentHold>, StoreError> {
        self.holds_where("project_id = ?", project_id).await
    }

    /// The holds of every ticket and epic of a project, oldest first (the board's cards, T-0362).
    pub async fn project_ticket_holds(
        &self,
        project_id: &str,
    ) -> Result<Vec<AgentHold>, StoreError> {
        self.holds_where(
            "ticket_id IN (SELECT id FROM tickets WHERE project_id = ?)",
            project_id,
        )
        .await
    }

    /// Every hold on a tool, oldest first (the banner's line, Resume and Move waiting to… per tool).
    pub async fn holds_on_tool(&self, tool: Tool) -> Result<Vec<AgentHold>, StoreError> {
        self.holds_where("tool = ?", tool.as_str()).await
    }

    /// Every hold for a reason, oldest first.
    pub async fn holds_for(&self, reason: HoldReason) -> Result<Vec<AgentHold>, StoreError> {
        self.holds_where("reason = ?", reason.as_str()).await
    }

    async fn holds_where(&self, filter: &str, value: &str) -> Result<Vec<AgentHold>, StoreError> {
        let sql = format!("SELECT * FROM agent_holds WHERE {filter} ORDER BY since, id");
        let rows: Vec<AgentHoldRecord> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(value)
            .fetch_all(self.pool())
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(AgentHoldRecord::into_hold)
            .collect())
    }
}

#[derive(Debug, Clone, FromRow)]
struct AgentHoldRecord {
    id: String,
    ticket_id: Option<String>,
    project_id: Option<String>,
    role: String,
    reason: String,
    tool: String,
    model: Option<String>,
    named_agent_id: Option<String>,
    fallback_agent_id: Option<String>,
    fallback_held: bool,
    since: i64,
}

impl AgentHoldRecord {
    /// `None` for a row of a tool this build doesn't know.
    fn into_hold(self) -> Option<AgentHold> {
        Some(AgentHold {
            role: self.role.parse().ok()?,
            reason: self.reason.parse().ok()?,
            tool: self.tool.parse().ok()?,
            id: self.id,
            ticket_id: self.ticket_id,
            project_id: self.project_id,
            model: self.model,
            named_agent_id: self.named_agent_id,
            fallback_agent_id: self.fallback_agent_id,
            fallback_held: self.fallback_held,
            since: self.since,
        })
    }
}

#[derive(Debug, Clone, FromRow)]
struct RowMoveRecord {
    moved_from_agent_id: Option<String>,
    moved_from_name: Option<String>,
    moved_reason: Option<String>,
    moved_tool: Option<String>,
    moved_at: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct ProjectMoveRecord {
    ticket_id: String,
    role: String,
    #[sqlx(flatten)]
    moved: RowMoveRecord,
}

#[derive(Debug, Clone, FromRow)]
struct RunningRecord {
    ticket_id: String,
    kind: String,
    name: String,
    tool: Option<String>,
    model: Option<String>,
}

/// One running session of a ticket ([`Store::running_agent_names`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningAgent {
    pub ticket_id: String,
    pub role: Role,
    /// The named agent's name at start.
    pub name: String,
    /// `None` for a row of a tool this build doesn't know (or written before tools were stored).
    pub tool: Option<Tool>,
    pub model: Option<String>,
}

impl RowMoveRecord {
    fn into_move(self) -> Option<RowMove> {
        Some(RowMove {
            from_id: self.moved_from_agent_id,
            from_name: self.moved_from_name.unwrap_or_default(),
            reason: self.moved_reason?.parse().ok()?,
            tool: self.moved_tool.and_then(|t| t.parse().ok()),
            at: self.moved_at?,
        })
    }
}

/// One agent session of a ticket and the named agent it ran on ([`Store::session_agents`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionAgentRun {
    pub id: String,
    /// Developer (agent, answer and PR sessions), Validator, or Epic lead.
    pub role: Role,
    pub running: bool,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// The tool it ran on (M23: the panel names it once several tools are connected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
    /// The named agent at start: its id (the entry may be gone since) and its name then.
    pub named_agent_id: Option<String>,
    pub named_agent_name: Option<String>,
    /// The model as passed (`None` = tool default) and the exact model the tool reported.
    pub model: Option<String>,
    pub model_ran: Option<String>,
    /// Shown tokens (input, output, reasoning); `None` before any were read.
    pub tokens: Option<u64>,
    /// A validator session's round outcome (`pass`, `fail`, `no_result`, …).
    pub round_outcome: Option<String>,
    /// The exact model moved on since the previous run on the same named agent ([`crate::session::moved_on`]); filled
    /// by `NamedAgents::ticket_agents`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved_on: Option<crate::session::MovedOn>,
}

#[derive(Debug, Clone, FromRow)]
struct SessionAgentRunRecord {
    id: String,
    kind: String,
    state: String,
    started_at: i64,
    ended_at: Option<i64>,
    tool: Option<String>,
    named_agent_id: Option<String>,
    named_agent_name: Option<String>,
    model: Option<String>,
    model_ran: Option<String>,
    tokens: Option<i64>,
    round_outcome: Option<String>,
}

impl SessionAgentRunRecord {
    fn into_session(self) -> SessionAgentRun {
        let role = match self.kind.as_str() {
            "validator" => Role::Validator,
            "project_agent" => Role::EpicLead,
            _ => Role::Developer,
        };
        SessionAgentRun {
            id: self.id,
            role,
            running: self.state == "running",
            started_at: self.started_at,
            ended_at: self.ended_at,
            tool: self.tool.and_then(|t| t.parse().ok()),
            named_agent_id: self.named_agent_id,
            named_agent_name: self.named_agent_name,
            model: self.model,
            model_ran: self.model_ran,
            tokens: self.tokens.and_then(|t| u64::try_from(t).ok()),
            round_outcome: self.round_outcome,
            moved_on: None,
        }
    }
}
