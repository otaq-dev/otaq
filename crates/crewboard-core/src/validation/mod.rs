//! Validation before Review (VAL-01, VAL-13, SCH-04; d0063): the agent's `review submit` starts a separate,
//! read-only **validator** instead of moving the ticket to Ready for Review.
//!
//! A submit is checked as before (REV-07: refused with uncommitted files), its summary and head are recorded, a
//! round opens in `validations` (round = counted rounds + 1, who started it, the head) and the ticket shows
//! **Validating…**. The service then starts the validator: a fresh `claude` in the ticket's worktree with the
//! project's Validate skill as its prompt, the validator role and no `PORT`
//! ([`crate::session::launch::launch_validator`]). The agent's submit is the go-ahead (a P1 exception, like
//! answering another agent). One validator per ticket; its Claude session ID is stored like every session's
//! (d0045).
//!
//! With validation off for the project the submit goes straight to Ready for Review, as before M12. While the
//! plan's usage limit holds (SCH-04) the round opens but no validator starts: the ticket shows "Waiting for
//! limit reset" and Resume all starts it ([`Validation::start_waiting`]).

use std::path::PathBuf;
use std::sync::Arc;

use crate::board::tickets::Ticket;
use crate::board::validation::{ValidationEvent, ValidationState, findings_message, transition};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Actor, Status, ValidationTrigger};
use crate::naming;
use crate::session::deliveries::{Deliveries, DeliveryKind};
use crate::session::launch::{LaunchContext, launch_validator};
use crate::session::{SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec, TerminalSize};
use crate::signals::LIVE;

/// What `crewboard review submit` answers while validation runs (VAL-01).
pub const SUBMITTED_VALIDATING: &str =
    "Submitted. Validation is running; the result will come back to you here. End your turn.";

/// The size a validator starts with until a terminal attaches.
const INITIAL_SIZE: TerminalSize = TerminalSize {
    rows: 40,
    cols: 120,
};

/// Starts and tracks validators. Cheap to clone.
#[derive(Clone)]
pub struct Validation {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    data: DataDir,
    sessions: Sessions,
    deliveries: Deliveries,
    launch: Option<LaunchContext>,
}

