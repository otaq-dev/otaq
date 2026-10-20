//! The Open PR and Push to PR flows (REV-10, REV-14; d0050, d0041, d0059), driven by the user's clicks:
//!
//! 1. **Open PR / Push to PR** (Approve ▾): refused with uncommitted files (REV-08); "changed since you reviewed
//!    it" first unless approved anyway (REV-06). The resolved Open PR (or Push) skill runs in the ticket's
//!    session: typed into the live session through the user-click path (unsent text asks first, d0048), or the
//!    stored conversation is resumed with the skill as its first prompt. The ticket stays in Ready for Review;
//!    the flow is "preparing" until the skill runs `otaq pr propose`.
//! 2. **Confirm** (the dialog's Push & open PR): Otaq applies the user's edits itself (reword, rename, lock)
//!    and then types the instruction to push; nothing is pushed before.
//! 3. **Cancel**: the proposal is discarded, nothing was pushed, the ticket stays in Ready for Review.
//! 4. `otaq pr opened <url>`: Approved · PR open with the PR's number.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::flow::{FlowKind, FlowState};
use super::proposal::Proposal;
use crate::agent_tools::{Adapters, FirstPrompt, LaunchRequest, LaunchRole, Launched};
use crate::agents::gate::{Held, LaunchGate, check};
use crate::agents::{RunsOn, resolve};
use crate::board::review::ApproveCheck;
use crate::board::settings::WorkflowStep;
use crate::board::tickets::{EventKind, Ticket, record_event};
use crate::board::{Board, BoardError, Refusal};
use crate::data_dir::DataDir;
use crate::domain::{Actor, Status, TicketKey};
use crate::git::merge::uncommitted_files_message;
use crate::naming;
use crate::protocol::Event;
use crate::service::EventBus;
use crate::session::deliveries::{Deliveries, Delivery};
use crate::session::{
    SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec, StoredConversation, TerminalSize,
};
use crate::store::now_ms;

/// The answer to an Open PR, Push to PR or confirm click.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PrOutcome {
    /// The skill was started (or told to push); the dialog follows the flow.
    Started { ticket: Box<Ticket> },
    /// The session's input line has unsent text: ask "Keep it and cancel / Clear it and send" (d0048). Nothing
    /// was changed. `unclearable_by` names the tool when the text is a multi-line draft it can't clear (SES-07).
    NeedsConfirmation {
        ticket: TicketKey,
        #[serde(default)]
        unclearable_by: Option<String>,
    },
    /// "SHOP-19 has 2 uncommitted files" · View · Send back (REV-08).
    RefusedUncommitted { files: Vec<String>, message: String },
    /// New commits since the review (REV-06): approve again with `approve_anyway`, or Send back.
    Changed { check: ApproveCheck },
}

/// Runs the PR flows. Cheap to clone.
#[derive(Clone)]
pub struct Prs {
    inner: Arc<Inner>,
}

struct Inner {
    board: Board,
    data: DataDir,
    sessions: Sessions,
    deliveries: Deliveries,
    adapters: Adapters,
    events: EventBus,
    /// The check before a delivery session's launch (set by the service).
    gate: std::sync::OnceLock<LaunchGate>,
}

/// The size a resumed session starts with until a terminal attaches.
const INITIAL_SIZE: TerminalSize = TerminalSize {
    rows: 40,
    cols: 120,
};

/// What a PR step types into the ticket's session, or starts or resumes it with.
enum PrStep<'a> {
    /// The Open PR or Push skill, as the Delivery stage's agent runs it, written into the worktree first by one name
    /// with that agent's skills (AGM-07) and named for the session's tool.
    Skill(WorkflowStep),
    /// Otaq's own words: the confirmed push ([`confirmed_instruction`]).
    Words(&'a str),
}

pub fn confirmed_instruction(kind: FlowKind) -> String {
    let cmd = naming::COMMAND_NAME;
    match kind {
        FlowKind::Open => format!(
            "The user confirmed the pull request. Run `{cmd} prime` for the confirmed branch, title and description, \
             then push the branch and open the PR, and report it with `{cmd} pr opened <url>`."
        ),
        FlowKind::Push => format!(
            "The user confirmed the push. Run `{cmd} prime`, push the branch, then report back with \
             `{cmd} pr opened <url>`."
        ),
    }
}

