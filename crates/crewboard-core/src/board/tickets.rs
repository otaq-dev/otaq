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
use crate::domain::{
    Actor, Column, Flags, Kind, ProjectKey, StartStep, Status, TicketKey, stored_enum,
};
use crate::store::projects::Project;
use crate::store::tickets::TicketRow;
use crate::store::{new_id, now_ms};

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
    /// Old keys that still resolve to this ticket (after Move to epic, d0044).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<TicketKey>,
    /// Epics only: the number the next child gets (child numbers are never reused, d0039).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_child_number: Option<u32>,
    pub flags: Flags,
    pub created_by: Actor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<Actor>,
    /// When the ticket first went to Dev; `None` for a ticket that never did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    /// When the ticket entered its current status (the age of a needs-you item).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_since: Option<i64>,
    /// When the user's drag last put the ticket in Dev · Queued; ready tickets start oldest first (SCH-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_at: Option<i64>,
    /// Where the ticket works once started: branch, worktree, ports (T-0046, T-0048).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<Work>,
    /// Why a Queued ticket is held or its start stopped halfway (SES-01, SES-02).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_problem: Option<StartProblem>,
    /// Interrupted only: the orange status it had when its session ended (REC-01, d0045). The label, its text
    /// (`signal`) and the question stay with the ticket, so answering from the inbox resumes and delivers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupted_from: Option<Status>,
    /// What the agent waits for or runs in the background (orange family and Working in background, d0042).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<AgentSignal>,
    /// Asks you: the open question with its answer options (d0061).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<super::questions::HumanQuestion>,
    /// After a push: the PR branch's content differs from what was reviewed (REV-06, T-0145).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_differs: Option<crate::pr::verify::PushedDiff>,
}

/// The detail of an orange or background status: SHOP-23's question, SHOP-22's prompt, the agent's last line,
/// or "background: test run, 1 helper agent".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSignal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// When this status began (the card's "· 6 min").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<i64>,
    /// Working in background with a scheduled wakeup: when it fires ("wakes up at 4:10pm").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wakeup_at: Option<i64>,
}

/// A started ticket's branch, worktree and ports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work {
    /// `crewboard/SHOP-18` (the real name is chosen at the first push, M10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// Where the ticket left its base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    /// `$PORT` in the agent's session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_port: Option<u16>,
    /// `$PORT` in Run and Shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_port: Option<u16>,
    /// The branch was pushed: its name can't change any more (GIT-07).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub branch_locked: bool,
    /// The pull request the skill reported (`crewboard pr opened`), for Open PR in browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
}

/// The step a start stopped at, and why: "setup command failed" + the command's last lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartProblem {
    pub step: StartStep,
    pub message: String,
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

    /// Orange as the user sees it: the agent waits and no terminal of the ticket has focus (ATT-03, d0042).
    pub fn shows_orange(&self) -> bool {
        self.status.is_orange() && !self.flags.your_turn
    }

    /// Whether the ticket waits for the user where they can see it: orange (unless its terminal has focus) or
    /// the inbox's second group. Such cards float to the top of Dev and count in the project's needs-you number.
    pub fn needs_user(&self) -> bool {
        super::order::needs_user(self.status) && !self.flags.your_turn
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
    /// Creates an epic child: the epic's key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,
    /// Overrides the project's base branch (standalone tickets and epics only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

/// Changes to a ticket's fields. Absent fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
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
        BaseBranch = "base_branch",
        ExternalId = "external_id",
    }
}

