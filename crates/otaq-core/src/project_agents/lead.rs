//! The epic lead's session (LEAD-01, LEAD-02, LEAD-11, LEAD-12; d0083): one read-only agent per epic, stored as a
//! Project agent row with its epic set and run as a Project agent session with the epic lead role.
//!
//! - **Name:** worked out from the epic's title ("Wishlist lead"), never stored, so renaming the epic renames the
//!   lead. `otaq agent name` and the user's rename are refused for it.
//! - **Open lead** ([`EpicLeads::open`]) starts it with no first message; the user types in the Lead tab. An ended
//!   lead is resumed instead of starting a second one. Refused while it is live, while the epic is in Ready for
//!   Review, Approved or Done, and while the plan's usage limit holds.
//! - **Resume** ([`EpicLeads::resume`]) continues the same conversation (`claude --resume`).
//! - **Queueing** an epic without a lead calls [`EpicLeads::start_for_queue`] (T-0279): the first message asks it to
//!   check the plan.
//! - **Status** ([`LeadStatus`]): from the row, the session, its turn state and the reason for the current turn,
//!   which whoever types sets ([`EpicLeads::set_turn_reason`]; the user's Enter in its terminal is
//!   [`TurnReason::UserTyped`]). Each change publishes [`crate::protocol::Event::LeadChanged`].
//! - **Tokens** ([`EpicLeads::lead_tokens`]) count on the epic and in today's total (d0020), never toward the agent
//!   limit (d0016).

use serde::{Deserialize, Serialize};

use super::{AgentState, ProjectAgent, ProjectAgents};
use crate::board::tickets::Ticket;
use crate::board::{BoardError, Refusal};
use crate::domain::{Column, TicketKey};
use crate::naming;
use crate::session::launch::{launch_epic_lead, resume_epic_lead};
use crate::store::{new_id, now_ms};

/// Why no lead opens or resumes while the plan's usage limit holds (SCH-04).
pub const LEAD_LIMIT_REFUSAL: &str = "Usage limit reached · the lead opens once it resets";

/// Why `otaq agent name` and the user's rename are refused for a lead.
pub const NAMED_AFTER_EPIC: &str = "The lead is named after its epic";

/// Why the current turn of a lead runs, set by whoever typed into it (d0083).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TurnReason {
    /// The user typed to it: "planning".
    UserTyped,
    /// The first turn after a drag started it: "checking the plan".
    CheckPlan,
    /// A wake-up (LEAD-04): "re-planning", or "answering SHOP-40.2" for a question.
    Wake { wake: Wake },
}

/// What woke a lead (LEAD-04): a child merged, was closed, got stuck, or asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Wake {
    Merge,
    Close,
    Stuck,
    /// The asking child.
    Question {
        ticket: TicketKey,
    },
}

/// What the lead is doing (LEAD-11): the words of *01 · Components · Epics on the board → Row · Lead Status*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LeadStatus {
    /// The epic has no lead: "Open lead".
    None,
    Idle,
    /// Its first turn after a drag started it.
    CheckingPlan,
    /// A turn the user started by typing.
    Planning,
    /// A merge, close or stuck wake-up.
    Replanning,
    /// A question wake-up: "answering SHOP-40.2".
    Answering {
        ticket: TicketKey,
    },
    /// Its own `ask human` or a permission prompt (turquoise).
    AsksYou,
    /// "ended by restart · Resume".
    EndedByRestart,
    /// "ended · epic in review".
    EndedInReview,
    /// Its `claude` exited for another reason: "ended · Resume".
    Ended,
}

/// What a lead's status is worked out from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadFacts {
    /// The lead row's state; `None` when the epic has no lead.
    pub state: Option<AgentState>,
    /// Its session runs.
    pub live: bool,
    /// The epic's column.
    pub epic_column: Column,
    /// It waits for the user: its own question or a permission prompt.
    pub waits_for_user: bool,
    /// Why its current turn runs; `None` between turns.
    pub turn: Option<TurnReason>,
}

