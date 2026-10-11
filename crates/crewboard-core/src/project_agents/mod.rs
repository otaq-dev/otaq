//! Project agents (PAG-01, PAG-04, PAG-06, PAG-07; d0068): `claude` sessions in a project's own folder (the user's
//! checkout) that investigate and file Backlog tickets. Several per project, each opened by the user's click (the
//! go-ahead, d0018), owned by the background service (d0022).
//!
//! - **Open** starts `claude` with the Project agent role and `/crewboard:project-agent`, no ticket
//!   ([`crate::session::launch::launch_project_agent`]). Refused while the plan's usage limit holds (SCH-04). They
//!   never count toward the agent limit, which counts tickets (SCH-03).
//! - **Naming** (PAG-04): "New agent" until named. The agent names itself once with `crewboard agent name`; a second
//!   call needs `--rename` (the user asked for it). If it hasn't by the end of its first turn after the first request
//!   (from the UserPromptSubmit payload), the request's first words become its name. The user's rename always wins:
//!   the agent can't change it.
//! - **Close** ends it. One that never got a request is discarded (no confirmation, not in Recently ended); one that
//!   is mid-turn asks first (`needs_confirmation`, "Stop and close").
//! - **After a restart** (REC-01) running agents are "Ended by restart" and resume on the user's click; those that
//!   never got a request disappear. The **10 most recent** ended agents of a project are listed, each resumable
//!   (`claude --resume`); Claude's transcripts are never deleted.

pub mod prime;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::board::questions::{Choice, HumanQuestion};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::stored_enum;
use crate::domain::{Actor, Column, Status, TicketKey};
use crate::naming;
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::deliveries::{Deliveries, Delivery, IdleSignal};
use crate::session::launch::{
    AgentCommand, LaunchContext, PROJECT_AGENT_SKILL, launch_project_agent, resume_project_agent,
};
use crate::session::{
    END_SERVICE_STOPPED, SessionInfo, SessionKind, Sessions, SpawnSpec, TerminalSize,
};
use crate::signals::{HookEvent, Signal};
use crate::store::projects::Project;
use crate::store::{new_id, now_ms};

/// What an unnamed Project agent is called.
pub const NEW_AGENT: &str = "New agent";

/// How many ended Project agents a project lists (PAG-06).
pub const RECENT: i64 = 10;

/// How many words of the first request make the fallback name, and its longest length in characters.
const FALLBACK_WORDS: usize = 4;
const NAME_MAX_CHARS: usize = 40;

/// Why no Project agent opens while the plan's usage limit holds (SCH-04).
pub const LIMIT_REFUSAL: &str = "Usage limit reached · Project agents open once it resets";

/// `agent name`, and `ask` without a ticket, from anything but a Project agent.
pub const ONLY_A_PROJECT_AGENT: &str = "Only a Project agent can do this";

/// The size a Project agent starts with until a terminal attaches.
const INITIAL_SIZE: TerminalSize = TerminalSize {
    rows: 40,
    cols: 120,
};

stored_enum! {
    /// Where a Project agent's name came from (`project_agents.name_source`).
    pub enum NameSource ("name source") {
        /// `crewboard agent name`.
        Agent = "agent",
        /// The first words of its first request.
        Fallback = "fallback",
        /// The user's rename; never overwritten.
        User = "user",
    }
}

stored_enum! {
    /// A Project agent's state (`project_agents.state`).
    pub enum AgentState ("project agent state") {
        Running = "running",
        /// It ended (closed, or `claude` exited).
        Ended = "ended",
        /// A restart ended it: "Ended by restart · Resume".
        EndedByRestart = "ended_by_restart",
    }
}