/// One recorded change: the before/after view of agent edits reads these (TKT-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: Field,
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
        /// The start sequence finished: the agent is working (SES-01).
        Started = "started",
        /// The start stopped at a step (SES-02).
        StartInterrupted = "start_interrupted",
        /// Held in Queued: the base diverged (d0059).
        Held = "held",
        /// The user clicked Retry start.
        StartRetried = "start_retried",
        /// Something worth knowing about a start that didn't stop it (a dirty checkout left the base behind).
        StartNote = "start_note",
        /// `crewboard review submit`: the payload has the summary (T-0072).
        ReviewSubmitted = "review_submitted",
        /// Send back: the comments and note went to the agent; the payload has the round (T-0086).
        SentBack = "sent_back",
        /// Local merge: Done · Merged; the payload has the target and the squash commit (T-0089).
        Merged = "merged",
        /// Send to agent to fix: back to Dev to resolve a conflict with the target (T-0090).
        SentToFix = "sent_to_fix",
        /// Reopen: Done · Closed → Backlog (T-0101).
        Reopened = "reopened",
        /// Park: Dev → Backlog with the work kept (T-0104).
        Parked = "parked",
        /// The rebase of a reopened ticket's branch onto the current base, on its way to Dev (GIT-06).
        Rebased = "rebased",
        /// The agent's session ended while it was at work: Interrupted (REC-01). The payload has the reason
        /// (`restart` or the session's end) and the status before.
        Interrupted = "interrupted",
        /// A Ready for Review or Approved ticket's session ended with a restart; not Interrupted (REC-01, d0045).
        SessionEnded = "session_ended",
        /// The user clicked Resume, Resume all or Retry (REC-01, REC-03).
        Resumed = "resumed",
        /// `claude --resume` couldn't find the conversation (REC-03); the payload has the Claude session ID.
        ResumeFailed = "resume_failed",
        /// Start from scratch after a failed resume: a new conversation in the same worktree; the payload has the
        /// old Claude session ID (REC-03).
        StartedFromScratch = "started_from_scratch",
        /// "This is the usage limit" (USG-07): the user folded an Idle agent into the usage-limit banner.
        LimitMarked = "limit_marked",
        /// `crewboard branch rename` (GIT-07): the payload has `from` and `to`.
        BranchRenamed = "branch_renamed",
        /// The Open PR / Push skill proposed the PR (REV-10): the payload has the branch and the title.
        PrProposed = "pr_proposed",
        /// The skill reported the PR (`crewboard pr opened <url>`): the payload has `url` and `number`.
        PrOpened = "pr_opened",
        /// The user confirmed the PR in the dialog; Crewboard applied the edits (REV-10). Payload: kind, branch,
        /// title.
        PrConfirmed = "pr_confirmed",
        /// The user cancelled the Open PR / Push to PR flow before anything was pushed.
        PrCancelled = "pr_cancelled",
        /// An agent asked a linked ticket (MSG-01); logged on both: payload `id`, `to` or `from`, `question`.
        QuestionAsked = "question_asked",
        /// The question was answered; logged on both: payload `id`, `with`, `answer`.
        QuestionAnswered = "question_answered",
    }
}

