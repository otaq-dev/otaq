//! The check before every launch (SCH-04 `[M23]`, AGT-05, AGT-08, USG-10; d0086, d0088, d0018; T-0357).
//!
//! Every **new session** of a role runs it right after M22's resolution ([`super::resolve`]): the Developer's start
//! (and Retry start), each Validator round, an epic lead's start (Open lead, the drag), a Project agent's first
//! message. **Resumes are never checked** (Send back, Park → Dev, Resume, the PR resume, Answer): a conversation
//! can't move tools, so a resume on a tool that isn't available or is at its limit fails the existing way.
//!
//! The checks, in order ([`check`]):
//! 1. **the tool available?** (its state on Settings → Agent tools) — no: `tool_unavailable` with the tool's
//!    reason ("logged out", "not installed", "hidden in Settings", "update needed");
//! 2. **the tool at its limit?** (an active limit of that tool not reset yet) — yes: `tool_limit`;
//! 3. **the model listed?** (Otaq's copy of the tool's list: an exact id or a follow choice must be in it, a family
//!    choice must resolve; an empty model always passes; a tool whose list was never read passes, except a family
//!    choice, which has nothing to resolve to) — no: `model_unavailable`, with the entry marked "Codex no longer
//!    lists gpt-5.4" ([`super::mark::no_longer_lists`]). A follow choice goes to the tool already resolved.
//!
//! For a limit or an unavailable tool, the named agent's **fallback** (one step: its own fallback is never used;
//! never for a model, never for a Project agent, Q-B) that passes the checks itself takes the work when "Use fallback
//! agents automatically" is on: the row **moves** to it, recording where from and why ([`super::RowMove`]), even a
//! row *set by you* (who set it stays; a default row becomes Otaq's pick), and the session launches there. Otherwise the role is **held** ([`super::AgentHold`]): with the
//! fallback offered (Use fallback), with "both held" when the fallback is blocked too, or with nothing to offer
//! (Change agent). A held Developer stays in Ready for Dev (Held, its step naming the reason); nothing locks.
//!
//! **A hold lifts** when its cause may have gone ([`LaunchGate::run`]): a tool's state changes, a limit is resumed or
//! resets, a list is read again, an entry, a default or a row changes, the automatic fallback is turned on, or the
//! user's Use fallback. Each lift re-runs the check once for every hold ([`LaunchGate::lift`]); the held start then
//! runs as the drag already asked for it (Q-D). Nothing retries on a timer.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Mutex, watch};

use super::{
    AgentHold, CameFrom, HoldReason, MoveReason, NamedAgent, Resolved, Role, RowMove, RowSetBy,
    RunsOn, mark, resolve_project_role, resolve_ticket_role,
};
use crate::agent_tools::Tool;
use crate::agent_tools::connected::{ConnectedTools, NotAvailable};
use crate::agent_tools::follow::FollowChoice;
use crate::agent_tools::models::{ModelCopy, ModelLists};
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::domain::{Actor, Column, StartStep, Status, TicketKey};
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::SessionAgent;
use crate::store::named_agents::HoldScope;
use crate::store::settings::AUTO_FALLBACK;
use crate::store::tickets::set_state_since;
use crate::store::{new_id, now_ms};

/// What the checks read at one moment: built by the service from its tool states, limits and lists
/// ([`LaunchGate::facts`]); tests build it directly.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// The tools that can't start an agent now, with why ("logged out").
    pub unavailable: HashMap<Tool, String>,
    /// The tools at their limit: an active limit that hasn't reset (USG-03).
    pub at_limit: HashSet<Tool>,
    /// Otaq's copy of each tool's list that was ever read, with its follow entries.
    pub lists: HashMap<Tool, ModelCopy>,
}

/// Why a named agent can't start now, in the order the checks run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// Its tool isn't available, with the tool's reason.
    Unavailable { reason: String },
    /// Its tool is at its limit.
    AtLimit,
    /// Its model isn't in its tool's list any more.
    ModelUnlisted,
}

impl Blocked {
    pub fn reason(&self) -> HoldReason {
        match self {
            Self::Unavailable { .. } => HoldReason::ToolUnavailable,
            Self::AtLimit => HoldReason::ToolLimit,
            Self::ModelUnlisted => HoldReason::ModelUnavailable,
        }
    }

    /// Why a row moves for it; `None` for a model (no fallback for it).
    pub fn move_reason(&self) -> Option<MoveReason> {
        match self {
            Self::Unavailable { .. } => Some(MoveReason::ToolUnavailable),
            Self::AtLimit => Some(MoveReason::ToolLimit),
            Self::ModelUnlisted => None,
        }
    }
}

