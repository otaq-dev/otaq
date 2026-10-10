//! The Open PR and Push to PR flows (REV-10, REV-14; d0050, d0041, d0059), driven by the user's clicks:
//!
//! 1. **Open PR / Push to PR** (Approve ▾): refused with uncommitted files (REV-08); "changed since you reviewed
//!    it" first unless approved anyway (REV-06). The resolved Open PR (or Push) skill runs in the ticket's
//!    session: typed into the live session through the user-click path (unsent text asks first, d0048), or the
//!    stored conversation is resumed with the skill as its first prompt. The ticket stays in Ready for Review;
//!    the flow is "preparing" until the skill runs `crewboard pr propose`.
//! 2. **Confirm** (the dialog's Push & open PR): Crewboard applies the user's edits itself (reword, rename, lock)
//!    and then types the instruction to push; nothing is pushed before.
//! 3. **Cancel**: the proposal is discarded, nothing was pushed, the ticket stays in Ready for Review.
//! 4. `crewboard pr opened <url>`: Approved · PR open with the PR's number.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::flow::{FlowKind, FlowState};
use super::proposal::Proposal;
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
use crate::session::launch::{LaunchContext, resume_agent_with};
use crate::session::{
    SessionInfo, SessionKind, SessionTicket, Sessions, SpawnSpec, TerminalSize, statusline,
};
use crate::store::now_ms;

/// The answer to an Open PR, Push to PR or confirm click.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PrOutcome {
    /// The skill was started (or told to push); the dialog follows the flow.
    Started { ticket: Box<Ticket> },
    /// The session's input line has unsent text: ask "Keep it and cancel / Clear it and send" (d0048). Nothing
    /// was changed.
    NeedsConfirmation { ticket: TicketKey },
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
    launch: Option<LaunchContext>,
    events: EventBus,
}

/// The size a resumed session starts with until a terminal attaches.
const INITIAL_SIZE: TerminalSize = TerminalSize {
    rows: 40,
    cols: 120,
};

/// What Crewboard types after the user confirmed (the skills wait for it).
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
        launch: Option<LaunchContext>,
        events: EventBus,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                board,
                data,
                sessions,
                deliveries,
                launch,
                events,
            }),
        }
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
        if ticket.status != Status::ReadyForReview {
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
        if ticket.is_epic() {
            return Err(BoardError::refused(
                Refusal::NotAvailableYet,
                "Open PR on an epic runs in a delivery session, which isn't available yet",
            ));
        }
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
        let skill = board
            .effective_settings(&project)
            .await?
            .workflow_skills
            .get(step)
            .to_owned();
        let live = self.live_session(&ticket);
        if let Some(outcome) = self.unsent_first(&ticket, live.as_ref(), clear_first)? {
            return Ok(outcome);
        }
        board.start_pr_flow(&ticket, kind).await?;
        if let Err(e) = self
            .run_in_session(&ticket, live.as_ref(), &skill, clear_first)
            .await
        {
            board.end_pr_flow(&ticket.id).await?;
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
            &confirmed_instruction(flow.kind),
            clear_first,
        )
        .await?;
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
        let mut tx = board.store().pool().begin().await?;
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

    /// `crewboard pr opened <url>`: the PR is recorded, the flow ends and the ticket is Approved · PR open.
    pub async fn opened(&self, key: &str, url: &str, actor: &Actor) -> Result<Ticket, BoardError> {
        let board = &self.inner.board;
        let ticket = board.pr_opened(key, url, actor).await?;
        board.end_pr_flow(&ticket.id).await?;
        if matches!(ticket.status, Status::ReadyForReview | Status::PrOpen) {
            board.record_pr_open(&ticket, actor).await?;
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
            .running_of(&ticket.id, SessionKind::Agent)
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
            }));
        }
        Ok(None)
    }

    /// Types `text` into the live session through the user-click path, or resumes the stored conversation with it
    /// as the first prompt.
    async fn run_in_session(
        &self,
        ticket: &Ticket,
        live: Option<&SessionInfo>,
        text: &str,
        clear_first: bool,
    ) -> Result<(), BoardError> {
        let deliveries = &self.inner.deliveries;
        match live {
            Some(session) => {
                let typed = if clear_first {
                    deliveries.clear_and_send(&session.id, Some(text)).await
                } else {
                    deliveries.deliver_from_click(&session.id, text).await
                };
                if let Delivery::NeedsConfirmation { .. } = typed.map_err(other)? {
                    tracing::warn!(key = %ticket.key, "unsent text appeared before the PR step was typed");
                }
                Ok(())
            }
            None => self.resume(ticket, text).await.map(|_| ()),
        }
    }

    /// `claude --resume <id> "<prompt>"` in the ticket's worktree, as an agent session; the ticket keeps its
    /// status.
    async fn resume(&self, ticket: &Ticket, prompt: &str) -> Result<SessionInfo, BoardError> {
        let inner = &self.inner;
        let Some(launch) = &inner.launch else {
            return Err(BoardError::refused(
                Refusal::NotAvailableYet,
                format!(
                    "{}'s Claude Code plugin wasn't found next to the {} command",
                    naming::PRODUCT_NAME,
                    naming::COMMAND_NAME
                ),
            ));
        };
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
            .filter(|s| s.kind == SessionKind::Agent)
            .find_map(|s| s.claude_session_id.clone())
            .ok_or_else(|| {
                BoardError::refused(
                    Refusal::SessionEnded,
                    format!("{} has no conversation to resume", ticket.key),
                )
            })?;
        let worktree = worktree_of(ticket)?;
        let port = ticket.work.as_ref().and_then(|w| w.agent_port).unwrap_or(0);
        let settings = statusline::write_settings(
            &inner.data,
            &ticket.key,
            &worktree,
            launch.claude_config_dir.as_deref(),
            &launch.bin_dir,
        )
        .inspect_err(|e| tracing::warn!(key = %ticket.key, error = %e, "can't write the status-line settings"))
        .ok();
        let command = resume_agent_with(
            launch,
            &ticket.key,
            &worktree,
            port,
            &conversation,
            Some(prompt.to_owned()),
            settings.as_deref(),
        );
        let project = inner.board.project(ticket.key.project.as_str()).await?;
        inner
            .sessions
            .spawn(SpawnSpec {
                kind: SessionKind::Agent,
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
            .map_err(other)
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
        let now = now_ms();
        sqlx::query(concat!(
            "UPDATE tickets SET board_column = ?, state = ?, updated_by = ?, updated_at = ?, ",
            crate::store::tickets::set_state_since!(),
            " WHERE id = ?"
        ))
        .bind(to.column().as_str())
        .bind(to.as_str())
        .bind(actor.to_string())
        .bind(now)
        .bind(to.as_str())
        .bind(now)
        .bind(&ticket.id)
        .execute(self.store().pool())
        .await?;
        Ok(())
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
