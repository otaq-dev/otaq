//! Maps every request to the [`Action`] the permission policy judges (CLI-02, CLI-03). The match is exhaustive,
//! so a new request can't be added without saying what it does.

use crate::board::{Board, BoardError};
use crate::caller::Caller;
use crate::policy::{Action, TicketFacts, authorize};
use crate::protocol::Request;

/// Checks `request` for `caller` before it runs. The user passes without any lookup.
pub(super) async fn check(
    caller: &Caller,
    request: &Request,
    board: &Board,
) -> Result<(), BoardError> {
    if !caller.is_agent() {
        return Ok(());
    }
    let action = action_for(request, board).await?;
    authorize(caller, &action)
}

async fn facts(board: &Board, key: &str) -> Result<TicketFacts, BoardError> {
    let ticket = board.ticket(key).await?;
    Ok(TicketFacts {
        id: ticket.id,
        key: ticket.key,
        created_by: ticket.created_by,
    })
}

async fn action_for(request: &Request, board: &Board) -> Result<Action, BoardError> {
    use Request as R;
    Ok(match request {
        R::Ping
        | R::ServiceStatus
        | R::Subscribe
        | R::ToolsCheck
        | R::OnboardingState
        | R::ProjectList
        | R::ProjectProbe(_)
        | R::ProjectBranches(_)
        | R::BoardSnapshot(_)
        | R::TicketGet(_)
        | R::TicketList(_)
        | R::DepReadiness(_)
        | R::TicketAgentEdits(_)
        | R::TicketDeletePreview(_)
        | R::TicketHistory(_)
        | R::EpicClosePlan(_)
        | R::DepTree(_)
        | R::DepCycles(_)
        | R::SettingsGet
        | R::ProjectSettingsGet(_)
        | R::SessionList(_)
        | R::SessionAttach(_)
        | R::SessionDetach(_) => Action::Read,
        // A hook only ever signals the caller's own session.
        R::Hook(_) => Action::Read,

        R::TicketUpdate(p) => Action::ChangeTicket(facts(board, &p.key).await?),
        R::TicketCreate(_) => Action::CreateTicket,
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
        R::SettingsSet(_) | R::ProjectSettingsSet(_) => Action::UserOnly("change settings"),
        R::SessionResize(_) | R::SessionInput(_) | R::SessionDeliverNow(_) | R::SessionFocus(_) => {
            Action::UserOnly("type into sessions")
        }
    })
}