/// The model `model` (a named agent's, as stored) passes to its tool, by `copy` (Otaq's copy of the tool's list, if
/// it was ever read): `Some(None)` passes no model option, `Some(Some(id))` passes `id` (a family choice resolved to
/// its newest listed model), `None` when the list doesn't have it (Model unavailable).
pub fn passed_model(model: Option<&str>, copy: Option<&ModelCopy>) -> Option<Option<String>> {
    let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) else {
        return Some(None);
    };
    let family = matches!(
        FollowChoice::parse(model),
        Some(FollowChoice::Newest { .. })
    );
    match copy {
        Some(copy) if family => copy.resolve_follow(model).map(Some),
        Some(copy) => copy.list.contains(model).then(|| Some(model.to_owned())),
        // Never block on a list that was never read (d0086), but a family has nothing to resolve to without one.
        None if family => None,
        None => Some(Some(model.to_owned())),
    }
}

/// The checks before a launch on `agent`, in order: its tool available, not at its limit, its model listed. The
/// model to pass when they pass.
pub fn check(agent: &NamedAgent, facts: &Facts) -> Result<Option<String>, Blocked> {
    if let Some(reason) = facts.unavailable.get(&agent.tool) {
        return Err(Blocked::Unavailable {
            reason: reason.clone(),
        });
    }
    if facts.at_limit.contains(&agent.tool) {
        return Err(Blocked::AtLimit);
    }
    passed_model(agent.model.as_deref(), facts.lists.get(&agent.tool)).ok_or(Blocked::ModelUnlisted)
}

/// Whether a held role may go to its named agent's fallback (Q-B): the Developer, a Validator round, an epic lead;
/// never a Project agent.
pub fn takes_fallback(role: Role) -> bool {
    role != Role::ProjectAgent
}

/// A role that can't start, and what can take it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The named agent it would have started on.
    pub agent: NamedAgent,
    pub blocked: Blocked,
    /// The fallback that could take it now (Use fallback); `None` without one or when it is blocked too.
    pub fallback: Option<String>,
    /// The fallback is blocked too ("Strong and its fallback Strong GPT are both at their limits").
    pub fallback_held: bool,
}

impl Held {
    /// The line stored with the hold: "Codex isn't available · logged out", "Claude Code limit reached", "Model
    /// unavailable · Codex no longer lists gpt-5.4". The cards and panes word it from the hold (T-0362, T-0363).
    pub fn message(&self) -> String {
        let tool = self.agent.tool;
        match &self.blocked {
            Blocked::Unavailable { reason } => NotAvailable {
                reason: reason.clone(),
            }
            .message(tool),
            Blocked::AtLimit => format!("{} limit reached", tool.display_name()),
            Blocked::ModelUnlisted => format!(
                "Model unavailable \u{b7} {}",
                mark::no_longer_lists_text(
                    tool.display_name(),
                    self.agent.model.as_deref().unwrap_or_default()
                )
            ),
        }
    }

    /// The Developer's start-problem step for it.
    pub fn step(&self) -> StartStep {
        match self.blocked.reason() {
            HoldReason::ToolLimit => StartStep::ToolLimit,
            HoldReason::ToolUnavailable => StartStep::ToolUnavailable,
            HoldReason::ModelUnavailable => StartStep::ModelUnavailable,
        }
    }
}

/// What the check decides for a new session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Launch on `agent`, passing `model`.
    Launch {
        agent: NamedAgent,
        model: Option<String>,
    },
    /// The automatic fallback.
    Move(Box<Moved>),
    Hold(Held),
}

/// The automatic fallback: the row moves from `from` to `agent` (`reason`, `tool`: why and which tool), and the
/// session launches there with `model`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub from: NamedAgent,
    pub agent: NamedAgent,
    pub model: Option<String>,
    pub reason: MoveReason,
    pub tool: Tool,
}

/// The decision for a new session of `role` on `agent` (pure): launch when the checks pass; else, for a limit or an
/// unavailable tool, the fallback (one step) when it passes them itself — moved to with `automatic` on, offered
/// otherwise; else held.
pub fn decide(
    role: Role,
    agent: &NamedAgent,
    agents: &[NamedAgent],
    facts: &Facts,
    automatic: bool,
) -> Decision {
    let blocked = match check(agent, facts) {
        Ok(model) => {
            return Decision::Launch {
                agent: agent.clone(),
                model,
            };
        }
        Err(blocked) => blocked,
    };
    let fallback = blocked
        .move_reason()
        .filter(|_| takes_fallback(role))
        .and(agent.fallback.as_deref())
        .and_then(|id| {
            agents
                .iter()
                .find(|a| a.id == id && a.id != agent.id && a.tool != agent.tool)
        });
    let held = |fallback: Option<String>, fallback_held: bool| {
        Decision::Hold(Held {
            agent: agent.clone(),
            blocked: blocked.clone(),
            fallback,
            fallback_held,
        })
    };
    match fallback.map(|f| (f, check(f, facts))) {
        Some((f, Ok(model))) if automatic => Decision::Move(Box::new(Moved {
            from: agent.clone(),
            agent: f.clone(),
            model,
            reason: blocked
                .move_reason()
                .expect("only a tool's problem has a fallback"),
            tool: agent.tool,
        })),
        Some((f, Ok(_))) => held(Some(f.id.clone()), false),
        Some((_, Err(_))) => held(None, true),
        None => held(None, false),
    }
}

