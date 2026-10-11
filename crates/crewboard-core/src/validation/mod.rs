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
//! **Read-only** (VAL-02, VAL-08): the round's head is recorded when it opens (the worktree has no uncommitted
//! files then). If tracked files or the head differ when the verdict lands and the agent's session took no turn
//! meanwhile, the validator changed them: the verdict is discarded ("Validation discarded: the validator changed
//! files"). If the agent took a turn and changed them, or the user edited the description, the validation is
//! cancelled: the validator stops, the round doesn't count, the agent works again and is told so. Changes are
//! noticed on the agent's turn ends and on verdicts; Crewboard never stashes or touches the worktree to check.
//!
//! With validation off for the project the submit goes straight to Ready for Review, as before M12. While the
//! plan's usage limit holds (SCH-04) the round opens but no validator starts: the ticket shows "Waiting for
//! limit reset" and Resume all starts it ([`Validation::start_waiting`]).

use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::board::tickets::Ticket;
use crate::board::validation::{
    Round, ValidationEvent, ValidationState, findings_message, transition,
};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Actor, Status, ValidationAction, ValidationTrigger};
use crate::naming;
use crate::session::deliveries::{Deliveries, DeliveryKind};
use crate::session::launch::{LaunchContext, launch_validator};
use crate::session::{SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec, TerminalSize};
use crate::signals::{LIVE, Signal};

/// What the agent is told when its validation is cancelled (VAL-08).
pub const CANCELLED: &str =
    "Validation was cancelled because the code changed. Submit again when you're done.";

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
    /// Tickets whose agent took a turn since their validation started (VAL-08 tells it from VAL-02).
    agent_turned: Mutex<HashSet<String>>,
    /// Resumes a gone agent with a first prompt (Review's resume; set by the service).
    resume_agent: Mutex<Option<AgentResume>>,
}

/// Resumes the agent of a ticket (its id) with a first prompt.
pub type AgentResume = Arc<
    dyn Fn(String, String) -> Pin<Box<dyn Future<Output = Result<(), BoardError>> + Send>>
        + Send
        + Sync,