/// A Project agent as clients see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgent {
    pub id: String,
    /// The project's key.
    pub project: String,
    /// Its name, or "New agent".
    pub name: String,
    /// Where the name came from; none while it is "New agent".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_source: Option<NameSource>,
    pub state: AgentState,
    /// The conversation Resume continues (d0045).
    pub claude_session_id: String,
    /// It got its first request: closing it ends it instead of discarding it.
    pub got_request: bool,
    /// Its session runs and its turn hasn't ended: closing it asks first.
    pub mid_turn: bool,
    /// The plan's usage limit stopped it; Resume all continues it.
    pub limit_stopped: bool,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    /// The user closed its tab.
    pub closed: bool,
    /// Its open `ask human` question (d0068, d0061).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<HumanQuestion>,
    /// Its newest session (attach to it with `session.attach {session}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionInfo>,
}

/// A project's Project agents: the open ones (tabs) and the most recent ended ones.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentList {
    /// Not closed: running, ended or ended by a restart; oldest first.
    pub open: Vec<ProjectAgent>,
    /// The [`RECENT`] most recently ended ones that got a request, newest first.
    pub recent: Vec<ProjectAgent>,
}

/// What the startup reconcile did (REC-01, REC-06).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupAgents {
    /// Running Project agents that are now "Ended by restart", by id.
    pub ended: Vec<String>,
    /// "New agent"s that never got a request and were discarded.
    pub discarded: u32,
}

/// What a Project agent created and changed (PAG-05): "Created SHOP-35, SHOP-36 · edited SHOP-31 · 2
/// dependencies".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedAndChanged {
    /// The tickets it created that still exist, in creation order.
    pub created: Vec<CreatedTicket>,
    /// Other tickets it edited, in key order.
    pub edited: Vec<TicketKey>,
    /// Dependencies it added.
    pub dependencies: u32,
    /// The one-line summary.
    pub summary: String,
}

/// A ticket a Project agent created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedTicket {
    pub key: TicketKey,
    pub title: String,
    pub status: Status,
    /// Never started: **Delete** removes it (TKT-08, the 0060 confirmation in the app).
    pub deletable: bool,
}

/// Opens and tracks Project agents. Cheap to clone.
#[derive(Clone)]
pub struct ProjectAgents {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    data: DataDir,
    sessions: Sessions,
    launch: Option<LaunchContext>,
    events: EventBus,
    /// Whether a session's turn has ended (the signals' turn state).
    idle: Arc<dyn IdleSignal>,
    /// Types the user's answers.
    deliveries: Deliveries,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    project_key: String,
    name: Option<String>,
    name_source: Option<String>,
    claude_session_id: String,
    state: String,
    first_request: Option<String>,
    limit_stopped: bool,
    created_at: i64,
    ended_at: Option<i64>,
    closed_at: Option<i64>,
}

const SELECT: &str =
    "SELECT a.id, p.key AS project_key, a.name, a.name_source, a.claude_session_id, a.state,
                             a.first_request, a.limit_stopped, a.created_at, a.ended_at, a.closed_at
                      FROM project_agents a JOIN projects p ON p.id = a.project_id";