/// Where a checked role belongs.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'a> {
    /// A ticket's Developer or Validator, an epic's lead.
    Ticket { id: &'a str, key: &'a TicketKey },
    /// A project's Project agent (its first message), with the project's name for the mark.
    Project { id: &'a str, name: &'a str },
}

impl<'a> Scope<'a> {
    pub fn ticket(ticket: &'a Ticket) -> Self {
        Self::Ticket {
            id: &ticket.id,
            key: &ticket.key,
        }
    }

    fn hold_scope(self) -> HoldScope<'a> {
        match self {
            Self::Ticket { id, .. } => HoldScope::Ticket(id),
            Self::Project { id, .. } => HoldScope::Project(id),
        }
    }

    /// Where a mark says it happened: the key, or the project.
    fn on(self) -> String {
        match self {
            Self::Ticket { key, .. } => key.to_string(),
            Self::Project { name, .. } => name.to_owned(),
        }
    }
}

/// What the check answers a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gated {
    /// Launch on this (the fallback after an automatic move).
    Launch(RunsOn),
    /// Held: nothing starts.
    Held(Box<Held>),
}

/// What a lift starts again once a validator's or a lead's hold goes (set once at service start).
#[derive(Clone)]
pub struct Restarts {
    pub validation: crate::validation::Validation,
    pub project_agents: crate::project_agents::ProjectAgents,
}

/// The check in the service. Cheap to clone.
#[derive(Clone)]
pub struct LaunchGate {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    connected: ConnectedTools,
    models: ModelLists,
    events: EventBus,
    /// Whether the tools' detected states count. Off (tests, which detect nothing): every tool counts as available,
    /// as the stand-in agents run as them.
    check_tool_states: bool,
    new_session_tool: Tool,
    restarts: OnceLock<Restarts>,
    /// One lift at a time.
    lifting: Mutex<()>,
    /// Asks the loop for a lift ([`LaunchGate::nudge`]).
    wake: tokio::sync::Notify,
}

