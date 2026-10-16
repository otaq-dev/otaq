//! Board requests (projects, tickets, epics, dependencies, pins, settings) mapped to [`Board`], and the events
//! their changes publish (SDD §5.4). The service enforces every rule; clients stay thin (d0055).
//!
//! Change events come from comparing the project's tickets before and after a mutation: every ticket that
//! differs gets `ticket_changed`, a changed status also `card_status_changed`, and a ticket that is gone
//! `ticket_deleted`. Projects being added, changed or removed publish `projects_changed`.

use std::collections::HashMap;
use std::pin::Pin;

use serde_json::json;

use super::Shared;
use crate::board::change_note::change_note;
use crate::board::questions::validate_options;
use crate::board::readiness::project_readiness;
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::caller::Caller;
use crate::domain::Actor;
use crate::protocol::{
    ErrorBody, ErrorKind, Event, GlobalSettingsReply, ProjectSettingsReply, Reply, Request,
};
use crate::session::SessionKind;
use crate::session::deliveries::DeliveryKind;
use crate::signals::LIVE;

/// Answers a board request. `None` when the request isn't a board request.
/// `origin` is the Project terminal the caller runs in, if any: tickets created from it are recorded for its summary
/// (T-0167).
pub(super) async fn handle(
    request: Request,
    caller: &Caller,
    requested_actor: Option<&str>,
    origin: Option<&str>,
    shared: &Shared,
) -> Option<Result<Reply, ErrorBody>> {
    // The actor recorded on changes: an agent caller always acts as itself (CLI-04).
    let actor = caller.actor(requested_actor);
    let board = &shared.board;
    // Every arm's future is built in its own call and boxed (see [`boxed`]), so this frame holds only the box.
    let answer = match request {
        Request::ProjectList => {
            boxed(move || async move { board.project_infos().await.map(Reply::Projects) })
        }
        Request::ProjectProbe(p) => {
            boxed(move || async move { board.probe_folder(&p.path).await.map(Reply::FolderProbe) })
        }
        Request::ProjectAdd(add) => boxed(move || async move {
            let result = board.add_project(&add).await;
            if result.is_ok() {
                shared.events.publish(Event::ProjectsChanged);
            }
            match result {
                Ok(project) => board
                    .project_info_of(&project.key)
                    .await
                    .map(Reply::Project),
                Err(e) => Err(e),
            }
        }),
        Request::ProjectUpdate(p) => boxed(move || async move {
            let result = board.update_project(&p.project, &p.update).await;
            if result.is_ok() {
                shared.events.publish(Event::ProjectsChanged);
            }
            match result {
                Ok(project) => board
                    .project_info_of(&project.key)
                    .await
                    .map(Reply::Project),
                Err(e) => Err(e),
            }
        }),
        Request::ProjectRemove(p) => boxed(move || async move {
            let before = board.tickets(&p.key).await.unwrap_or_default();
            let result = board.remove_project(&p.key).await;
            if result.is_ok() {
                publish_changes(shared, &p.key, &before, &[]);
                shared.events.publish(Event::ProjectsChanged);
            }
            result.map(|()| Reply::Ok)
        }),
        Request::ProjectBranches(p) => {
            boxed(move || async move { board.branches(&p.project).await.map(Reply::Branches) })
        }
        Request::ProjectPrProvider(p) => boxed(move || async move {
            board
                .pr_provider(&p.project, &shared.config.tools)
                .await
                .map(Reply::PrProvider)
        }),
        Request::BoardSnapshot(p) => {
            boxed(move || async move { board.board_snapshot(&p.project).await.map(Reply::Board) })
        }

        Request::TicketGet(p) => boxed(move || async move {
            board
                .ticket(&p.key)
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketList(p) => {
            boxed(move || async move { board.tickets(&p.project).await.map(Reply::Tickets) })
        }
        Request::DepReadiness(p) => boxed(move || async move {
            board
                .tickets(&p.project)
                .await
                .map(|tickets| Reply::Readiness(project_readiness(&tickets)))
        }),
        Request::SchedulerStatus => {
            boxed(
                move || async move { shared.scheduler.status().await.map(Reply::SchedulerStatus) },
            )
        }
        Request::UsageSummary(p) => boxed(move || async move {
            shared
                .usage
                .summary(p.project.as_deref())
                .await
                .map(|s| Reply::Usage(Box::new(s)))
        }),
        Request::UsageRecordLimits(limits) => boxed(move || async move {
            match shared
                .plan_limit
                .record_windows(&limits, crate::store::now_ms())
                .await
            {
                Ok(_) => Ok(Reply::Ok),
                Err(e) => Err(e.into()),
            }
        }),
        Request::UsageResumeAll => boxed(move || async move {
            match shared.plan_limit.resume_all().await {
                Ok(mut resumed) => {
                    // Validations that waited for the reset start now (SCH-04, d0063).
                    for id in Box::pin(shared.validation.start_waiting())
                        .await
                        .unwrap_or_default()
                    {
                        if let Ok(ticket) = board.ticket_by_id(&id).await {
                            publish_ticket_changed(shared, &ticket.key).await;
                            resumed.push(ticket.key);
                        }
                    }
                    resumed.sort();
                    Ok(Reply::Resumed(resumed))
                }
                Err(e) => Err(e),
            }
        }),
        Request::UsageMarkLimit => boxed(move || async move {
            shared
                .plan_limit
                .mark_idle(&actor)
                .await
                .map(Reply::LimitMarked)
        }),
        Request::UsageSessions(p) => boxed(move || async move {
            match board.ticket(&p.key).await {
                Ok(ticket) => shared
                    .usage
                    .sessions_of(&ticket.id)
                    .await
                    .map(Reply::SessionUsage)
                    .map_err(Into::into),
                Err(e) => Err(e),
            }
        }),
        Request::TicketCreate(input) => boxed(move || async move {
            let project = input.project.clone();
            let created = mutate(shared, &project, board.create_ticket(&input, &actor)).await;
            match created {
                Ok(t) => created_in(shared, origin, std::slice::from_ref(&t))
                    .await
                    .map(|()| Reply::Ticket(Box::new(t))),
                Err(e) => Err(e),
            }
        }),
        Request::TicketCreateBatch(p) => boxed(move || async move {
            let project = p.project.clone();
            let created = mutate(
                shared,
                &project,
                board.create_batch(&p.project, &p.tickets, &actor),
            )
            .await;
            match created {
                Ok(tickets) => created_in(shared, origin, &tickets)
                    .await
                    .map(|()| Reply::Tickets(tickets)),
                Err(e) => Err(e),
            }
        }),
        Request::TicketUpdate(p) => boxed(move || async move {
            let project = project_of(&p.key);
            let before = board.ticket(&p.key).await.ok();
            let after = mutate(
                shared,
                &project,
                board.update_ticket(&p.key, &p.update, &actor),
            )
            .await;
            // The user's description edit cancels a validation first (VAL-08); then the agent gets the note.
            let after = match (&before, after) {
                (Some(before), Ok(after)) if !actor.is_agent() => {
                    if let Err(e) = Box::pin(shared.validation.ticket_edited(before, &after)).await
                    {
                        tracing::warn!(ticket = %after.key, error = %e, "can't cancel the validation");
                    }
                    board.ticket_by_id(&after.id).await
                }
                (_, after) => after,
            };
            if let (Some(before), Ok(after)) = (&before, &after) {
                queue_change_note(shared, before, after, &actor).await;
            }
            after.map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketMove(p) => boxed(move || async move {
            let project = project_of(&p.key);
            let moved = mutate(
                shared,
                &project,
                board.move_ticket(&p.key, p.target, p.confirmed, &actor),
            )
            .await;
            // Entering Queued only queues: the scheduler starts it once it's ready (its change event wakes it).
            moved.map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketHistory(p) => {
            boxed(move || async move { board.history(&p.key).await.map(Reply::History) })
        }
        Request::SessionBeforeRestart(p) => boxed(move || async move {
            crate::recovery::before_restart(
                board,
                shared.config.tools.claude_config_dir.as_deref(),
                &p.key,
                p.limit
                    .map_or(crate::recovery::transcript::DEFAULT_ENTRIES, |n| n as usize),
            )
            .await
            .map(|b| Reply::BeforeRestart(Box::new(b)))
        }),
        Request::DiffFiles(p) => boxed(move || async move {
            board
                .diff_summary(&p.key)
                .await
                .map(|d| Reply::DiffSummary(Box::new(d)))
        }),
        Request::DiffFile(p) => boxed(move || async move {
            match &p.since {
                Some(since) => board.diff_file_since(&p.key, &p.path, since).await,
                None => board.diff_file(&p.key, &p.path, p.child.as_deref()).await,
            }
            .map(|d| Reply::FileDiff(Box::new(d)))
        }),
        Request::ReviewCommentAdd(p) => boxed(move || async move {
            let added = board.add_comment(&p).await;
            comment_changed(shared, added).await
        }),
        Request::ReviewCommentReply(p) => boxed(move || async move {
            let added = board.reply_to_comment(&p.id, &p.body).await;
            comment_changed(shared, added).await
        }),
        Request::ReviewCommentEdit(p) => boxed(move || async move {
            let edited = board.edit_comment(&p.id, &p.body).await;
            comment_changed(shared, edited).await
        }),
        Request::ReviewCommentDelete(p) => boxed(move || async move {
            match board.delete_comment(&p.id).await {
                Ok(key) => {
                    publish_ticket_changed(shared, &key).await;
                    Ok(Reply::Ok)
                }
                Err(e) => Err(e),
            }
        }),
        Request::TicketSendBack(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared
                    .review
                    .send_back(&p.key, &p.note, p.clear_first, &actor),
            )
            .await
            .map(Reply::Returned)
        }),
        Request::TicketConflict(p) => boxed(move || async move {
            match board.ticket(&p.key).await {
                Ok(t) => board.conflict(&t.id).await.map(Reply::Conflict),
                Err(e) => Err(e),
            }
        }),
        Request::TicketApproveCheck(p) => {
            boxed(move || async move { board.approve_check(&p.key).await.map(Reply::ApproveCheck) })
        }
        Request::TicketMergeProposal(p) => {
            boxed(
                move || async move { board.merge_proposal(&p.key).await.map(Reply::MergeProposal) },
            )
        }
        Request::TicketLocalMerge(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared
                    .review
                    .local_merge(&p.key, &p.message, p.approve_anyway, &actor),
            )
            .await
            .map(Reply::LocalMerge)
        }),
        Request::TicketSendToFix(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.send_to_fix(&p.key, p.clear_first, &actor),
            )
            .await
            .map(Reply::Returned)
        }),
        // PR comments are fetched only here, while the agent asks for its feedback (REV-05, d0017).
        Request::ReviewFeedback(p) => boxed(move || async move {
            match board.review_feedback(&p.key).await {
                Ok(mut feedback) => board
                    .pr_feedback(&p.key, &shared.config.tools.path)
                    .await
                    .map(|pr| {
                        feedback.pr = pr;
                        Reply::ReviewFeedback(feedback)
                    }),
                Err(e) => Err(e),
            }
        }),
        Request::ReviewComments(p) => boxed(move || async move {
            board
                .comments(&p.key, p.round)
                .await
                .map(Reply::ReviewComments)
        }),
        Request::TicketAskHuman(p) => {
            boxed(move || async move { ask_human(shared, caller, &p).await })
        }
        Request::TicketReviewSubmit(p) => boxed(move || async move {
            let project = project_of(&p.key);
            // From M12 the submit starts a validator when validation is on for the project (VAL-01).
            let submitted = mutate(
                shared,
                &project,
                Box::pin(shared.validation.submit(
                    &p.key,
                    &p.summary,
                    &actor,
                    crate::domain::ValidationTrigger::Agent,
                )),
            )
            .await;
            if submitted.is_ok()
                && let Caller::Agent { session, .. } = caller
            {
                shared.signals.mark_submitted_or_asked(session);
            }
            submitted.map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::ValidationVerdict(p) => boxed(move || async move {
            let Caller::Validator { session, .. } = caller else {
                return Err(BoardError::refused(
                    Refusal::NotAllowed,
                    crate::policy::ONLY_A_VALIDATOR,
                ));
            };
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                Box::pin(
                    shared
                        .validation
                        .verdict(&p.key, p.pass, &p.report, session),
                ),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketValidationAction(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                // Boxed: the validation futures are large, and this match is every request's frame.
                Box::pin(
                    shared
                        .validation
                        .user_action(&p.key, p.action, &p.note, &actor),
                ),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketValidations(p) => boxed(move || async move {
            match board.ticket(&p.key).await {
                Ok(ticket) => validation_records(shared, &ticket)
                    .await
                    .map(Reply::Validations),
                Err(e) => Err(e),
            }
        }),
        Request::TicketCreateFromNote(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.create_from_note(&p.key, &p.note, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketPrimeValidate(p) => boxed(move || async move {
            Box::pin(board.validate_prime(&p.key))
                .await
                .map(|c| Reply::PrimeValidate(Box::new(c)))
        }),
        Request::TicketPrPropose(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.pr_propose(&p.key, &p.proposal, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketPrOpened(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, shared.prs.opened(&p.key, &p.url, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketOpenPr(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.prs.open(&p.key, p.approve_anyway, p.clear, &actor),
            )
            .await
            .map(Reply::PrOutcome)
        }),
        Request::TicketPrConfirm(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.prs.confirm(&p.key, &p.proposal, p.clear, &actor),
            )
            .await
            .map(Reply::PrOutcome)
        }),
        Request::TicketPrRetry(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, shared.prs.retry(&p.key, p.clear))
                .await
                .map(Reply::PrOutcome)
        }),
        Request::TicketPrCancel(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, shared.prs.cancel(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketMarkMerged(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, shared.review.mark_merged(&p.key, &actor))
                .await
                .map(Reply::MarkedMerged)
        }),
        Request::TicketMarkClosed(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.mark_closed(&p.key, p.confirm, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketPrFlow(p) => boxed(move || async move {
            board
                .pr_flow_view(&p.key)
                .await
                .map(|v| Reply::PrFlow(Box::new(v)))
        }),
        Request::TicketPrBranchCheck(p) => boxed(move || async move {
            board
                .check_branch_name(&p.key, &p.name)
                .await
                .map(Reply::BranchCheck)
        }),
        Request::TicketAsk(p) => boxed(move || async move {
            shared
                .messages
                .ask(&p.key, &p.target, &p.question, &actor)
                .await
                .map(|m| Reply::Message(Box::new(m)))
        }),
        Request::MessageAnswer(p) => boxed(move || async move {
            shared
                .messages
                .answer(&p.id, &p.answer, &actor)
                .await
                .map(|m| Reply::Message(Box::new(m)))
        }),
        Request::MessageResume(p) => boxed(move || async move {
            shared
                .messages
                .resume_target(&p.id)
                .await
                .map(|m| Reply::Message(Box::new(m)))
        }),
        Request::TicketQuestions(p) => boxed(move || async move {
            match board.ticket(&p.key).await {
                Ok(t) => board.pending_questions(&t.id).await.map(Reply::Messages),
                Err(e) => Err(e),
            }
        }),
        Request::TicketHumanAnswers(p) => boxed(move || async move {
            match board.ticket(&p.key).await {
                Ok(t) => board
                    .answered_human_questions(&t.id)
                    .await
                    .map(Reply::HumanAnswers),
                Err(e) => Err(e),
            }
        }),
        Request::TicketMessages(p) => boxed(move || async move {
            match board.ticket(&p.key).await {
                Ok(t) => board.messages_of(&t.id).await.map(Reply::Messages),
                Err(e) => Err(e),
            }
        }),
        Request::TicketBranchRename(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.rename_branch(&p.key, &p.name, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketPrime(p) => boxed(move || async move {
            board
                .prime_context(&p.key)
                .await
                .map(|c| Reply::Prime(Box::new(c)))
        }),
        Request::TicketRetryStart(p) => boxed(move || async move {
            let project = project_of(&p.key);
            // Back to Queued; the scheduler's next pass runs the start sequence again.
            let retried = mutate(shared, &project, shared.starter.retry(&p.key, &actor)).await;
            retried.map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketPin(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, board.pin(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketUnpin(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, board.unpin(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketDeletePreview(p) => {
            boxed(
                move || async move { board.delete_preview(&p.key).await.map(Reply::DeletePreview) },
            )
        }
        Request::TicketDelete(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, board.delete_ticket(&p.key))
                .await
                .map(Reply::DeletePreview)
        }),
        Request::TicketAgentEdits(p) => {
            boxed(move || async move { board.agent_edits(&p.key).await.map(Reply::AgentEdits) })
        }
        Request::TicketAgentEditsSeen(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.mark_agent_edits_seen(&p.key, &actor),
            )
            .await
            .map(|()| Reply::Ok)
        }),

        Request::EpicMoveTo(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.move_to_epic(&p.key, &p.epic, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::EpicQueue(p) => boxed(move || async move {
            let project = project_of(&p.key);
            match &p.child {
                Some(child) => {
                    let queued = async {
                        let ticket = board.ticket(child).await?;
                        if ticket.epic.as_ref().map(ToString::to_string).as_deref()
                            != Some(p.key.as_str())
                        {
                            return Err(BoardError::refused(
                                Refusal::MoveRefused,
                                format!("{child} isn't a child of {}", p.key),
                            ));
                        }
                        board.queue_epic_child(child, &actor).await
                    };
                    mutate(shared, &project, queued).await
                }
                None => mutate(shared, &project, board.queue_epic(&p.key, &actor)).await,
            }
            .map(Reply::EpicQueued)
        }),
        Request::EpicTakeBack(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.take_back_epic(&p.key, p.confirm, &actor),
            )
            .await
            .map(Reply::EpicTakenBack)
        }),
        Request::EpicUndoQueue(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.board.undo_epic_queue(&p.key, &p.keys, &actor),
            )
            .await
            .map(Reply::EpicQueueUndone)
        }),
        Request::EpicClosePlan(p) => {
            boxed(move || async move { board.epic_close_plan(&p.key).await.map(Reply::ClosePlan) })
        }
        Request::EpicClose(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.close_epic(&p.key, p.confirm, &actor),
            )
            .await
            .map(Reply::ClosePlan)
        }),
        Request::TicketSubmitByUser(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                board.submit_by_user(&p.key, &p.note, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketResume(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, shared.review.resume(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketRetryResume(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, shared.review.retry_resume(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketStartFromScratch(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.start_from_scratch(&p.key, p.confirm, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketsResumeAll(p) => {
            boxed(move || async move { resume_all(shared, p.project.as_deref(), &actor).await })
        }
        Request::InboxList => {
            boxed(
                move || async move { shared.inbox.list().await.map(|l| Reply::Inbox(Box::new(l))) },
            )
        }
        Request::InboxCounts => {
            boxed(move || async move { shared.inbox.counts().await.map(Reply::InboxCounts) })
        }
        Request::CleanupScan => {
            boxed(move || async move { shared.cleanup.scan().await.map(Reply::CleanupScan) })
        }
        Request::CleanupDelete(p) => boxed(move || async move {
            shared
                .cleanup
                .delete(&p.items, p.confirm)
                .await
                .map(Reply::CleanupOutcome)
        }),
        Request::InboxAnswer(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.inbox.answer(&p.key, &p.choice, p.clear, &actor),
            )
            .await
            .map(Reply::Delivery)
        }),
        Request::TicketPark(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.park(&p.key, p.confirm, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketReopen(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(shared, &project, board.reopen(&p.key, &actor))
                .await
                .map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::TicketClose(p) => boxed(move || async move {
            let project = project_of(&p.key);
            mutate(
                shared,
                &project,
                shared.review.close(&p.key, p.confirm, &actor),
            )
            .await
            .map(|t| Reply::Ticket(Box::new(t)))
        }),

        Request::DepAdd(p) => boxed(move || async move {
            let project = project_of(&p.child);
            let before = board.ticket(&p.child).await.ok();
            let after = mutate(
                shared,
                &project,
                board.add_dependency(&p.child, &p.parent, &actor),
            )
            .await;
            if let (Some(before), Ok(after)) = (&before, &after) {
                queue_change_note(shared, before, after, &actor).await;
            }
            after.map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::DepRemove(p) => boxed(move || async move {
            let project = project_of(&p.child);
            let before = board.ticket(&p.child).await.ok();
            let after = mutate(
                shared,
                &project,
                board.remove_dependency(&p.child, &p.parent, &actor),
            )
            .await;
            if let (Some(before), Ok(after)) = (&before, &after) {
                queue_change_note(shared, before, after, &actor).await;
            }
            after.map(|t| Reply::Ticket(Box::new(t)))
        }),
        Request::DepTree(p) => {
            boxed(move || async move { board.dependency_tree(&p.key).await.map(Reply::DepTree) })
        }
        Request::DepCycles(p) => {
            boxed(
                move || async move { board.dependency_cycles(&p.project).await.map(Reply::Cycles) },
            )
        }

        Request::SettingsGet => boxed(move || async move { global_settings(board).await }),
        Request::SettingsSet(settings) => boxed(move || async move {
            match board.set_global_settings(&settings).await {
                Ok(()) => {
                    // The agent limits apply at once (SCH-03).
                    shared.scheduler.wake();
                    shared.events.publish(Event::SettingsChanged);
                    global_settings(board).await
                }
                Err(e) => Err(e),
            }
        }),
        Request::ProjectSettingsGet(p) => {
            boxed(move || async move { project_settings(board, &p.project).await })
        }
        Request::ProjectSettingsSet(p) => boxed(move || async move {
            match board.set_project_settings(&p.project, &p.settings).await {
                Ok(()) => project_settings(board, &p.project).await,
                Err(e) => Err(e),
            }
        }),

        Request::Ping
        | Request::ServiceStatus
        | Request::StartupReport
        | Request::ServiceStop
        | Request::Subscribe
        | Request::ToolsCheck
        | Request::OnboardingState
        | Request::DismissPermissionWarning
        | Request::SessionList(_)
        | Request::SessionAttach(_)
        | Request::SessionDetach(_)
        | Request::SessionResize(_)
        | Request::SessionInput(_)
        | Request::SessionDeliverNow(_)
        | Request::SessionFocus(_)
        | Request::Hook(_)
        | Request::RunStart(_)
        | Request::RunStop(_)
        | Request::RunStatus(_)
        | Request::RunList
        | Request::RunSuggest(_)
        | Request::RunSuggestCancel(_)
        | Request::ShellOpen(_)
        | Request::ProjectTerminalOpen(_)
        | Request::ProjectTerminalList(_)
        | Request::ProjectTerminalClose(_)
        | Request::ProjectTerminalCreated(_)
        | Request::ProjectTerminalDismiss(_)
        | Request::ProjectAgentOpen(_)
        | Request::ProjectAgentList(_)
        | Request::ProjectAgentClose(_)
        | Request::ProjectAgentResume(_)
        | Request::ProjectAgentRename(_)
        | Request::AgentName(_)
        | Request::AgentAsk(_)
        | Request::AgentAskHuman(_)
        | Request::ProjectAgentAnswer(_)
        | Request::ProjectAgentPrime(_)
        | Request::ProjectAgentCreated(_) => return None,
    };
    Some(answer.await.map_err(|e| error_body(&e)))
}

/// A board request's future, boxed.
type Answer<'a> = Pin<Box<dyn Future<Output = Result<Reply, BoardError>> + Send + 'a>>;

/// Builds one arm's future in this call's own frame and boxes it. A debug build gives every future that
/// [`handle`] holds inline its own slot in `handle`'s frame; with about a hundred arms that frame reached 1.5 MB, and
/// on Windows a request with a deep future of its own (a review submit starting a validator) overflowed the 2 MB
/// tokio worker stack (T-0275). Built here, each arm's future lives on the heap and `handle` holds only the box.
fn boxed<'a, F>(make: impl FnOnce() -> F) -> Answer<'a>
where
    F: Future<Output = Result<Reply, BoardError>> + Send + 'a,
{
    Box::pin(make())
}

async fn global_settings(board: &Board) -> Result<Reply, BoardError> {
    Ok(Reply::GlobalSettings(Box::new(GlobalSettingsReply {
        settings: board.global_settings().await?,
        problems: board.global_setting_problems().await?,
    })))
}

async fn project_settings(board: &Board, project: &str) -> Result<Reply, BoardError> {
    Ok(Reply::ProjectSettings(Box::new(ProjectSettingsReply {
        settings: board.project_settings(project).await?,
        problems: board.project_setting_problems(project).await?,
        effective: board.effective_settings(project).await?,
    })))
}

/// Resume all, then tells clients about every ticket it resumed.
async fn resume_all(
    shared: &Shared,
    project: Option<&str>,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    let resumed = shared.review.resume_all(project, actor).await?;
    for key in &resumed {
        if let Ok(ticket) = shared.board.ticket(&key.to_string()).await {
            shared.events.publish(Event::TicketChanged {
                project: key.project.to_string(),
                id: ticket.id.clone(),
                key: key.clone(),
            });
            shared.events.publish(Event::CardStatusChanged {
                project: key.project.to_string(),
                id: ticket.id,
                key: key.clone(),
                status: ticket.status,
            });
        }
    }
    Ok(Reply::Resumed(resumed))
}

/// `otaq ask human`: the question (with its options) is stored and the ticket shows Asks you (ATT-01,
/// ATT-12).
async fn ask_human(
    shared: &Shared,
    caller: &Caller,
    p: &crate::protocol::AskHumanParams,
) -> Result<Reply, BoardError> {
    if p.question.trim().is_empty() {
        return Err(BoardError::refused(
            Refusal::BadQuestion,
            "Write the question on stdin",
        ));
    }
    validate_options(&p.options, p.recommended)?;
    let board = &shared.board;
    let ticket = board.ticket(&p.key).await?;
    // During Open PR / Push to PR the skill reports what went wrong (a rejected push, a missing login) to the
    // dialog instead of the board (REV-10).
    if board.pr_flow(&ticket.id).await?.is_some() {
        board
            .report_pr_problem(&ticket.id, p.question.trim())
            .await?;
        return board
            .ticket_by_id(&ticket.id)
            .await
            .map(|t| Reply::Ticket(Box::new(t)));
    }
    board
        .store_question(&ticket.id, &p.question, &p.options, p.recommended)
        .await?;
    shared
        .signals
        .ask_human(caller, &ticket.id, p.question.trim())
        .await?;
    board
        .ticket_by_id(&ticket.id)
        .await
        .map(|t| Reply::Ticket(Box::new(t)))
}

/// SES-08: a user's change to a ticket whose agent is at work queues a note with what changed. Agent edits and
/// tickets that aren't working (Queued, Parked, Backlog, …) get nothing.
async fn queue_change_note(shared: &Shared, before: &Ticket, after: &Ticket, actor: &Actor) {
    let validating = matches!(
        after.status,
        crate::domain::Status::Validating | crate::domain::Status::ValidatorNeedsYou
    );
    if actor.is_agent() || !(LIVE.contains(&after.status) || validating) {
        return;
    }
    let Some(session) = shared.sessions.running_of(&after.id, SessionKind::Agent) else {
        return;
    };
    let Some(note) = change_note(before, after) else {
        return;
    };
    if let Err(e) = shared
        .deliveries
        .queue_delivery(&session.id, DeliveryKind::TicketChange, &note)
        .await
    {
        tracing::warn!(ticket = %after.key, error = %e, "can't queue the ticket-change note");
    }
}

/// A comment was added or changed: its ticket's pending counter changed.
async fn comment_changed(
    shared: &Shared,
    result: Result<crate::board::comments::ReviewComment, BoardError>,
) -> Result<Reply, BoardError> {
    let comment = result?;
    if let Ok((key, _)) = shared.board.comment(&comment.id).await {
        publish_ticket_changed(shared, &key).await;
    }
    Ok(Reply::ReviewComment(comment))
}

/// The ticket's validation rounds with each validator's tokens (History, VAL-12).
async fn validation_records(
    shared: &Shared,
    ticket: &Ticket,
) -> Result<Vec<crate::board::validation::ValidationRecord>, BoardError> {
    let mut records = shared.board.validations(&ticket.key.to_string()).await?;
    let usage = shared.usage.sessions_of(&ticket.id).await?;
    for record in &mut records {
        record.tokens = record
            .session
            .as_ref()
            .and_then(|s| usage.iter().find(|u| &u.session == s))
            .and_then(|u| u.tokens);
    }
    Ok(records)
}

/// Tells clients a ticket changed in a way its row doesn't show (its review comments).
async fn publish_ticket_changed(shared: &Shared, key: &crate::domain::TicketKey) {
    if let Ok(ticket) = shared.board.ticket(&key.to_string()).await {
        shared.events.publish(Event::TicketChanged {
            project: ticket.key.project.to_string(),
            id: ticket.id,
            key: ticket.key,
        });
    }
}

/// The project part of a ticket key (`SHOP-18` → `SHOP`). Aliases keep their project, so this holds for them too.
fn project_of(key: &str) -> String {
    key.split_once('-').map_or(key, |(p, _)| p).to_owned()
}

/// Runs a mutation of one project's tickets and publishes the changes it made.
async fn mutate<T>(
    shared: &Shared,
    project: &str,
    change: impl Future<Output = Result<T, BoardError>>,
) -> Result<T, BoardError> {
    let before = shared.board.tickets(project).await.unwrap_or_default();
    let result = change.await?;
    match shared.board.tickets(project).await {
        Ok(after) => publish_changes(shared, project, &before, &after),
        Err(e) => tracing::warn!(error = %e, project, "can't read the board after a change"),
    }
    Ok(result)
}

fn publish_changes(shared: &Shared, project: &str, before: &[Ticket], after: &[Ticket]) {
    let old: HashMap<&str, &Ticket> = before.iter().map(|t| (t.id.as_str(), t)).collect();
    for ticket in after {
        let previous = old.get(ticket.id.as_str());
        if previous.is_some_and(|p| *p == ticket) {
            continue;
        }
        shared.events.publish(Event::TicketChanged {
            project: project.to_owned(),
            id: ticket.id.clone(),
            key: ticket.key.clone(),
        });
        if previous.is_none_or(|p| p.status != ticket.status) {
            shared.events.publish(Event::CardStatusChanged {
                project: project.to_owned(),
                id: ticket.id.clone(),
                key: ticket.key.clone(),
                status: ticket.status,
            });
        }
    }
    for gone in before
        .iter()
        .filter(|b| !after.iter().any(|a| a.id == b.id))
    {
        shared.events.publish(Event::TicketDeleted {
            project: project.to_owned(),
            id: gone.id.clone(),
            key: gone.key.clone(),
        });
    }
}

/// A board error as the protocol's error body: refusals keep their code in `data.refusal`.
/// Tickets created from a Project terminal are recorded for its summary, and their cards refreshed (T-0167).
async fn created_in(
    shared: &Shared,
    origin: Option<&str>,
    tickets: &[crate::board::tickets::Ticket],
) -> Result<(), BoardError> {
    let Some(session) = origin else {
        return Ok(());
    };
    let ids: Vec<String> = tickets.iter().map(|t| t.id.clone()).collect();
    shared.board.mark_created_in(&ids, session).await?;
    for t in tickets {
        shared.events.publish(Event::TicketChanged {
            project: t.key.project.to_string(),
            id: t.id.clone(),
            key: t.key.clone(),
        });
    }
    Ok(())
}

pub(super) fn error_body(e: &BoardError) -> ErrorBody {
    match e {
        BoardError::Refused { code, message } => {
            let mut body = ErrorBody::new(ErrorKind::Refused, message.clone());
            body.data = Some(json!({ "refusal": code }));
            body
        }
        BoardError::NotFound(message) => ErrorBody::new(ErrorKind::NotFound, message.clone()),
        BoardError::Git(_) | BoardError::Store(_) => {
            tracing::error!(error = %e, "board request failed");
            ErrorBody::new(ErrorKind::Internal, e.to_string())
        }
    }
}