/// The lead's status from its facts (LEAD-11).
pub fn lead_status(facts: &LeadFacts) -> LeadStatus {
    let Some(state) = facts.state else {
        return LeadStatus::None;
    };
    if !facts.live {
        let in_review = matches!(
            facts.epic_column,
            Column::ReadyForReview | Column::Approved | Column::Done
        );
        return if in_review {
            LeadStatus::EndedInReview
        } else if state == AgentState::EndedByRestart {
            LeadStatus::EndedByRestart
        } else {
            LeadStatus::Ended
        };
    }
    if facts.waits_for_user {
        return LeadStatus::AsksYou;
    }
    match &facts.turn {
        None => LeadStatus::Idle,
        Some(TurnReason::UserTyped) => LeadStatus::Planning,
        Some(TurnReason::CheckPlan) => LeadStatus::CheckingPlan,
        Some(TurnReason::Wake {
            wake: Wake::Merge | Wake::Close | Wake::Stuck,
        }) => LeadStatus::Replanning,
        Some(TurnReason::Wake {
            wake: Wake::Question { ticket },
        }) => LeadStatus::Answering {
            ticket: ticket.clone(),
        },
    }
}

/// An epic's lead as clients see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lead {
    /// The epic's key and id.
    pub epic: TicketKey,
    pub epic_id: String,
    /// "Wishlist lead".
    pub name: String,
    pub status: LeadStatus,
    /// Its Project agent row: state, session, question, tokens.
    pub agent: ProjectAgent,
}

/// "Wishlist lead": the lead's name, from its epic's title.
pub fn lead_name(epic_title: &str) -> String {
    format!("{} lead", epic_title.trim())
}

/// The first message of a lead a drag started (LEAD-02).
pub fn check_plan_message(epic: &TicketKey) -> String {
    format!(
        "{epic} was queued. Check the plan before the first child starts: `{} prime --epic`.",
        naming::COMMAND_NAME
    )
}

/// `agent name` and the user's rename: refused for a lead.
pub(super) fn refuse_naming(agent: &ProjectAgent) -> Result<(), BoardError> {
    if agent.epic.is_some() {
        return Err(BoardError::refused(Refusal::NotAllowed, NAMED_AFTER_EPIC));
    }
    Ok(())
}

/// A lead's turn: why the current one runs (while in one), and why the next one will.
#[derive(Debug, Clone, Default)]
pub(super) struct LeadTurn {
    current: Option<TurnReason>,
    next: Option<TurnReason>,
}

/// An epic's lead: open, resume, start for a queue, status, tokens. Cheap to clone.
#[derive(Clone)]
pub struct EpicLeads {
    agents: ProjectAgents,
}

impl ProjectAgents {
    /// The epic leads (d0083), which share the Project agents' rows and sessions.
    pub fn leads(&self) -> EpicLeads {
        EpicLeads {
            agents: self.clone(),
        }
    }

    /// A hook signal of the Project agent `id`: if it is a lead, a turn starts (with the reason set before it, else
    /// the user's) or ends. Whether its status changed.
    pub(super) fn lead_turn_signal(&self, id: &str, signal: &crate::signals::Signal) -> bool {
        if !self.is_lead(id) {
            return false;
        }
        let mut turns = self.inner.lead_turns.lock().expect("lead turns");
        let turn = turns.entry(id.to_owned()).or_default();
        if matches!(signal, crate::signals::Signal::Working) {
            if turn.current.is_none() {
                turn.current = Some(turn.next.take().unwrap_or(TurnReason::UserTyped));
                return true;
            }
        } else if signal.ends_turn() {
            return turn.current.take().is_some();
        }
        false
    }

    /// Enter in a lead's terminal: the user typed, so its next turn is theirs.
    pub(super) fn lead_typed(&self, id: &str) {
        if !self.is_lead(id) {
            return;
        }
        let mut turns = self.inner.lead_turns.lock().expect("lead turns");
        let turn = turns.entry(id.to_owned()).or_default();
        if turn.current.is_none() {
            turn.next = Some(TurnReason::UserTyped);
        }
    }

    fn is_lead(&self, id: &str) -> bool {
        self.inner
            .lead_epics
            .lock()
            .expect("lead epics")
            .contains_key(id)
    }
}