impl LaunchGate {
    pub fn new(
        board: Board,
        connected: ConnectedTools,
        models: ModelLists,
        events: EventBus,
        check_tool_states: bool,
        new_session_tool: Tool,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                connected,
                models,
                events,
                check_tool_states,
                new_session_tool,
                restarts: OnceLock::new(),
                lifting: Mutex::new(()),
                wake: tokio::sync::Notify::new(),
            }),
        }
    }

    /// Gives the gate what starts a validator or a lead again when their hold lifts.
    pub fn set_restarts(&self, restarts: Restarts) {
        let _ = self.inner.restarts.set(restarts);
    }

    /// The facts now: every tool's state, the tools at their limit, the lists.
    pub async fn facts(&self) -> Result<Facts, BoardError> {
        let inner = &self.inner;
        let mut facts = Facts::default();
        for &tool in Tool::ALL {
            if inner.check_tool_states
                && let Err(not) = inner.connected.is_available(tool)
            {
                facts.unavailable.insert(tool, not.reason);
            }
            if let Some(copy) = inner.models.copy(tool) {
                facts.lists.insert(tool, copy);
            }
        }
        facts.at_limit = crate::usage::plan::limits(inner.board.store())
            .await?
            .into_iter()
            .filter(|l| l.reset_seen_at.is_none())
            .map(|l| l.tool)
            .collect();
        Ok(facts)
    }

    /// "Use fallback agents automatically" (off by default).
    pub async fn automatic(&self) -> Result<bool, BoardError> {
        Ok(self
            .inner
            .board
            .store()
            .setting(AUTO_FALLBACK)
            .await?
            .is_some_and(|v| v.trim() == "true"))
    }

    /// The check before a new session of `role` in `scope`, on `resolved` (`None` while there are no named agents:
    /// the new-session tool with its default model, whose tool is checked all the same). A launch clears an earlier
    /// hold; a move writes the row's move; a hold writes the hold (and the "no longer lists" mark).
    pub async fn before_launch(
        &self,
        role: Role,
        scope: Scope<'_>,
        resolved: Option<&Resolved>,
    ) -> Result<Gated, BoardError> {
        let Some(resolved) = resolved else {
            let agent = self.unnamed();
            return match check(&agent, &self.facts().await?) {
                Ok(_) => {
                    self.inner
                        .board
                        .store()
                        .clear_agent_hold(scope.hold_scope(), role)
                        .await?;
                    Ok(Gated::Launch(RunsOn::default()))
                }
                Err(blocked) => {
                    let held = Held {
                        agent,
                        blocked,
                        fallback: None,
                        fallback_held: false,
                    };
                    self.write_hold(role, scope, &held).await?;
                    Ok(Gated::Held(Box::new(held)))
                }
            };
        };
        let store = self.inner.board.store();
        let agents = store.named_agents().await?;
        let facts = self.facts().await?;
        let decision = decide(
            role,
            &resolved.agent,
            &agents,
            &facts,
            self.automatic().await?,
        );
        self.apply(role, scope, resolved, decision).await
    }

    /// A launch failed because of its tool (the program missing, the adapter's refusal; ADP-10): the tool is checked
    /// again, then the check decides once more. `Some` when the role is held now (an automatic move is written and
    /// lifted at once); `None` leaves the failure as it is.
    pub async fn after_failed_launch(
        &self,
        role: Role,
        scope: Scope<'_>,
        resolved: &Resolved,
    ) -> Result<Option<Held>, BoardError> {
        if !self.inner.check_tool_states {
            return Ok(None);
        }
        self.inner
            .connected
            .recheck(Some(resolved.agent.tool))
            .await?;
        let agents = self.inner.board.store().named_agents().await?;
        let facts = self.facts().await?;
        let decision = decide(
            role,
            &resolved.agent,
            &agents,
            &facts,
            self.automatic().await?,
        );
        let held = match &decision {
            Decision::Launch { .. } => return Ok(None),
            Decision::Hold(held) => held.clone(),
            Decision::Move(moved) => Held {
                agent: moved.from.clone(),
                blocked: check(&moved.from, &facts).err().unwrap_or(Blocked::AtLimit),
                fallback: Some(moved.agent.id.clone()),
                fallback_held: false,
            },
        };
        if let Decision::Move { .. } = decision {
            // The row moved: held for now, the next lift starts it on the fallback ([`Self::nudge`]).
            self.apply(role, scope, resolved, decision).await?;
        }
        self.write_hold(role, scope, &held).await?;
        Ok(Some(held))
    }

    /// Asks for a lift soon: a hold was just written with everything it holds (a held Developer's ticket is Held),
    /// so a cause that went meanwhile (or a row moved after a failed launch) isn't missed.
    pub fn nudge(&self) {
        self.inner.wake.notify_one();
    }

    /// What a session runs on while there are no named agents: the new-session tool, its default model (no entry).
    fn unnamed(&self) -> NamedAgent {
        let tool = self.inner.new_session_tool;
        NamedAgent {
            id: String::new(),
            name: tool.display_name().to_owned(),
            tool,
            model: None,
            use_for: String::new(),
            fallback: None,
            mark: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// The decision for a held `role` now: on its resolved agent, else the new-session tool.
    fn decide_now(
        &self,
        role: Role,
        resolved: Option<&Resolved>,
        agents: &[NamedAgent],
        facts: &Facts,
        automatic: bool,
    ) -> Decision {
        match resolved {
            Some(resolved) => decide(role, &resolved.agent, agents, facts, automatic),
            None => decide(role, &self.unnamed(), agents, facts, automatic),
        }
    }

    async fn apply(
        &self,
        role: Role,
        scope: Scope<'_>,
        resolved: &Resolved,
        decision: Decision,
    ) -> Result<Gated, BoardError> {
        let store = self.inner.board.store();
        match decision {
            Decision::Launch { agent, model } => {
                store.clear_agent_hold(scope.hold_scope(), role).await?;
                Ok(Gated::Launch(runs_on(&agent, model)))
            }
            Decision::Move(moved) => {
                let Moved {
                    from,
                    agent,
                    model,
                    reason,
                    tool,
                } = *moved;
                if let Scope::Ticket { id, key } = scope {
                    let moved = RowMove {
                        from_id: Some(from.id.clone()),
                        from_name: from.name.clone(),
                        reason,
                        tool: Some(tool),
                        at: now_ms(),
                    };
                    self.move_row(id, key, role, resolved, &agent, &moved, true)
                        .await?;
                    tracing::info!(%key, %role, from = from.name, to = agent.name, %reason, "moved to the fallback");
                }
                store.clear_agent_hold(scope.hold_scope(), role).await?;
                Ok(Gated::Launch(runs_on(&agent, model)))
            }
            Decision::Hold(held) => {
                self.write_hold(role, scope, &held).await?;
                Ok(Gated::Held(Box::new(held)))
            }
        }
    }

    /// Moves a ticket's `role` row to `agent`, keeping who set it. An empty row (the project's or the app's default)
    /// moved by the `automatic` fallback becomes Otaq's pick ("picked by Otaq: Codex not available"), never "set by
    /// you"; moved by Use fallback, it is the user's click.
    #[allow(clippy::too_many_arguments)]
    async fn move_row(
        &self,
        ticket_id: &str,
        key: &TicketKey,
        role: Role,
        resolved: &Resolved,
        agent: &NamedAgent,
        moved: &RowMove,
        automatic: bool,
    ) -> Result<(), BoardError> {
        let set_by = match &resolved.came_from {
            CameFrom::Row { set_by } => set_by.clone(),
            // A default the automatic fallback moved: Otaq's pick, never "set by you".
            CameFrom::ProjectDefault | CameFrom::AppDefault if automatic => RowSetBy::Agent {
                picked_by: crate::naming::PRODUCT_NAME.to_owned(),
                reason: automatic_reason(moved),
            },
            // Use fallback on a default: the user's click.
            CameFrom::ProjectDefault | CameFrom::AppDefault => RowSetBy::User,
        };
        let store = self.inner.board.store();
        store
            .move_ticket_agent_row(ticket_id, role, &agent.id, &set_by, moved)
            .await?;
        let mut tx = store.begin_write().await?;
        record_event(
            &mut tx,
            ticket_id,
            EventKind::AgentRowSet,
            &Actor::current_user(),
            super::rows::row_move_event(role, &agent.name, agent.tool, moved, &set_by, !automatic),
        )
        .await?;
        tx.commit().await?;
        self.ticket_changed(ticket_id, key);
        Ok(())
    }

    /// Writes the hold of `role` in `scope`; a held Developer's ticket keeps its message current. A model no longer
    /// listed marks the entry.
    async fn write_hold(
        &self,
        role: Role,
        scope: Scope<'_>,
        held: &Held,
    ) -> Result<(), BoardError> {
        let store = self.inner.board.store();
        let (ticket_id, project_id) = match scope {
            Scope::Ticket { id, .. } => (Some(id.to_owned()), None),
            Scope::Project { id, .. } => (None, Some(id.to_owned())),
        };
        store
            .set_agent_hold(&AgentHold {
                id: new_id(),
                ticket_id,
                project_id,
                role,
                reason: held.blocked.reason(),
                tool: held.agent.tool,
                model: held.agent.model.clone(),
                named_agent_id: Some(held.agent.id.clone()).filter(|id| !id.is_empty()),
                fallback_agent_id: held.fallback.clone(),
                fallback_held: held.fallback_held,
                since: now_ms(),
            })
            .await?;
        if held.blocked == Blocked::ModelUnlisted
            && let Some(model) = held.agent.model.as_deref()
            && mark::set_if_new(
                store,
                &held.agent,
                &mark::no_longer_lists(held.agent.tool.display_name(), model, &scope.on()),
            )
            .await?
        {
            self.inner.events.publish(Event::AgentsChanged);
        }
        if let Scope::Ticket { id, key } = scope {
            if role == Role::Developer {
                // Already held (a lift that holds again): the reason may have changed.
                sqlx::query(
                    "UPDATE tickets SET start_problem_step = ?, start_problem_message = ? WHERE id = ? AND state = ?",
                )
                .bind(held.step().as_str())
                .bind(held.message())
                .bind(id)
                .bind(Status::Held.as_str())
                .execute(store.pool())
                .await?;
            }
            self.ticket_changed(id, key);
        }
        tracing::info!(on = scope.on(), %role, reason = %held.blocked.reason(), tool = %held.agent.tool,
            agent = held.agent.name, "held before launch");
        Ok(())
    }

    /// **Use fallback** (`ticket.use_fallback`, the user's click): the held `role` of `key` moves to its named
    /// agent's fallback, recording the move (who set the row is kept), and starts there. Refused when nothing is
    /// held, for a model (no fallback), for a Project agent, or when the fallback can't start either.
    pub async fn use_fallback(&self, key: &str, role: Role) -> Result<(), BoardError> {
        let board = &self.inner.board;
        let store = board.store();
        let ticket = board.ticket(key).await?;
        let key = ticket.key.to_string();
        let refused = |message: String| BoardError::refused(Refusal::MoveRefused, message);
        if !takes_fallback(role) || !role.is_row_of(ticket.kind) {
            return Err(refused(format!("{key}'s {} has no fallback", role.label())));
        }
        let hold = store
            .ticket_holds(&ticket.id)
            .await?
            .into_iter()
            .find(|h| h.role == role)
            .ok_or_else(|| refused(format!("{key}'s {} isn't waiting", role.label())))?;
        if hold.reason == HoldReason::ModelUnavailable {
            return Err(refused(format!(
                "{key}'s {} waits for its model: change the agent",
                role.label()
            )));
        }
        let resolved = resolve_ticket_role(board, &ticket, role)
            .await?
            .ok_or_else(|| refused(format!("{key} has no agent")))?;
        let agents = store.named_agents().await?;
        let fallback = resolved
            .agent
            .fallback
            .as_deref()
            .and_then(|id| {
                agents
                    .iter()
                    .find(|a| a.id == id && a.tool != resolved.agent.tool)
            })
            .ok_or_else(|| refused(format!("{} has no fallback", resolved.agent.name)))?;
        let facts = self.facts().await?;
        if let Err(blocked) = check(fallback, &facts) {
            let why = Held {
                agent: fallback.clone(),
                blocked,
                fallback: None,
                fallback_held: false,
            }
            .message();
            return Err(refused(format!(
                "{} can't start either: {why}",
                fallback.name
            )));
        }
        let reason = match hold.reason {
            HoldReason::ToolUnavailable => MoveReason::ToolUnavailable,
            _ => MoveReason::ToolLimit,
        };
        let moved = RowMove {
            from_id: Some(resolved.agent.id.clone()),
            from_name: resolved.agent.name.clone(),
            reason,
            tool: Some(hold.tool),
            at: now_ms(),
        };
        self.move_row(
            &ticket.id,
            &ticket.key,
            role,
            &resolved,
            fallback,
            &moved,
            false,
        )
        .await?;
        store
            .clear_agent_hold(HoldScope::Ticket(&ticket.id), role)
            .await?;
        tracing::info!(%key, %role, to = fallback.name, "use fallback");
        self.release(role, &ticket).await
    }

    /// `settings.auto_fallback`: "Use fallback agents automatically" (AGT-08, off by default; an exception to d0018).
    /// Turned on, the waiting work moves to its fallbacks at once. (Settings shows it disabled with one connected
    /// tool; a fallback on a tool that isn't available never takes work anyway.)
    pub async fn set_automatic(&self, on: bool) -> Result<(), BoardError> {
        self.inner
            .board
            .store()
            .set_setting(AUTO_FALLBACK, if on { "true" } else { "false" })
            .await?;
        self.inner.events.publish(Event::SettingsChanged);
        if on {
            self.lift().await?;
        }
        Ok(())
    }

    /// Runs the check once more for every hold (a lift): a hold whose cause is gone starts as it was asked to (the
    /// Developer back to Queued for the scheduler, a validator, a lead; a Project agent's hold just goes: its message
    /// waits in the pane for the user's send), one still blocked is written again, one whose work went away goes.
    /// Also clears the "no longer lists" marks whose model is listed again. The number of holds that started again.
    pub async fn lift(&self) -> Result<usize, BoardError> {
        let _one = self.inner.lifting.lock().await;
        let store = self.inner.board.store();
        let agents = store.named_agents().await?;
        let facts = self.facts().await?;
        let cleared = mark::clear_listed_again(store, &agents, |tool, model| {
            passed_model(Some(model), facts.lists.get(&tool)).is_some()
        })
        .await?;
        if !cleared.is_empty() {
            tracing::info!(agents = ?cleared, "the tool lists the model again: marks cleared");
            self.inner.events.publish(Event::AgentsChanged);
        }
        let mut holds = Vec::new();
        for &reason in HoldReason::ALL {
            holds.extend(store.holds_for(reason).await?);
        }
        if holds.is_empty() {
            return Ok(0);
        }
        let agents = store.named_agents().await?;
        let automatic = self.automatic().await?;
        let mut started = 0;
        for hold in holds {
            match self.lift_one(&hold, &agents, &facts, automatic).await {
                Ok(true) => started += 1,
                Ok(false) => {}
                Err(e) => tracing::warn!(hold = hold.id, error = %e, "can't check a hold again"),
            }
        }
        Ok(started)
    }

    async fn lift_one(
        &self,
        hold: &AgentHold,
        agents: &[NamedAgent],
        facts: &Facts,
        automatic: bool,
    ) -> Result<bool, BoardError> {
        let board = &self.inner.board;
        let store = board.store();
        if let Some(project_id) = &hold.project_id {
            let Some(project) = board
                .projects()
                .await?
                .into_iter()
                .find(|p| &p.id == project_id)
            else {
                return Ok(false);
            };
            let resolved = resolve_project_role(board, &project, hold.role).await?;
            let scope = Scope::Project {
                id: &project.id,
                name: &project.name,
            };
            if let Decision::Hold(held) =
                self.decide_now(hold.role, resolved.as_ref(), agents, facts, automatic)
            {
                self.write_hold(hold.role, scope, &held).await?;
                return Ok(false);
            }
            store
                .clear_agent_hold(scope.hold_scope(), hold.role)
                .await?;
            return Ok(false);
        }
        let Some(ticket_id) = &hold.ticket_id else {
            return Ok(false);
        };
        let ticket = board.ticket_by_id(ticket_id).await?;
        let scope = Scope::ticket(&ticket);
        if !still_waits(&ticket, hold.role) {
            store
                .clear_agent_hold(scope.hold_scope(), hold.role)
                .await?;
            return Ok(false);
        }
        // A start that is holding it right now writes the ticket's status after the hold.
        if hold.role == Role::Developer && ticket.status != Status::Held {
            return Ok(false);
        }
        let resolved = resolve_ticket_role(board, &ticket, hold.role).await?;
        if let Decision::Hold(held) =
            self.decide_now(hold.role, resolved.as_ref(), agents, facts, automatic)
        {
            if !same_hold(hold, &held) {
                self.write_hold(hold.role, scope, &held).await?;
            }
            return Ok(false);
        }
        // The start itself checks once more (and moves to the fallback, if that is what passes).
        store
            .clear_agent_hold(scope.hold_scope(), hold.role)
            .await?;
        tracing::info!(key = %ticket.key, role = %hold.role, "hold lifted");
        self.release(hold.role, &ticket).await?;
        Ok(true)
    }

    /// Starts again what a lifted hold held back.
    async fn release(&self, role: Role, ticket: &Ticket) -> Result<(), BoardError> {
        match role {
            Role::Developer => self.requeue(ticket).await,
            Role::Validator => match self.inner.restarts.get() {
                Some(restarts) => restarts.validation.start_held(&ticket.id).await,
                None => Ok(()),
            },
            Role::EpicLead => match self.inner.restarts.get() {
                Some(restarts) => restarts
                    .project_agents
                    .leads()
                    .start_held(&ticket.key.to_string())
                    .await
                    .map(|_| ()),
                None => Ok(()),
            },
            Role::ProjectAgent => Ok(()),
        }
    }

    /// A held Developer goes back to Queued, its start problem cleared; the scheduler starts it (SCH-02: the drag
    /// asked for it).
    async fn requeue(&self, ticket: &Ticket) -> Result<(), BoardError> {
        let store = self.inner.board.store();
        let queued = Status::Queued;
        let now = now_ms();
        let mut tx = store.begin_write().await?;
        let done = sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, start_problem_step = NULL, start_problem_message = NULL,
                                updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ? AND start_problem_step IN (?, ?, ?)"
        ))
        .bind(queued.column().as_str())
        .bind(queued.as_str())
        .bind(now)
        .bind(queued.as_str())
        .bind(now)
        .bind(&ticket.id)
        .bind(Status::Held.as_str())
        .bind(StartStep::ToolLimit.as_str())
        .bind(StartStep::ToolUnavailable.as_str())
        .bind(StartStep::ModelUnavailable.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if done == 0 {
            return Ok(());
        }
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Moved,
            &Actor::current_user(),
            json!({ "from": Status::Held, "to": queued }),
        )
        .await?;
        tx.commit().await?;
        self.ticket_changed(&ticket.id, &ticket.key);
        self.inner.events.publish(Event::CardStatusChanged {
            project: ticket.key.project.to_string(),
            id: ticket.id.clone(),
            key: ticket.key.clone(),
            status: queued,
        });
        Ok(())
    }

    /// A ticket's holds whose work went away (the ticket moved on): forgotten, nothing started.
    async fn forget_stale(&self, ticket_id: &str) -> Result<(), BoardError> {
        let store = self.inner.board.store();
        let holds = store.ticket_holds(ticket_id).await?;
        if holds.is_empty() {
            return Ok(());
        }
        let Ok(ticket) = self.inner.board.ticket_by_id(ticket_id).await else {
            return Ok(());
        };
        for hold in holds.iter().filter(|h| !still_waits(&ticket, h.role)) {
            store
                .clear_agent_hold(HoldScope::Ticket(ticket_id), hold.role)
                .await?;
            tracing::debug!(key = %ticket.key, role = %hold.role, "a hold whose work went away");
        }
        Ok(())
    }

    /// Lifts holds whenever their cause may have gone, until `stop` is set: a tool's state changed, a list was read,
    /// a limit came, reset or was resumed, an entry or a default changed. Never on a timer (d0086). A ticket's
    /// change only forgets the holds whose work went away.
    pub async fn run(self, mut stop: watch::Receiver<bool>) {
        let mut tools = self.inner.connected.subscribe();
        let mut lists = self.inner.models.subscribe();
        let mut listener = self.inner.events.listen();
        enum Next {
            Lift,
            Ticket(String),
            Nothing,
        }
        loop {
            let next = tokio::select! {
                _ = stop.wait_for(|s| *s) => return,
                _ = self.inner.wake.notified() => Next::Lift,
                changed = tools.changed() => match changed { Ok(()) => Next::Lift, Err(_) => return },
                changed = lists.changed() => match changed { Ok(()) => Next::Lift, Err(_) => return },
                event = listener.receiver.recv() => match event {
                    Ok(Event::UsageChanged | Event::AgentsChanged) | Err(RecvError::Lagged(_)) => Next::Lift,
                    Ok(Event::TicketChanged { id, .. } | Event::CardStatusChanged { id, .. }) => Next::Ticket(id),
                    Ok(_) => Next::Nothing,
                    Err(RecvError::Closed) => return,
                },
            };
            match next {
                Next::Lift => {
                    if let Err(e) = self.lift().await {
                        tracing::warn!(error = %e, "can't lift the holds");
                    }
                }
                Next::Ticket(id) => {
                    if let Err(e) = self.forget_stale(&id).await {
                        tracing::warn!(ticket = id, error = %e, "can't check the ticket's holds");
                    }
                }
                Next::Nothing => {}
            }
        }
    }

    fn ticket_changed(&self, id: &str, key: &TicketKey) {
        self.inner.events.publish(Event::TicketChanged {
            project: key.project.to_string(),
            id: id.to_owned(),
            key: key.clone(),
        });
    }
}

