//! The M12 rows of SDD §7.3 as one pure function (d0063, d0064): validation before Review and Auto-merge, on any
//! agent check of a project's flow (STG-06).
//!
//! [`transition`] takes where a ticket stands ([`ValidationState`]) and what happened ([`ValidationEvent`]) and
//! says where it goes, how it entered Review, what happens to the round counter, and whether the validator's
//! findings go to the agent. Where the ticket goes is the flow engine's ([`crate::flow::engine::step`]); this adds
//! the validation's own bookkeeping and Auto-merge, which keeps today's behaviour on top of the stages until M33.
//! It does no I/O: the service carries the result out (T-0183 … T-0189), and every row is tested on its own.

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::stage_runs;
use super::tickets::{EventKind, Ticket, record_event};
use super::{Board, BoardError, Refusal};
use crate::domain::{
    Actor, AutoMergeStop, Place, ReviewEntry, Status, ValidationOutcome, ValidationTrigger,
};
use crate::flow::Flow;
use crate::flow::engine::{self, At, Move, RunResult};
use crate::store::tickets::set_state_since;
use crate::store::{new_id, now_ms};

/// Conflicts in a row before Auto-merge stops trying (AMG-03).
pub const AUTO_MERGE_CONFLICT_LIMIT: u32 = 3;

/// What the rules need to know about a ticket.
#[derive(Debug, Clone, Copy)]
pub struct ValidationState<'a> {
    /// The project's flow (the derived flow until M33: Validation off has no Validate stage).
    pub flow: &'a Flow,
    pub place: &'a Place,
    pub status: Status,
    /// Rounds counted so far (`tickets.validation_round`).
    pub round: u32,
    /// The check's rounds plus the extra rounds the user allowed.
    pub rounds: u32,
    /// The project's Auto-merge applies to this ticket (its kind and the setting; never an epic).
    pub auto_merge: bool,
    /// Auto-merge conflicts in a row so far.
    pub conflicts: u32,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationEvent {
    /// The agent's `review submit`.
    AgentSubmit,
    /// The user's Validate and submit.
    ValidateAndSubmit,
    /// The user's Submit for review: skips validation (d0052, d0063).
    SubmitByUser,
    Pass,
    Fail,
    /// The validator's turn ended without `validate pass|fail`.
    NoVerdict,
    /// The validator changed tracked files or committed (VAL-02).
    ValidatorChangedFiles,
    /// The validator waits on a permission prompt.
    ValidatorNeedsYou,
    /// The user typed into the waiting validator.
    ValidatorContinues,
    /// Tracked files or the head changed through the agent, or the user edited the description (VAL-08).
    CodeChanged,
    StopValidation,
    SkipToReview,
    MoveToReviewAnyway,
    AnotherRound,
    AnswerAgent,
    RetryValidation,
    /// Auto-merge's Local merge landed.
    AutoMerged,
    /// Auto-merge's rebase conflicted.
    AutoMergeConflict,
    /// Auto-merge couldn't run: the base diverged or the user's checkout is dirty.
    AutoMergeBlocked(AutoMergeStop),
    RetryMerge,
    /// From an Auto-merge stop: into Ready for Review, still "Validated · round N".
    MoveToReview,
}

/// What a ticket's round counter does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    Keep,
    /// The round that just ended counts.
    Count,
    /// Back to 0 (Answer the agent, an Auto-merge conflict).
    Reset,
}

/// The result of a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub to: Status,
    /// Where the ticket is afterwards (its stage).
    pub place: Place,
    /// Set when the ticket enters Ready for Review.
    pub entry: Option<ReviewEntry>,
    pub round: Round,
    /// How the open validation round ends, if one does.
    pub outcome: Option<ValidationOutcome>,
    /// A fresh validator starts (submit, Retry validation).
    pub start_validator: bool,
    /// The findings go to the agent as an automatic message (VAL-04, Give it another round, Answer the agent).
    pub deliver_findings: bool,
    /// The agent is told the validation was cancelled because the code changed (VAL-08).
    pub tell_cancelled: bool,
    /// Run the Local merge without the confirmation (AMG-02).
    pub auto_merge: bool,
    /// Hand the conflict to the agent with the restack skill, no click (AMG-03).
    pub restack: bool,
    /// One more round is allowed (Give it another round).
    pub extra_round: bool,
    /// Why Auto-merge stopped, when it did.
    pub stop: Option<AutoMergeStop>,
    /// Stages walked over without running (Submit for review skips the checks): each gets a skipped run.
    pub skipped: Vec<String>,
}

impl Transition {
    fn at(to: Status, place: Place) -> Self {
        Self {
            to,
            place,
            entry: None,
            round: Round::Keep,
            outcome: None,
            start_validator: false,
            deliver_findings: false,
            tell_cancelled: false,
            auto_merge: false,
            restack: false,
            extra_round: false,
            stop: None,
            skipped: Vec::new(),
        }
    }

    /// Where the engine's step goes.
    fn of(step: &engine::Step<'_>) -> Self {
        Self {
            start_validator: step.start_check,
            skipped: step
                .passed_through
                .iter()
                .filter(|(_, result)| *result == RunResult::Skipped)
                .map(|(stage, _)| stage.id.clone())
                .collect(),
            ..Self::at(step.status, step.place.clone())
        }
    }

    /// Staying where the ticket is, with another status.
    fn stay(s: &ValidationState<'_>, to: Status) -> Self {
        Self::at(to, s.place.clone())
    }

    fn review(mut self, entry: ReviewEntry) -> Self {
        self.entry = Some(entry);
        self
    }

