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
use crate::agent_tools::SetupOutcome;
use crate::board::change_note::change_note;
use crate::board::questions::validate_options;
use crate::board::readiness::project_readiness;
use crate::board::tickets::Ticket;
use crate::board::{Board, BoardError, Refusal};
use crate::caller::Caller;
use crate::domain::Actor;
use crate::epic_lead::changes::LeadChange;
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
            let (project, trust) = board.add_project_trusted(&add).await?;
            shared.events.publish(Event::ProjectsChanged);
            // Onboarding ends with a project: the starter entry, if a tool got connected since the start (AGT-01).
            super::spawn_starter(
                &shared.adapters,
                &shared.store,
                shared.config.launch.is_some(),
            );
            // A failed trust lists the project in the notices (PRJ-08).
            if matches!(trust, Some(SetupOutcome::Failed { .. })) {
                shared.events.publish(Event::TrustChanged);
            }
            board
                .project_info_of(&project.key)
                .await
                .map(|project| Reply::ProjectAdded { project, trust })
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
        Request::UsageActivity => boxed(move || async move {
            match shared.usage.activity().await {
                Ok(activity) => Ok(Reply::Activity(Box::new(activity))),
                Err(e) => Err(e.into()),
            }
        }),
        Request::UsageRecordLimits(p) => boxed(move || async move {
            match shared
                .plan_limit
                .record_windows(p.tool, &p.windows, crate::store::now_ms())
                .await
            {
                Ok(_) => Ok(Reply::Ok),
                Err(e) => Err(e.into()),
            }
        }),
        Request::AgentsModels(p) => {
            boxed(
                move || async move { Ok(Reply::Models(shared.models.copy(p.tool).map(Box::new))) },
            )
        }
        Request::AgentsModelsRefresh(p) => boxed(move || async move {
            Ok(Reply::Models(
                shared.models.refresh(p.tool).await.map(Box::new),
            ))
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
                Ok(t) => {
                    lead_changed(shared, caller, &t, LeadChange::Created, &actor).await;
                    created_in(shared, origin, std::slice::from_ref(&t))
                        .await
                        .map(|()| Reply::Ticket(Box::new(t)))
                }
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
                Ok(tickets) => {
                    for t in &tickets {
                        lead_changed(shared, caller, t, LeadChange::Created, &actor).await;
                    }
                    created_in(shared, origin, &tickets)
                        .await
                        .map(|()| Reply::Tickets(tickets))
                }
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
            if let Ok(after) = &after {
                let fields = updated_fields(&p.update);
                lead_changed(
                    shared,
                    caller,
                    after,
                    LeadChange::Updated { fields },
                    &actor,
                )
                .await;
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
                shared.usage.adapters(),
                &p.key,
                p.limit
                    .map_or(crate::recovery::DEFAULT_ENTRIES, |n| n as usize),
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
                crate::decisions::outcome::local_merge_keeping(
                    &shared.review,
                    &p.key,
                    &p.message,
                    p.approve_anyway,
                    &actor,
                ),
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
            boxed(move || async move { ask_human(shared, caller, &p, &actor).await })
        }
        Request::DecisionRecord(p) => {
            boxed(move || async move { record_decision(shared, &p, &actor).await })
        }
        Request::DecisionList(p) => boxed(move || async move {
            crate::decisions::list(board, &p.key)
                .await
                .map(Reply::Decisions)
        }),
        Request::DecisionChange(p) => {
            boxed(move || async move { change_decision(shared, &p, &actor).await })
        }
        Request::DecisionMark(p) => {
            boxed(move || async move { mark_decision(shared, caller, &p, &actor).await })
        }
        Request::LeadSuggest(p) => {
            boxed(move || async move { lead_suggest(shared, caller, &p, &actor).await })
        }
        Request::SuggestionConfirm(p) => {
            boxed(move || async move { confirm_suggestion(shared, &p.id, &actor).await })
        }
        Request::SuggestionDismiss(p) => {
            boxed(move || async move { dismiss_suggestion(shared, &p.id, &actor).await })
        }
        Request::LeadPassOn(p) => {
            boxed(move || async move { pass_on(shared, caller, &p, &actor).await })
        }
        Request::LeadQuestions => {
            boxed(move || async move { lead_questions_waiting(shared, caller).await })
        }
        Request::LeadPrime(p) => boxed(move || async move { epic_prime(shared, &p.key).await }),
        Request::LeadNote(p) => {
            boxed(move || async move { lead_note(shared, caller, &p, &actor).await })
        }
        Request::TicketNotes(p) => boxed(move || async move { read_notes(shared, &p.key).await }),
        Request::TicketReviewSubmit(p) => boxed(move || async move {
            // Not while the lead's note is unread (CLI-08); the user's Submit for review is another request.
            board.refuse_unread_notes(&p.key).await?;
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
            // A child's question to its epic's lead (LEAD-06) is answered by the lead; any other is a message's.
            match crate::epic_lead::questions::find(board, &p.id).await {
                Ok(Some(_)) => lead_questions(shared)
                    .answer(caller, &p.id, &p.answer, &actor)
                    .await
                    .map(|q| Reply::LeadQuestion(Box::new(q))),
                Ok(None) => shared
                    .messages
                    .answer(&p.id, &p.answer, &actor)
                    .await
                    .map(|m| Reply::Message(Box::new(m))),
                Err(e) => Err(e),
            }
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
        Request::TicketLeadLog(p) => boxed(move || async move {
            crate::epic_lead::log::of(board, &p.key)
                .await
                .map(|log| Reply::LeadLog(Box::new(log)))
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
        Request::TicketPrime(p) => {
            boxed(move || async move { prime(shared, caller, &p.key).await })
        }
        Request::TicketTerminal(p) => boxed(move || async move { terminal(shared, &p).await }),
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
        Request::WarmList => boxed(move || async move {
            shared
                .warm
                .list()
                .await
                .map(|list| Reply::WarmList(Box::new(list)))
        }),
        Request::WarmClear(p) => boxed(move || async move {
            let project = match &p.project {
                Some(key) => Some(board.project(key).await?),
                None => None,
            };
            let repo = match (&project, p.repo) {
                (Some(project), _) => crate::warm::store::repo_name(project),
                (None, Some(repo)) => repo,
                (None, None) => {
                    return Err(BoardError::refused(
                        Refusal::EmptyField,
                        "name the project or the warm build to clear",
                    ));
                }
            };
            Ok(Reply::WarmClear(
                shared.warm.start_clear(project.as_ref(), &repo).await,
            ))
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
                if let Ok(parent) = board.ticket(&p.parent).await {
                    let change = LeadChange::DepAdded { parent: parent.key };
                    lead_changed(shared, caller, after, change, &actor).await;
                }
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
                if let Ok(parent) = board.ticket(&p.parent).await {
                    let change = LeadChange::DepRemoved { parent: parent.key };
                    lead_changed(shared, caller, after, change, &actor).await;
                }
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
                Ok(()) => {
                    // Turning the switch off clears the warm build, after saving (WRM-05); the confirmation is
                    // the app's. A Clear that waits for a copy goes on in the background.
                    if !p.settings.warm_builds {
                        let project = board.project(&p.project).await?;
                        let repo = crate::warm::store::repo_name(&project);
                        shared.warm.start_clear(Some(&project), &repo).await;
                    }
                    project_settings(board, &p.project).await
                }
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
        | Request::DismissTrustNote
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
        | Request::ProjectAgentCloseEnded(_)
        | Request::ProjectAgentResume(_)
        | Request::ProjectAgentRename(_)
        | Request::AgentName(_)
        | Request::AgentAsk(_)
        | Request::AgentAskHuman(_)
        | Request::ProjectAgentAnswer(_)
        | Request::ProjectAgentPrime(_)
        | Request::ProjectAgentCreated(_)
        | Request::LeadOpen(_)
        | Request::LeadResume(_)
        | Request::LeadList(_)
        | Request::AgentsList
        | Request::AgentsAdd(_)
        | Request::AgentsEdit(_)
        | Request::AgentsDuplicate(_)
        | Request::AgentsDelete(_)
        | Request::AgentsSetDefault(_)
        | Request::ProjectSetAgentDefault(_)
        | Request::TicketAgentRow(_) => return None,
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
    actor: &Actor,
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
    let question = board
        .store_question(&ticket.id, &p.question, &p.options, p.recommended)
        .await?;
    // An epic child's question goes to its live lead first (LEAD-06); otherwise the ticket asks the user.
    if !lead_questions(shared)
        .route(caller, &ticket, &question, actor)
        .await?
    {
        shared
            .signals
            .ask_human(caller, &ticket.id, p.question.trim())
            .await?;
    }
    board
        .ticket_by_id(&ticket.id)
        .await
        .map(|t| Reply::Ticket(Box::new(t)))
}

/// `otaq decision record` (DEC-02, d0084): stores the decision and its History line. The ticket's state doesn't
/// change and nobody is notified; only clients showing its History or Decisions refresh.
async fn record_decision(
    shared: &Shared,
    p: &crate::protocol::DecisionRecordParams,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    let decision = crate::decisions::record(&shared.board, &p.key, &p.decision, actor).await?;
    publish_ticket_changed(shared, &decision.id.ticket).await;
    Ok(Reply::Decision(Box::new(decision)))
}

/// Change… on a recorded decision (DEC-06, d0084): Send back with the text, or a fix child in the epic's review.
async fn change_decision(
    shared: &Shared,
    p: &crate::protocol::DecisionChangeParams,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    use crate::decisions::outcome;
    let project =
        p.id.parse::<crate::decisions::DecisionId>()?
            .ticket
            .project
            .to_string();
    let changed = match (&p.text, &p.title) {
        (Some(text), _) => {
            mutate(
                shared,
                &project,
                outcome::change_and_send_back(&shared.review, &p.id, text, p.clear_first, actor),
            )
            .await?
        }
        (None, Some(title)) => {
            mutate(
                shared,
                &project,
                outcome::change_with_fix_child(
                    &shared.board,
                    &p.id,
                    title,
                    p.description.as_deref().unwrap_or_default(),
                    actor,
                ),
            )
            .await?
        }
        (None, None) => {
            return Err(BoardError::refused(
                crate::board::Refusal::EmptyField,
                "Change\u{2026} needs the text to send back, or the fix child's title",
            ));
        }
    };
    Ok(Reply::DecisionChanged(changed))
}

/// `otaq show <KEY> --terminal` (SHL-10, d0087): the ticket's agent session (running, or the last ended one still
/// in memory), or with `validator` its running validator, replayed into plain text. Read-only; nothing is kept
/// between requests. While the ticket validates, the agent's text starts with a line pointing at `--validator`.
async fn terminal(
    shared: &Shared,
    p: &crate::protocol::TerminalParams,
) -> Result<Reply, BoardError> {
    use crate::session::screen;
    let ticket = shared.board.ticket(&p.key).await?;
    let key = &ticket.key;
    let validator = shared
        .sessions
        .running_of(&ticket.id, SessionKind::Validator);
    let (session, bytes, size) = if p.validator {
        match validator.as_ref().and_then(|_| {
            shared
                .sessions
                .output_of(&ticket.id, SessionKind::Validator)
        }) {
            Some(output) => output,
            None => {
                return Err(BoardError::refused(
                    Refusal::SessionEnded,
                    format!("{key} has no running validator"),
                ));
            }
        }
    } else {
        match shared.sessions.output_of(&ticket.id, SessionKind::Agent) {
            Some(output) => output,
            None => {
                // Output never goes to disk (d0045): a stored session means a restart cleared it.
                let stored: Option<(i64,)> = sqlx::query_as(
                    "SELECT 1 FROM sessions WHERE ticket_id = ? AND kind = ? LIMIT 1",
                )
                .bind(&ticket.id)
                .bind(SessionKind::Agent.as_str())
                .fetch_optional(shared.board.store().pool())
                .await?;
                let message = match stored {
                    Some(_) => format!("{key}'s terminal output isn't kept after a restart."),
                    None => format!("{key} has no terminal output: its agent hasn't started."),
                };
                return Err(BoardError::refused(Refusal::SessionEnded, message));
            }
        }
    };
    let mut text = screen::render(&bytes, size);
    if !p.validator && validator.is_some() {
        text = format!("{key} is validating \u{b7} its validator's terminal: --validator\n{text}");
    }
    Ok(Reply::TicketTerminal(Box::new(
        crate::protocol::TicketTerminal {
            key: key.clone(),
            session: session.id,
            text,
        },
    )))
}

/// `otaq prime`: the ticket's context. Its own agent reads the unread lead notes in it, so they are marked read
/// (LEAD-10); anyone else's prime leaves them unread. Skills are named for the caller's session's tool (CLI-01,
/// ADP-05); a caller with no agent session (by hand, with `OTAQ_TICKET`) gets a new session's tool.
async fn prime(shared: &Shared, caller: &Caller, key: &str) -> Result<Reply, BoardError> {
    let session = match caller {
        Caller::Agent { session, .. }
        | Caller::Validator { session, .. }
        | Caller::ProjectAgent { session, .. }
        | Caller::EpicLead { session, .. } => Some(session),
        Caller::User => None,
    };
    let tool = session
        .and_then(|s| shared.sessions.info(s).ok())
        .and_then(|info| info.tool);
    let context = shared
        .board
        .prime_context(key, shared.adapters.of_session(tool))
        .await?;
    if let Caller::Agent { ticket_id, .. } = caller
        && *ticket_id == context.ticket.id
        && !context.unread_notes.is_empty()
    {
        let ids: Vec<String> = context.unread_notes.iter().map(|n| n.id.clone()).collect();
        crate::epic_lead::notes::mark_read(
            &shared.board,
            &ids,
            crate::epic_lead::notes::ReadHow::Prime,
        )
        .await?;
        publish_ticket_changed(shared, &context.ticket.key).await;
    }
    Ok(Reply::Prime(Box::new(context)))
}

/// `otaq note <KEY> [--view "…"] --stdin` (LEAD-10): the lead's unread note for a running child of its epic, with
/// History on both. A child already idle between turns gets it typed in at once (d0048). A child whose first stop
/// waits for the lead gets it as Answer the agent (LEAD-07).
async fn lead_note(
    shared: &Shared,
    caller: &Caller,
    p: &crate::protocol::LeadNoteParams,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    use crate::epic_lead::notes;
    let (epic_id, _) = the_lead(shared, caller, notes::ONLY_A_LEAD_NOTES).await?;
    let lead_id = caller
        .project_agent()
        .map(|(agent, _)| agent)
        .unwrap_or_default();
    let note = notes::leave(
        &shared.board,
        epic_id,
        lead_id,
        &p.key,
        &p.text,
        p.view.as_deref(),
        actor,
    )
    .await?;
    // A child whose first stop waits for the lead: the note answers it (LEAD-07).
    let answered = stuck(shared).answer(&note, actor).await?;
    let note = if answered {
        notes::get(&shared.board, &note.id).await?
    } else {
        note
    };
    let child = shared.board.ticket_by_id(&note.child_id).await?;
    if !answered
        && child.status == crate::domain::Status::Idle
        && let Err(e) = notes::type_in(
            &shared.board,
            &shared.sessions,
            &shared.deliveries,
            &child.id,
        )
        .await
    {
        tracing::warn!(child = %child.key, error = %e, "can't type the lead's note in");
    }
    publish_ticket_changed(shared, &note.child).await;
    publish_ticket_changed(shared, &note.epic).await;
    Ok(Reply::LeadNote(Box::new(note)))
}

/// `otaq notes` (LEAD-10): the ticket's unread lead notes, now read; the card's marker clears.
async fn read_notes(shared: &Shared, key: &str) -> Result<Reply, BoardError> {
    let notes = crate::epic_lead::notes::read_unread(&shared.board, key).await?;
    if let Some(note) = notes.first() {
        publish_ticket_changed(shared, &note.child).await;
    }
    Ok(Reply::LeadNotes(notes))
}

/// The epic lead caller's epic id and name ("Wishlist lead"); refused for anyone else with `only`.
async fn the_lead<'a>(
    shared: &Shared,
    caller: &'a Caller,
    only: &str,
) -> Result<(&'a str, String), BoardError> {
    let Caller::EpicLead { epic_id, .. } = caller else {
        return Err(BoardError::refused(Refusal::NotAllowed, only));
    };
    let epic = shared.board.ticket_by_id(epic_id).await?;
    Ok((epic_id, crate::project_agents::lead::lead_name(&epic.title)))
}

/// Stuck children and their epic's lead (LEAD-07), over the service's parts.
fn stuck(shared: &Shared) -> crate::epic_lead::stuck::Stuck {
    crate::epic_lead::stuck::Stuck::new(crate::epic_lead::stuck::StuckParts {
        board: shared.board.clone(),
        leads: shared.project_agents.leads(),
        signals: shared.signals.clone(),
        validation: shared.validation.clone(),
        review: shared.review.clone(),
        deliveries: shared.deliveries.clone(),
        sessions: shared.sessions.clone(),
        events: shared.events.clone(),
    })
}

/// Children's questions to their epic's lead (LEAD-06), over the service's parts.
fn lead_questions(shared: &Shared) -> crate::epic_lead::questions::LeadQuestions {
    crate::epic_lead::questions::LeadQuestions::new(
        shared.board.clone(),
        shared.signals.clone(),
        shared.project_agents.leads(),
        shared.deliveries.clone(),
        shared.sessions.clone(),
        shared.events.clone(),
    )
}

/// `otaq ask human --pass-on` (LEAD-06): the lead passes a child's question on; the child asks the user.
async fn pass_on(
    shared: &Shared,
    caller: &Caller,
    p: &crate::protocol::PassOnParams,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    lead_questions(shared)
        .pass_on(caller, &p.id, &p.options, p.recommended, &p.reason, actor)
        .await
        .map(|q| Reply::LeadQuestion(Box::new(q)))
}

/// `otaq questions` from a lead: its children's questions waiting for it.
async fn lead_questions_waiting(shared: &Shared, caller: &Caller) -> Result<Reply, BoardError> {
    let Caller::EpicLead { epic_id, .. } = caller else {
        return Err(BoardError::refused(
            Refusal::NotAllowed,
            "Only an epic's lead has questions waiting for it here; a ticket's agent runs `questions` on its ticket",
        ));
    };
    crate::epic_lead::questions::waiting(&shared.board, epic_id)
        .await
        .map(Reply::LeadQuestions)
}

/// `otaq prime --epic` (LEAD-03): the epic lead's context.
async fn epic_prime(shared: &Shared, key: &str) -> Result<Reply, BoardError> {
    crate::epic_lead::prime::context(&shared.board, key)
        .await
        .map(|c| Reply::EpicPrime(Box::new(c)))
}

/// `otaq decision mark` (DEC-04, d0083): the lead's mark on a child's decision; the policy checked the child is its
/// epic's. Only clients showing the child's History or Decisions refresh.
async fn mark_decision(
    shared: &Shared,
    caller: &Caller,
    p: &crate::protocol::DecisionMarkParams,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    let (_, lead) = the_lead(shared, caller, crate::policy::ONLY_A_LEAD_MARKS).await?;
    let decision =
        crate::decisions::mark::mark(&shared.board, &p.id, &p.source, &p.text, &lead, actor)
            .await?;
    publish_ticket_changed(shared, &decision.id.ticket).await;
    Ok(Reply::Decision(Box::new(decision)))
}

/// `otaq suggest close|delete` (LEAD-08): stores the suggestion; the inbox picks it up (no notification, no status
/// change). The child's card doesn't change.
async fn lead_suggest(
    shared: &Shared,
    caller: &Caller,
    p: &crate::protocol::SuggestParams,
    actor: &Actor,
) -> Result<Reply, BoardError> {
    let (epic_id, lead) = the_lead(shared, caller, crate::policy::ONLY_A_LEAD_SUGGESTS).await?;
    let suggestion = crate::epic_lead::suggest::suggest(
        &shared.board,
        epic_id,
        &p.key,
        p.kind,
        &p.reason,
        &lead,
        actor,
    )
    .await?;
    // The inbox derives its items again on the child's change event.
    publish_ticket_changed(shared, &suggestion.child).await;
    Ok(Reply::Suggestion(Box::new(suggestion)))
}

/// Close… / Delete… on a suggestion (LEAD-08): the usual close (d0044) or delete (d0060) of the child as the user,
/// then the suggestion is confirmed with its History lines.
async fn confirm_suggestion(shared: &Shared, id: &str, actor: &Actor) -> Result<Reply, BoardError> {
    use crate::epic_lead::suggest::{self, SuggestionKind, SuggestionState};
    let suggestion = suggest::open_one(&shared.board, id).await?;
    let key = suggestion.child.to_string();
    let project = project_of(&key);
    match suggestion.kind {
        SuggestionKind::Close => {
            Box::pin(mutate(
                shared,
                &project,
                shared.review.close(&key, true, actor),
            ))
            .await?;
        }
        SuggestionKind::Delete => {
            Box::pin(mutate(shared, &project, shared.board.delete_ticket(&key))).await?;
        }
    }
    suggest::resolve(
        &shared.board,
        &suggestion,
        SuggestionState::Confirmed,
        actor,
    )
    .await?;
    publish_ticket_changed(shared, &suggestion.epic).await;
    // A delete took the row with the child: the reply is the suggestion as it ended.
    let confirmed = match suggest::get(&shared.board, id).await {
        Ok(confirmed) => confirmed,
        Err(_) => suggest::Suggestion {
            state: SuggestionState::Confirmed,
            resolved_by: Some(actor.clone()),
            resolved_at: Some(crate::store::now_ms()),
            ..suggestion
        },
    };
    Ok(Reply::Suggestion(Box::new(confirmed)))
}

/// Dismiss on a suggestion (LEAD-08): it doesn't wake the lead; `prime --epic` lists it as dismissed.
async fn dismiss_suggestion(shared: &Shared, id: &str, actor: &Actor) -> Result<Reply, BoardError> {
    use crate::epic_lead::suggest::{self, SuggestionState};
    let suggestion = suggest::open_one(&shared.board, id).await?;
    suggest::resolve(
        &shared.board,
        &suggestion,
        SuggestionState::Dismissed,
        actor,
    )
    .await?;
    publish_ticket_changed(shared, &suggestion.child).await;
    publish_ticket_changed(shared, &suggestion.epic).await;
    Ok(Reply::Suggestion(Box::new(
        suggest::get(&shared.board, id).await?,
    )))
}

/// The fields an `update` set, as History names them.
fn updated_fields(update: &crate::board::tickets::TicketUpdate) -> Vec<&'static str> {
    [
        (update.title.is_some(), "title"),
        (update.body.is_some(), "description"),
        (update.priority.is_some(), "priority"),
        (update.base_branch.is_some(), "base branch"),
        (update.external_id.is_some(), "External ID"),
    ]
    .into_iter()
    .filter_map(|(set, name)| set.then_some(name))
    .collect()
}

/// An epic lead's change of a child goes into the child's and the epic's History (CLI-07, d0083); other callers'
/// changes are left as they are.
async fn lead_changed(
    shared: &Shared,
    caller: &Caller,
    child: &Ticket,
    change: LeadChange,
    actor: &Actor,
) {
    let Caller::EpicLead { epic_id, .. } = caller else {
        return;
    };
    let Ok(epic) = shared.board.ticket_by_id(epic_id).await else {
        return;
    };
    let lead = crate::project_agents::lead::lead_name(&epic.title);
    if let Err(e) =
        crate::epic_lead::changes::record(&shared.board, &lead, epic_id, child, &change, actor)
            .await
    {
        tracing::warn!(child = %child.key, error = %e, "can't record the lead's change");
    }
    publish_ticket_changed(shared, &epic.key).await;
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