>;

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
                agent_turned: Mutex::new(HashSet::new()),
                resume_agent: Mutex::new(None),
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
        // The worktree must still be what was submitted (VAL-02, VAL-08).
        if let Some(head) = open.as_ref().and_then(|(_, _, _, h)| h.clone())
            && board.worktree_changed(&ticket, &head).await?
        {
            return Err(self.changed_under_validation(&ticket).await?);
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

    /// The worktree changed during validation: cancelled when the agent took a turn, else discarded (the
    /// validator changed it). Returns the refusal the validator's verdict gets.
    async fn changed_under_validation(&self, ticket: &Ticket) -> Result<BoardError, BoardError> {
        if self.agent_took_a_turn(&ticket.id) {
            self.cancel(ticket).await?;
            Ok(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{}'s validation was cancelled: the code changed",
                    ticket.key
                ),
            ))
        } else {
            self.discard(ticket).await?;
            Ok(BoardError::refused(
                Refusal::MoveRefused,
                format!("{}: {}", ticket.key, crate::domain::validation::DISCARDED),
            ))
        }
    }

    /// VAL-02: the validator changed tracked files or committed. Its verdict is thrown away; the ticket shows
    /// "Validation discarded: the validator changed files" with the no-result actions. Not a round.
    async fn discard(&self, ticket: &Ticket) -> Result<(), BoardError> {
        let step = self
            .step(ticket, ValidationEvent::ValidatorChangedFiles)
            .await?;
        self.stop_validator(ticket).await;
        self.inner
            .board
            .end_round(
                ticket,
                &step,
                "",
                Some(crate::domain::validation::DISCARDED),
                &Actor::current_user(),
            )
            .await?;
        tracing::warn!(key = %ticket.key, "validator changed files: verdict discarded");
        Ok(())
    }

    /// VAL-08: the code changed through the agent, or the user edited the description. The validator stops, the
    /// round doesn't count, the agent works again and is told so.
    pub async fn cancel(&self, ticket: &Ticket) -> Result<(), BoardError> {
        let step = self.step(ticket, ValidationEvent::CodeChanged).await?;
        self.stop_validator(ticket).await;
        self.inner
            .board
            .end_round(ticket, &step, "", None, &Actor::current_user())
            .await?;
        self.forget_turns(&ticket.id);
        if step.tell_cancelled {
            self.tell_agent(ticket, CANCELLED).await;
        }
        tracing::info!(key = %ticket.key, "validation cancelled: the code changed");
        Ok(())
    }

    /// The rules' row for `event` where the ticket stands.
    async fn step(
        &self,
        ticket: &Ticket,
        event: ValidationEvent,
    ) -> Result<crate::board::validation::Transition, BoardError> {
        let board = &self.inner.board;
        let settings = board
            .effective_settings(ticket.key.project.as_str())
            .await?;
        let extra = board.validation_rounds_extra(&ticket.id).await?;
        transition(
            ValidationState {
                status: ticket.status,
                round: ticket.validation_round,
                rounds: settings.validation_rounds + extra,
                validation: settings.validation,
                auto_merge: false,
                conflicts: ticket.auto_merge_conflicts,
            },
            event,
        )
        .map_err(|e| BoardError::refused(Refusal::MoveRefused, e.0))
    }

    async fn stop_validator(&self, ticket: &Ticket) {
        if let Some(session) = self
            .inner
            .sessions
            .running_of(&ticket.id, SessionKind::Validator)
            && let Err(e) = self.inner.sessions.stop(&session.id).await
        {
            tracing::warn!(key = %ticket.key, error = %e, "can't stop the validator");
        }
    }

    fn agent_took_a_turn(&self, ticket_id: &str) -> bool {
        self.inner
            .agent_turned
            .lock()
            .expect("turns")
            .contains(ticket_id)
    }

    fn forget_turns(&self, ticket_id: &str) {
        self.inner
            .agent_turned
            .lock()
            .expect("turns")
            .remove(ticket_id);
    }

    /// The agent's session started a turn (its prompt was submitted). While its ticket validates, that turn may
    /// change the code (VAL-08).
    pub async fn agent_working(&self, ticket_id: &str) {
        if let Ok(ticket) = self.inner.board.ticket_by_id(ticket_id).await
            && validating(ticket.status)
        {
            self.inner
                .agent_turned
                .lock()
                .expect("turns")
                .insert(ticket_id.to_owned());
        }
    }

    /// The agent's turn ended: if its ticket validates and tracked files or the head changed, the validation is
    /// cancelled (VAL-08).
    pub async fn agent_turn_ended(&self, ticket_id: &str) -> Result<(), BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket_by_id(ticket_id).await?;
        if !validating(ticket.status) || !self.agent_took_a_turn(ticket_id) {
            return Ok(());
        }
        let Some((_, _, _, Some(head))) = board.open_round(ticket_id).await? else {
            return Ok(());
        };
        if board.worktree_changed(&ticket, &head).await? {
            self.cancel(&ticket).await?;
        }
        Ok(())
    }

    /// The user saved a change to the ticket (SES-08). A description edit while it validates cancels the
    /// validation (VAL-08); a title-only edit doesn't.
    pub async fn ticket_edited(&self, before: &Ticket, after: &Ticket) -> Result<(), BoardError> {
        if validating(after.status) && before.body.trim() != after.body.trim() {
            self.cancel(after).await?;
        }
        Ok(())
    }

    /// The user's actions (VAL-06, VAL-07, VAL-09). None of the user's routes to Review auto-merges.
    pub async fn user_action(
        &self,
        key: &str,
        action: ValidationAction,
        note: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        match action {
            ValidationAction::ValidateAndSubmit => {
                if !crate::board::review::USER_SUBMITS_FROM.contains(&ticket.status) {
                    return Err(BoardError::refused(
                        Refusal::MoveRefused,
                        format!(
                            "{} can't be validated now: it is {}",
                            ticket.key,
                            ticket.status.label()
                        ),
                    ));
                }
                return self
                    .submit(
                        key,
                        &crate::board::review::user_summary(note),
                        actor,
                        ValidationTrigger::User,
                    )
                    .await;
            }
            ValidationAction::Stop | ValidationAction::Skip => {
                let event = if action == ValidationAction::Stop {
                    ValidationEvent::StopValidation
                } else {
                    ValidationEvent::SkipToReview
                };
                let step = self.step(&ticket, event).await?;
                self.stop_validator(&ticket).await;
                board.end_round(&ticket, &step, "", None, actor).await?;
                self.forget_turns(&ticket.id);
            }
            ValidationAction::MoveToReviewAnyway => {
                let step = self
                    .step(&ticket, ValidationEvent::MoveToReviewAnyway)
                    .await?;
                board.move_to_review(&ticket, step.entry, actor).await?;
            }
            ValidationAction::AnotherRound | ValidationAction::AnswerAgent => {
                let event = if action == ValidationAction::AnotherRound {
                    ValidationEvent::AnotherRound
                } else {
                    ValidationEvent::AnswerAgent
                };
                let step = self.step(&ticket, event).await?;
                let findings = board.last_report(&ticket.id).await?.unwrap_or_default();
                board
                    .back_to_agent(&ticket, step.extra_round, step.round == Round::Reset, actor)
                    .await?;
                let after = board.ticket_by_id(&ticket.id).await?;
                let text = user_findings_message(&after, &findings, note, step.extra_round);
                self.tell_agent(&after, &text).await;
            }
            ValidationAction::Retry => {
                self.step(&ticket, ValidationEvent::RetryValidation).await?;
                let head = board.submit_head(&ticket).await?;
                board.reopen_round(&ticket, &head, actor).await?;
                self.forget_turns(&ticket.id);
                if let Err(e) = self.start_validator(&ticket.id).await {
                    board
                        .validator_did_not_start(&ticket.id, &e.to_string())
                        .await?;
                }
            }
        }
        board.ticket_by_id(&ticket.id).await
    }

    /// Resume of a ticket interrupted while it validated (REC-01, VAL-07): a fresh validator on a new row with
    /// the same round number (not counted); the old validator conversation is never resumed and the agent's
    /// session is left alone.
    pub async fn resume_interrupted(&self, ticket_id: &str) -> Result<(), BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket_by_id(ticket_id).await?;
        let head = board.submit_head(&ticket).await?;
        board
            .reopen_round(&ticket, &head, &Actor::current_user())
            .await?;
        self.forget_turns(ticket_id);
        if let Err(e) = self.start_validator(ticket_id).await {
            board
                .validator_did_not_start(ticket_id, &e.to_string())
                .await?;
        }
        tracing::info!(key = %ticket.key, "validation resumed: a fresh validator");
        Ok(())
    }

    /// Park and Close stop the agent and the validator: the validation is discarded, recorded as stopped
    /// (CLS-01, CLS-06).
    pub async fn abandon(&self, ticket_id: &str) -> Result<(), BoardError> {
        if let Some(session) = self
            .inner
            .sessions
            .running_of(ticket_id, SessionKind::Validator)
        {
            self.inner.sessions.stop(&session.id).await.map_err(other)?;
        }
        if self
            .inner
            .board
            .abandon_round(ticket_id, crate::domain::ValidationOutcome::Stopped)
            .await?
        {
            tracing::info!(ticket = ticket_id, "validation stopped with its ticket");
        }
        self.forget_turns(ticket_id);
        Ok(())
    }

    /// How a gone agent is resumed (the service registers Review's resume).
    pub fn on_resume_agent(&self, hook: AgentResume) {
        *self.inner.resume_agent.lock().expect("resume agent") = Some(hook);
    }

    /// Queues `text` for the ticket's agent (SES-06 rules: typed once it is idle and its input line is empty).
    /// An agent whose session is gone (a restart ended it) is resumed with `text` as its first prompt: the only
    /// time a validation resumes the agent (VAL-07).
    async fn tell_agent(&self, ticket: &Ticket, text: &str) {
        let inner = &self.inner;
        let Some(session) = inner.sessions.running_of(&ticket.id, SessionKind::Agent) else {
            let hook = inner.resume_agent.lock().expect("resume agent").clone();
            match hook {
                Some(hook) => {
                    if let Err(e) = hook(ticket.id.clone(), text.to_owned()).await {
                        tracing::warn!(key = %ticket.key, error = %e, "can't resume the agent");
                    }
                }
                None => tracing::warn!(key = %ticket.key, "no agent session to tell"),
            }
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

    /// A hook of the ticket's validator (SDD §11.4 M12 rows, VAL-07, VAL-11): a permission prompt is "Needs you ·
    /// validator"; the user typing into it makes it Validating… again; its turn ending without `validate
    /// pass|fail` is "Validation ended without a result" (or "discarded" when it changed files); the usage limit
    /// is "Waiting for limit reset" until Resume all starts a fresh validator. Crewboard never nudges it (d0018).
    pub async fn validator_signal(
        &self,
        ticket_id: &str,
        session: &str,
        signal: &Signal,
    ) -> Result<(), BoardError> {
        let board = &self.inner.board;
        if signal.ends_turn() && board.round_of_session_ended(session).await? {
            return self.after_validator_turn(session).await;
        }
        let Some((_, _, Some(own), head)) = board.open_round(ticket_id).await? else {
            return Ok(());
        };
        if own != session {
            return Ok(());
        }
        let ticket = board.ticket_by_id(ticket_id).await?;
        match signal {
            Signal::NeedsYou { prompt } => {
                board
                    .set_validation_status(
                        ticket_id,
                        &[Status::Validating],
                        Status::ValidatorNeedsYou,
                        Some(prompt),
                        false,
                    )
                    .await?;
            }
            Signal::Working => {
                board
                    .set_validation_status(
                        ticket_id,
                        &[Status::ValidatorNeedsYou],
                        Status::Validating,
                        None,
                        false,
                    )
                    .await?;
            }
            Signal::Idle { .. } | Signal::TurnEnded if validating(ticket.status) => {
                // No verdict this turn.
                if let Some(head) = head
                    && board.worktree_changed(&ticket, &head).await?
                {
                    let _ = self.changed_under_validation(&ticket).await?;
                    return Ok(());
                }
                let step = self.step(&ticket, ValidationEvent::NoVerdict).await?;
                self.stop_validator(&ticket).await;
                board
                    .end_round(&ticket, &step, "", None, &Actor::current_user())
                    .await?;
                tracing::info!(key = %ticket.key, "validation ended without a result");
            }
            Signal::LimitReached { .. } if validating(ticket.status) => {
                board
                    .set_validation_status(
                        ticket_id,
                        &[Status::Validating, Status::ValidatorNeedsYou],
                        Status::WaitingForLimitReset,
                        None,
                        true,
                    )
                    .await?;
            }
            _ => {}
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
        self.forget_turns(&ticket.id);
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

/// What the agent gets on Give it another round or Answer the agent: the user's note first, then the findings.
fn user_findings_message(ticket: &Ticket, findings: &str, note: &str, extra: bool) -> String {
    let mut text = String::new();
    if !note.trim().is_empty() {
        text.push_str(&format!("The user answered: {}\n\n", note.trim()));
    }
    let what = if extra {
        "The user gave you another round"
    } else {
        "The validation's findings"
    };
    text.push_str(&format!(
        "{what} for {}. Fix these, commit, and submit again:\n\n{}",
        ticket.key,
        findings.trim()
    ));
    text
}

/// Validating…, or its validator waits on a prompt.
fn validating(status: Status) -> bool {
    matches!(status, Status::Validating | Status::ValidatorNeedsYou)
}

fn other(e: impl std::fmt::Display) -> BoardError {
    BoardError::refused(Refusal::SessionEnded, e.to_string())
}