    fn ended(mut self, outcome: ValidationOutcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    fn round(mut self, round: Round) -> Self {
        self.round = round;
        self
    }

    /// Whether the ticket enters a human check (today's Ready for Review).
    pub fn enters_review(&self) -> bool {
        self.to == Status::WaitingForYou
    }
}

/// Why an event doesn't apply where the ticket stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotNow(pub String);

/// Agent Working, with or without the needs-you family: where a submit is accepted.
fn agent_works(status: Status) -> bool {
    matches!(
        status,
        Status::Working
            | Status::WorkingInBackground
            | Status::AsksYou
            | Status::NeedsYou
            | Status::Idle
            | Status::MergeConflictFixing
    )
}

fn validating(status: Status) -> bool {
    matches!(status, Status::Checking | Status::CheckerNeedsYou)
}

/// One M12 row of SDD §7.3.
pub fn transition(s: ValidationState<'_>, event: ValidationEvent) -> Result<Transition, NotNow> {
    use ValidationEvent as E;
    let not_now = || {
        NotNow(format!(
            "{event:?} doesn't apply while the ticket is {}",
            s.status.label()
        ))
    };
    let at = At {
        fails: s.round,
        rounds: s.rounds,
        ..At::new(s.flow, s.place, s.status)
    };
    let go = |mv: Move| {
        engine::step(at, mv)
            .map(|step| Transition::of(&step))
            .map_err(|_| not_now())
    };
    // Where Auto-merge's stops wait (in the Agent stage that would fix the conflict, as today's card does in Dev)
    // and where its retries go (the human check after it).
    let agent_stage = || {
        s.place
            .stage()
            .and_then(|id| engine::back_to(s.flow, id, &[]))
            .ok_or_else(not_now)
    };
    let review_stage = || {
        s.place
            .stage()
            .and_then(|id| engine::next_human_check(s.flow, id))
            .ok_or_else(not_now)
    };
    let to_stage = |stage: &crate::flow::Stage, status: Status| {
        Transition::at(status, Place::stage_of(&stage.id))
    };
    Ok(match event {
        E::AgentSubmit | E::ValidateAndSubmit if agent_works(s.status) => {
            let t = go(Move::NextStage)?;
            // A fix after an Auto-merge conflict validates again with the counter at 0 (AMG-03).
            if t.start_validator && s.status == Status::MergeConflictFixing {
                t.round(Round::Reset)
            } else {
                t
            }
        }
        E::SubmitByUser if agent_works(s.status) => {
            go(Move::SubmitByUser)?.review(ReviewEntry::SubmittedByUser)
        }
        E::Pass if validating(s.status) => {
            let mut t = go(Move::Pass)?.round(Round::Count);
            if t.enters_review() {
                t.entry = Some(ReviewEntry::Validated);
                t.auto_merge = s.auto_merge;
            }
            t.ended(ValidationOutcome::Pass)
        }
        E::Fail if validating(s.status) => {
            let mut t = go(Move::Fail)?.round(Round::Count);
            t.deliver_findings = t.to != Status::CheckFailed;
            t.ended(ValidationOutcome::Fail)
        }
        E::NoVerdict if validating(s.status) => {
            go(Move::NoResult)?.ended(ValidationOutcome::NoResult)
        }
        E::ValidatorChangedFiles if validating(s.status) => {
            go(Move::NoResult)?.ended(ValidationOutcome::Discarded)
        }
        E::ValidatorNeedsYou if s.status == Status::Checking => {
            Transition::stay(&s, Status::CheckerNeedsYou)
        }
        E::ValidatorContinues if s.status == Status::CheckerNeedsYou => {
            Transition::stay(&s, Status::Checking)
        }
        E::CodeChanged if validating(s.status) => Transition {
            tell_cancelled: true,
            ..go(Move::CancelCheck)?
        }
        .ended(ValidationOutcome::Cancelled),
        E::StopValidation if validating(s.status) => {
            go(Move::CancelCheck)?.ended(ValidationOutcome::Stopped)
        }
        E::SkipToReview if validating(s.status) => go(Move::SkipCheck)?
            .review(ReviewEntry::Skipped)
            .ended(ValidationOutcome::Skipped),
        E::MoveToReviewAnyway if s.status == Status::CheckFailed => {
            go(Move::MoveOnAnyway)?.review(ReviewEntry::FailedMoved)
        }
        // No verdict: "Validation skipped by you" (d0063 changelog).
        E::MoveToReviewAnyway if s.status == Status::CheckNoResult => {
            go(Move::MoveOnAnyway)?.review(ReviewEntry::Skipped)
        }
        E::AnotherRound if s.status == Status::CheckFailed => Transition {
            deliver_findings: true,
            extra_round: true,
            ..go(Move::AnotherRound)?
        },
        E::AnswerAgent if s.status == Status::CheckFailed => Transition {
            deliver_findings: true,
            ..go(Move::AnswerAgent)?
        }
        .round(Round::Reset),
        E::RetryValidation if s.status == Status::CheckNoResult => go(Move::RetryCheck)?,
        E::AutoMerged if s.status == Status::WaitingForYou => go(Move::Delivered)?,
        E::AutoMergeConflict if s.status == Status::WaitingForYou => {
            let stage = agent_stage()?;
            if s.conflicts + 1 >= AUTO_MERGE_CONFLICT_LIMIT {
                Transition {
                    stop: Some(AutoMergeStop::KeepsConflicting),
                    ..to_stage(stage, Status::DeliveryStopped)
                }
            } else {
                Transition {
                    restack: true,
                    ..to_stage(stage, Status::MergeConflictFixing)
                }
                .round(Round::Reset)
            }
        }
        E::AutoMergeBlocked(stop) if s.status == Status::WaitingForYou => Transition {
            stop: Some(stop),
            ..to_stage(agent_stage()?, Status::DeliveryStopped)
        },
        E::RetryMerge if s.status == Status::DeliveryStopped => Transition {
            auto_merge: true,
            ..to_stage(review_stage()?, Status::WaitingForYou)
        },
        E::MoveToReview if s.status == Status::DeliveryStopped => {
            to_stage(review_stage()?, Status::WaitingForYou)
        }
        _ => return Err(not_now()),
    })
}

impl Board {
    /// Every validation round of the ticket, newest first (History, VAL-12). Tokens are left for the service.
    pub async fn validations(&self, key: &str) -> Result<Vec<ValidationRecord>, BoardError> {
        let id = self.ticket_id(key).await?;
        type Row = (
            i64,
            String,
            Option<String>,
            i64,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT round, trigger, outcome, started_at, ended_at, session_id, report, notes_json FROM validations
             WHERE ticket_id = ? ORDER BY started_at DESC, id DESC",
        )
        .bind(&id)
        .fetch_all(self.store.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(round, trigger, outcome, started_at, ended_at, session, report, notes)| {
                    ValidationRecord {
                        round: u32::try_from(round).unwrap_or(0),
                        trigger: trigger.parse().unwrap_or(ValidationTrigger::Agent),
                        outcome: outcome.and_then(|o| o.parse().ok()),
                        started_at,
                        duration_ms: ended_at.map(|end| (end - started_at).max(0)),
                        ended_at,
                        session,
                        report,
                        notes: notes
                            .and_then(|n| serde_json::from_str(&n).ok())
                            .unwrap_or_default(),
                        tokens: None,
                    }
                },
            )
            .collect())
    }