impl Prs {
    pub fn new(
        board: Board,
        data: DataDir,
        sessions: Sessions,
        deliveries: Deliveries,
        adapters: Adapters,
        events: EventBus,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                sessions,
                deliveries,
                adapters,
                events,
                gate: std::sync::OnceLock::new(),
            }),
        }
    }

    /// Gives the delivery sessions the check before every launch (SCH-04 `[M23]`).
    pub fn set_gate(&self, gate: LaunchGate) {
        let _ = self.inner.gate.set(gate);
    }

    pub fn board(&self) -> &Board {
        &self.inner.board
    }

    /// **Open PR** or **Push to PR #N** (a ticket that has a PR) on a delivery unit in Ready for Review.
    pub async fn open(
        &self,
        key: &str,
        approve_anyway: bool,
        clear_first: bool,
        _actor: &Actor,
    ) -> Result<PrOutcome, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        if ticket.status != Status::WaitingForYou {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{} is {}; Open PR works on a ticket in Ready for Review",
                    ticket.key,
                    ticket.status.label()
                ),
            ));
        }
        if ticket.epic.is_some() {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                "Open PR isn\u{2019}t offered for epic children: the epic opens one PR for all of them.",
            ));
        }
        let kind = match ticket.flags.pr_number {
            Some(_) => FlowKind::Push,
            None => FlowKind::Open,
        };
        let project = ticket.key.project.to_string();
        if kind == FlowKind::Open {
            let provider = self.inner.board.pr_provider_from_remote(&project).await?;
            if let Some(why) = provider.open_pr_blocked() {
                return Err(BoardError::refused(
                    Refusal::NotAllowed,
                    format!(
                        "{why}: use your own Open PR skill (Settings \u{2192} Workflow skills)"
                    ),
                ));
            }
        }
        // A delivery session that can't start now is refused before anything is made for it.
        if ticket.is_epic() && self.live_session(&ticket).is_none() {
            self.delivery_runs_on(&ticket).await?;
        }
        // An epic has no worktree, and its lead never delivers (d0083): its delivery session runs in a worktree on
        // the epic branch (EPC-08).
        let ticket = if ticket.is_epic() {
            self.delivery_worktree(&ticket).await?
        } else {
            ticket
        };
        let worktree = worktree_of(&ticket)?;
        let git = board.git()?;
        let files = git.uncommitted_files(&worktree).await?;
        if !files.is_empty() {
            return Ok(PrOutcome::RefusedUncommitted {
                message: uncommitted_files_message(&ticket.key.to_string(), files.len()),
                files,
            });
        }
        if !approve_anyway {
            let check = board.approve_check(key).await?;
            if check != ApproveCheck::Unchanged {
                return Ok(PrOutcome::Changed { check });
            }
        }
        let step = match kind {
            FlowKind::Open => WorkflowStep::OpenPr,
            FlowKind::Push => WorkflowStep::Push,
        };
        let skill = board.delivery_skill(&project, step).await?;
        let live = self.live_session(&ticket);
        if let Some(outcome) = self.unsent_first(&ticket, live.as_ref(), clear_first)? {
            return Ok(outcome);
        }
        board.start_pr_flow(&ticket, kind).await?;
        let started = match (ticket.is_epic(), &live) {
            (true, None) => self.start_delivery(&ticket, step).await.map(|_| ()),
            _ => {
                self.run_in_session(&ticket, live.as_ref(), PrStep::Skill(step), clear_first)
                    .await
            }
        };
        if let Err(e) = started {
            board.end_pr_flow(&ticket.id).await?;
            if ticket.is_epic() {
                self.end_delivery(&ticket).await?;
            }
            return Err(e);
        }
        tracing::info!(key = %ticket.key, %skill, "PR flow started");
        Ok(PrOutcome::Started {
            ticket: Box::new(board.ticket(key).await?),
        })
    }

    /// **Push & open PR** / **Push**: the user's edits are applied, then the skill is told to push.
    pub async fn confirm(
        &self,
        key: &str,
        edits: &Proposal,
        clear_first: bool,
        actor: &Actor,
    ) -> Result<PrOutcome, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        let flow = board
            .pr_flow(&ticket.id)
            .await?
            .filter(|f| f.state == FlowState::Proposed)
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::MoveRefused,
                    format!("{} has no proposal to confirm", ticket.key),
                )
            })?;
        let mut edits = edits.clone();
        edits.branch = edits.branch.trim().to_owned();
        let checked = board.check_proposal(&ticket, flow.kind, &edits).await?;
        if flow.kind == FlowKind::Open
            && let super::flow::BranchCheck::Problem { message } =
                board.check_branch_name(key, &checked.branch).await?
        {
            return Err(BoardError::refused(Refusal::BranchTaken, message));
        }
        if let Some(e) = checked.commits.iter().find(|c| c.message.trim().is_empty()) {
            return Err(BoardError::refused(
                Refusal::EmptyField,
                format!("Commit {} has no message", &e.sha[..7.min(e.sha.len())]),
            ));
        }
        let worktree = worktree_of(&ticket)?;
        let files = board.git()?.uncommitted_files(&worktree).await?;
        if !files.is_empty() {
            return Ok(PrOutcome::RefusedUncommitted {
                message: uncommitted_files_message(&ticket.key.to_string(), files.len()),
                files,
            });
        }
        let live = self.live_session(&ticket);
        if let Some(outcome) = self.unsent_first(&ticket, live.as_ref(), clear_first)? {
            return Ok(outcome);
        }
        board
            .apply_confirmed(&ticket, flow.kind, &checked, actor)
            .await?;
        self.run_in_session(
            &ticket,
            live.as_ref(),
            PrStep::Words(&confirmed_instruction(flow.kind)),
            clear_first,
        )
        .await?;
        Ok(PrOutcome::Started {
            ticket: Box::new(board.ticket(key).await?),
        })
    }

    /// **Retry** after the skill reported a problem: the problem is cleared and the skill is asked again (the
    /// skill while preparing, the push after the confirmation).
    pub async fn retry(&self, key: &str, clear_first: bool) -> Result<PrOutcome, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        let flow = board.pr_flow(&ticket.id).await?.ok_or_else(|| {
            BoardError::refused(
                Refusal::MoveRefused,
                format!("{} has no Open PR in progress", ticket.key),
            )
        })?;
        let live = self.live_session(&ticket);
        if let Some(outcome) = self.unsent_first(&ticket, live.as_ref(), clear_first)? {
            return Ok(outcome);
        }
        let pushing = flow.state == FlowState::Pushing;
        let text = confirmed_instruction(flow.kind);
        let skill_step = match flow.kind {
            FlowKind::Open => WorkflowStep::OpenPr,
            FlowKind::Push => WorkflowStep::Push,
        };
        let step = match pushing {
            true => PrStep::Words(&text),
            false => PrStep::Skill(skill_step),
        };
        sqlx::query("UPDATE pr_flows SET reported = NULL WHERE ticket_id = ?")
            .bind(&ticket.id)
            .execute(board.store().pool())
            .await?;
        match (ticket.is_epic(), &live) {
            (true, None) if !pushing => {
                self.start_delivery(&ticket, skill_step).await.map(|_| ())?
            }
            _ => {
                self.run_in_session(&ticket, live.as_ref(), step, clear_first)
                    .await?
            }
        }
        Ok(PrOutcome::Started {
            ticket: Box::new(board.ticket(key).await?),
        })
    }

    /// **Cancel** before pushing: the proposal is discarded; the ticket stays in Ready for Review.
    pub async fn cancel(&self, key: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.ticket(key).await?;
        let Some(flow) = board.pr_flow(&ticket.id).await? else {
            return Ok(ticket);
        };
        if flow.state == FlowState::Pushing {
            return Err(BoardError::refused(
                Refusal::MoveRefused,
                format!(
                    "{}'s PR was already confirmed and is being pushed",
                    ticket.key
                ),
            ));
        }
        board.end_pr_flow(&ticket.id).await?;
        if ticket.is_epic() {
            self.end_delivery(&ticket).await?;
        }
        let mut tx = board.store().begin_write().await?;
        record_event(
            &mut tx,
            &ticket.id,
            EventKind::PrCancelled,
            actor,
            json!({ "kind": flow.kind }),
        )
        .await?;
        tx.commit().await?;
        board.ticket(key).await
    }

    /// `otaq pr opened <url>`: the PR is recorded, the flow ends and the ticket is Approved · PR open.
    pub async fn opened(&self, key: &str, url: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.pr_opened(key, url, actor).await?;
        board.end_pr_flow(&ticket.id).await?;
        if ticket.is_epic() {
            // The delivery session's job is done (EPC-08).
            self.end_delivery(&ticket).await?;
        }
        if matches!(ticket.status, Status::WaitingForYou | Status::PrOpen) {
            board.record_pr_open(&ticket, actor).await?;
            // The user's Open PR approved it: its decisions are kept by the user, not the skill (DEC-05).
            crate::decisions::keep_all(board, key, &Actor::current_user()).await?;
        }
        // The pushed content must be what the user reviewed (REV-10 step 4); reworded commits don't count.
        match board.verify_pushed(&ticket).await {
            Ok(Some(diff)) => {
                tracing::warn!(key = %ticket.key, files = diff.files.len(), "pushed content differs from the review");
                self.inner.events.publish(Event::PushedContentDiffers {
                    project: ticket.key.project.to_string(),
                    key: ticket.key.clone(),
                    message: diff.message(&ticket.key.to_string()),
                });
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(key = %ticket.key, error = %e, "can't compare the pushed content")
            }
        }
        board.ticket(key).await
    }

    fn live_session(&self, ticket: &Ticket) -> Option<SessionInfo> {
        self.inner
            .sessions
            .running_of(&ticket.id, session_kind(ticket))
    }

    /// The epic's delivery worktree on its branch (created, or reused): `wt/<repo>/SHOP-40`. Returns the epic
    /// with the worktree recorded.
    async fn delivery_worktree(&self, epic: &Ticket) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let row = board
            .store()
            .ticket(&epic.id)
            .await?
            .ok_or_else(|| BoardError::NotFound(epic.key.to_string()))?;
        let branch = row.branch_name.ok_or_else(|| {
            BoardError::refused(
                Refusal::NoBranch,
                format!("{} has no branch: no child has started", epic.key),
            )
        })?;
        let project = board.project(epic.key.project.as_str()).await?;
        let repo = PathBuf::from(&project.repo_path);
        let repo_name = repo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.name.clone());
        let path = self
            .inner
            .data
            .worktree_path(&repo_name, &epic.key.to_string())
            .map_err(|e| BoardError::refused(Refusal::NoWorktree, e.to_string()))?;
        crate::start::worktree::add_worktree(board.git()?, &repo, &path, &branch)
            .await
            .map_err(|e| BoardError::refused(Refusal::NoWorktree, e.to_string()))?;
        sqlx::query("UPDATE tickets SET worktree_path = ?, worktree_kept = FALSE WHERE id = ?")
            .bind(path.display().to_string())
            .bind(&epic.id)
            .execute(board.store().pool())
            .await?;
        board.ticket_by_id(&epic.id).await
    }

    /// Starts the epic's short-lived delivery session: a new conversation in the delivery worktree with the skill
    /// in force (written into it first when imported, named by the adapter) as its first prompt, `OTAQ_TICKET` the
    /// epic (EPC-08). The one agent session an epic runs, on the project's Developer default (Q-D: an epic has no
    /// Developer row).
    async fn start_delivery(
        &self,
        epic: &Ticket,
        step: WorkflowStep,
    ) -> Result<SessionInfo, BoardError> {
        let runs_on = self.delivery_runs_on(epic).await?;
        self.inner.adapters.unavailable(runs_on.tool)?;
        let worktree = worktree_of(epic)?;
        let first_prompt = self.copy_skill(epic, &worktree, step).await?;
        let launched = self.inner.adapters.launch(
            runs_on.tool,
            &LaunchRequest {
                role: LaunchRole::Delivery,
                cwd: worktree,
                project: epic.key.project.to_string(),
                ticket: Some(epic.key.clone()),
                epic: None,
                port: 0,
                first_prompt: Some(first_prompt),
                resume: None,
                model: runs_on.model.clone(),
                effort: runs_on.effort.clone(),
                title: Some(epic.title.clone()),
            },
        )?;
        self.spawn(epic, SessionKind::Delivery, launched, &runs_on)
            .await
    }

    /// What the epic's delivery session runs on: the Delivery stage's agent (AGM-05; Q-D: an epic has no Agent stage
    /// of its own), through the check before every launch (T-0357's Q3): refused with the hold's message ("Codex
    /// isn't available · logged out", "Claude Code limit reached", "Model unavailable · Codex no longer lists
    /// gpt-5.4") when it can't start now, nothing held (it is the user's click); a family choice resolved to the
    /// model it passes.
    async fn delivery_runs_on(&self, epic: &Ticket) -> Result<RunsOn, BoardError> {
        let board = &self.inner.board;
        let agent = resolve::delivery_agent(board, epic.key.project.as_str()).await?;
        let Some(gate) = self.inner.gate.get() else {
            return Ok(RunsOn::agent(
                &agent,
                board.store().default_model().await?.as_ref(),
            ));
        };
        let mut runs_on = gate.runs_on(&agent).await?;
        let on = gate.model_of(&agent).await?;
        match check(&on, &gate.facts().await?) {
            Ok(model) => {
                runs_on.model = model;
                Ok(runs_on)
            }
            Err(blocked) => {
                let held = Held {
                    agent,
                    on,
                    blocked,
                    fallback: None,
                    fallback_held: false,
                };
                tracing::info!(key = %epic.key, reason = %held.blocked.reason(), "delivery refused before launch");
                Err(BoardError::refused(Refusal::MoveRefused, held.message()))
            }
        }
    }

    /// Ends the epic's delivery: the session stops and the delivery worktree goes, unless it has uncommitted
    /// changes (then it is kept and listed in Cleanup, d0018).
    async fn end_delivery(&self, epic: &Ticket) -> Result<(), BoardError> {
        let inner = &self.inner;
        let board = &inner.board;
        if let Some(session) = inner.sessions.running_of(&epic.id, SessionKind::Delivery) {
            inner.sessions.stop(&session.id).await.map_err(other)?;
        }
        let epic = board.ticket_by_id(&epic.id).await?;
        let Some(worktree) = epic
            .work
            .as_ref()
            .and_then(|w| w.worktree.clone())
            .map(PathBuf::from)
        else {
            return Ok(());
        };
        let git = board.git()?;
        let dirty = if worktree.is_dir() {
            git.uncommitted_files(&worktree).await?
        } else {
            Vec::new()
        };
        if dirty.is_empty() {
            if worktree.is_dir() {
                let project = board.project(epic.key.project.as_str()).await?;
                crate::review::finish::remove_worktree(
                    &board.project_git(&project.key).await?,
                    std::path::Path::new(&project.repo_path),
                    &worktree,
                )
                .await?;
            }
            sqlx::query(
                "UPDATE tickets SET worktree_path = NULL, worktree_kept = FALSE WHERE id = ?",
            )
            .bind(&epic.id)
            .execute(board.store().pool())
            .await?;
        } else {
            sqlx::query("UPDATE tickets SET worktree_kept = TRUE WHERE id = ?")
                .bind(&epic.id)
                .execute(board.store().pool())
                .await?;
            tracing::info!(key = %epic.key, files = dirty.len(), "delivery worktree kept: uncommitted changes");
        }
        Ok(())
    }

    async fn spawn(
        &self,
        ticket: &Ticket,
        kind: SessionKind,
        launched: Launched,
        runs_on: &RunsOn,
    ) -> Result<SessionInfo, BoardError> {
        let project = self
            .inner
            .board
            .project(ticket.key.project.as_str())
            .await?;
        self.inner
            .sessions
            .prepared(launched.prepare)
            .spawn(SpawnSpec {
                kind,
                project_id: project.id,
                project_key: project.key,
                ticket: Some(SessionTicket {
                    id: ticket.id.clone(),
                    key: ticket.key.clone(),
                }),
                program: launched.plan.program,
                args: launched.plan.args,
                cwd: launched.plan.cwd,
                env: launched.plan.env,
                size: INITIAL_SIZE,
                tool: Some(launched.tool),
                tool_session_id: launched.tool_session_id,
                model: launched.model,
                agent: runs_on.agent.clone(),
                effort: runs_on.effort.clone(),
                fallback: runs_on.fallback.clone(),
            })
            .await
            .map_err(other)
    }

    /// "Needs confirmation" when the live session's input line has unsent text and the click didn't say to clear
    /// it.
    fn unsent_first(
        &self,
        ticket: &Ticket,
        live: Option<&SessionInfo>,
        clear_first: bool,
    ) -> Result<Option<PrOutcome>, BoardError> {
        if clear_first {
            return Ok(None);
        }
        if let Some(session) = live
            && self
                .inner
                .sessions
                .has_unsent_input(&session.id)
                .map_err(other)?
        {
            return Ok(Some(PrOutcome::NeedsConfirmation {
                ticket: ticket.key.clone(),
                unclearable_by: self
                    .inner
                    .deliveries
                    .unclearable_by(&session.id)
                    .map_err(other)?,
            }));
        }
        Ok(None)
    }

    /// Writes the Delivery stage's agent's skills and the skill it runs for `step` into `worktree` (AGM-07): the
    /// prompt that runs it.
    async fn copy_skill(
        &self,
        ticket: &Ticket,
        worktree: &std::path::Path,
        step: WorkflowStep,
    ) -> Result<FirstPrompt, BoardError> {
        let board = &self.inner.board;
        let agent = resolve::delivery_agent(board, ticket.key.project.as_str())
            .await
            .ok();
        let prompt = FirstPrompt::Skill {
            name: step.own_skill().to_owned(),
            message: None,
        };
        Ok(board
            .copy_for_session(
                worktree,
                self.inner.adapters.shipped_skills(),
                agent.as_ref(),
                Some(prompt.clone()),
                crate::skills::Marker::Worktree,
            )
            .await
            .0
            .unwrap_or(prompt))
    }

    /// Types `step` into the live session through the user-click path, or resumes the stored conversation with it
    /// as the first prompt. A skill is written into the worktree first.
    async fn run_in_session(
        &self,
        ticket: &Ticket,
        live: Option<&SessionInfo>,
        step: PrStep<'_>,
        clear_first: bool,
    ) -> Result<(), BoardError> {
        let deliveries = &self.inner.deliveries;
        match live {
            Some(session) => {
                let text = match step {
                    PrStep::Skill(skill_step) => self
                        .copy_skill(ticket, &worktree_of(ticket)?, skill_step)
                        .await?
                        .text(self.inner.adapters.of_session(session.tool)),
                    PrStep::Words(text) => text.to_owned(),
                };
                let typed = if clear_first {
                    deliveries.clear_and_send(&session.id, Some(&text)).await
                } else {
                    deliveries.deliver_from_click(&session.id, &text).await
                };
                if let Delivery::NeedsConfirmation { .. } = typed.map_err(other)? {
                    tracing::warn!(key = %ticket.key, "unsent text appeared before the PR step was typed");
                }
                Ok(())
            }
            None => self.resume(ticket, step).await.map(|_| ()),
        }
    }

    /// The stored conversation resumed on its tool and model with `step` first, in the ticket's worktree, as an
    /// agent session; the ticket keeps its status.
    async fn resume(&self, ticket: &Ticket, step: PrStep<'_>) -> Result<SessionInfo, BoardError> {
        let inner = &self.inner;
        let kind = session_kind(ticket);
        let history = inner
            .sessions
            .history(&SessionTicket {
                id: ticket.id.clone(),
                key: ticket.key.clone(),
            })
            .await
            .map_err(other)?;
        let conversation = history
            .iter()
            .filter(|s| s.kind == kind)
            .find_map(StoredConversation::of)
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::SessionEnded,
                    format!("{} has no conversation to resume", ticket.key),
                )
            })?;
        let runs_on = RunsOn::resumed(&conversation);
        inner.adapters.unavailable(runs_on.tool)?;
        let worktree = worktree_of(ticket)?;
        let first_prompt = match step {
            PrStep::Skill(step) => self.copy_skill(ticket, &worktree, step).await?,
            PrStep::Words(text) => FirstPrompt::Text(text.to_owned()),
        };
        let port = ticket.work.as_ref().and_then(|w| w.agent_port).unwrap_or(0);
        let role = match kind {
            SessionKind::Delivery => LaunchRole::Delivery,
            _ => LaunchRole::TicketAgent,
        };
        let launched = inner.adapters.launch(
            runs_on.tool,
            &LaunchRequest {
                role,
                cwd: worktree,
                project: ticket.key.project.to_string(),
                ticket: Some(ticket.key.clone()),
                epic: None,
                port,
                first_prompt: Some(first_prompt),
                resume: Some(conversation.id),
                model: runs_on.model.clone(),
                effort: runs_on.effort.clone(),
                title: Some(ticket.title.clone()),
            },
        )?;
        self.spawn(ticket, kind, launched, &runs_on).await
    }
}