/// Why the automatic fallback picked a row's agent: "Codex not available", "Claude Code limit".
fn automatic_reason(moved: &RowMove) -> String {
    let tool = moved.tool.map_or("its tool", |t| t.display_name());
    match moved.reason {
        MoveReason::ToolLimit => format!("{tool} limit"),
        MoveReason::ToolUnavailable | MoveReason::UserMove => format!("{tool} not available"),
    }
}

/// A new session on `agent` passing `model`.
fn runs_on(agent: &NamedAgent, model: Option<String>) -> RunsOn {
    RunsOn {
        tool: Some(agent.tool),
        model,
        agent: Some(SessionAgent {
            id: agent.id.clone(),
            name: agent.name.clone(),
        }),
    }
}

/// Whether `stored` already says what `held` would write.
fn same_hold(stored: &AgentHold, held: &Held) -> bool {
    stored.reason == held.blocked.reason()
        && stored.tool == held.agent.tool
        && stored.model == held.agent.model
        && stored.named_agent_id.as_deref().unwrap_or_default() == held.agent.id
        && stored.fallback_agent_id == held.fallback
        && stored.fallback_held == held.fallback_held
}

/// Whether the ticket still waits for its held `role`: a Developer held in Ready for Dev, a validation round that
/// opened without its validator, an epic whose lead could still start.
pub fn still_waits(ticket: &Ticket, role: Role) -> bool {
    match role {
        Role::Developer => ticket.column == Column::ReadyForDev,
        Role::Validator => matches!(
            ticket.status,
            Status::Validating | Status::WaitingForLimitReset
        ),
        Role::EpicLead => !matches!(
            ticket.column,
            Column::ReadyForReview | Column::Approved | Column::Done
        ),
        Role::ProjectAgent => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_tools::models::{ModelEntry, ModelKind, ModelList};

    fn agent(id: &str, tool: Tool, model: Option<&str>, fallback: Option<&str>) -> NamedAgent {
        NamedAgent {
            id: id.into(),
            name: id.into(),
            tool,
            model: model.map(str::to_owned),
            use_for: String::new(),
            fallback: fallback.map(str::to_owned),
            mark: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn copy(tool: Tool, ids: &[&str]) -> ModelCopy {
        ModelCopy {
            tool,
            list: ModelList {
                entries: ids
                    .iter()
                    .map(|id| ModelEntry {
                        id: (*id).into(),
                        display_name: (*id).into(),
                        description: None,
                        resolves_to: None,
                        kind: ModelKind::Exact,
                        family: None,
                        released_at: None,
                    })
                    .collect(),
            },
            read_at: 0,
        }
    }

    #[test]
    fn the_model_passes_when_listed_empty_or_never_read() {
        let tool = Tool::ALL[1];
        let list = copy(tool, &["gpt-5.5"]);
        assert_eq!(passed_model(None, Some(&list)), Some(None));
        assert_eq!(passed_model(Some(" "), Some(&list)), Some(None));
        assert_eq!(
            passed_model(Some("gpt-5.5"), Some(&list)),
            Some(Some("gpt-5.5".into()))
        );
        assert_eq!(passed_model(Some("gpt-5.4"), Some(&list)), None);
        assert_eq!(
            passed_model(Some("gpt-5.4"), None),
            Some(Some("gpt-5.4".into()))
        );
        assert_eq!(passed_model(Some("newest:gpt-sol"), None), None);
        assert_eq!(passed_model(Some("newest:gpt-sol"), Some(&list)), None);
    }

    #[test]
    fn a_fallbacks_fallback_is_never_used() {
        let [a, b, c] = [Tool::ALL[0], Tool::ALL[1], Tool::ALL[2]];
        let agents = [
            agent("strong", a, None, Some("gpt")),
            agent("gpt", b, None, Some("pilot")),
            agent("pilot", c, None, None),
        ];
        let facts = Facts {
            at_limit: [a, b].into(),
            ..Facts::default()
        };
        let Decision::Hold(held) = decide(Role::Developer, &agents[0], &agents, &facts, true)
        else {
            panic!("held");
        };
        assert_eq!((held.fallback.clone(), held.fallback_held), (None, true));
        assert_eq!(
            held.message(),
            format!("{} limit reached", a.display_name())
        );
    }
}