impl ProjectAgents {
    pub fn new(
        board: Board,
        data: DataDir,
        sessions: Sessions,
        launch: Option<LaunchContext>,
        events: EventBus,
        idle: Arc<dyn IdleSignal>,
        deliveries: Deliveries,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                sessions,
                launch,
                events,
                idle,
                deliveries,
            }),
        }
    }

    /// The startup reconcile (REC-01, d0068): running Project agents ended with the previous service and show
    /// "Ended by restart · Resume"; a "New agent" that never got a request disappears.
    pub async fn startup(&self) -> Result<StartupAgents, BoardError> {
        let pool = self.inner.board.store().pool();
        let discarded = sqlx::query("DELETE FROM project_agents WHERE first_request IS NULL")
            .execute(pool)
            .await?
            .rows_affected();
        let ended: Vec<(String,)> = sqlx::query_as(
            "UPDATE project_agents SET state = ?, ended_at = ? WHERE state = ? RETURNING id",
        )
        .bind(AgentState::EndedByRestart.as_str())
        .bind(now_ms())
        .bind(AgentState::Running.as_str())
        .fetch_all(pool)
        .await?;
        let report = StartupAgents {
            ended: ended.into_iter().map(|(id,)| id).collect(),
            discarded: u32::try_from(discarded).unwrap_or(u32::MAX),
        };
        if !report.ended.is_empty() || report.discarded > 0 {
            tracing::info!(
                ended = report.ended.len(),
                discarded = report.discarded,
                "project agents after the restart"
            );
        }
        Ok(report)
    }

    /// Opens a new Project agent in `project` (PAG-01): the user's click is the go-ahead. Refused while the
    /// usage limit holds.
    pub async fn open(&self, project: &str) -> Result<ProjectAgent, BoardError> {
        let project = self.inner.board.project(project).await?;
        self.refuse_during_limit().await?;
        let launch = self.launch()?;
        let repo = std::path::PathBuf::from(&project.repo_path);
        let settings = self.settings(launch, &project);
        let command = launch_project_agent(launch, &repo, settings.as_deref());
        let id = new_id();
        sqlx::query(
            "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(&command.claude_session_id)
        .bind(AgentState::Running.as_str())
        .bind(now_ms())
        .execute(self.inner.board.store().pool())
        .await?;
        if let Err(e) = self.spawn(&project, command, &id).await {
            sqlx::query("DELETE FROM project_agents WHERE id = ?")
                .bind(&id)
                .execute(self.inner.board.store().pool())
                .await?;
            return Err(e);
        }
        tracing::info!(project = %project.key, agent = id, "project agent opened");
        self.changed(&project.key, &id);
        self.get(&id).await
    }

    /// Resumes an ended Project agent's conversation (Ended by restart · Resume, Recently ended, PAG-06): a new
    /// session with `claude --resume`, the user's click. A running one is returned as it is. Refused while the
    /// usage limit holds.
    pub async fn resume(&self, id: &str) -> Result<ProjectAgent, BoardError> {
        let agent = self.get(id).await?;
        if agent.state == AgentState::Running {
            return Ok(agent);
        }
        self.refuse_during_limit().await?;
        let launch = self.launch()?;
        let project = self.inner.board.project(&agent.project).await?;
        let repo = std::path::PathBuf::from(&project.repo_path);
        let settings = self.settings(launch, &project);
        let command =
            resume_project_agent(launch, &repo, &agent.claude_session_id, settings.as_deref());
        self.set_state(id, AgentState::Running).await?;
        sqlx::query(
            "UPDATE project_agents SET ended_at = NULL, closed_at = NULL, limit_stopped = 0 WHERE id = ?",
        )
        .bind(id)
        .execute(self.inner.board.store().pool())
        .await?;
        if let Err(e) = self.spawn(&project, command, id).await {
            self.set_state(id, agent.state).await?;
            sqlx::query("UPDATE project_agents SET ended_at = ?, closed_at = ? WHERE id = ?")
                .bind(agent.ended_at)
                .bind(agent.closed.then(now_ms))
                .bind(id)
                .execute(self.inner.board.store().pool())
                .await?;
            return Err(e);
        }
        tracing::info!(agent = id, "project agent resumed");
        self.changed(&agent.project, id);
        self.get(id).await
    }

    /// Closes a Project agent (PAG-06). One that never got a request is discarded; a running one mid-turn needs
    /// `confirm` ("Stop and close"); otherwise it ends and leaves the tabs (it stays in Recently ended).
    pub async fn close(&self, id: &str, confirm: bool) -> Result<(), BoardError> {
        let agent = self.get(id).await?;
        if agent.mid_turn && agent.got_request && !confirm {
            return Err(BoardError::refused(
                Refusal::NeedsConfirmation,
                format!(
                    "{} is in the middle of a turn. Stop it and close it?",
                    agent.name
                ),
            ));
        }
        let pool = self.inner.board.store().pool();
        if agent.got_request {
            sqlx::query(
                "UPDATE project_agents SET state = CASE WHEN state = ? THEN ? ELSE state END,
                                           ended_at = COALESCE(ended_at, ?), closed_at = ?
                 WHERE id = ?",
            )
            .bind(AgentState::Running.as_str())
            .bind(AgentState::Ended.as_str())
            .bind(now_ms())
            .bind(now_ms())
            .bind(id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM project_agents WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await?;
        }
        if let Some(session) = agent.session.as_ref().filter(|s| s.is_running()) {
            self.inner
                .sessions
                .stop(&session.id)
                .await
                .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        }
        tracing::info!(
            agent = id,
            discarded = !agent.got_request,
            "project agent closed"
        );
        self.changed(&agent.project, id);
        Ok(())
    }

    /// The user's rename (PAG-04): always wins, and the agent can't change it afterwards.
    pub async fn rename(&self, id: &str, name: &str) -> Result<ProjectAgent, BoardError> {
        let name = clean_name(name)?;
        let agent = self.get(id).await?;
        self.set_name(id, &name, NameSource::User).await?;
        self.changed(&agent.project, id);
        self.get(id).await
    }

    /// `crewboard agent name "<name>"` from the Project agent `id` (PAG-04): once, again only with `rename` (the
    /// user asked it to), never over the user's own name. A fallback name isn't its own, so it may replace it.
    pub async fn name_by_agent(
        &self,
        id: &str,
        name: &str,
        rename: bool,
    ) -> Result<ProjectAgent, BoardError> {
        let name = clean_name(name)?;
        let agent = self.get(id).await?;
        let refuse = |message: String| Err(BoardError::refused(Refusal::NotAllowed, message));
        match agent.name_source {
            Some(NameSource::User) => {
                return refuse(format!(
                    "The user named this Project agent \"{}\"; their name stays",
                    agent.name
                ));
            }
            Some(NameSource::Agent) if !rename => {
                return refuse(format!(
                    "This Project agent is already named \"{}\". Rename it only when the user asks \
                     (`{} agent name --rename \"<name>\"`)",
                    agent.name,
                    naming::COMMAND_NAME
                ));
            }
            _ => {}
        }
        self.set_name(id, &name, NameSource::Agent).await?;
        self.changed(&agent.project, id);
        self.get(id).await
    }

    /// A hook of the Project agent `id` (§11.4 M12 rows), after the signals mapped it to `signal`: its first
    /// request is kept, the fallback name is given at the end of its first turn after it, and the plan limit's
    /// stop is recorded.
    pub async fn hook(
        &self,
        id: &str,
        event: HookEvent,
        payload: &Value,
        signal: &Signal,
    ) -> Result<(), BoardError> {
        let pool = self.inner.board.store().pool();
        let mut changed = false;
        if event == HookEvent::UserPromptSubmit
            && let Some(request) = payload
                .get("prompt")
                .and_then(Value::as_str)
                .and_then(request_text)
        {
            changed |= sqlx::query(
                "UPDATE project_agents SET first_request = ? WHERE id = ? AND first_request IS NULL",
            )
            .bind(&request)
            .bind(id)
            .execute(pool)
            .await?
            .rows_affected()
                > 0;
        }
        if matches!(signal, Signal::Working) {
            changed |= sqlx::query(
                "UPDATE project_agents SET limit_stopped = 0 WHERE id = ? AND limit_stopped = 1",
            )
            .bind(id)
            .execute(pool)
            .await?
            .rows_affected()
                > 0;
        }
        if let Signal::LimitReached { .. } = signal {
            sqlx::query("UPDATE project_agents SET limit_stopped = 1 WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await?;
            changed = true;
        }
        if signal.ends_turn() {
            let unnamed: Option<(Option<String>,)> = sqlx::query_as(
                "SELECT first_request FROM project_agents WHERE id = ? AND name_source IS NULL",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?;
            if let Some((Some(request),)) = unnamed
                && let Some(name) = fallback_name(&request)
            {
                sqlx::query(
                    "UPDATE project_agents SET name = ?, name_source = ? WHERE id = ? AND name_source IS NULL",
                )
                .bind(&name)
                .bind(NameSource::Fallback.as_str())
                .bind(id)
                .execute(pool)
                .await?;
            }
            // Its turn ended: mid-turn changed, and maybe its name.
            changed = true;
        }
        if changed || matches!(signal, Signal::Working) {
            let project: Option<(String,)> = sqlx::query_as(
                "SELECT p.key FROM project_agents a JOIN projects p ON p.id = a.project_id WHERE a.id = ?",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?;
            if let Some((project,)) = project {
                self.changed(&project, id);
            }
        }
        Ok(())
    }

    /// `crewboard ask human --stdin` from the Project agent `id` (PAG-02, d0061): the question, with 2–4 options
    /// or free text, waits on the Project agent for the user's answer.
    pub async fn ask_human(
        &self,
        id: &str,
        question: &str,
        options: &[String],
        recommended: Option<u32>,
    ) -> Result<ProjectAgent, BoardError> {
        let question = question.trim();
        if question.is_empty() {
            return Err(BoardError::refused(
                Refusal::BadQuestion,
                "Write the question on stdin",
            ));
        }
        let agent = self.get(id).await?;
        self.inner
            .board
            .store_agent_question(id, question, options, recommended)
            .await?;
        tracing::info!(agent = id, "project agent asks the user");
        self.changed(&agent.project, id);
        self.get(id).await
    }

    /// The user's answer to the Project agent's question, typed into its session as the user's click (with
    /// unsent text in its input line and no `clear`: `needs_confirmation`, d0048).
    pub async fn answer(
        &self,
        id: &str,
        choice: &Choice,
        clear: bool,
    ) -> Result<Delivery, BoardError> {
        let agent = self.get(id).await?;
        let question = agent.question.as_ref().ok_or_else(|| {
            BoardError::refused(
                Refusal::MoveRefused,
                format!("{} isn't waiting for an answer", agent.name),
            )
        })?;
        let text = crate::attention::inbox::answer_text(Some(question), choice)?;
        let session = agent
            .session
            .as_ref()
            .filter(|s| s.is_running())
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::SessionEnded,
                    format!("{}'s session has ended", agent.name),
                )
            })?;
        let deliveries = &self.inner.deliveries;
        let typed = if clear {
            deliveries.clear_and_send(&session.id, Some(&text)).await
        } else {
            deliveries.deliver_from_click(&session.id, &text).await
        };
        let delivery =
            typed.map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        if matches!(delivery, Delivery::Sent { .. }) {
            self.inner.board.record_answer(&question.id, &text).await?;
            self.changed(&agent.project, id);
        }
        Ok(delivery)
    }

    /// What the Project agent created and changed (PAG-05), from its events.
    pub async fn created_and_changed(&self, id: &str) -> Result<CreatedAndChanged, BoardError> {
        let agent = self.get(id).await?;
        let actor = Actor::ProjectAgent(id.to_owned()).to_string();
        let events: Vec<(String, String)> =
            sqlx::query_as("SELECT ticket_id, kind FROM events WHERE actor = ? ORDER BY at, id")
                .bind(&actor)
                .fetch_all(self.inner.board.store().pool())
                .await?;
        let tickets = self.inner.board.tickets(&agent.project).await?;
        let by_id = |ticket: &str| tickets.iter().find(|t| t.id == ticket);
        let mut out = CreatedAndChanged::default();
        let mut created_ids: Vec<&str> = Vec::new();
        for (ticket, kind) in &events {
            match kind.as_str() {
                "created" => {
                    if let Some(t) = by_id(ticket) {
                        created_ids.push(&t.id);
                        out.created.push(CreatedTicket {
                            key: t.key.clone(),
                            title: t.title.clone(),
                            status: t.status,
                            deletable: t.column == Column::Backlog && !t.has_started(),
                        });
                    }
                }
                "dependency_added" => out.dependencies += 1,
                _ => {}
            }
        }
        for (ticket, kind) in &events {
            if kind == "updated"
                && !created_ids.contains(&ticket.as_str())
                && let Some(t) = by_id(ticket)
                && !out.edited.contains(&t.key)
            {
                out.edited.push(t.key.clone());
            }
        }
        out.edited.sort();
        out.summary = summary(&out);
        Ok(out)
    }

    /// A project's open Project agents and its most recent ended ones.
    pub async fn list(&self, project: &str) -> Result<ProjectAgentList, BoardError> {
        let project = self.inner.board.project(project).await?;
        let pool = self.inner.board.store().pool();
        let open: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE a.project_id = ? AND a.closed_at IS NULL ORDER BY a.created_at, a.id"
        )))
        .bind(&project.id)
        .fetch_all(pool)
        .await?;
        let recent: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE a.project_id = ? AND a.state <> ? AND a.first_request IS NOT NULL
             ORDER BY a.ended_at DESC, a.id DESC LIMIT ?"
        )))
        .bind(&project.id)
        .bind(AgentState::Running.as_str())
        .bind(RECENT)
        .fetch_all(pool)
        .await?;
        let mut list = ProjectAgentList::default();
        for row in open {
            list.open.push(self.agent(row).await?);
        }
        for row in recent {
            list.recent.push(self.agent(row).await?);
        }
        Ok(list)
    }

    /// One Project agent.
    pub async fn get(&self, id: &str) -> Result<ProjectAgent, BoardError> {
        let row: Row = sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE a.id = ?")))
            .bind(id)
            .fetch_optional(self.inner.board.store().pool())
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("no Project agent {id}")))?;
        self.agent(row).await
    }

    /// The running Project agents stopped by the plan limit, with their sessions (Resume all continues them).
    pub async fn limit_stopped(&self) -> Result<Vec<(String, String)>, BoardError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT a.id, s.id FROM project_agents a JOIN sessions s ON s.project_agent_id = a.id
             WHERE a.limit_stopped = 1 AND a.state = ? AND s.ended_at IS NULL",
        )
        .bind(AgentState::Running.as_str())
        .fetch_all(self.inner.board.store().pool())
        .await?;
        Ok(rows)
    }

    async fn agent(&self, row: Row) -> Result<ProjectAgent, BoardError> {
        let newest: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM sessions WHERE project_agent_id = ? ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .bind(&row.id)
        .fetch_optional(self.inner.board.store().pool())
        .await?;
        let session = match newest {
            Some((session,)) => match self.inner.sessions.info(&session) {
                Ok(info) => Some(info),
                Err(_) => self.inner.sessions.stored(&session).await.ok(),
            },
            None => None,
        };
        let mid_turn = session
            .as_ref()
            .is_some_and(|s| s.is_running() && !self.inner.idle.is_idle(&s.id));
        let name_source = row.name_source.and_then(|s| s.parse().ok());
        let question = self.inner.board.open_agent_question(&row.id).await?;
        Ok(ProjectAgent {
            question,
            name: row
                .name
                .filter(|_| name_source.is_some())
                .unwrap_or_else(|| NEW_AGENT.to_owned()),
            name_source,
            state: row.state.parse().unwrap_or(AgentState::Ended),
            id: row.id,
            project: row.project_key,
            claude_session_id: row.claude_session_id,
            got_request: row.first_request.is_some(),
            mid_turn,
            limit_stopped: row.limit_stopped,
            created_at: row.created_at,
            ended_at: row.ended_at,
            closed: row.closed_at.is_some(),
            session,
        })
    }

    async fn spawn(
        &self,
        project: &Project,
        command: AgentCommand,
        id: &str,
    ) -> Result<SessionInfo, BoardError> {
        let sessions = &self.inner.sessions;
        let session = sessions
            .spawn(SpawnSpec {
                kind: SessionKind::ProjectAgent,
                project_id: project.id.clone(),
                project_key: project.key.clone(),
                ticket: None,
                program: command.program,
                args: command.args,
                cwd: command.cwd,
                env: command.env,
                size: INITIAL_SIZE,
                claude_session_id: Some(command.claude_session_id),
            })
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        sessions
            .set_project_agent(&session.id, id)
            .await
            .map_err(|e| BoardError::refused(Refusal::SessionEnded, e.to_string()))?;
        // `claude` exiting on its own ends the agent; the service stopping doesn't (the next start reconciles).
        let this = self.clone();
        let (session_id, id, project) = (session.id.clone(), id.to_owned(), project.key.clone());
        tokio::spawn(async move {
            let Some(ended) = this.inner.sessions.wait_ended(&session_id).await else {
                return;
            };
            if ended.end_reason.as_deref() == Some(END_SERVICE_STOPPED) {
                return;
            }
            let done = sqlx::query(
                "UPDATE project_agents SET state = ?, ended_at = ?
                 WHERE id = ? AND state = ?
                   AND ? = (SELECT id FROM sessions WHERE project_agent_id = ?
                            ORDER BY started_at DESC, id DESC LIMIT 1)",
            )
            .bind(AgentState::Ended.as_str())
            .bind(now_ms())
            .bind(&id)
            .bind(AgentState::Running.as_str())
            .bind(&session_id)
            .bind(&id)
            .execute(this.inner.board.store().pool())
            .await;
            match done {
                Ok(_) => this.changed(&project, &id),
                Err(e) => tracing::warn!(agent = id, error = %e, "can't record the agent's end"),
            }
        });
        Ok(session)
    }

    async fn set_name(&self, id: &str, name: &str, source: NameSource) -> Result<(), BoardError> {
        sqlx::query("UPDATE project_agents SET name = ?, name_source = ? WHERE id = ?")
            .bind(name)
            .bind(source.as_str())
            .bind(id)
            .execute(self.inner.board.store().pool())
            .await?;
        Ok(())
    }

    async fn set_state(&self, id: &str, state: AgentState) -> Result<(), BoardError> {
        sqlx::query("UPDATE project_agents SET state = ? WHERE id = ?")
            .bind(state.as_str())
            .bind(id)
            .execute(self.inner.board.store().pool())
            .await?;
        Ok(())
    }

    async fn refuse_during_limit(&self) -> Result<(), BoardError> {
        let held = crate::usage::plan::latest(self.inner.board.store())
            .await?
            .and_then(|s| s.limit)
            .is_some_and(|l| l.reset_seen_at.is_none());
        if held {
            return Err(BoardError::refused(Refusal::MoveRefused, LIMIT_REFUSAL));
        }
        Ok(())
    }

    fn launch(&self) -> Result<&LaunchContext, BoardError> {
        self.inner.launch.as_ref().ok_or_else(|| {
            BoardError::refused(
                Refusal::NotAvailableYet,
                format!(
                    "{}'s Claude Code plugin wasn't found next to the {} command",
                    naming::PRODUCT_NAME,
                    naming::COMMAND_NAME
                ),
            )
        })
    }

    /// The status-line settings file the project's Project agents share (USG-04).
    fn settings(&self, launch: &LaunchContext, project: &Project) -> Option<std::path::PathBuf> {
        crate::session::statusline::write_settings_at(
            crate::session::statusline::project_agent_settings_path(&self.inner.data, &project.key),
            std::path::Path::new(&project.repo_path),
            launch.claude_config_dir.as_deref(),
            &launch.bin_dir,
        )
        .inspect_err(
            |e| tracing::warn!(project = %project.key, error = %e, "can't write the status-line settings"),
        )
        .ok()
    }

    fn changed(&self, project: &str, id: &str) {
        self.inner.events.publish(Event::ProjectAgentChanged {
            project: project.to_owned(),
            id: id.to_owned(),
        });
    }
}