/// One entry of a ticket's history (the `events` log): what happened, who did it, when (History tab).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub kind: EventKind,
    pub actor: Actor,
    pub at: i64,
    /// The event's details (moves: from/to; starts: session and Claude session ID; …).
    #[serde(default)]
    pub payload: Value,
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
        // A new child returns an epic in Review to Backlog until it is merged (EPC-05).
        if let Some(epic) = &ticket.epic {
            self.epic_back_to_backlog(&epic.to_string(), actor).await?;
        }
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
                changes.push(change(Field::Title, Some(&title), Some(&new)));
                title = new;
            }
        }
        let mut body = ticket.body.clone();
        if let Some(new) = &update.body
            && *new != body
        {
            changes.push(change(Field::Body, Some(&body), Some(new)));
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
                changes.push(change(Field::BaseBranch, base_branch.as_deref(), Some(new)));
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
                    external_id.as_deref(),
                    new.as_deref(),
                ));
                external_id = new;
            }
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

    /// The ticket's history, newest first (History tab, d0045).
    pub async fn history(&self, key: &str) -> Result<Vec<HistoryEvent>, BoardError> {
        let id = self.ticket_id(key).await?;
        let rows: Vec<(String, String, i64, String)> = sqlx::query_as(
            "SELECT kind, actor, at, payload_json FROM events WHERE ticket_id = ? ORDER BY at DESC, id DESC",
        )
        .bind(id)
        .fetch_all(self.store.pool())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(kind, actor, at, payload)| {
                Some(HistoryEvent {
                    kind: kind.parse().ok()?,
                    actor: actor.parse().expect("infallible"),
                    at,
                    payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
                })
            })
            .collect())
    }

    pub async fn ticket_by_id(&self, id: &str) -> Result<Ticket, BoardError> {
        let sql = format!("{SELECT_TICKETS} WHERE t.id = ?");
        let record: TicketRecord = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(self.store.pool())
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("unknown ticket id {id}")))?;
        let project_id = record.row.project_id.clone();
        // Boxed: assembling is a large future, and nearly every request awaits it (keeps callers' frames small).
        let mut tickets = Box::pin(self.assemble(vec![record], &project_id)).await?;
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
        Box::pin(self.assemble(records, &project.id)).await
    }

    /// Turns rows of one project into tickets, with their dependencies.
    async fn assemble(
        &self,
        records: Vec<TicketRecord>,
        project_id: &str,
    ) -> Result<Vec<Ticket>, BoardError> {
        let pool = self.store.pool();
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
        let aliases: Vec<(String, String)> = sqlx::query_as(
            "SELECT a.ticket_id, a.old_key FROM ticket_key_aliases a
             JOIN tickets t ON t.id = a.ticket_id WHERE t.project_id = ? ORDER BY a.old_key",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        let waiting: std::collections::HashMap<String, i64> = sqlx::query_as(
            "SELECT s.ticket_id, COUNT(*) FROM pending_deliveries d JOIN sessions s ON s.id = d.session_id
             WHERE s.project_id = ? AND s.ticket_id IS NOT NULL GROUP BY s.ticket_id",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
        // "answering SHOP-40.2" (MSG-03): the asker of the question a running answer session serves.
        let answering: std::collections::HashMap<String, String> = sqlx::query_as(
            "SELECT m.to_ticket_id, m.from_ticket_id FROM messages m
             JOIN tickets t ON t.id = m.to_ticket_id
             WHERE t.project_id = ? AND m.delivery = 'resumed' AND m.answer IS NULL
               AND EXISTS (SELECT 1 FROM sessions s WHERE s.ticket_id = m.to_ticket_id AND s.kind = 'answer'
                                                     AND s.state = 'running')
             ORDER BY m.asked_at DESC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
        // "asked SHOP-40.1 · waiting" (MSG-06): open questions from each ticket, oldest first, until the answer was
        // typed into the asker's session.
        let mut asked: std::collections::HashMap<String, (String, String, u32)> =
            std::collections::HashMap::new();
        let open: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT m.from_ticket_id, m.to_ticket_id, m.question FROM messages m
             JOIN tickets t ON t.id = m.from_ticket_id
             WHERE t.project_id = ? AND m.answer_delivered_at IS NULL ORDER BY m.asked_at, m.id",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        for (from, to, question) in open {
            asked
                .entry(from)
                .and_modify(|(_, _, more)| *more += 1)
                .or_insert((to, question, 0));
        }
        // Sent comments per ticket and round, for "Sent back · 3 comments · round 2".
        let sent_comments: std::collections::HashMap<(String, i64), i64> = sqlx::query_as(
            "SELECT c.ticket_id, c.round, COUNT(*) FROM review_comments c JOIN tickets t ON t.id = c.ticket_id
             WHERE t.project_id = ? AND c.sent_at IS NOT NULL GROUP BY c.ticket_id, c.round",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id, round, n): (String, i64, i64)| ((id, round), n))
        .collect();
        // The latest review submit of each ticket, for "Submitted by you" (d0052).
        let submits: std::collections::HashMap<String, (String, String)> = sqlx::query_as(
            "SELECT e.ticket_id, e.actor, e.payload_json FROM events e JOIN tickets t ON t.id = e.ticket_id
             WHERE t.project_id = ? AND e.kind = ? ORDER BY e.at, e.id",
        )
        .bind(project_id)
        .bind(EventKind::ReviewSubmitted.as_str())
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id, actor, payload): (String, String, String)| (id, (actor, payload)))
        .collect();
        let mut questions: std::collections::HashMap<String, super::questions::HumanQuestion> =
            std::collections::HashMap::new();
        for (ticket_id, question) in self.open_questions(project_id).await? {
            questions.entry(ticket_id).or_insert(question);
        }
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
                let ticket_aliases = aliases
                    .iter()
                    .filter(|(id, _)| *id == row.id)
                    .filter_map(|(_, old)| old.parse().ok())
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
                let sent_back = row.sent_back_at.map(|_| crate::domain::SentBack {
                    comments: sent_comments
                        .get(&(row.id.clone(), row.sent_back_round))
                        .and_then(|n| u32::try_from(*n).ok())
                        .unwrap_or(0),
                    round: u32::try_from(row.sent_back_round).unwrap_or(0),
                });
                let submitted_by_you = submits
                    .get(&row.id)
                    .filter(|_| row.state == Status::ReadyForReview.as_str())
                    .filter(|(actor, _)| actor.parse::<Actor>().is_ok_and(|a| !a.is_agent()))
                    .map(|(_, payload)| {
                        let summary = serde_json::from_str::<Value>(payload)
                            .ok()
                            .and_then(|p| p.get("summary")?.as_str().map(str::to_owned))
                            .unwrap_or_default();
                        super::review::user_note(&summary)
                    });
                let mut flags = Flags {
                    your_turn: status.is_orange() && self.focus.is_focused(&row.id),
                    run_port: self.run_ports().get(&row.id),
                    resuming_conversation: (row.parked
                        || row.return_mode.as_deref() == Some(crate::recovery::RETURN_RESUME))
                        && status.column() == Column::Dev,
                    submitted_by_you,
                    conflicts_with: row.conflicts_with.clone(),
                    sent_back,
                    pinned_at: row.pinned_at,
                    pr_number: row.pr_number.and_then(|n| u32::try_from(n).ok()),
                    pr_rework: row.pr_rework && status == Status::ReadyForReview,
                    updated_by_agent: agent_edited.contains(&row.id),
                    last_signal_at: row.last_signal_at,
                    answering: answering
                        .get(&row.id)
                        .and_then(|from| key_of.get(from.as_str()).cloned()),
                    asked: asked
                        .get(&row.id)
                        .and_then(|(to, _, _)| key_of.get(to.as_str()).cloned()),
                    asked_question: asked.get(&row.id).map(|(_, q, _)| q.clone()),
                    asked_more: asked.get(&row.id).map_or(0, |(_, _, more)| *more),
                    messages_waiting: waiting
                        .get(&row.id)
                        .and_then(|n| u32::try_from(*n).ok())
                        .unwrap_or(0),
                    ..Flags::default()
                };
                let port = |p: Option<i64>| p.and_then(|p| u16::try_from(p).ok());
                let work = Work {
                    branch: row.branch_name,
                    worktree: row.worktree_path,
                    base_sha: row.base_sha,
                    agent_port: port(row.agent_port),
                    run_port: port(row.run_port),
                    branch_locked: row.branch_pushed,
                    pr_url: row.pr_url.clone(),
                };
                // While a start runs, its current step is marked here too (no message yet); that isn't a problem.
                let start_problem = row
                    .start_problem_step
                    .as_deref()
                    .filter(|_| matches!(status, Status::Held | Status::StartInterrupted))
                    .and_then(|s| s.parse().ok())
                    .map(|step| StartProblem {
                        step,
                        message: row.start_problem_message.unwrap_or_default(),
                    });
                let interrupted_from = row
                    .interrupted_from
                    .as_deref()
                    .filter(|_| status == Status::Interrupted)
                    .and_then(|s| s.parse::<Status>().ok())
                    .filter(|s| s.is_orange());
                let signal = (status.is_orange()
                    || status == Status::WorkingInBackground
                    || interrupted_from.is_some())
                .then_some(AgentSignal {
                    text: row.signal_text,
                    since: row.signal_since,
                    wakeup_at: row.wakeup_at,
                });
                let question = (status == Status::AsksYou
                    || interrupted_from == Some(Status::AsksYou))
                .then(|| questions.get(&row.id).cloned())
                .flatten();
                let pushed_differs: Option<crate::pr::verify::PushedDiff> = row
                    .pushed_differs
                    .as_deref()
                    .and_then(|j| serde_json::from_str(j).ok());
                if pushed_differs.is_some() {
                    flags.changed_since_review = true;
                }
                Ok(Ticket {
                    pushed_differs,
                    interrupted_from,
                    question,
                    signal,
                    work: (work != Work::default()).then_some(work),
                    start_problem,
                    key,
                    kind,
                    title: row.title,
                    body: row.body,
                    epic,
                    column: status.column(),
                    status,
                    base,
                    base_locked: row.branch_locked,
                    external_id: row.external_id,
                    depends_on,
                    aliases: ticket_aliases,
                    next_child_number: (kind == Kind::Epic)
                        .then(|| u32::try_from(row.next_child_number).ok())
                        .flatten(),
                    flags,
                    created_by: row.created_by.parse().expect("infallible"),
                    updated_by: row.updated_by.map(|a| a.parse().expect("infallible")),
                    started_at: row.started_at,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    status_since: row.state_since,
                    queued_at: row.queued_at,
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

fn change(field: Field, before: Option<&str>, after: Option<&str>) -> FieldChange {
    FieldChange {
        field,
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
