//! Maps every request to the [`Action`] the permission policy judges (CLI-02, CLI-03). The match is exhaustive,
//! so a new request can't be added without saying what it does.

use crate::board::batch::{BatchTicket, Ref};
use crate::board::{Board, BoardError};
use crate::caller::Caller;
use crate::policy::{Action, TicketFacts, authorize};
use crate::protocol::Request;

/// Checks `request` for `caller` before it runs. The user passes without any lookup; every other caller (an
/// agent, a validator, a Project agent) is judged by the policy.
pub(super) async fn check(
    caller: &Caller,
    request: &Request,
    board: &Board,
) -> Result<(), BoardError> {
    if *caller == Caller::User {
        return Ok(());
    }
    let action = action_for(caller, request, board).await?;
    authorize(caller, &action)
}

async fn facts(board: &Board, key: &str) -> Result<TicketFacts, BoardError> {
    let ticket = board.ticket(key).await?;
    Ok(TicketFacts {
        backlog: ticket.column == crate::domain::Column::Backlog && !ticket.has_started(),
        id: ticket.id,
        label: ticket.key.to_string(),
        created_by: ticket.created_by,
    })
}

/// A batch creates tickets (always allowed) and links them: each link is judged with the new tickets created
/// by the caller.
async fn batch_action(
    caller: &Caller,
    tickets: &[BatchTicket],
    board: &Board,
) -> Result<Action, BoardError> {
    let new = |n: usize| TicketFacts {
        id: format!("batch:{n}"),
        label: format!("the new ticket \"{}\"", tickets[n].title),
        created_by: caller.actor(None),
        backlog: true,
    };
    let mut actions = vec![Action::CreateTicket];
    for (n, ticket) in tickets.iter().enumerate() {
        for dep in &ticket.depends_on {
            let parent = match board.batch_ref(tickets, dep).await? {
                Ref::Batch(m) => new(m),
                Ref::Existing(t) => TicketFacts {
                    backlog: t.column == crate::domain::Column::Backlog && !t.has_started(),
                    label: t.key.to_string(),
                    id: t.id,
                    created_by: t.created_by,
                },
            };
            actions.push(Action::Dependency {
                child: new(n),
                parent,
            });
        }
    }
    Ok(Action::All(actions))
}