impl EpicLeads {
    fn inner(&self) -> &super::Inner {
        &self.agents.inner
    }

    /// **Open lead** (LEAD-02): the user's click is the go-ahead. It starts the lead with no first message, or
    /// resumes an ended one. Refused when the epic has a live lead, is in Ready for Review / Approved / Done, or
    /// while the usage limit holds.
    pub async fn open(&self, epic: &str) -> Result<Lead, BoardError> {
        let epic = self.epic(epic).await?;
        refuse_in_review(&epic)?;
        if let Some(lead) = self.get_for(&epic).await? {
            if is_live(&lead.agent) {
                return Err(BoardError::refused(
                    Refusal::AlreadyRunning,
                    format!("{} is already open", lead.name),
                ));
            }
            return self.resume_for(&epic, lead.agent).await;
        }
        self.refuse_during_limit().await?;
        self.launch_new(&epic, None).await
    }

    /// **Resume** (LEAD-12): the same conversation, `claude --resume`, the user's click; it clears "ended by
    /// restart". A live lead is returned as it is. Refused for an epic without a lead, in review, or while the
    /// usage limit holds.
    pub async fn resume(&self, epic: &str) -> Result<Lead, BoardError> {
        let epic = self.epic(epic).await?;
        refuse_in_review(&epic)?;
        let lead = self
            .get_for(&epic)
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("{} has no lead to resume", epic.key)))?;
        if is_live(&lead.agent) {
            return Ok(lead);
        }
        self.resume_for(&epic, lead.agent).await
    }

    /// Starts the lead of a queued epic (LEAD-02, called when the epic is queued; T-0279): the first message asks
    /// it to check the plan, and its status is "checking the plan". Refused when the epic already has a lead.
    pub async fn start_for_queue(&self, epic: &str) -> Result<Lead, BoardError> {
        let epic = self.epic(epic).await?;
        if let Some(lead) = self.get_for(&epic).await? {
            return Err(BoardError::refused(
                Refusal::AlreadyRunning,
                format!("{} already has {}", epic.key, lead.name),
            ));
        }
        let message = check_plan_message(&epic.key);
        self.launch_new(&epic, Some((&message, TurnReason::CheckPlan)))
            .await
    }

    /// The epic's lead, if it has one.
    pub async fn get(&self, epic: &str) -> Result<Option<Lead>, BoardError> {
        let epic = self.epic(epic).await?;
        self.get_for(&epic).await
    }

    /// The epic's Lead Status (LEAD-11): [`LeadStatus::None`] when it has no lead.
    pub async fn status(&self, epic: &str) -> Result<LeadStatus, BoardError> {
        Ok(self
            .get(epic)
            .await?
            .map_or(LeadStatus::None, |lead| lead.status))
    }

    /// Sets why the lead's next turn runs (or the current one, when `reason` is typed into a turn it starts now):
    /// a wake-up sets [`TurnReason::Wake`] before its message is typed. Nothing happens for an epic without a lead.
    pub async fn set_turn_reason(&self, epic: &str, reason: TurnReason) -> Result<(), BoardError> {
        let Some(lead) = self.get(epic).await? else {
            return Ok(());
        };
        {
            let mut turns = self.inner().lead_turns.lock().expect("lead turns");
            let turn = turns.entry(lead.agent.id.clone()).or_default();
            turn.next = Some(reason);
        }
        self.agents.changed(&lead.agent.project, &lead.agent.id);
        Ok(())
    }

    /// The lead's tokens, every session and day (LEAD-12): zero for an epic without a lead.
    pub async fn lead_tokens(&self, epic: &str) -> Result<crate::usage::Tokens, BoardError> {
        Ok(self
            .get(epic)
            .await?
            .map(|lead| lead.agent.tokens)
            .unwrap_or_default())
    }

    async fn epic(&self, key: &str) -> Result<Ticket, BoardError> {
        let epic = self.inner().board.ticket(key).await?;
        if !epic.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAnEpic,
                format!("{} isn't an epic", epic.key),
            ));
        }
        Ok(epic)
    }

    async fn get_for(&self, epic: &Ticket) -> Result<Option<Lead>, BoardError> {
        let id: Option<(String,)> =
            sqlx::query_as("SELECT id FROM project_agents WHERE epic_id = ?")
                .bind(&epic.id)
                .fetch_optional(self.inner().board.store().pool())
                .await?;
        let Some((id,)) = id else {
            return Ok(None);
        };
        let agent = self.agents.get(&id).await?;
        let turn = self
            .inner()
            .lead_turns
            .lock()
            .expect("lead turns")
            .get(&id)
            .and_then(|t| t.current.clone());
        let status = lead_status(&LeadFacts {
            state: Some(agent.state),
            live: is_live(&agent),
            epic_column: epic.column,
            waits_for_user: agent.question.is_some() || agent.needs_you.is_some(),
            turn,
        });
        Ok(Some(Lead {
            epic: epic.key.clone(),
            epic_id: epic.id.clone(),
            name: agent.name.clone(),
            status,
            agent,
        }))
    }

    /// Starts a new lead for `epic`, with `first` (its first message, and why that turn runs) if any.
    async fn launch_new(
        &self,
        epic: &Ticket,
        first: Option<(&str, TurnReason)>,
    ) -> Result<Lead, BoardError> {
        let (message, reason) = first.unzip();
        let agents = &self.agents;
        let launch = agents.launch()?;
        let project = self
            .inner()
            .board
            .project(&epic.key.project.to_string())
            .await?;
        let repo = std::path::PathBuf::from(&project.repo_path);
        let settings = agents.settings(launch, &project);
        let command = launch_epic_lead(launch, &repo, &epic.key, message, settings.as_deref());
        let id = new_id();
        let pool = self.inner().board.store().pool();
        let inserted = sqlx::query(
            "INSERT INTO project_agents (id, project_id, claude_session_id, state, created_at, epic_id)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&project.id)
        .bind(&command.claude_session_id)
        .bind(AgentState::Running.as_str())
        .bind(now_ms())
        .bind(&epic.id)
        .execute(pool)
        .await;
        if let Err(e) = inserted {
            // Another click opened it first: one lead per epic.
            tracing::info!(epic = %epic.key, error = %e, "the epic's lead wasn't added");
            return Err(BoardError::refused(
                Refusal::AlreadyRunning,
                format!("{} already has a lead", epic.key),
            ));
        }
        // Known as a lead, with its first turn's reason, before its first hook can arrive.
        self.inner()
            .lead_epics
            .lock()
            .expect("lead epics")
            .insert(id.clone(), (project.key.clone(), epic.key.clone()));
        self.inner().lead_turns.lock().expect("lead turns").insert(
            id.clone(),
            LeadTurn {
                current: reason,
                next: None,
            },
        );
        if let Err(e) = agents
            .spawn(&project, command, &id, Some((&epic.id, &epic.key)))
            .await
        {
            sqlx::query("DELETE FROM project_agents WHERE id = ?")
                .bind(&id)
                .execute(pool)
                .await?;
            self.inner()
                .lead_turns
                .lock()
                .expect("lead turns")
                .remove(&id);
            return Err(e);
        }
        tracing::info!(epic = %epic.key, agent = id, "epic lead started");
        agents.changed(&project.key, &id);
        self.get_for(epic)
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("{}'s lead is gone", epic.key)))
    }

    async fn resume_for(&self, epic: &Ticket, agent: ProjectAgent) -> Result<Lead, BoardError> {
        self.refuse_during_limit().await?;
        let agents = &self.agents;
        let launch = agents.launch()?;
        let project = self.inner().board.project(&agent.project).await?;
        let repo = std::path::PathBuf::from(&project.repo_path);
        let settings = agents.settings(launch, &project);
        let command = resume_epic_lead(
            launch,
            &repo,
            &epic.key,
            &agent.claude_session_id,
            settings.as_deref(),
        );
        self.inner()
            .lead_turns
            .lock()
            .expect("lead turns")
            .remove(&agent.id);
        agents
            .restart(&agent, &project, command, Some((&epic.id, &epic.key)))
            .await?;
        tracing::info!(epic = %epic.key, agent = agent.id, "epic lead resumed");
        agents.changed(&agent.project, &agent.id);
        self.get_for(epic)
            .await?
            .ok_or_else(|| BoardError::NotFound(format!("{}'s lead is gone", epic.key)))
    }

    async fn refuse_during_limit(&self) -> Result<(), BoardError> {
        if self.agents.limit_held().await? {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                LEAD_LIMIT_REFUSAL,
            ));
        }
        Ok(())
    }
}