    /// **Create ticket** from a "Not covered by any ticket" note of the ticket's validation reports (VAL-12, the
    /// user's click; the validator stays read-only): a prefilled Backlog ticket in the same epic.
    pub async fn create_from_note(
        &self,
        key: &str,
        note: &str,
        actor: &Actor,
    ) -> Result<Ticket, BoardError> {
        let source = self.ticket(key).await?;
        let known = self
            .validations(key)
            .await?
            .iter()
            .any(|r| r.notes.iter().any(|n| n.trim() == note.trim()));
        let input = ticket_from_note(&source, note)
            .filter(|_| known)
            .ok_or_else(|| {
                BoardError::NotFound(format!(
                    "\"{}\" isn't a \"{}\" note of {}'s validation",
                    note.trim(),
                    NOT_COVERED.trim_end_matches(':'),
                    source.key
                ))
            })?;
        let created = self.create_ticket(&input, actor).await?;
        tracing::info!(source = %source.key, created = %created.key, "ticket created from a validation note");
        Ok(created)
    }

    /// Opens a validation round for a submit (VAL-01): records the summary and head like any submit, opens the
    /// round in `validations` and sets Validating… (or "Waiting for limit reset" while the usage limit holds,
    /// marked as a validation that waits).
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn enter_validation(
        &self,
        ticket: &Ticket,
        step: &Transition,
        head: &str,
        summary: &str,
        trigger: ValidationTrigger,
        held: bool,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let counted = if step.round == Round::Reset {
            0
        } else {
            ticket.validation_round
        };
        let status = if held {
            Status::WaitingForLimitReset
        } else {
            step.to
        };
        let mut tx = self.store.begin_write().await?;
        stage_runs::end_open(&mut tx, &ticket.id, RunResult::Done, Some(summary)).await?;
        sqlx::query(concat!(
            "UPDATE tickets SET stage_id = ?, state = ?, review_entry = NULL, validation_round = ?,
                                interrupted_from = ?, signal_text = NULL, signal_since = NULL, wakeup_at = NULL,
                                resume_skill = NULL, conflicts_with = NULL,
                                conflict_files = NULL, conflict_reason = NULL, auto_merge_stop = NULL,
                                updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(step.place.stage_id())
        .bind(status.as_str())
        .bind(i64::from(counted))
        .bind(held.then_some(Status::Checking.as_str()))
        .bind(actor.to_string())
        .bind(now)
        .bind(status.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO validations (id, ticket_id, round, trigger, started_at, head_sha) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&ticket.id)
        .bind(i64::from(counted + 1))
        .bind(trigger.as_str())
        .bind(now)
        .bind(head)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::ReviewSubmitted,
            actor,
            json!({ "summary": summary.trim(), "head": head, "validation": true }),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, %actor, round = counted + 1, held, "validation opened");
        Ok(())
    }

    /// The validator of the ticket's open round started: its session is stored on the round, and a validation
    /// that waited for the usage limit shows Validating… again.
    pub(crate) async fn validator_started(
        &self,
        ticket_id: &str,
        session_id: &str,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let mut tx = self.store.begin_write().await?;
        sqlx::query(
            "UPDATE validations SET session_id = ? WHERE id = (
                 SELECT id FROM validations WHERE ticket_id = ? AND ended_at IS NULL
                 ORDER BY started_at DESC, id DESC LIMIT 1)",
        )
        .bind(session_id)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        let validating = Status::Checking;
        sqlx::query(concat!(
            "UPDATE tickets SET state = ?, interrupted_from = NULL, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ? AND state = ? AND interrupted_from = ?"
        ))
        .bind(validating.as_str())
        .bind(now)
        .bind(validating.as_str())
        .bind(now)
        .bind(ticket_id)
        .bind(Status::WaitingForLimitReset.as_str())
        .bind(validating.as_str())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The validator couldn't start: "Validation ended without a result" with the reason, the round ends without
    /// one (it doesn't count) and Retry validation or Move to Review anyway are the user's (VAL-07).
    pub(crate) async fn validator_did_not_start(
        &self,
        ticket_id: &str,
        reason: &str,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let to = Status::CheckNoResult;
        let mut tx = self.store.begin_write().await?;
        sqlx::query(
            "UPDATE validations SET ended_at = ?, outcome = ?, report = ? WHERE ticket_id = ? AND ended_at IS NULL",
        )
        .bind(now)
        .bind(ValidationOutcome::NoResult.as_str())
        .bind(format!("The validator didn't start: {reason}"))
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(concat!(
            "UPDATE tickets SET state = ?, signal_text = ?, signal_since = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.as_str())
        .bind(format!("The validator didn't start: {reason}"))
        .bind(now)
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The open round of a ticket: its id, number and validator session.
    pub(crate) async fn open_round(
        &self,
        ticket_id: &str,
    ) -> Result<Option<(String, u32, Option<String>, Option<String>)>, BoardError> {
        let row: Option<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, round, session_id, head_sha FROM validations WHERE ticket_id = ? AND ended_at IS NULL
             ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .bind(ticket_id)
        .fetch_optional(self.store.pool())
        .await?;
        Ok(row.map(|(id, round, session, head)| {
            (id, u32::try_from(round).unwrap_or(0), session, head)
        }))
    }

    /// Carries out a verdict's row (VAL-03 … VAL-06): the round ends with its report and notes, the ticket moves,
    /// the counter counts, and a pass enters Review with the round's head as the reviewed head.
    pub(crate) async fn record_verdict(
        &self,
        ticket: &Ticket,
        step: &Transition,
        report: &str,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        self.end_round(ticket, step, report, None, actor).await
    }

    /// Ends the ticket's open round as `step` says (a verdict, a cancel, a discard, a stop…). `signal` is the
    /// card's text for the new status ("Validation discarded: the validator changed files").
    pub(crate) async fn end_round(
        &self,
        ticket: &Ticket,
        step: &Transition,
        report: &str,
        signal: Option<&str>,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let Some((round_id, round, _, head)) = self.open_round(&ticket.id).await? else {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!("{} has no validation running", ticket.key),
            ));
        };
        let outcome = step.outcome.unwrap_or(ValidationOutcome::NoResult);
        let counted = match step.round {
            Round::Count => round,
            Round::Reset => 0,
            Round::Keep => ticket.validation_round,
        };
        let now = now_ms();
        let notes: Vec<&str> = report.lines().filter_map(note_of).collect();
        let review = step.enters_review();
        let mut tx = self.store.begin_write().await?;
        sqlx::query(
            "UPDATE validations SET ended_at = ?, outcome = ?, report = ?, notes_json = ? WHERE id = ?",
        )
        .bind(now)
        .bind(outcome.as_str())
        .bind(Some(report.trim()).filter(|r| !r.is_empty()))
        .bind(serde_json::to_string(&notes).expect("notes serialize"))
        .bind(&round_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(concat!(
            "UPDATE tickets SET stage_id = ?, state = ?, validation_round = ?, review_entry = ?,
                                review_head_sha = CASE WHEN ? THEN ? ELSE review_head_sha END,
                                review_entered_at = CASE WHEN ? THEN ? ELSE review_entered_at END,
                                sent_back_at = CASE WHEN ? THEN NULL ELSE sent_back_at END,
                                pr_rework = CASE WHEN ? THEN FALSE ELSE pr_rework END,
                                signal_text = ?, signal_since = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(step.place.stage_id())
        .bind(step.to.as_str())
        .bind(i64::from(counted))
        .bind(step.entry.map(|e| e.as_str()))
        .bind(review)
        .bind(head.as_deref())
        .bind(review)
        .bind(now)
        .bind(review)
        .bind(review)
        .bind(signal)
        .bind(signal.map(|_| now))
        .bind(now)
        .bind(step.to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::ValidationEnded,
            actor,
            json!({"round": round, "outcome": outcome, "counted": outcome.counts()}),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(key = %ticket.key, round, %outcome, to = %step.to, "validation round ended");
        Ok(())
    }

    /// Whether tracked files or the head of the ticket's worktree differ from `head` (VAL-02, VAL-08). Files ignored
    /// by `.gitignore` and untracked files don't count; nothing in the worktree is touched (never a stash).
    pub(crate) async fn worktree_changed(
        &self,
        ticket: &Ticket,
        head: &str,
    ) -> Result<bool, BoardError> {
        let Some(worktree) = ticket.work.as_ref().and_then(|w| w.worktree.clone()) else {
            return Ok(false);
        };
        let git = self.git()?;
        let dir = std::path::Path::new(&worktree);
        let now = git.run(dir, &["rev-parse", "HEAD"]).await?;
        if now.trim() != head {
            return Ok(true);
        }
        let tracked = git
            .run(dir, &["status", "--porcelain", "--untracked-files=no"])
            .await?;
        Ok(!tracked.trim().is_empty())
    }

    /// Moves a validating ticket between the states its validator's hooks set (VAL-07, VAL-11), only if it is
    /// still in `from`. `behind` marks a validation that waits (the usage limit).
    pub(crate) async fn set_validation_status(
        &self,
        ticket_id: &str,
        from: &[Status],
        to: Status,
        text: Option<&str>,
        behind: bool,
    ) -> Result<bool, BoardError> {
        let now = now_ms();
        // A held stop handed back moves from the Agent stage into its check again; a validation that waits stays.
        let ticket = self.ticket_by_id(ticket_id).await?;
        let flow = self.effective_flow(ticket.key.project.as_str()).await?;
        let stage = (!behind)
            .then(|| engine::stage_for(&flow, &ticket.place, to))
            .flatten()
            .map(|s| s.id.clone());
        let from: Vec<&str> = from.iter().map(|s| s.as_str()).collect();
        let sql = format!(
            concat!(
                "UPDATE tickets SET stage_id = COALESCE(?, stage_id), state = ?, signal_text = ?, signal_since = ?,
                                    interrupted_from = ?, updated_at = ?, ",
                set_state_since!(),
                " WHERE id = ? AND state IN ({})"
            ),
            vec!["?"; from.len()].join(", ")
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(stage)
            .bind(to.as_str())
            .bind(text)
            .bind(text.map(|_| now))
            .bind(behind.then_some(Status::Checking.as_str()))
            .bind(now)
            .bind(to.as_str())
            .bind(now)
            .bind(ticket_id);
        for status in from {
            query = query.bind(status);
        }
        Ok(query.execute(self.store.pool()).await?.rows_affected() > 0)
    }

    /// Into Ready for Review without a verdict, by the user (Move to Review anyway, VAL-06, VAL-07): the last
    /// round's head is the reviewed head; never auto-merged. The ticket goes to the first human check after its
    /// stage (the default flow's Review).
    pub(crate) async fn move_to_review(
        &self,
        ticket: &Ticket,
        entry: Option<ReviewEntry>,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let flow = self.effective_flow(ticket.key.project.as_str()).await?;
        let stage = ticket
            .place
            .stage()
            .and_then(|id| engine::next_human_check(&flow, id))
            .or_else(|| flow.stages.iter().rev().find(|s| s.is_human_check()))
            .map(|s| s.id.clone());
        let head: Option<Option<String>> = sqlx::query_scalar(
            "SELECT head_sha FROM validations WHERE ticket_id = ? ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .bind(&ticket.id)
        .fetch_optional(self.store.pool())
        .await?;
        let now = now_ms();
        let to = Status::WaitingForYou;
        let mut tx = self.store.begin_write().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET stage_id = ?, state = ?, review_head_sha = COALESCE(?, review_head_sha),
                                review_entered_at = ?, review_entry = COALESCE(?, review_entry),
                                sent_back_at = NULL, pr_rework = FALSE, auto_merge_stop = NULL,
                                signal_text = NULL, signal_since = NULL, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(stage)
        .bind(to.as_str())
        .bind(head.flatten())
        .bind(now)
        .bind(entry.map(|e| e.as_str()))
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Moved,
            actor,
            json!({"from": ticket.status, "to": to}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Back to the agent after the last failed round (Give it another round, Answer the agent): Agent Working in
    /// the stage `step` names, one more round allowed or the counter at 0.
    pub(crate) async fn back_to_agent(
        &self,
        ticket: &Ticket,
        step: &Transition,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let (extra_round, reset) = (step.extra_round, step.round == Round::Reset);
        let now = now_ms();
        let to = Status::Working;
        let mut tx = self.store.begin_write().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET stage_id = ?, state = ?,
                                validation_round = CASE WHEN ? THEN 0 ELSE validation_round END,
                                validation_rounds_extra = CASE WHEN ? THEN 0 ELSE validation_rounds_extra + ? END,
                                signal_text = NULL, signal_since = NULL, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(step.place.stage_id())
        .bind(to.as_str())
        .bind(reset)
        .bind(reset)
        .bind(i64::from(extra_round))
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::Moved,
            actor,
            json!({"from": ticket.status, "to": to}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The last ended round's report (its findings), for Give it another round and Answer the agent.
    pub(crate) async fn last_report(&self, ticket_id: &str) -> Result<Option<String>, BoardError> {
        let report: Option<Option<String>> = sqlx::query_scalar(
            "SELECT report FROM validations WHERE ticket_id = ? AND ended_at IS NOT NULL
             ORDER BY started_at DESC, id DESC LIMIT 1",
        )
        .bind(ticket_id)
        .fetch_optional(self.store.pool())
        .await?;
        Ok(report.flatten())
    }

    /// Ends whatever round is open with `outcome`, the ticket's status left alone (Park, Close).
    pub(crate) async fn abandon_round(
        &self,
        ticket_id: &str,
        outcome: ValidationOutcome,
    ) -> Result<bool, BoardError> {
        let ended = sqlx::query(
            "UPDATE validations SET ended_at = ?, outcome = ? WHERE ticket_id = ? AND ended_at IS NULL",
        )
        .bind(now_ms())
        .bind(outcome.as_str())
        .bind(ticket_id)
        .execute(self.store.pool())
        .await?
        .rows_affected();
        Ok(ended > 0)
    }

    /// A fresh round for Retry validation (VAL-07): the same round number, not counted; the ticket validates
    /// again.
    pub(crate) async fn reopen_round(
        &self,
        ticket: &Ticket,
        head: &str,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let to = Status::Checking;
        let mut tx = self.store.begin_write().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET state = ?, signal_text = NULL, signal_since = NULL,
                                interrupted_from = NULL, updated_by = ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.as_str())
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO validations (id, ticket_id, round, trigger, started_at, head_sha) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&ticket.id)
        .bind(i64::from(ticket.validation_round + 1))
        .bind(ValidationTrigger::User.as_str())
        .bind(now)
        .bind(head)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Auto-merge landed (AMG-02): "Merged automatically", the conflict count back at 0, a History line.
    pub(crate) async fn record_auto_merged(
        &self,
        ticket_id: &str,
        target: &str,
    ) -> Result<(), BoardError> {
        let mut tx = self.store.begin_write().await?;
        sqlx::query(
            "UPDATE tickets SET merged_automatically = TRUE, auto_merge_conflicts = 0, auto_merge_stop = NULL
             WHERE id = ?",
        )
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            ticket_id,
            EventKind::MergedAutomatically,
            &Actor::current_user(),
            json!({ "target": target }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Auto-merge's conflict went back to the agent (AMG-03): "Merge conflict · fixing", one more in a row.
    pub(crate) async fn auto_merge_conflict(&self, ticket_id: &str) -> Result<(), BoardError> {
        let now = now_ms();
        let to = Status::MergeConflictFixing;
        sqlx::query(concat!(
            "UPDATE tickets SET auto_merge_conflicts = auto_merge_conflicts + 1,
                                state = CASE WHEN state = ? THEN ? ELSE state END, updated_at = ?, ",
            "state_since = CASE WHEN state = ? THEN ? ELSE state_since END",
            " WHERE id = ?"
        ))
        .bind(Status::Working.as_str())
        .bind(to.as_str())
        .bind(now)
        .bind(Status::Working.as_str())
        .bind(now)
        .bind(ticket_id)
        .execute(self.store.pool())
        .await?;
        Ok(())
    }

    /// Auto-merge stopped (AMG-03, AMG-04): turquoise with the reason as its text. `conflicted` counts this conflict.
    pub(crate) async fn auto_merge_stopped(
        &self,
        ticket_id: &str,
        stop: AutoMergeStop,
        message: &str,
        conflicted: bool,
    ) -> Result<(), BoardError> {
        let now = now_ms();
        let to = Status::DeliveryStopped;
        // It stops in the Agent stage whose session would fix the conflict (rule 12 of d0108's second gap review).
        let ticket = self.ticket_by_id(ticket_id).await?;
        let flow = self.effective_flow(ticket.key.project.as_str()).await?;
        let stage = match ticket.place.stage() {
            Some(id) if ticket.status.at_work() => Some(id.to_owned()),
            Some(id) => engine::back_to(&flow, id, &[]).map(|s| s.id.clone()),
            None => None,
        }
        .or_else(|| flow.first_agent_stage().map(|s| s.id.clone()));
        let mut tx = self.store.begin_write().await?;
        sqlx::query(concat!(
            "UPDATE tickets SET stage_id = ?, state = ?, auto_merge_stop = ?, signal_text = ?, signal_since = ?,
                                auto_merge_conflicts = auto_merge_conflicts + ?, updated_at = ?, ",
            set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(stage)
        .bind(to.as_str())
        .bind(stop.as_str())
        .bind(message)
        .bind(now)
        .bind(i64::from(conflicted))
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            ticket_id,
            EventKind::AutoMergeStopped,
            &Actor::current_user(),
            json!({ "stop": stop, "message": message }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Extra rounds the user allowed with "Give it another round".
    pub(crate) async fn validation_rounds_extra(&self, ticket_id: &str) -> Result<u32, BoardError> {
        let extra: Option<i64> =
            sqlx::query_scalar("SELECT validation_rounds_extra FROM tickets WHERE id = ?")
                .bind(ticket_id)
                .fetch_optional(self.store.pool())
                .await?;
        Ok(extra.and_then(|n| u32::try_from(n).ok()).unwrap_or(0))
    }

    /// Whether the round this validator session judged has ended (its verdict is in).
    pub(crate) async fn round_of_session_ended(&self, session: &str) -> Result<bool, BoardError> {
        let ended: Option<Option<i64>> =
            sqlx::query_scalar("SELECT ended_at FROM validations WHERE session_id = ?")
                .bind(session)
                .fetch_optional(self.store.pool())
                .await?;
        Ok(matches!(ended, Some(Some(_))))
    }

    /// Tickets whose validation waits for the usage limit to reset (SCH-04).
    pub(crate) async fn validations_waiting_for_limit(&self) -> Result<Vec<String>, BoardError> {
        let ids: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM tickets WHERE state = ? AND interrupted_from = ?")
                .bind(Status::WaitingForLimitReset.as_str())
                .bind(Status::Checking.as_str())
                .fetch_all(self.store.pool())
                .await?;
        Ok(ids.into_iter().map(|(id,)| id).collect())
    }
}

/// One validation round as History shows it (VAL-12, d0063): its outcome, duration, report and notes. Tokens are
/// its validator session's, added by the service from the usage counts (`ticket.validations`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationRecord {
    pub round: u32,
    pub trigger: ValidationTrigger,
    /// `None` while the round runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<ValidationOutcome>,
    pub started_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    /// From start to end, once the round ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// The validator's session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The validator's whole report (a verdict's), else why the round ended without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// The report's notes ("Not covered by any ticket: refunds endpoint").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// The validator's tokens; `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<crate::usage::Tokens>,
}

/// The start of a note about a gap no ticket covers (the validate skill's wording, d0063).
pub const NOT_COVERED: &str = "Not covered by any ticket:";

/// The gap of a "Not covered by any ticket: refunds endpoint" note ("refunds endpoint"); `None` for other notes.
pub fn not_covered(note: &str) -> Option<&str> {
    let note = note.trim();
    let head = note.get(..NOT_COVERED.len())?;
    if !head.eq_ignore_ascii_case(NOT_COVERED) {
        return None;
    }
    let gap = note[NOT_COVERED.len()..]
        .trim()
        .trim_end_matches('.')
        .trim();
    (!gap.is_empty()).then_some(gap)
}

/// The prefilled ticket for a "Not covered by any ticket" note of `source`'s validation (VAL-12): the gap as the
/// title (first letter upper-case), a description naming the source ticket and quoting the note, in the source's
/// epic. `None` when the note isn't about an uncovered gap.
pub fn ticket_from_note(source: &Ticket, note: &str) -> Option<super::tickets::NewTicketInput> {
    let gap = not_covered(note)?;
    let mut chars = gap.chars();
    let title = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect::<String>())
        .unwrap_or_default();
    Some(super::tickets::NewTicketInput {
        project: source.key.project.to_string(),
        title,
        body: format!(
            "Noted while validating {} ({}):\n\n> {}\n",
            source.key,
            source.title,
            note.trim()
        ),
        epic: source.epic.as_ref().map(ToString::to_string),
        ..super::tickets::NewTicketInput::default()
    })
}

/// A report line that is a note, not a finding: "Note: covered by SHOP-40.2", "- Note: Not covered by any
/// ticket: removing items" (the validate skill's format).
pub fn note_of(line: &str) -> Option<&str> {
    let line = line.trim().trim_start_matches(['-', '*']).trim_start();
    let rest = line
        .strip_prefix("Note:")
        .or_else(|| line.strip_prefix("note:"))?;
    let note = rest.trim();
    (!note.is_empty()).then_some(note)
}

/// The findings sent back to the agent (VAL-04): "Validation failed (round 1 of 3). Fix these, commit, and
/// submit again: …".
pub fn findings_message(round: u32, rounds: u32, report: &str) -> String {
    format!(
        "Validation failed (round {round} of {rounds}). Fix these, commit, and submit again:\n\n{}",
        report.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ValidationEvent as E;

    fn state(status: Status) -> ValidationState<'static> {
        ValidationState {
            flow: &crate::flow::default_file().flow,
            place: Box::leak(Box::new(Place::of(status, None, None))),
            status,
            round: 0,
            rounds: 3,
            auto_merge: false,
            conflicts: 0,
        }
    }

    /// Validation off: the derived flow has no Validate stage.
    fn without_validation(s: ValidationState<'static>) -> ValidationState<'static> {
        let mut flow = crate::flow::default_flow();
        flow.stages
            .retain(|s| s.id != crate::flow::default::VALIDATE);
        ValidationState {
            flow: Box::leak(Box::new(flow)),
            ..s
        }
    }

    /// T-0195 (VAL-12): only "Not covered by any ticket" notes make a ticket; the gap is its title.
    #[test]
    fn a_ticket_from_a_not_covered_note() {
        assert_eq!(
            not_covered("Not covered by any ticket: refunds endpoint."),
            Some("refunds endpoint")
        );
        assert_eq!(
            not_covered("not covered by any ticket:removing items"),
            Some("removing items")
        );
        assert_eq!(not_covered("covered by SHOP-40.2"), None);
        assert_eq!(not_covered("Not covered by any ticket:"), None);
        let source = Ticket {
            id: "t401".into(),
            key: "SHOP-40.1".parse().unwrap(),
            kind: crate::domain::Kind::Ticket,
            title: "Save items".into(),
            body: String::new(),
            priority: Default::default(),
            priority_inherited: true,
            epic: Some("SHOP-40".parse().unwrap()),
            place: Place::stage_of(crate::flow::default::REVIEW),
            status: Status::WaitingForYou,
            base: crate::board::tickets::Base::Branch("main".into()),
            base_locked: true,
            external_id: None,
            depends_on: vec![],
            aliases: vec![],
            next_child_number: None,
            flags: crate::domain::Flags::default(),
            created_by: Actor::User("me".into()),
            updated_by: None,
            started_at: Some(1),
            created_at: 0,
            updated_at: 0,
            status_since: None,
            queued_at: None,
            ready_since: None,
            validation_round: 1,
            auto_merge_conflicts: 0,
            auto_merge_stop: None,
            work: None,
            start_problem: None,
            interrupted_from: None,
            signal: None,
            question: None,
            pushed_differs: None,
            epic_queue_mode: Default::default(),
            joined_queue_by: None,
        };
        let input = ticket_from_note(&source, "Not covered by any ticket: removing items").unwrap();
        assert_eq!(input.title, "Removing items");
        assert_eq!(input.project, "SHOP");
        assert_eq!(input.epic.as_deref(), Some("SHOP-40"));
        assert!(input.body.contains("SHOP-40.1 (Save items)"));
        assert!(ticket_from_note(&source, "covered by SHOP-40.2").is_none());
    }

    fn go(s: ValidationState, event: ValidationEvent) -> Transition {
        transition(s, event).unwrap_or_else(|e| panic!("{e:?}"))
    }

    /// Working → Validating on the agent's submit and on Validate and submit; with validation off, straight to
    /// Ready for Review.
    #[test]
    fn submit_starts_a_validator() {
        for event in [E::AgentSubmit, E::ValidateAndSubmit] {
            let t = go(state(Status::Working), event);
            assert_eq!(t.to, Status::Checking);
            assert!(t.start_validator && t.round == Round::Keep && t.outcome.is_none());
        }
        let off = without_validation(state(Status::Working));
        let t = go(off, E::AgentSubmit);
        assert_eq!((t.to, t.entry), (Status::WaitingForYou, None));
        assert!(!t.start_validator);
        assert!(transition(state(Status::Checking), E::AgentSubmit).is_err());
    }

    /// Submit for review skips validation: "Submitted by you", never auto-merged.
    #[test]
    fn submit_by_user_skips_validation() {
        let t = go(
            ValidationState {
                auto_merge: true,
                ..state(Status::Working)
            },
            E::SubmitByUser,
        );
        assert_eq!(
            (t.to, t.entry, t.auto_merge),
            (
                Status::WaitingForYou,
                Some(ReviewEntry::SubmittedByUser),
                false
            )
        );
        assert!(transition(state(Status::Checking), E::SubmitByUser).is_err());
    }

    /// Validating → Ready for Review "Validated · round N" on pass; with Auto-merge, the merge runs.
    #[test]
    fn pass() {
        let t = go(state(Status::Checking), E::Pass);
        assert_eq!(
            (t.to, t.entry, t.round, t.outcome, t.auto_merge),
            (
                Status::WaitingForYou,
                Some(ReviewEntry::Validated),
                Round::Count,
                Some(ValidationOutcome::Pass),
                false
            )
        );
        let auto = ValidationState {
            auto_merge: true,
            ..state(Status::Checking)
        };
        assert!(go(auto, E::Pass).auto_merge);
    }

    /// Fail with rounds left → back to the agent with the findings; the last round → turquoise, nothing delivered.
    #[test]
    fn fail_and_the_last_fail() {
        let t = go(state(Status::Checking), E::Fail);
        assert_eq!(
            (t.to, t.deliver_findings, t.round),
            (Status::Working, true, Round::Count)
        );
        let last = ValidationState {
            round: 2,
            ..state(Status::Checking)
        };
        let t = go(last, E::Fail);
        assert_eq!(
            (t.to, t.deliver_findings, t.round, t.outcome),
            (
                Status::CheckFailed,
                false,
                Round::Count,
                Some(ValidationOutcome::Fail)
            )
        );
        // One round: the first failure is the last; five rounds: the fourth isn't.
        let one = ValidationState {
            rounds: 1,
            ..state(Status::Checking)
        };
        assert_eq!(go(one, E::Fail).to, Status::CheckFailed);
        let five = ValidationState {
            rounds: 5,
            round: 3,
            ..state(Status::Checking)
        };
        assert_eq!(go(five, E::Fail).to, Status::Working);
    }

    /// No verdict and a discarded verdict: turquoise, not a round.
    #[test]
    fn no_verdict() {
        for (event, outcome) in [
            (E::NoVerdict, ValidationOutcome::NoResult),
            (E::ValidatorChangedFiles, ValidationOutcome::Discarded),
        ] {
            let t = go(state(Status::Checking), event);
            assert_eq!(
                (t.to, t.round, t.outcome),
                (Status::CheckNoResult, Round::Keep, Some(outcome))
            );
            assert!(!outcome.counts());
        }
    }

    #[test]
    fn validator_permission_prompt_and_back() {
        assert_eq!(
            go(state(Status::Checking), E::ValidatorNeedsYou).to,
            Status::CheckerNeedsYou
        );
        assert_eq!(
            go(state(Status::CheckerNeedsYou), E::ValidatorContinues).to,
            Status::Checking
        );
        // A verdict also lands while the prompt shows.
        assert_eq!(
            go(state(Status::CheckerNeedsYou), E::Pass).to,
            Status::WaitingForYou
        );
    }

    /// Code changed, Stop validation and Skip to Review: none counts as a round.
    #[test]
    fn changes_stop_and_skip() {
        let t = go(state(Status::Checking), E::CodeChanged);
        assert_eq!(
            (t.to, t.tell_cancelled, t.round, t.outcome),
            (
                Status::Working,
                true,
                Round::Keep,
                Some(ValidationOutcome::Cancelled)
            )
        );
        let t = go(state(Status::Checking), E::StopValidation);
        assert_eq!(
            (t.to, t.tell_cancelled, t.outcome),
            (Status::Working, false, Some(ValidationOutcome::Stopped))
        );
        let t = go(
            ValidationState {
                auto_merge: true,
                ..state(Status::Checking)
            },
            E::SkipToReview,
        );
        assert_eq!(
            (t.to, t.entry, t.auto_merge),
            (Status::WaitingForYou, Some(ReviewEntry::Skipped), false)
        );
        for event in [E::StopValidation, E::SkipToReview, E::CodeChanged] {
            assert!(
                transition(state(Status::Working), event).is_err(),
                "{event:?}"
            );
        }
    }

    /// After the last failed round: Move to Review anyway, Give it another round, Answer the agent.
    #[test]
    fn after_the_last_failed_round() {
        let failed = state(Status::CheckFailed);
        let t = go(failed, E::MoveToReviewAnyway);
        assert_eq!(
            (t.to, t.entry, t.auto_merge),
            (Status::WaitingForYou, Some(ReviewEntry::FailedMoved), false)
        );
        let t = go(failed, E::AnotherRound);
        assert_eq!(
            (t.to, t.deliver_findings, t.extra_round, t.round),
            (Status::Working, true, true, Round::Keep)
        );
        let t = go(failed, E::AnswerAgent);
        assert_eq!(
            (t.to, t.deliver_findings, t.round),
            (Status::Working, true, Round::Reset)
        );
    }

    /// Ended without a result: Retry validation (a fresh validator, not a round) or Move to Review anyway
    /// ("Validation skipped by you").
    #[test]
    fn after_no_result() {
        let t = go(state(Status::CheckNoResult), E::RetryValidation);
        assert_eq!(
            (t.to, t.start_validator, t.round),
            (Status::Checking, true, Round::Keep)
        );
        let t = go(state(Status::CheckNoResult), E::MoveToReviewAnyway);
        assert_eq!(t.entry, Some(ReviewEntry::Skipped));
        assert!(transition(state(Status::CheckFailed), E::RetryValidation).is_err());
    }

    /// Auto-merge: success → Merged; a conflict → back to the agent with restack, the counter at 0; the third
    /// conflict in a row → stopped; base diverged / checkout dirty → stopped; Retry merge and Move to Review.
    #[test]
    fn auto_merge() {
        let review = state(Status::WaitingForYou);
        assert_eq!(go(review, E::AutoMerged).to, Status::Merged);
        let t = go(review, E::AutoMergeConflict);
        assert_eq!(
            (t.to, t.restack, t.round),
            (Status::MergeConflictFixing, true, Round::Reset)
        );
        let third = ValidationState {
            conflicts: 2,
            ..review
        };
        let t = go(third, E::AutoMergeConflict);
        assert_eq!(
            (t.to, t.restack, t.stop),
            (
                Status::DeliveryStopped,
                false,
                Some(AutoMergeStop::KeepsConflicting)
            )
        );
        for stop in [AutoMergeStop::BaseDiverged, AutoMergeStop::CheckoutDirty] {
            let t = go(review, E::AutoMergeBlocked(stop));
            assert_eq!((t.to, t.stop), (Status::DeliveryStopped, Some(stop)));
        }
        let stopped = state(Status::DeliveryStopped);
        let t = go(stopped, E::RetryMerge);
        assert_eq!((t.to, t.auto_merge), (Status::WaitingForYou, true));
        let t = go(stopped, E::MoveToReview);
        assert_eq!(
            (t.to, t.auto_merge, t.entry),
            (Status::WaitingForYou, false, None),
            "still Validated · round N"
        );
        // The fix after a conflict validates again with the counter at 0.
        let t = go(state(Status::MergeConflictFixing), E::AgentSubmit);
        assert_eq!((t.to, t.round), (Status::Checking, Round::Reset));
    }
}