/// "Created SHOP-35, SHOP-36 · edited SHOP-31 · 2 dependencies"; "Nothing created or changed yet".
fn summary(c: &CreatedAndChanged) -> String {
    let keys = |keys: Vec<&TicketKey>| {
        keys.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut parts = Vec::new();
    if !c.created.is_empty() {
        parts.push(format!(
            "Created {}",
            keys(c.created.iter().map(|t| &t.key).collect())
        ));
    }
    if !c.edited.is_empty() {
        parts.push(format!("edited {}", keys(c.edited.iter().collect())));
    }
    match c.dependencies {
        0 => {}
        1 => parts.push("1 dependency".to_owned()),
        n => parts.push(format!("{n} dependencies")),
    }
    if parts.is_empty() {
        return "Nothing created or changed yet".to_owned();
    }
    let mut text = parts.join(" · ");
    if let Some(first) = text.get(..1) {
        text = first.to_uppercase() + &text[1..];
    }
    text
}

/// The request in a UserPromptSubmit prompt: the prompt without the skill invocation that starts the session
/// (`/crewboard:project-agent`, alone it is no request). `None` when nothing is left.
pub fn request_text(prompt: &str) -> Option<String> {
    let prompt = prompt.trim();
    let skill = naming::skill(PROJECT_AGENT_SKILL);
    let rest = prompt.strip_prefix(&skill).unwrap_or(prompt).trim();
    (!rest.is_empty()).then(|| rest.to_owned())
}

/// The fallback name (PAG-04): the request's first words, at most 40 characters, without trailing
/// punctuation. `None` for a request without words.
pub fn fallback_name(request: &str) -> Option<String> {
    let words: Vec<&str> = request.split_whitespace().take(FALLBACK_WORDS).collect();
    let mut name: String = words.join(" ").chars().take(NAME_MAX_CHARS).collect();
    name = name
        .trim_end_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
        .to_owned();
    (!name.is_empty()).then_some(name)
}

/// A name as given: one line, spaces collapsed, at most 40 characters; refused when empty.
fn clean_name(name: &str) -> Result<String, BoardError> {
    let name: String = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(NAME_MAX_CHARS)
        .collect();
    if name.is_empty() {
        return Err(BoardError::refused(
            Refusal::EmptyField,
            "A Project agent's name can't be empty",
        ));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_invocation_is_no_request() {
        let skill = naming::skill(PROJECT_AGENT_SKILL);
        assert_eq!(request_text(&skill), None);
        assert_eq!(request_text(&format!("  {skill}  ")), None);
        assert_eq!(
            request_text(&format!("{skill} why is checkout slow?")).as_deref(),
            Some("why is checkout slow?")
        );
        assert_eq!(
            request_text("Import the GitHub issues labelled ready").as_deref(),
            Some("Import the GitHub issues labelled ready")
        );
        assert_eq!(request_text("   "), None);
    }

    #[test]
    fn the_fallback_is_the_first_words() {
        assert_eq!(
            fallback_name("Why is checkout so slow? Make tickets for it").as_deref(),
            Some("Why is checkout so")
        );
        assert_eq!(
            fallback_name("Footer audit.").as_deref(),
            Some("Footer audit")
        );
        assert_eq!(
            fallback_name("Supercalifragilisticexpialidocious-and-then-some-more-text")
                .map(|n| n.len()),
            Some(40)
        );
        assert_eq!(fallback_name(" ?! "), None);
    }

    #[test]
    fn the_summary_names_what_it_did() {
        let key = |k: &str| k.parse::<TicketKey>().unwrap();
        let created = |k: &str| CreatedTicket {
            key: key(k),
            title: "t".into(),
            status: Status::Backlog,
            deletable: true,
        };
        let mut c = CreatedAndChanged::default();
        assert_eq!(summary(&c), "Nothing created or changed yet");
        c.created = vec![created("SHOP-35"), created("SHOP-36")];
        c.edited = vec![key("SHOP-31")];
        c.dependencies = 2;
        assert_eq!(
            summary(&c),
            "Created SHOP-35, SHOP-36 · edited SHOP-31 · 2 dependencies"
        );
        c.created.clear();
        c.dependencies = 1;
        assert_eq!(summary(&c), "Edited SHOP-31 · 1 dependency");
    }

    #[test]
    fn names_are_one_line_and_not_empty() {
        assert_eq!(
            clean_name("  Checkout \n slowness ").unwrap(),
            "Checkout slowness"
        );
        assert_eq!(
            clean_name("  ").unwrap_err().refusal(),
            Some(Refusal::EmptyField)
        );
    }
}