impl Validation {
    pub fn new(
        board: Board,
        data: DataDir,
        sessions: Sessions,
        deliveries: Deliveries,
        launch: Option<LaunchContext>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                sessions,
                deliveries,
                launch,
            }),
        }
    }

    /// `crewboard validate pass|fail --stdin` from the ticket's validator (VAL-03 … VAL-06): the report is stored
    /// on the round. Pass: Ready for Review, "Validated · round N". Fail with rounds left: the findings go to the
    /// agent as an automatic message and it works again, "Validation failed · round N of M". The last round:
    /// "Validation failed M times", nothing delivered. The validator ends once its turn ends.
    pub async fn verdict(
        &self,
        key: &str,
        pass: bool,
        report: &str,
        session: &str,
    ) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        let open = board.open_round(&ticket.id).await?;
        if open.as_ref().and_then(|(_, _, s, _)| s.as_deref()) != Some(session) {
            return Err(BoardError::refused(
                Refusal::NotAllowed,
                format!("This isn't {}'s running validator", ticket.key),
            ));
        }
        let settings = board
            .effective_settings(ticket.key.project.as_str())
            .await?;
        let extra = board.validation_rounds_extra(&ticket.id).await?;
        let rounds = settings.validation_rounds + extra;
        let state = ValidationState {
            status: ticket.status,
            round: ticket.validation_round,
            rounds,
            validation: true,
            auto_merge: false,
            conflicts: ticket.auto_merge_conflicts,
        };
        let event = if pass {
            ValidationEvent::Pass
        } else {
            ValidationEvent::Fail
        };
        let step =
            transition(state, event).map_err(|e| BoardError::refused(Refusal::MoveRefused, e.0))?;
        board
            .record_verdict(&ticket, &step, report, &Actor::Agent(ticket.key.clone()))
            .await?;
        if step.deliver_findings {
            let round = ticket.validation_round + 1;
            self.tell_agent(&ticket, &findings_message(round, rounds, report))
                .await;
        }
        board.ticket_by_id(&ticket.id).await
    }

    /// Queues `text` for the ticket's agent (SES-06 rules: typed once it is idle and its input line is empty).
    async fn tell_agent(&self, ticket: &Ticket, text: &str) {
        let inner = &self.inner;
        let Some(session) = inner
            .sessions
            .running_of(&ticket.id, SessionKind::Agent)
            .or_else(|| inner.sessions.latest_of(&ticket.id, SessionKind::Agent))
        else {
            tracing::warn!(key = %ticket.key, "no agent session to tell");
            return;
        };
        if let Err(e) = inner
            .deliveries
            .queue_delivery(&session.id, DeliveryKind::Validation, text)
            .await
        {
            tracing::warn!(key = %ticket.key, error = %e, "can't queue the validation message");
        }
    }

    /// The validator's turn ended: once its round has a verdict, it is done and its session ends (VAL-03).
    pub async fn after_validator_turn(&self, session: &str) -> Result<(), BoardError> {
        if self.inner.board.round_of_session_ended(session).await? {
            self.inner.sessions.stop(session).await.map_err(other)?;
        }
        Ok(())
    }

    /// The agent's `review submit` (or, with `trigger` user, Validate and submit): Validating… and a validator,
    /// or Ready for Review when validation is off for the project.
    pub async fn submit(
        &self,
        key: &str,
        summary: &str,
        actor: &Actor,
        trigger: ValidationTrigger,
    ) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        let settings = board
            .effective_settings(ticket.key.project.as_str())
            .await?;
        if !settings.validation {
            return board.submit_for_review(key, summary, actor).await;
        }
        if !LIVE.contains(&ticket.status) && ticket.status != Status::MergeConflictFixing {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; only a ticket whose agent is working can be submitted",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        let event = match trigger {
            ValidationTrigger::Agent => ValidationEvent::AgentSubmit,
            ValidationTrigger::User => ValidationEvent::ValidateAndSubmit,
        };
        let state = ValidationState {
            status: ticket.status,
            round: ticket.validation_round,
            rounds: settings.validation_rounds,
            validation: true,
            auto_merge: false,
            conflicts: ticket.auto_merge_conflicts,
        };
        let step =
            transition(state, event).map_err(|e| BoardError::refused(Refusal::MoveRefused, e.0))?;
        let head = board.submit_head(&ticket).await?;
        let held = crate::usage::plan::latest(board.store())
            .await?
            .and_then(|s| s.limit)
            .is_some_and(|l| l.reset_seen_at.is_none());
        board
            .enter_validation(&ticket, &step, &head, summary, trigger, held, actor)
            .await?;
        if !held && let Err(e) = self.start_validator(&ticket.id).await {
            // Shown, never retried by itself (P2): Retry validation is the user's click.
            tracing::warn!(key = %ticket.key, error = %e, "the validator didn't start");
            board
                .validator_did_not_start(&ticket.id, &e.to_string())
                .await?;
        }
        board.ticket_by_id(&ticket.id).await
    }

    /// Starts the validator of the ticket's open round (a submit, Retry validation, Resume all after the limit).
    /// Any validator still running for the ticket is stopped first: one per ticket.
    pub async fn start_validator(&self, ticket_id: &str) -> Result<SessionInfo, BoardError> {
        let inner = &self.inner;
        let board = &inner.board;
        let ticket = board.ticket_by_id(ticket_id).await?;
        if let Some(old) = inner
            .sessions
            .running_of(&ticket.id, SessionKind::Validator)
        {
            inner.sessions.stop(&old.id).await.map_err(other)?;
        }
        let launch = inner.launch.as_ref().ok_or_else(|| {
            BoardError::refused(
                Refusal::NotAvailableYet,
                format!(
                    "{}'s Claude Code plugin wasn't found next to the {} command",
                    naming::PRODUCT_NAME,
                    naming::COMMAND_NAME
                ),
            )
        })?;
        let worktree = ticket
            .work
            .as_ref()
            .and_then(|w| w.worktree.clone())
            .map(PathBuf::from)
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} has no worktree to validate", ticket.key),
                )
            })?;
        let skill = board
            .effective_settings(ticket.key.project.as_str())
            .await?
            .workflow_skills
            .validate;
        let settings = crate::session::statusline::write_settings(
            &inner.data,
            &ticket.key,
            &worktree,
            launch.claude_config_dir.as_deref(),
            &launch.bin_dir,
        )
        .inspect_err(
            |e| tracing::warn!(key = %ticket.key, error = %e, "can't write the status-line settings"),
        )
        .ok();
        let command = launch_validator(launch, &ticket.key, &worktree, &skill, settings.as_deref());
        let project = board.project(ticket.key.project.as_str()).await?;
        let session = inner
            .sessions
            .spawn(SpawnSpec {
                kind: SessionKind::Validator,
                project_id: project.id,
                project_key: project.key,
                ticket: Some(SessionTicket {
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                }),
                program: command.program,
                args: command.args,
                cwd: command.cwd,
                env: command.env,
                size: INITIAL_SIZE,
                claude_session_id: Some(command.claude_session_id),
            })
            .await
            .map_err(other)?;
        board.validator_started(&ticket.id, &session.id).await?;
        tracing::info!(key = %ticket.key, session = %session.id, %skill, "validator started");
        Ok(session)
    }

    /// Resume all after the usage limit (SCH-04): starts the validators that waited for it. Returns their
    /// tickets' ids.
    pub async fn start_waiting(&self) -> Result<Vec<String>, BoardError> {
        let waiting = self.inner.board.validations_waiting_for_limit().await?;
        let mut started = Vec::new();
        for id in waiting {
            match self.start_validator(&id).await {
                Ok(_) => started.push(id),
                Err(e) => {
                    tracing::warn!(ticket = %id, error = %e, "can't start a waiting validator")
                }
            }
        }
        Ok(started)
    }
}

fn other(e: impl std::fmt::Display) -> BoardError {
    BoardError::refused(Refusal::SessionEnded, e.to_string())
}