async fn action_for(
    caller: &Caller,
    request: &Request,
    board: &Board,
) -> Result<Action, BoardError> {
    use Request as R;
    Ok(match request {
        R::Ping
        | R::ServiceStatus
        | R::StartupReport
        | R::Subscribe
        | R::ToolsCheck
        | R::OnboardingState
        | R::ProjectList
        | R::ProjectProbe(_)
        | R::ProjectBranches(_)
        | R::ProjectPrProvider(_)
        | R::TicketPrFlow(_)
        | R::TicketQuestions(_)
        | R::TicketMessages(_)
        | R::TicketHumanAnswers(_)
        | R::TicketPrBranchCheck(_)
        | R::BoardSnapshot(_)
        | R::TicketGet(_)
        | R::TicketList(_)
        | R::DepReadiness(_)
        | R::SchedulerStatus
        | R::UsageSummary(_)
        | R::UsageSessions(_)
        | R::TicketAgentEdits(_)
        | R::TicketDeletePreview(_)
        | R::TicketHistory(_)
        | R::DiffFiles(_)
        | R::DiffFile(_)
        | R::ReviewComments(_)
        | R::ReviewFeedback(_)
        | R::TicketMergeProposal(_)
        | R::TicketApproveCheck(_)
        | R::TicketConflict(_)
        | R::TicketPrime(_)
        | R::DecisionList(_)
        | R::TicketPrimeValidate(_)
        | R::TicketValidations(_)
        | R::EpicClosePlan(_)
        | R::DepTree(_)
        | R::DepCycles(_)
        | R::SettingsGet
        | R::ProjectSettingsGet(_)
        | R::SessionList(_)
        | R::SessionBeforeRestart(_)
        | R::InboxList
        | R::InboxCounts
        | R::CleanupScan
        | R::SessionAttach(_)
        | R::SessionDetach(_)
        | R::RunStatus(_)
        | R::RunList
        | R::ProjectTerminalCreated(_)
        | R::ProjectTerminalList(_)
        | R::ProjectAgentList(_)
        | R::ProjectAgentPrime(_)
        | R::ProjectAgentCreated(_) => Action::Read,
        // Only a Project agent names itself and asks with these (the handler refuses everyone else).
        R::AgentName(_) | R::AgentAsk(_) | R::AgentAskHuman(_) => Action::Read,
        R::ProjectAgentAnswer(_) => Action::UserOnly("answer Project agents"),
        // A hook only ever signals the caller's own session.
        R::Hook(_) => Action::Read,
        // Bookkeeping from the agent's own status line: the plan windows Claude Code reported.
        R::UsageRecordLimits(_) => Action::Read,

        R::TicketUpdate(p) => Action::EditTicket(facts(board, &p.key).await?),
        R::TicketAskHuman(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::DecisionRecord(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::DecisionChange(_) => Action::UserOnly("change recorded decisions"),
        R::TicketReviewSubmit(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::ValidationVerdict(p) => Action::Verdict(facts(board, &p.key).await?),
        R::TicketValidationAction(_) => Action::UserOnly("act on a validation"),
        R::TicketCreateFromNote(_) => Action::UserOnly("create tickets from validation notes"),
        R::TicketPrPropose(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::TicketPrOpened(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::TicketBranchRename(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        // Asking changes the asker's own ticket; answering, the ticket that was asked (CLI-02).
        R::TicketAsk(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::MessageAnswer(p) => {
            let to = board.message(&p.id).await?.to;
            Action::ChangeTicket(facts(board, &to.to_string()).await?)
        }
        R::TicketCreate(_) => Action::CreateTicket,
        R::TicketCreateBatch(p) => batch_action(caller, &p.tickets, board).await?,
        R::DepAdd(p) | R::DepRemove(p) => Action::Dependency {
            child: facts(board, &p.child).await?,
            parent: facts(board, &p.parent).await?,
        },

        R::ServiceStop => Action::UserOnly("stop the background service"),
        R::DismissPermissionWarning => Action::UserOnly("change onboarding"),
        R::ProjectAdd(_) | R::ProjectUpdate(_) | R::ProjectRemove(_) => {
            Action::UserOnly("change projects")
        }
        R::TicketMove(_) => Action::UserOnly("move tickets"),
        R::TicketPin(_) | R::TicketUnpin(_) => Action::UserOnly("pin tickets"),
        R::TicketAgentEditsSeen(_) => Action::UserOnly("mark agent edits as seen"),
        R::TicketDelete(_) => Action::UserOnly("delete tickets"),
        R::TicketRetryStart(_) => Action::UserOnly("retry starts"),
        R::EpicMoveTo(_) => Action::UserOnly("move tickets into epics"),
        R::EpicClose(_) => Action::UserOnly("close epics"),
        R::EpicQueue(_) => Action::UserOnly("queue epics"),
        R::EpicTakeBack(_) => Action::UserOnly("take back epics"),
        R::EpicUndoQueue(_) => Action::UserOnly("take back epics"),
        R::TicketClose(_) => Action::UserOnly("close tickets"),
        R::TicketReopen(_) => Action::UserOnly("reopen tickets"),
        R::TicketPark(_) => Action::UserOnly("park tickets"),
        R::TicketResume(_)
        | R::TicketRetryResume(_)
        | R::TicketsResumeAll(_)
        | R::UsageResumeAll => Action::UserOnly("resume agents"),
        R::TicketStartFromScratch(_) => Action::UserOnly("start agents from scratch"),
        R::InboxAnswer(_) => Action::UserOnly("answer agents from the inbox"),
        R::UsageMarkLimit => Action::UserOnly("mark agents as stopped by the usage limit"),
        R::CleanupDelete(_) => Action::UserOnly("delete leftovers"),
        R::RunStart(_) | R::RunStop(_) => Action::UserOnly("start or stop Run"),
        R::RunSuggest(_) | R::RunSuggestCancel(_) => {
            Action::UserOnly("ask Claude for a run command")
        }
        R::ShellOpen(_) | R::ProjectTerminalOpen(_) => Action::UserOnly("open shells"),
        R::ProjectTerminalClose(_) => Action::UserOnly("close terminals"),
        R::ProjectAgentOpen(_) | R::ProjectAgentResume(_) => {
            Action::UserOnly("open Project agents")
        }
        R::ProjectAgentClose(_) => Action::UserOnly("close Project agents"),
        R::LeadOpen(_) | R::LeadResume(_) => Action::UserOnly("open epic leads"),
        R::ProjectAgentRename(_) => Action::UserOnly("rename Project agents"),
        R::ProjectTerminalDismiss(_) => Action::UserOnly("dismiss the Project terminal's summary"),
        R::TicketSubmitByUser(_) => Action::UserOnly("submit for review in the user's name"),
        R::SettingsSet(_) | R::ProjectSettingsSet(_) => Action::UserOnly("change settings"),
        R::ReviewCommentAdd(_)
        | R::ReviewCommentReply(_)
        | R::ReviewCommentEdit(_)
        | R::ReviewCommentDelete(_) => Action::UserOnly("write review comments"),
        R::TicketSendBack(_) => Action::UserOnly("send tickets back"),
        R::TicketLocalMerge(_) => Action::UserOnly("merge tickets"),
        R::TicketMarkMerged(_) | R::TicketMarkClosed(_) => Action::UserOnly("mark pull requests"),
        R::MessageResume(_) => Action::UserOnly("resume finished tickets to answer"),
        R::TicketOpenPr(_) | R::TicketPrConfirm(_) | R::TicketPrCancel(_) | R::TicketPrRetry(_) => {
            Action::UserOnly("open or push pull requests")
        }
        R::TicketSendToFix(_) => Action::UserOnly("send tickets back"),
        R::SessionResize(_) | R::SessionInput(_) | R::SessionDeliverNow(_) | R::SessionFocus(_) => {
            Action::UserOnly("type into sessions")
        }
    })
}