impl Board {
    /// Approved · PR open (REV-13): after the skill reported the PR.
    pub(crate) async fn record_pr_open(
        &self,
        ticket: &Ticket,
        actor: &Actor,
    ) -> Result<(), BoardError> {
        let to = Status::PrOpen;
        // Into the Delivery stage, "PR open" (STG-06): from the human check whose Pass offered it, or (pushed again)
        // where it is.
        let moved = self
            .moved(ticket, ticket.status, crate::flow::engine::Move::PrOpened)
            .await
            .ok();
        let stage = moved
            .as_ref()
            .and_then(|m| m.place.stage_id().map(str::to_owned))
            .or_else(|| crate::flow::engine::default_stage(to, None).map(str::to_owned));
        let now = now_ms();
        let mut tx = self.store().begin_write().await?;
        if let Some(moved) = &moved {
            moved.record(&mut tx, &ticket.id, None).await?;
        }
        sqlx::query(concat!(
            "UPDATE tickets SET stage_id = ?, state = ?, updated_by = ?, updated_at = ?, ",
            crate::store::tickets::set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(stage)
        .bind(to.as_str())
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// The session a ticket's PR skill runs in: its agent's, or an epic's delivery session.
fn session_kind(ticket: &Ticket) -> SessionKind {
    if ticket.is_epic() {
        SessionKind::Delivery
    } else {
        SessionKind::Agent
    }
}

/// The ticket's worktree folder.
fn worktree_of(ticket: &Ticket) -> Result<PathBuf, BoardError> {
    ticket
        .work
        .as_ref()
        .and_then(|w| w.worktree.as_ref())
        .map(PathBuf::from)
        .ok_or_else(|| {
            BoardError::refused(
                Refusal::NoWorktree,
                format!("{} has no worktree", ticket.key),
            )
        })
}

fn other(e: impl std::fmt::Display) -> BoardError {
    BoardError::refused(Refusal::SessionEnded, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The open-pr and push skills wait for these words before they push.
    #[test]
    fn the_confirmation_says_the_user_confirmed() {
        assert!(
            confirmed_instruction(FlowKind::Open)
                .starts_with("The user confirmed the pull request.")
        );
        assert!(confirmed_instruction(FlowKind::Push).starts_with("The user confirmed the push."));
        for kind in [FlowKind::Open, FlowKind::Push] {
            let text = confirmed_instruction(kind);
            assert!(text.contains(&format!("`{} prime`", naming::COMMAND_NAME)));
            assert!(!text.contains('\n'), "one line, typed with Enter");
        }
    }
}