/// A lead whose session runs.
fn is_live(agent: &ProjectAgent) -> bool {
    agent.state == AgentState::Running
        && agent
            .session
            .as_ref()
            .is_some_and(crate::session::SessionInfo::is_running)
}

/// "SHOP-40 is in review; its lead has ended": no lead opens for an epic in Ready for Review, Approved or Done.
fn refuse_in_review(epic: &Ticket) -> Result<(), BoardError> {
    if matches!(
        epic.column,
        Column::ReadyForReview | Column::Approved | Column::Done
    ) {
        return Err(BoardError::refused(
            Refusal::MoveRefused,
            format!("{} is in review; its lead has ended", epic.key),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: &str) -> TicketKey {
        k.parse().unwrap()
    }

    fn live(turn: Option<TurnReason>) -> LeadFacts {
        LeadFacts {
            state: Some(AgentState::Running),
            live: true,
            epic_column: Column::Dev,
            waits_for_user: false,
            turn,
        }
    }

    #[test]
    fn every_status_from_its_inputs() {
        let none = LeadFacts {
            state: None,
            live: false,
            epic_column: Column::Backlog,
            waits_for_user: false,
            turn: None,
        };
        assert_eq!(lead_status(&none), LeadStatus::None);
        assert_eq!(lead_status(&live(None)), LeadStatus::Idle);
        assert_eq!(
            lead_status(&live(Some(TurnReason::CheckPlan))),
            LeadStatus::CheckingPlan
        );
        assert_eq!(
            lead_status(&live(Some(TurnReason::UserTyped))),
            LeadStatus::Planning
        );
        for wake in [Wake::Merge, Wake::Close, Wake::Stuck] {
            assert_eq!(
                lead_status(&live(Some(TurnReason::Wake { wake }))),
                LeadStatus::Replanning
            );
        }
        assert_eq!(
            lead_status(&live(Some(TurnReason::Wake {
                wake: Wake::Question {
                    ticket: key("SHOP-40.2")
                }
            }))),
            LeadStatus::Answering {
                ticket: key("SHOP-40.2")
            }
        );
        // Its own question or permission prompt wins over the turn.
        let asks = LeadFacts {
            waits_for_user: true,
            ..live(Some(TurnReason::UserTyped))
        };
        assert_eq!(lead_status(&asks), LeadStatus::AsksYou);

        let ended = |state, column| LeadFacts {
            state: Some(state),
            live: false,
            epic_column: column,
            waits_for_user: false,
            turn: None,
        };
        assert_eq!(
            lead_status(&ended(AgentState::EndedByRestart, Column::Dev)),
            LeadStatus::EndedByRestart
        );
        assert_eq!(
            lead_status(&ended(AgentState::Ended, Column::Dev)),
            LeadStatus::Ended
        );
        for column in [Column::ReadyForReview, Column::Approved, Column::Done] {
            assert_eq!(
                lead_status(&ended(AgentState::Ended, column)),
                LeadStatus::EndedInReview
            );
        }
        // A row still running whose `claude` exited isn't live.
        let exited = LeadFacts {
            live: false,
            ..live(Some(TurnReason::UserTyped))
        };
        assert_eq!(lead_status(&exited), LeadStatus::Ended);
    }

    #[test]
    fn the_lead_is_named_after_its_epic_and_the_queue_message_names_prime() {
        assert_eq!(lead_name("Wishlist"), "Wishlist lead");
        assert_eq!(lead_name(" Search "), "Search lead");
        assert_eq!(
            check_plan_message(&key("SHOP-40")),
            format!(
                "SHOP-40 was queued. Check the plan before the first child starts: `{} prime --epic`.",
                naming::COMMAND_NAME
            )
        );
    }
}
