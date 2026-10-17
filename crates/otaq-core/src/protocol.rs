//! The one protocol spoken by the app, the CLI and the background service (d0055): JSON messages over the
//! user-locked local socket, as requests and replies plus a live event stream.
//!
//! # Messages
//!
//! Every message is one JSON object carrying the [`PROTOCOL_VERSION`], an optional request id and exactly one
//! body: `request`, `reply`, `event` or `error`.
//!
//! ```text
//! {"protocol_version":1,"id":7,"request":{"method":"service.status"}}
//! {"protocol_version":1,"id":7,"reply":{"service_status":{"pid":4120,...}}}
//! {"protocol_version":1,"event":{"kind":"test","seq":3}}
//! {"protocol_version":1,"id":8,"error":{"kind":"bad_request","message":"..."}}
//! ```
//!
//! A reply or error answers the request with the same id. Events have no id.
//!
//! # Framing (implementation choice, SDD §26)
//!
//! Newline-delimited JSON: each message is serialized compactly (JSON never contains a raw newline there)
//! and followed by `\n`. A frame may be at most [`MAX_FRAME_BYTES`] long. Chosen over length prefixes because
//! it is trivial to read in logs and to poke at by hand (`nc -U ~/.otaq/otaq.sock`).
//!
//! # Versions
//!
//! A peer with another `protocol_version` is reported as [`DecodeError::IncompatibleVersion`]
//! ([`ErrorKind::IncompatibleProtocol`]), which drives "Restart required" (d0022). The envelope fields and the
//! [`VERSION_INDEPENDENT_METHODS`] (`ping`, `service.status`, `service.stop`) never change incompatibly, so an
//! app can always learn the old service's status and restart it.

use std::fmt;
use std::io;

use crate::agent_tools::Tool;
use crate::board::agent_edits::AgentEdits;
use crate::board::batch::BatchTicket;
use crate::board::comments::{NewComment, ReviewComment, ReviewComments, ReviewFeedback};
use crate::board::conflicts::Conflict;
use crate::board::delete::DeletePreview;
use crate::board::deps::DepTree;
use crate::board::diff::DiffSummary;
use crate::board::epic_flight::{EpicQueueOutcome, TakeBackPlan, UndoQueueOutcome};
use crate::board::epics::ClosePlan;
use crate::board::merge::MergeProposal;
use crate::board::moves::DropTarget;
use crate::board::projects::{AddProject, Branches, FolderProbe, ProjectUpdate};
use crate::board::readiness::Readiness;
use crate::board::review::ApproveCheck;
use crate::board::settings::{EffectiveSettings, GlobalSettings, ProjectSettings, SettingProblem};
use crate::board::snapshot::{BoardSnapshot, ProjectInfo};
use crate::board::tickets::{HistoryEvent, NewTicketInput, Ticket, TicketUpdate};
use crate::domain::{Status, TicketKey};
use crate::git::diff::FileDiff;
use crate::prime::PrimeContext;
use crate::review::{LocalMergeOutcome, ReturnOutcome};
use crate::scheduler::SchedulerStatus;
use crate::session::deliveries::Delivery;
use crate::session::{SessionInfo, TerminalSize};
use crate::tools::ToolsReport;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Bumped whenever a message changes incompatibly. Adding optional fields or new methods doesn't need a bump.
pub const PROTOCOL_VERSION: u32 = 1;

/// The longest frame accepted, in bytes (newline included).
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Requests every service answers whatever protocol version the caller speaks. Their request and reply
/// shapes may only grow optional fields.
pub const VERSION_INDEPENDENT_METHODS: &[&str] = &["ping", "service.status", "service.stop"];

/// This build's ID, generated at compile time by `build.rs`: `<git hash>[-dirty].<build time>`.
/// The app, the CLI and the service all report the same value when built together.
pub fn build_id() -> &'static str {
    env!("BUILD_ID")
}

/// One message on the socket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub protocol_version: u32,
    /// Set on requests, and on the reply or error that answers one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// Who a request asks to be recorded as (`--actor`, for example `agent:SHOP-18`, CLI-04). The service
    /// decides the actor from the resolved caller; an agent always acts as itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    /// The sending process's PID on requests (CLI-01). The service checks it against the socket's peer PID, so a
    /// client can't claim another process's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(flatten)]
    pub body: Body,
}

impl Message {
    /// A request from this process: carries its PID.
    pub fn request(id: u64, request: Request) -> Self {
        Self {
            pid: Some(std::process::id()),
            ..Self::new(Some(id), Body::Request(request))
        }
    }

    pub fn reply(id: u64, reply: Reply) -> Self {
        Self::new(Some(id), Body::Reply(reply))
    }

    pub fn error(id: Option<u64>, error: ErrorBody) -> Self {
        Self::new(id, Body::Error(error))
    }

    pub fn event(event: Event) -> Self {
        Self::new(None, Body::Event(event))
    }

    fn new(id: Option<u64>, body: Body) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            id,
            caller: None,
            pid: None,
            body,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    Request(Request),
    Reply(Reply),
    Event(Event),
    Error(ErrorBody),
}

/// What a client asks the service. Message names are an implementation choice (SDD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Request {
    /// Answered with [`Reply::Pong`].
    #[serde(rename = "ping")]
    Ping,
    /// Answered with [`Reply::ServiceStatus`].
    #[serde(rename = "service.status")]
    ServiceStatus,
    /// What this start of the service found (REC-01, REC-06): the interrupted tickets (and which were validating),
    /// the Project agents a restart ended and the unused "New agent"s it discarded. Answered with
    /// [`Reply::StartupReport`].
    #[serde(rename = "service.startup_report")]
    StartupReport,
    /// Asks the service to shut down; answered with [`Reply::Ok`] before it stops.
    #[serde(rename = "service.stop")]
    ServiceStop,
    /// Starts the event stream on this connection; answered with [`Reply::Ok`], then [`Event`]s follow.
    #[serde(rename = "events.subscribe")]
    Subscribe,
    /// The first-run tools checklist and permission-mode check; answered with [`Reply::ToolsCheck`].
    #[serde(rename = "tools.check")]
    ToolsCheck,
    /// What the first-run screens need; answered with [`Reply::OnboardingState`].
    #[serde(rename = "onboarding.state")]
    OnboardingState,
    /// Remembers that the user closed the permission-mode warning; answered with [`Reply::Ok`].
    #[serde(rename = "onboarding.dismiss_permission_warning")]
    DismissPermissionWarning,
    /// The user dismissed the one-time folder trust note (PRJ-08); it never comes back. Answered with [`Reply::Ok`].
    #[serde(rename = "notices.dismiss_trust_note")]
    DismissTrustNote,

    // Projects (PRJ-01…04). Answered with [`Reply::Projects`], [`Reply::FolderProbe`] or [`Reply::Project`]
    // ([`Reply::ProjectAdded`] for `project.add`).
    #[serde(rename = "project.list")]
    ProjectList,
    /// What adding a folder would give (repo root, suggested key, detected base).
    #[serde(rename = "project.probe")]
    ProjectProbe(PathParams),
    #[serde(rename = "project.add")]
    ProjectAdd(AddProject),
    #[serde(rename = "project.update")]
    ProjectUpdate(ProjectUpdateParams),
    /// Answered with [`Reply::Ok`].
    #[serde(rename = "project.remove")]
    ProjectRemove(KeyParams),
    /// The repository's local and remote branches; answered with [`Reply::Branches`].
    #[serde(rename = "project.branches")]
    ProjectBranches(ProjectParams),
    /// The project's PR provider, detected from `origin`, and its CLI's login (REV-11); answered with
    /// [`Reply::PrProvider`]. Only reads.
    #[serde(rename = "project.pr_provider")]
    ProjectPrProvider(ProjectParams),

    /// A project's columns with sorted cards; answered with [`Reply::Board`].
    #[serde(rename = "board.snapshot")]
    BoardSnapshot(ProjectParams),

    // Tickets. Mutations answer with the ticket as it is afterwards ([`Reply::Ticket`]).
    #[serde(rename = "ticket.get")]
    TicketGet(KeyParams),
    /// Every ticket and epic of a project, by key; answered with [`Reply::Tickets`].
    #[serde(rename = "ticket.list")]
    TicketList(ProjectParams),
    #[serde(rename = "ticket.create")]
    TicketCreate(NewTicketInput),
    /// Creates several tickets with links between them, all or nothing (`otaq create -f`, `--deps`).
    /// Answered with [`Reply::Tickets`] (the new tickets, in order).
    #[serde(rename = "ticket.create_batch")]
    TicketCreateBatch(BatchParams),
    #[serde(rename = "ticket.update")]
    TicketUpdate(TicketUpdateParams),
    #[serde(rename = "ticket.move")]
    TicketMove(MoveParams),
    #[serde(rename = "ticket.pin")]
    TicketPin(KeyParams),
    #[serde(rename = "ticket.unpin")]
    TicketUnpin(KeyParams),
    /// Answered with [`Reply::AgentEdits`].
    #[serde(rename = "ticket.agent_edits")]
    TicketAgentEdits(KeyParams),
    /// Answered with [`Reply::Ok`].
    #[serde(rename = "ticket.agent_edits_seen")]
    TicketAgentEditsSeen(KeyParams),
    /// What deleting a never-started ticket would remove; answered with [`Reply::DeletePreview`].
    #[serde(rename = "ticket.delete_preview")]
    TicketDeletePreview(KeyParams),
    /// Deletes a never-started ticket (or epic with its children); answered with [`Reply::DeletePreview`]
    /// (what was removed). Not for agents (CLI-02).
    #[serde(rename = "ticket.delete")]
    TicketDelete(KeyParams),
    /// Retry start (SES-02): a user's click on a ticket whose start stopped (Start interrupted, or Held
    /// because its base diverged). Answered with the ticket back in Queued ([`Reply::Ticket`]); the whole start
    /// sequence then runs again.
    #[serde(rename = "ticket.retry_start")]
    TicketRetryStart(KeyParams),
    /// The context an agent reads first (`otaq prime`, PLG-02). Answered with [`Reply::Prime`].
    #[serde(rename = "ticket.prime")]
    TicketPrime(KeyParams),
    /// `otaq ask human`: the ticket shows Asks you with the question (ATT-01). Answered with
    /// [`Reply::Ticket`].
    #[serde(rename = "ticket.ask_human")]
    TicketAskHuman(AskHumanParams),
    /// `otaq decision record --stdin` (DEC-02, d0084): a recorded decision on the ticket; an agent records on its
    /// own ticket only. Answered with [`Reply::Decision`].
    #[serde(rename = "decision.record")]
    DecisionRecord(DecisionRecordParams),
    /// A ticket's recorded decisions (`otaq show`); answered with [`Reply::Decisions`].
    #[serde(rename = "decision.list")]
    DecisionList(KeyParams),
    /// Change… on a recorded decision in review (DEC-06, d0084): with `text`, Send back with it; with `title` and
    /// `description`, a fix child in the epic's review. Answered with [`Reply::DecisionChanged`].
    #[serde(rename = "decision.change")]
    DecisionChange(DecisionChangeParams),
    /// `otaq decision mark <ID> --source "<source>" --stdin` (DEC-04, d0083): an epic lead marks a child's recorded
    /// decision that a written source contradicts; a second mark replaces the first. Lead only. Answered with
    /// [`Reply::Decision`].
    #[serde(rename = "decision.mark")]
    DecisionMark(DecisionMarkParams),
    /// `otaq review submit`: Ready for Review with the summary; refused with uncommitted files (REV-07).
    /// Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.review_submit")]
    TicketReviewSubmit(ReviewSubmitParams),
    /// `otaq validate pass|fail --stdin`: the validator's verdict and report (VAL-03; validators only).
    #[serde(rename = "validation.verdict")]
    ValidationVerdict(VerdictParams),
    /// The user's validation actions (VAL-06, VAL-07, VAL-09): Validate and submit, Stop validation, Skip to
    /// Review, Move to Review anyway, Give it another round, Answer the agent, Retry validation.
    #[serde(rename = "ticket.validation_action")]
    TicketValidationAction(ValidationActionParams),
    /// `otaq prime --validate`: what the validator works from (VAL-10).
    #[serde(rename = "ticket.prime_validate")]
    TicketPrimeValidate(KeyParams),
    /// The ticket's validation rounds, newest first (History, VAL-12): outcome, duration, tokens, report and
    /// notes. Answered with [`Reply::Validations`].
    #[serde(rename = "ticket.validations")]
    TicketValidations(KeyParams),
    /// **Create ticket** from a "Not covered by any ticket" note of the ticket's validation (VAL-12; the user's
    /// click, never the validator's): a prefilled Backlog ticket in the same epic. Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.create_from_note")]
    TicketCreateFromNote(NoteParams),
    /// `otaq pr propose --stdin` (REV-10): the Open PR / Push skill's proposal (JSON text). Answered with
    /// [`Reply::Ticket`].
    #[serde(rename = "ticket.pr_propose")]
    TicketPrPropose(PrProposeParams),
    /// `otaq pr opened <url>`: the skill reports the PR. Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.pr_opened")]
    TicketPrOpened(PrOpenedParams),
    /// `otaq branch rename <name>` (GIT-07): refused once the branch was pushed. Answered with
    /// [`Reply::Ticket`].
    #[serde(rename = "ticket.branch_rename")]
    TicketBranchRename(BranchRenameParams),
    /// `otaq ask <KEY> --stdin` (MSG-01): `key` asks the linked `target`. Answered with [`Reply::Message`].
    #[serde(rename = "ticket.ask")]
    TicketAsk(AskTicketParams),
    /// `otaq answer <ID> --stdin`. Answered with [`Reply::Message`].
    #[serde(rename = "message.answer")]
    MessageAnswer(AnswerParams),
    /// The inbox's **Resume SHOP-17** on a question sent to the user (MSG-05): the finished ticket answers it.
    /// Answered with [`Reply::Message`].
    #[serde(rename = "message.resume")]
    MessageResume(IdParams),
    /// `otaq questions`: the questions waiting for the ticket's answer. Answered with [`Reply::Messages`].
    #[serde(rename = "ticket.questions")]
    TicketQuestions(KeyParams),
    /// Every question to or from the ticket, oldest first (the panel's Messages). Answered with
    /// [`Reply::Messages`].
    #[serde(rename = "ticket.messages")]
    TicketMessages(KeyParams),
    /// The ticket's answered `ask human` questions, for the Messages section. Answered with
    /// [`Reply::HumanAnswers`].
    #[serde(rename = "ticket.human_answers")]
    TicketHumanAnswers(KeyParams),
    /// **Open PR** / **Push to PR #N** (REV-10, REV-14): runs the Open PR or Push skill in the ticket's session.
    /// Answered with [`Reply::PrOutcome`].
    #[serde(rename = "ticket.open_pr")]
    TicketOpenPr(OpenPrParams),
    /// The dialog's **Push & open PR** / **Push** with the user's edits. Answered with [`Reply::PrOutcome`].
    #[serde(rename = "ticket.pr_confirm")]
    TicketPrConfirm(PrConfirmParams),
    /// The dialog's **Cancel**: the proposal is discarded. Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.pr_cancel")]
    TicketPrCancel(KeyParams),
    /// The dialog's **Retry** after the skill reported a problem. Answered with [`Reply::PrOutcome`].
    #[serde(rename = "ticket.pr_retry")]
    TicketPrRetry(OpenPrParams),
    /// What the Open PR / Push to PR dialog shows. Answered with [`Reply::PrFlow`].
    #[serde(rename = "ticket.pr_flow")]
    TicketPrFlow(KeyParams),
    /// The live branch-name check (valid ref, not taken locally or on origin). Answered with
    /// [`Reply::BranchCheck`].
    #[serde(rename = "ticket.pr_branch_check")]
    TicketPrBranchCheck(BranchRenameParams),
    /// **Mark merged** (REV-13): Done · Merged, cleanup, base fetch and waiting tickets started. Answered with
    /// [`Reply::MarkedMerged`].
    #[serde(rename = "ticket.mark_merged")]
    TicketMarkMerged(KeyParams),
    /// **Mark closed** (REV-13, CLS-02): needs `confirm`; closes like Close ticket. Answered with
    /// [`Reply::Ticket`].
    #[serde(rename = "ticket.mark_closed")]
    TicketMarkClosed(CloseParams),
    /// The ticket's events, newest first (History tab). Answered with [`Reply::History`].
    #[serde(rename = "ticket.history")]
    TicketHistory(KeyParams),
    /// **Close ticket** (CLS-01): Done · Closed · not merged from any unfinished state. With a running agent and
    /// no `confirm`, refused with `needs_confirmation` ("it will be stopped first"). Answered with
    /// [`Reply::Ticket`].
    #[serde(rename = "ticket.close")]
    TicketClose(CloseParams),
    /// **Reopen** (CLS-03): Done · Closed → Backlog; Merged tickets are refused. Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.reopen")]
    TicketReopen(KeyParams),
    /// **Submit for review** by the user (CLS-05): when the agent isn't mid-turn; refused with uncommitted files.
    /// Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.submit_by_user")]
    TicketSubmitByUser(SubmitByUserParams),
    /// **Park** (CLS-06): Dev → Backlog keeping the branch, files and conversation; without `confirm`, refused
    /// with `needs_confirmation` and the confirmation's text. Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.park")]
    TicketPark(CloseParams),
    /// **Resume** an Interrupted ticket (REC-01): the tool resumes its conversation in a new session in the same worktree,
    /// through the start sequence (Queued while the agent limit is full). Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.resume")]
    TicketResume(KeyParams),
    /// **Retry** after Resume failed: the same resume again (REC-03). Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.retry_resume")]
    TicketRetryResume(KeyParams),
    /// **Start from scratch** after Resume failed (REC-03): a new conversation in the same worktree. Without
    /// `confirm`, refused with `needs_confirmation` and the confirmation's text. Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.start_from_scratch")]
    TicketStartFromScratch(CloseParams),
    /// **Resume all** (REC-01, d0045): every Interrupted ticket, of `project` or of every project. Answered with
    /// [`Reply::Resumed`].
    #[serde(rename = "tickets.resume_all")]
    TicketsResumeAll(ResumeAllParams),

    // The inbox (ATT-06…ATT-10).
    /// **Answer** an Asks you / Idle item (ATT-08): typed into the session as the user's click; with unsent text
    /// and no `clear`, answered `needs_confirmation`. An Interrupted ticket that waited for an answer is resumed
    /// with it in one step. Answered with [`Reply::Delivery`].
    #[serde(rename = "inbox.answer")]
    InboxAnswer(InboxAnswerParams),
    /// Every needs-you item across all projects in inbox order, with the counts (ATT-06, ATT-10). Answered with
    /// [`Reply::Inbox`].
    #[serde(rename = "inbox.list")]
    InboxList,
    /// The inbox count in total and per project (ATT-10). Answered with [`Reply::InboxCounts`].
    #[serde(rename = "inbox.counts")]
    InboxCounts,

    // Cleanup (REC-04).
    /// The leftovers no ticket owns, grouped; changes nothing. Answered with [`Reply::CleanupScan`].
    #[serde(rename = "cleanup.scan")]
    CleanupScan,
    /// Deletes exactly the listed items, each still a leftover; without `confirm`, refused with
    /// `needs_confirmation`. Answered with [`Reply::CleanupOutcome`]. Nothing deletes leftovers otherwise.
    #[serde(rename = "cleanup.delete")]
    CleanupDelete(CleanupDeleteParams),

    // Warm builds (WRM-05…WRM-07, d0082).
    /// Every warm build in `<app-data>/warm/`, and whether the data folder's disk can clone. Answered with
    /// [`Reply::WarmList`].
    #[serde(rename = "warm.list")]
    WarmList,
    /// Clears a project's warm build (or, with `repo`, a folder no project owns). Answers at once: cleared, or
    /// waiting for the copies running from it (it clears when they end and publishes [`Event::WarmBuildChanged`]).
    /// Never turns the switch off. Answered with [`Reply::WarmClear`].
    #[serde(rename = "warm.clear")]
    WarmClear(WarmClearParams),

    // Review (REV-01…REV-03).
    /// The ticket's changed files since its `base_sha` (epics: also by child). Answered with
    /// [`Reply::DiffSummary`].
    #[serde(rename = "diff.files")]
    DiffFiles(KeyParams),
    /// One file's hunks; answered with [`Reply::FileDiff`].
    #[serde(rename = "diff.file")]
    DiffFile(DiffFileParams),
    /// A line comment in the ticket's current round; answered with [`Reply::ReviewComment`].
    #[serde(rename = "review.comment_add")]
    ReviewCommentAdd(NewComment),
    /// A reply in the thread of comment `id` (same file, line and side); answered with
    /// [`Reply::ReviewComment`].
    #[serde(rename = "review.comment_reply")]
    ReviewCommentReply(CommentTextParams),
    /// Changes a pending comment; answered with [`Reply::ReviewComment`].
    #[serde(rename = "review.comment_edit")]
    ReviewCommentEdit(CommentTextParams),
    /// Deletes a pending comment; answered with [`Reply::Ok`].
    #[serde(rename = "review.comment_delete")]
    ReviewCommentDelete(CommentIdParams),
    /// The ticket's comments (every round, or one) with the pending counter; answered with
    /// [`Reply::ReviewComments`].
    #[serde(rename = "review.comments")]
    ReviewComments(CommentsParams),
    /// Send back (REV-04): the pending comments and the note go to the agent, and the ticket returns to Dev.
    /// Answered with [`Reply::Returned`]: done, or `needs_confirmation` when the session's input line has
    /// unsent text (resend with `clear_first` for "Clear it and send").
    #[serde(rename = "ticket.send_back")]
    TicketSendBack(SendBackParams),
    /// The latest Send back's comments and note (`otaq review feedback`, REV-05). Answered with
    /// [`Reply::ReviewFeedback`].
    #[serde(rename = "review.feedback")]
    ReviewFeedback(KeyParams),
    /// What the Local merge confirmation shows: the target branch and the proposed squash message. Answered with
    /// [`Reply::MergeProposal`].
    #[serde(rename = "ticket.merge_proposal")]
    TicketMergeProposal(KeyParams),
    /// **Local merge** with the confirmed squash message (GIT-03). Answered with [`Reply::LocalMerge`]: merged,
    /// refused (uncommitted files, diverged base, dirty checkout) or a conflict.
    #[serde(rename = "ticket.local_merge")]
    TicketLocalMerge(LocalMergeParams),
    /// The check before any approve (REV-06): new commits since the review? Answered with
    /// [`Reply::ApproveCheck`].
    #[serde(rename = "ticket.approve_check")]
    TicketApproveCheck(KeyParams),
    /// The ticket's "conflicts with `main`" with its files, if any. Answered with [`Reply::Conflict`].
    #[serde(rename = "ticket.conflict")]
    TicketConflict(KeyParams),
    /// **Send to agent to fix** (GIT-03): a ticket whose Local merge conflicted returns to Dev with the restack
    /// skill. Answered with [`Reply::Returned`], like Send back (`clear_first` for "Clear it and send").
    #[serde(rename = "ticket.send_to_fix")]
    TicketSendToFix(SendToFixParams),

    // Epics.
    #[serde(rename = "epic.move_to")]
    EpicMoveTo(MoveToEpicParams),
    /// Answered with [`Reply::ClosePlan`].
    #[serde(rename = "epic.close_plan")]
    EpicClosePlan(KeyParams),
    /// Answered with [`Reply::ClosePlan`] (what was closed). Without `confirm`, refused (`needs_confirmation`)
    /// when agents would be stopped.
    #[serde(rename = "epic.close")]
    EpicClose(CloseParams),
    /// **Queue an epic** (EPC-12): every child in Backlog goes to Ready for Dev (parked ones resume), or with
    /// `child` only that one row. Answered with [`Reply::EpicQueued`], what the queue toast reads (EPC-15).
    #[serde(rename = "epic.queue")]
    EpicQueue(EpicQueueParams),
    /// **Take back an epic** (EPC-14): its track dropped on Backlog. Waiting children are unqueued; when nothing
    /// waits, the running ones are parked, which without `confirm` is refused (`needs_confirmation`). Answered with
    /// [`Reply::EpicTakenBack`], the plan carried out.
    #[serde(rename = "epic.take_back")]
    EpicTakeBack(CloseParams),
    /// The queue toast's **Undo** (EPC-15) with the outcome's keys: takes back the ones still waiting, never parks.
    /// Answered with [`Reply::EpicQueueUndone`] ("SHOP-45.1 already started").
    #[serde(rename = "epic.undo_queue")]
    EpicUndoQueue(EpicUndoQueueParams),

    // Dependencies.
    #[serde(rename = "dep.add")]
    DepAdd(DepParams),
    #[serde(rename = "dep.remove")]
    DepRemove(DepParams),
    /// Answered with [`Reply::DepTree`].
    #[serde(rename = "dep.tree")]
    DepTree(KeyParams),
    /// Answered with [`Reply::Cycles`].
    #[serde(rename = "dep.cycles")]
    DepCycles(ProjectParams),
    /// Each unfinished ticket's dependency state (`otaq ready` / `blocked`); answered with
    /// [`Reply::Readiness`].
    #[serde(rename = "dep.readiness")]
    DepReadiness(ProjectParams),
    /// The agent limits as the scheduler sees them now (the header's "3 / 5 agents", Settings' "Right now");
    /// answered with [`Reply::SchedulerStatus`].
    #[serde(rename = "scheduler.status")]
    SchedulerStatus,
    /// Today's tokens across all projects and, with a project, its tickets' tokens (USG-01).
    #[serde(rename = "usage.summary")]
    UsageSummary(UsageParams),
    /// The tokens of each session of a ticket (the History tab, USG-01).
    #[serde(rename = "usage.sessions")]
    UsageSessions(KeyParams),
    /// Activity's data per agent tool (SHL-03, USG-09): agents running, today's tokens, limit windows; answered
    /// with [`Reply::Activity`].
    #[serde(rename = "usage.activity")]
    UsageActivity,
    /// A tool's usage windows (USG-04, USG-09; Claude Code's from `otaq statusline`): they replace the tool's set.
    #[serde(rename = "usage.record_limits")]
    UsageRecordLimits(RecordLimitsParams),
    /// Resume all after the usage limit (USG-06): "continue" for every agent still waiting for the reset; clears
    /// the banner. Replies with the tickets that got it.
    #[serde(rename = "usage.resume_all")]
    UsageResumeAll,
    /// "This is the usage limit" on an Idle inbox item (USG-07): every Idle agent waits for the limit reset.
    /// Replies with the tickets it folded into the banner.
    #[serde(rename = "usage.mark_limit")]
    UsageMarkLimit,

    // Model lists (AGT-02).
    /// Otaq's copy of a tool's model list, answered at once with [`Reply::Models`] (none before the first read).
    #[serde(rename = "agents.models")]
    AgentsModels(ModelsParams),
    /// Reads a tool's model list again (Settings → Agents opening); answered with [`Reply::Models`] after the
    /// read: the new copy, or the old one when the read failed.
    #[serde(rename = "agents.models_refresh")]
    AgentsModelsRefresh(ModelsParams),

    // Settings (SET-01).
    #[serde(rename = "settings.get")]
    SettingsGet,
    #[serde(rename = "settings.set")]
    SettingsSet(GlobalSettings),
    #[serde(rename = "project_settings.get")]
    ProjectSettingsGet(ProjectParams),
    #[serde(rename = "project_settings.set")]
    ProjectSettingsSet(ProjectSettingsParams),

    // Sessions (SES-04, SVC-05).
    /// With `key`, every session the ticket ever had (newest first); without, every session in the service's
    /// memory. Answered with [`Reply::Sessions`].
    #[serde(rename = "session.list")]
    SessionList(SessionListParams),
    /// The session's stored output, then its live output as `session_output` events on this connection.
    /// Attaching again on the same connection keeps the live output already flowing, so its events may come
    /// before the reply or start before the reply's `end`: skip by offset. Answered with [`Reply::SessionAttached`].
    #[serde(rename = "session.attach")]
    SessionAttach(SessionTarget),
    /// Stops the live output started by `session.attach`; the session keeps running. Answered with
    /// [`Reply::Ok`].
    #[serde(rename = "session.detach")]
    SessionDetach(SessionIdParams),
    /// Answered with [`Reply::Ok`].
    #[serde(rename = "session.resize")]
    SessionResize(ResizeParams),
    /// Keystrokes typed by a person, relayed to the session (SVC-06). Answered with [`Reply::Ok`].
    #[serde(rename = "session.input")]
    SessionInput(InputParams),
    /// **Deliver now** (SES-06): types the session's waiting messages. With unsent text in its input line the
    /// answer is `needs_confirmation`; `clear_first` is the user's "Clear it and send" (SES-07). Answered with
    /// [`Reply::Delivery`].
    #[serde(rename = "session.deliver_now")]
    SessionDeliverNow(DeliverNowParams),
    /// A client's terminal for the session gained or lost keyboard focus (SES-10). Answered with
    /// [`Reply::Ok`].
    #[serde(rename = "session.focus")]
    SessionFocus(FocusParams),
    /// "Before the restart" (REC-02): the last messages and actions of the ticket's latest conversation, read
    /// from Claude Code's saved conversation. Answered with [`Reply::BeforeRestart`].
    #[serde(rename = "session.before_restart")]
    SessionBeforeRestart(BeforeRestartParams),

    // Agent signals (SDD §11.4).
    /// An agent tool's hook in the caller's agent session (`otaq hook --tool <tool> <event>`, PLG-04, ADP-03).
    /// Answered with [`Reply::Ok`]; from outside an agent session it changes nothing.
    #[serde(rename = "hook")]
    Hook(HookParams),

    // Run (RUN-01, RUN-04): the project's run command in a ticket's worktree.
    /// Starts the ticket's Run; refused without a run command (`no_run_command`) or a worktree (`no_worktree`).
    #[serde(rename = "run.start")]
    RunStart(KeyParams),
    #[serde(rename = "run.stop")]
    RunStop(KeyParams),
    /// The ticket's Run: state, port, command, Open URL, session.
    #[serde(rename = "run.status")]
    RunStatus(KeyParams),
    /// Every Run that is up, across projects (the header's "▶ 3 processes").
    #[serde(rename = "run.list")]
    RunList,
    /// Ask Claude for the project's run command (RUN-02): answers at once; progress and the outcome arrive as
    /// `run_suggestion` events. Only on the user's click.
    #[serde(rename = "run.suggest")]
    RunSuggest(ProjectParams),
    /// Stops a running Ask Claude.
    #[serde(rename = "run.suggest_cancel")]
    RunSuggestCancel(ProjectParams),
    /// The ticket's Shell (RUN-05): the running or last one, or a new one on first use; `reopen` starts a new one.
    /// Answered with the session; attach to it with `session.attach {key, kind: "shell"}`.
    #[serde(rename = "shell.open")]
    ShellOpen(ShellOpenParams),
    /// A Project terminal of the project (RUN-06, d0068): the newest running one (else the newest ended one still
    /// open, else a new one); `new` always starts a new one, "Terminal 1", "Terminal 2"…. Answered with
    /// [`Reply::ProjectTerminal`]; attach with `session.attach {session}`.
    #[serde(rename = "project_terminal.open")]
    ProjectTerminalOpen(ProjectTerminalParams),
    /// The project's open Project terminals, by number ([`Reply::ProjectTerminals`]).
    #[serde(rename = "project_terminal.list")]
    ProjectTerminalList(ProjectParams),
    /// Closes a Project terminal: its shell ends and it leaves the list.
    #[serde(rename = "project_terminal.close")]
    ProjectTerminalClose(SessionIdParams),
    /// The tickets created from the project's Project terminal since its summary was last dismissed (T-0167).
    #[serde(rename = "project_terminal.created")]
    ProjectTerminalCreated(ProjectParams),
    /// Closes the summary: it lists only tickets created from now on.
    #[serde(rename = "project_terminal.dismiss")]
    ProjectTerminalDismiss(ProjectParams),

    // Project agents (PAG-01, PAG-04, PAG-06; d0068).
    /// Opens a new Project agent in the project with the user's first message (sending it is the go-ahead, d0087):
    /// the skill and the message are its first prompt. Refused without words, and while the usage limit holds.
    /// Answered with [`Reply::ProjectAgent`]; attach to its `session`.
    #[serde(rename = "project_agent.open")]
    ProjectAgentOpen(ProjectAgentOpenParams),
    /// The project's open Project agents and its 10 most recent ended ones ([`Reply::ProjectAgents`]).
    #[serde(rename = "project_agent.list")]
    ProjectAgentList(ProjectParams),
    /// Closes a Project agent ("End agent"): it ends and stays in Recently ended; a running one mid-turn needs
    /// `confirm`. Answered with [`Reply::Ok`].
    #[serde(rename = "project_agent.close")]
    ProjectAgentClose(ProjectAgentCloseParams),
    /// **Close ended agents** (d0087): closes every ended, not yet closed Project agent of the project (not epic
    /// leads). Answered with the project's [`Reply::ProjectAgents`].
    #[serde(rename = "project_agent.close_ended")]
    ProjectAgentCloseEnded(ProjectParams),
    /// Resumes an ended Project agent's conversation (Ended by restart · Resume, Recently ended). Answered with
    /// [`Reply::ProjectAgent`].
    #[serde(rename = "project_agent.resume")]
    ProjectAgentResume(IdParams),
    /// The user's rename; the agent never overwrites it. Answered with [`Reply::ProjectAgent`].
    #[serde(rename = "project_agent.rename")]
    ProjectAgentRename(ProjectAgentRenameParams),
    /// `otaq agent name "<name>"` from a Project agent session (PAG-04): names it once; again only with
    /// `rename`. Answered with [`Reply::ProjectAgent`].
    #[serde(rename = "agent.name")]
    AgentName(AgentNameParams),
    /// `otaq ask <KEY> --stdin` from a Project agent (PAG-02): to any ticket, §16 rules; the answer is typed
    /// into its session. Answered with [`Reply::Message`].
    #[serde(rename = "agent.ask")]
    AgentAsk(AgentAskParams),
    /// `otaq ask human --stdin` from a Project agent: the question waits on it for the user's answer.
    /// Answered with [`Reply::ProjectAgent`].
    #[serde(rename = "agent.ask_human")]
    AgentAskHuman(AgentAskHumanParams),
    /// The user's answer to a Project agent's question, typed into its session (with unsent text and no `clear`:
    /// `needs_confirmation`). Answered with [`Reply::Delivery`].
    #[serde(rename = "project_agent.answer")]
    ProjectAgentAnswer(ProjectAgentAnswerParams),
    /// `otaq prime --project` (PAG-03). Answered with [`Reply::ProjectPrime`].
    #[serde(rename = "project_agent.prime")]
    ProjectAgentPrime(ProjectParams),
    /// A Project agent's "Created and changed" (PAG-05). Answered with [`Reply::CreatedAndChanged`].
    #[serde(rename = "project_agent.created")]
    ProjectAgentCreated(IdParams),

    // Epic leads (LEAD-02, LEAD-12; d0083).
    /// **Open lead**: starts the epic's lead with no first message, or resumes an ended one (the user's click).
    /// Refused while it is live, while the epic is in review, and during the usage limit. Answered with
    /// [`Reply::Lead`]; attach to its agent's `session`.
    #[serde(rename = "lead.open")]
    LeadOpen(LeadParams),
    /// Resumes the epic's lead's conversation (Ended by restart · Resume). Answered with [`Reply::Lead`].
    #[serde(rename = "lead.resume")]
    LeadResume(LeadParams),
    /// `otaq suggest close|delete <KEY> --stdin` (LEAD-08): an epic lead suggests closing or deleting a never-started
    /// child of its epic; an inbox item the user confirms or dismisses. Lead only. Answered with
    /// [`Reply::Suggestion`].
    #[serde(rename = "lead.suggest")]
    LeadSuggest(SuggestParams),
    /// The user's Close… / Delete… on a lead's suggestion: runs the usual close (d0044) or delete (d0060) of the
    /// child and marks the suggestion confirmed. Answered with [`Reply::Suggestion`].
    #[serde(rename = "suggestion.confirm")]
    SuggestionConfirm(IdParams),
    /// The user's Dismiss on a lead's suggestion; it doesn't wake the lead. Answered with [`Reply::Suggestion`].
    #[serde(rename = "suggestion.dismiss")]
    SuggestionDismiss(IdParams),
    /// Every epic lead of the project, with its status (the board's Lead Status, T-0289). Answered with
    /// [`Reply::Leads`].
    #[serde(rename = "lead.list")]
    LeadList(ProjectParams),
    /// `otaq ask human --pass-on <QUESTION-ID> [--option …] [--recommended N] --stdin` (LEAD-06): the epic's lead
    /// passes a child's question that waits for it on to the user, with its recommendation and reason. Lead only.
    /// Answered with [`Reply::LeadQuestion`].
    #[serde(rename = "lead.pass_on")]
    LeadPassOn(PassOnParams),
    /// `otaq questions` from an epic's lead (LEAD-06): its children's questions waiting for it. Lead only. Answered
    /// with [`Reply::LeadQuestions`].
    #[serde(rename = "lead.questions")]
    LeadQuestions,
    /// `otaq prime --epic` (LEAD-03): the epic lead's context, for the lead (`$OTAQ_EPIC`) or the user's terminal.
    /// Answered with [`Reply::EpicPrime`].
    #[serde(rename = "lead.prime")]
    LeadPrime(KeyParams),
    /// `otaq note <KEY> --stdin` (LEAD-10): an epic lead leaves an unread note for a running child of its epic. Lead
    /// only. Answered with [`Reply::LeadNote`].
    #[serde(rename = "lead.note")]
    LeadNote(LeadNoteParams),
    /// `otaq notes` (LEAD-10): the ticket's unread lead notes, marked read by this request. Answered with
    /// [`Reply::LeadNotes`].
    #[serde(rename = "ticket.notes")]
    TicketNotes(KeyParams),
    /// The epic lead's log on a ticket (T-0290): its children's questions to the lead and the lead's notes, for
    /// the Messages section of the epic and the child. Answered with [`Reply::LeadLog`].
    #[serde(rename = "ticket.lead_log")]
    TicketLeadLog(KeyParams),
    /// `otaq show <KEY> --terminal` (SHL-10, CLI-09, d0087): the ticket's terminal as the user sees it, as plain
    /// text; read-only, for the user, a Project agent of its project and the lead of its epic. Answered with
    /// [`Reply::TicketTerminal`].
    #[serde(rename = "ticket.terminal")]
    TicketTerminal(TerminalParams),

    // Named agents (AGT-01…AGT-04; d0086, d0088). The user's requests; agents set rows through T-0338's CLI.
    /// Every named agent (with its mark) and the app-wide role defaults. Answered with [`Reply::Agents`].
    #[serde(rename = "agents.list")]
    AgentsList,
    /// `{"name": "Strong", "tool": "claude", "model": "opus", "use_for": "Complex features"}`: adds an entry; the
    /// model must be in the tool's list (or empty). Answered with [`Reply::Agent`].
    #[serde(rename = "agents.add")]
    AgentsAdd(crate::named_agents::AgentFields),
    /// `{"id": "0199…", "name": …, "tool": …, "model": …, "use_for": …}`: the whole form; a model change clears the
    /// mark. Answered with [`Reply::Agent`].
    #[serde(rename = "agents.edit")]
    AgentsEdit(AgentEditParams),
    /// `{"id": "0199…"}`: Duplicate's draft, "<name> copy", not saved. Answered with [`Reply::AgentDraft`].
    #[serde(rename = "agents.duplicate")]
    AgentsDuplicate(IdParams),
    /// `{"id": "0199…"}`: deletes an unused entry. Refused (`agent_in_use`, the parts in the error's `in_use`)
    /// while in use, and for the last entry (`last_agent`).
    #[serde(rename = "agents.delete")]
    AgentsDelete(IdParams),
    /// `{"id": "0199…"}`: what uses an entry, before Delete asks (T-0340): the refused dialog or the plain
    /// confirmation. Refused for the last entry (`last_agent`), as Delete is. Answered with [`Reply::AgentInUse`].
    #[serde(rename = "agents.in_use")]
    AgentsInUse(IdParams),
    /// `{"role": "validator", "agent": "0199…"}`: an app-wide role default. Answered with
    /// [`Reply::AgentDefaults`].
    #[serde(rename = "agents.set_default")]
    AgentsSetDefault(AgentDefaultParams),
    /// `{"project": "SHOP", "role": "developer", "agent": "0199…"}`; `agent` null = "Use app default". Answered with
    /// [`Reply::AgentDefaults`] (the project's overrides).
    #[serde(rename = "project.set_agent_default")]
    ProjectSetAgentDefault(ProjectAgentDefaultParams),
    /// `{"project": "SHOP"}`: the project's role default overrides (T-0340: the project's Agents block). Answered
    /// with [`Reply::AgentDefaults`].
    #[serde(rename = "project.agent_defaults")]
    ProjectAgentDefaults(ProjectParams),
    /// `{"key": "SHOP-19", "role": "developer", "agent": "0199…"}`: the user sets a row (*set by you*); `agent`
    /// null = "Let agents pick again". Refused on a locked row and on a row the ticket doesn't have.
    #[serde(rename = "ticket.agent_row")]
    TicketAgentRow(AgentRowParams),
    /// `{"key": "SHOP-19"}`, or `{"project": "SHOP", "new_epic": true}` for the ⌘N form: the ticket's or epic's rows with what they resolve to, their locks, the sessions with
    /// their named agents, an epic's children's rows. Answered with [`Reply::TicketAgents`].
    #[serde(rename = "ticket.agents")]
    TicketAgents(TicketAgentsParams),
}

/// `agents.edit`: the entry's id and its whole form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEditParams {
    pub id: String,
    #[serde(flatten)]
    pub fields: crate::named_agents::AgentFields,
}

/// `agents.set_default`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefaultParams {
    pub role: crate::named_agents::Role,
    pub agent: String,
}

/// `project.set_agent_default`: `agent` `None` = "Use app default".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentDefaultParams {
    pub project: String,
    pub role: crate::named_agents::Role,
    #[serde(default)]
    pub agent: Option<String>,
}

/// `ticket.agents`: an existing ticket by `key`, else a new one in `project` (an epic when `new_epic`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketAgentsParams {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub new_epic: bool,
}

/// `ticket.agent_row`: `agent` `None` empties the row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRowParams {
    pub key: String,
    pub role: crate::named_agents::Role,
    #[serde(default)]
    pub agent: Option<String>,
}

/// `{"id": "<question id>", "options": ["Logged-in users only", "Guests too"], "recommended": 1, "reason": "PAY-123
/// only mentions signed-in users."}`; without `options` the child's stay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassOnParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<u32>,
    pub reason: String,
}

/// `{"key": "SHOP-40.4", "text": "40.1 returns items newest first …"}`; to a stuck child also `"view": "The agent
/// keeps changing the API instead of the test"` (`--view`, LEAD-07), shown only if the child reaches the user later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadNoteParams {
    pub key: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
}

/// `{"key": "SHOP-40.6", "kind": "close", "reason": "PAY-123 doesn't ask for an export."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestParams {
    pub key: String,
    pub kind: crate::epic_lead::suggest::SuggestionKind,
    pub reason: String,
}

/// `{"id": "SHOP-40.1#1", "source": "PAY-123", "text": "items are listed in the order they were added"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionMarkParams {
    pub id: String,
    pub source: String,
    pub text: String,
}

/// `{"epic": "SHOP-40"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadParams {
    pub epic: String,
}

/// `{"target": "SHOP-18", "question": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAskParams {
    pub target: String,
    pub question: String,
}

/// `{"question": "Import closed issues too?", "options": ["Yes", "No"], "recommended": 2}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAskHumanParams {
    pub question: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<u32>,
}

/// `{"id": "…", "choice": {"option": 1}}`; `clear` is the user's "Clear it and send".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentAnswerParams {
    pub id: String,
    pub choice: crate::board::questions::Choice,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
}

/// `{"project": "SHOP", "message": "Why is checkout slow?"}`: open a Project agent with its first message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentOpenParams {
    pub project: String,
    pub message: String,
}

/// `{"id": "…", "confirm": true}`: close a Project agent, stopping it mid-turn when confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentCloseParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub confirm: bool,
}

/// `{"id": "…", "name": "Checkout slowness"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAgentRenameParams {
    pub id: String,
    pub name: String,
}

/// `{"name": "Checkout slowness"}`; `rename` when the user asked the agent for a new name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentNameParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rename: bool,
}

/// `otaq hook --tool claude Stop` (ADP-03): the tool, its event name as the tool spells it, and the JSON it passed
/// on stdin. Only the tool's adapter reads the event and the payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookParams {
    pub tool: Tool,
    pub event: String,
    #[serde(default)]
    pub payload: Value,
    /// The session's role (`validator`, `project_agent`) from its environment; the agent's sessions have none
    /// (d0063, d0068). The service still tells sessions apart by their processes (CLI-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// `agents.models` and `agents.models_refresh`: whose list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelsParams {
    pub tool: Tool,
}

/// `usage.record_limits`: a tool's current usage windows, as its adapter read them from its report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordLimitsParams {
    pub tool: Tool,
    pub windows: Vec<crate::agent_tools::LimitWindow>,
}

/// `{"key": "SHOP-23", "question": "…", "options": ["Postmark", "SES"], "recommended": 1}` (d0061).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskHumanParams {
    pub key: String,
    pub question: String,
    /// 2–4 answer options, or none for a free-text question.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// The recommended option, 1-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<u32>,
}

/// `{"key": "SHOP-23", "question": "…", "built": "…", "other_options": ["…"], "why": "…"}` (DEC-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionRecordParams {
    pub key: String,
    #[serde(flatten)]
    pub decision: crate::decisions::NewDecision,
}

/// Change… (DEC-06): `{"id": "SHOP-23#1", "text": "Split fullName everywhere"}` sends SHOP-23 back (`clear_first`
/// is the user's "Clear it and send"); `{"id": "SHOP-40.1#1", "title": "Change: …", "description": "…"}` adds a fix
/// child to the epic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionChangeParams {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_first: bool,
}

/// `{"key": "SHOP-18", "path": "lib/pricing.ts"}`; `child` (epics) limits it to that child's squash commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffFileParams {
    pub key: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
    /// Only what changed after this commit (the reviewed head: "changed since you reviewed it", REV-06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// `{"key": "SHOP-18"}`; `clear_first` is the user's "Clear it and send".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendToFixParams {
    pub key: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_first: bool,
}

/// `{"key": "SHOP-22", "note": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitByUserParams {
    pub key: String,
    #[serde(default)]
    pub note: String,
}

/// `{"key": "SHOP-25"}`; `limit` caps the entries (default 40).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeforeRestartParams {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// `{"key": "SHOP-23", "choice": {"option": 1}}` or `{"key": "SHOP-23", "choice": {"other": "Mailgun"}}`;
/// `clear` is the user's "Clear it and send".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxAnswerParams {
    pub key: String,
    pub choice: crate::board::questions::Choice,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
}

/// `{"items": [{"group": "branches", "id": "SHOP:otaq/SHOP-9"}], "confirm": true}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupDeleteParams {
    pub items: Vec<crate::recovery::cleanup::CleanupTarget>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub confirm: bool,
}

/// `{"project": "SHOP"}`, or `{"repo": "old-app"}` for a warm build no project owns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmClearParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
}

/// `{}` for every project, `{"project": "SHOP"}` for one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeAllParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// `{"key": "SHOP-19", "confirm": true}`: `confirm` is the user's yes to stopping running agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseParams {
    pub key: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub confirm: bool,
}

/// `{"key": "SHOP-18", "message": "SHOP-18: Checkout API\n\n…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMergeParams {
    pub key: String,
    pub message: String,
    /// The user saw "changed since you reviewed it" and chose Approve anyway (REV-06).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub approve_anyway: bool,
}

/// `{"key": "SHOP-18", "note": "…"}`; `clear_first` is the user's "Clear it and send".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendBackParams {
    pub key: String,
    #[serde(default)]
    pub note: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear_first: bool,
}

/// `{"id": "…", "body": "…"}`: a reply to, or the new text of, a comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentTextParams {
    pub id: String,
    pub body: String,
}

/// `{"id": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentIdParams {
    pub id: String,
}

/// `{"key": "SHOP-18"}`, or `{"key": "SHOP-18", "round": 2}` for one round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentsParams {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round: Option<u32>,
}

/// `{"key": "SHOP-18", "summary": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSubmitParams {
    pub key: String,
    #[serde(default)]
    pub summary: String,
}

/// `{"key": "SHOP-32", "action": "answer_agent", "note": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationActionParams {
    pub key: String,
    pub action: crate::domain::ValidationAction,
    /// Answer the agent's note; Validate and submit's optional note.
    #[serde(default)]
    pub note: String,
}

/// `{"key": "SHOP-29", "note": "Not covered by any ticket: refunds endpoint"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteParams {
    pub key: String,
    pub note: String,
}

/// `{"key": "SHOP-29", "pass": false, "report": "…"}`: a validator's verdict (VAL-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictParams {
    pub key: String,
    pub pass: bool,
    /// The findings (one line each, file and line) and the notes.
    #[serde(default)]
    pub report: String,
}

/// `{"key": "SHOP-18", "proposal": "{…}"}`: the proposal as the skill wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrProposeParams {
    pub key: String,
    pub proposal: String,
}

/// `{"key": "SHOP-18", "url": "https://github.com/acme/shop-web/pull/481"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrOpenedParams {
    pub key: String,
    pub url: String,
}

/// `{"key": "SHOP-20", "target": "SHOP-18", "question": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskTicketParams {
    pub key: String,
    pub target: String,
    pub question: String,
}

/// `{"id": "0199…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdParams {
    pub id: String,
}

/// `{"id": "0199…", "answer": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnswerParams {
    pub id: String,
    pub answer: String,
}

/// `{"key": "SHOP-18", "approve_anyway": false, "clear": false}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenPrParams {
    pub key: String,
    /// The user chose Approve anyway after "changed since you reviewed it" (REV-06).
    #[serde(default)]
    pub approve_anyway: bool,
    /// "Clear it and send" after a needs-confirmation answer (d0048).
    #[serde(default)]
    pub clear: bool,
}

/// The confirmed proposal (every field as the user left it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrConfirmParams {
    pub key: String,
    pub proposal: crate::pr::proposal::Proposal,
    #[serde(default)]
    pub clear: bool,
}

/// `{"key": "SHOP-18", "name": "bugfix/SHOP-18-login-redirect"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchRenameParams {
    pub key: String,
    pub name: String,
}

/// A batch of new tickets for one project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchParams {
    pub project: String,
    pub tickets: Vec<BatchTicket>,
}

/// `{"path": "/Users/me/code/shop-web"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathParams {
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageParams {
    /// The project whose tickets' tokens to include (its key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// `{"key": "SHOP-18"}` (a ticket) or `{"key": "SHOP"}` (a project).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyParams {
    pub key: String,
}

/// `{"key": "SHOP-22"}`: the ticket's agent session; `{"key": "SHOP-29", "validator": true}`: its running
/// validator's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalParams {
    pub key: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub validator: bool,
}

/// The answer to `ticket.terminal`: the screen and the lines above it as plain text
/// ([`crate::session::screen::render`]), and the session it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketTerminal {
    pub key: TicketKey,
    pub session: String,
    pub text: String,
}

/// `{"project": "SHOP"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectParams {
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectUpdateParams {
    /// The project's key. A `key` among the changes is the (refused) attempt to change it.
    pub project: String,
    #[serde(flatten)]
    pub update: ProjectUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketUpdateParams {
    pub key: String,
    #[serde(flatten)]
    pub update: TicketUpdate,
}

/// A drop or "Start". `confirmed` answers a confirmation, a choice or a warning ("Start anyway").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveParams {
    pub key: String,
    pub target: DropTarget,
    #[serde(default)]
    pub confirmed: bool,
}

/// An epic dropped on Ready for Dev or Dev (or ⇧⌘D on it); `child` when one row of it was dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpicQueueParams {
    /// The epic, e.g. SHOP-45.
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
}

/// The queue toast's Undo: the epic and the keys its queue moved (the outcome's `queued` and `resumed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpicUndoQueueParams {
    /// The epic, e.g. SHOP-45.
    pub key: String,
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveToEpicParams {
    pub key: String,
    pub epic: String,
}

/// `child` depends on `parent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepParams {
    pub child: String,
    pub parent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSettingsParams {
    pub project: String,
    /// Boxed: the settings are the largest request by far (clippy `large_enum_variant`); the JSON is the same.
    pub settings: Box<ProjectSettings>,
}

/// A ticket's Shell; `reopen` is the user's click after it ended.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellOpenParams {
    pub key: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reopen: bool,
}

/// A project's Project terminal; `new` is the user's click for another one (or Reopen after one ended).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTerminalParams {
    pub project: String,
    #[serde(default, alias = "reopen", skip_serializing_if = "std::ops::Not::not")]
    pub new: bool,
}

/// `{"key": "SHOP-18"}` for a ticket's sessions, `{}` for every session in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// A session by id (`{"session": "…"}`), a ticket's newest agent session by key (`{"key": "SHOP-18"}`), its newest
/// session of another kind (`{"key": "SHOP-18", "kind": "shell"}`), or a project's Project terminal
/// (`{"project": "SHOP", "kind": "project"}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The kind of the ticket's session (agent when not given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::session::SessionKind>,
    /// A project's key, for its Project terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdParams {
    pub session: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeParams {
    pub session: String,
    #[serde(flatten)]
    pub size: TerminalSize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliverNowParams {
    #[serde(flatten)]
    pub target: SessionTarget,
    /// The user chose "Clear it and send".
    #[serde(default)]
    pub clear_first: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusParams {
    pub session: String,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputParams {
    pub session: String,
    /// The bytes typed, base64 ([`encode_bytes`]).
    pub data: String,
    /// The client's terminal answering the program (cursor position, colours), not a person typing: the bytes
    /// don't touch the unsent-input flag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reply: bool,
}

/// The answer to `session.attach`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAttach {
    pub session: SessionInfo,
    /// The stored output, base64 ([`encode_bytes`]).
    pub scrollback: String,
    /// The offset of the first stored byte in the session's whole output.
    pub start: u64,
    /// The offset after the last stored byte; `session_output` events continue from here.
    pub end: u64,
}

/// Terminal bytes travel as base64 inside JSON (they need not be UTF-8).
pub fn encode_bytes(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub fn decode_bytes(text: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(text)
}

impl Request {
    /// The method name as sent on the wire.
    pub fn method(&self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::ServiceStatus => "service.status",
            Self::ServiceStop => "service.stop",
            Self::StartupReport => "service.startup_report",
            Self::Subscribe => "events.subscribe",
            Self::ToolsCheck => "tools.check",
            Self::OnboardingState => "onboarding.state",
            Self::DismissPermissionWarning => "onboarding.dismiss_permission_warning",
            Self::DismissTrustNote => "notices.dismiss_trust_note",
            Self::ProjectList => "project.list",
            Self::ProjectProbe(_) => "project.probe",
            Self::ProjectAdd(_) => "project.add",
            Self::ProjectUpdate(_) => "project.update",
            Self::ProjectRemove(_) => "project.remove",
            Self::ProjectBranches(_) => "project.branches",
            Self::ProjectPrProvider(_) => "project.pr_provider",
            Self::BoardSnapshot(_) => "board.snapshot",
            Self::TicketGet(_) => "ticket.get",
            Self::TicketList(_) => "ticket.list",
            Self::TicketCreate(_) => "ticket.create",
            Self::TicketCreateBatch(_) => "ticket.create_batch",
            Self::TicketUpdate(_) => "ticket.update",
            Self::TicketMove(_) => "ticket.move",
            Self::TicketPin(_) => "ticket.pin",
            Self::TicketUnpin(_) => "ticket.unpin",
            Self::TicketAgentEdits(_) => "ticket.agent_edits",
            Self::TicketAgentEditsSeen(_) => "ticket.agent_edits_seen",
            Self::TicketDeletePreview(_) => "ticket.delete_preview",
            Self::TicketDelete(_) => "ticket.delete",
            Self::TicketRetryStart(_) => "ticket.retry_start",
            Self::TicketHistory(_) => "ticket.history",
            Self::TicketClose(_) => "ticket.close",
            Self::TicketReopen(_) => "ticket.reopen",
            Self::TicketSubmitByUser(_) => "ticket.submit_by_user",
            Self::TicketPark(_) => "ticket.park",
            Self::TicketResume(_) => "ticket.resume",
            Self::TicketRetryResume(_) => "ticket.retry_resume",
            Self::TicketStartFromScratch(_) => "ticket.start_from_scratch",
            Self::TicketsResumeAll(_) => "tickets.resume_all",
            Self::InboxAnswer(_) => "inbox.answer",
            Self::InboxList => "inbox.list",
            Self::InboxCounts => "inbox.counts",
            Self::CleanupScan => "cleanup.scan",
            Self::CleanupDelete(_) => "cleanup.delete",
            Self::WarmList => "warm.list",
            Self::WarmClear(_) => "warm.clear",
            Self::DiffFiles(_) => "diff.files",
            Self::DiffFile(_) => "diff.file",
            Self::ReviewCommentAdd(_) => "review.comment_add",
            Self::ReviewCommentReply(_) => "review.comment_reply",
            Self::ReviewCommentEdit(_) => "review.comment_edit",
            Self::ReviewCommentDelete(_) => "review.comment_delete",
            Self::ReviewComments(_) => "review.comments",
            Self::TicketSendBack(_) => "ticket.send_back",
            Self::ReviewFeedback(_) => "review.feedback",
            Self::TicketMergeProposal(_) => "ticket.merge_proposal",
            Self::TicketLocalMerge(_) => "ticket.local_merge",
            Self::TicketApproveCheck(_) => "ticket.approve_check",
            Self::TicketConflict(_) => "ticket.conflict",
            Self::TicketSendToFix(_) => "ticket.send_to_fix",
            Self::TicketPrime(_) => "ticket.prime",
            Self::TicketAskHuman(_) => "ticket.ask_human",
            Self::DecisionRecord(_) => "decision.record",
            Self::DecisionList(_) => "decision.list",
            Self::DecisionChange(_) => "decision.change",
            Self::DecisionMark(_) => "decision.mark",
            Self::TicketReviewSubmit(_) => "ticket.review_submit",
            Self::ValidationVerdict(_) => "validation.verdict",
            Self::TicketPrimeValidate(_) => "ticket.prime_validate",
            Self::TicketValidations(_) => "ticket.validations",
            Self::TicketCreateFromNote(_) => "ticket.create_from_note",
            Self::TicketValidationAction(_) => "ticket.validation_action",
            Self::TicketPrPropose(_) => "ticket.pr_propose",
            Self::TicketPrOpened(_) => "ticket.pr_opened",
            Self::TicketBranchRename(_) => "ticket.branch_rename",
            Self::TicketAsk(_) => "ticket.ask",
            Self::MessageAnswer(_) => "message.answer",
            Self::MessageResume(_) => "message.resume",
            Self::TicketQuestions(_) => "ticket.questions",
            Self::TicketMessages(_) => "ticket.messages",
            Self::TicketHumanAnswers(_) => "ticket.human_answers",
            Self::TicketOpenPr(_) => "ticket.open_pr",
            Self::TicketPrConfirm(_) => "ticket.pr_confirm",
            Self::TicketPrCancel(_) => "ticket.pr_cancel",
            Self::TicketPrRetry(_) => "ticket.pr_retry",
            Self::TicketPrFlow(_) => "ticket.pr_flow",
            Self::TicketPrBranchCheck(_) => "ticket.pr_branch_check",
            Self::TicketMarkMerged(_) => "ticket.mark_merged",
            Self::TicketMarkClosed(_) => "ticket.mark_closed",
            Self::EpicMoveTo(_) => "epic.move_to",
            Self::EpicClosePlan(_) => "epic.close_plan",
            Self::EpicClose(_) => "epic.close",
            Self::EpicQueue(_) => "epic.queue",
            Self::EpicTakeBack(_) => "epic.take_back",
            Self::EpicUndoQueue(_) => "epic.undo_queue",
            Self::DepAdd(_) => "dep.add",
            Self::DepRemove(_) => "dep.remove",
            Self::DepTree(_) => "dep.tree",
            Self::DepCycles(_) => "dep.cycles",
            Self::DepReadiness(_) => "dep.readiness",
            Self::SchedulerStatus => "scheduler.status",
            Self::UsageSummary(_) => "usage.summary",
            Self::UsageSessions(_) => "usage.sessions",
            Self::UsageActivity => "usage.activity",
            Self::UsageRecordLimits(_) => "usage.record_limits",
            Self::AgentsModels(_) => "agents.models",
            Self::AgentsModelsRefresh(_) => "agents.models_refresh",
            Self::UsageResumeAll => "usage.resume_all",
            Self::UsageMarkLimit => "usage.mark_limit",
            Self::SettingsGet => "settings.get",
            Self::SettingsSet(_) => "settings.set",
            Self::ProjectSettingsGet(_) => "project_settings.get",
            Self::ProjectSettingsSet(_) => "project_settings.set",
            Self::SessionList(_) => "session.list",
            Self::SessionAttach(_) => "session.attach",
            Self::SessionDetach(_) => "session.detach",
            Self::SessionResize(_) => "session.resize",
            Self::SessionInput(_) => "session.input",
            Self::SessionDeliverNow(_) => "session.deliver_now",
            Self::SessionFocus(_) => "session.focus",
            Self::SessionBeforeRestart(_) => "session.before_restart",
            Self::Hook(_) => "hook",
            Self::RunStart(_) => "run.start",
            Self::RunStop(_) => "run.stop",
            Self::RunStatus(_) => "run.status",
            Self::RunList => "run.list",
            Self::RunSuggest(_) => "run.suggest",
            Self::RunSuggestCancel(_) => "run.suggest_cancel",
            Self::ShellOpen(_) => "shell.open",
            Self::ProjectTerminalOpen(_) => "project_terminal.open",
            Self::ProjectTerminalList(_) => "project_terminal.list",
            Self::ProjectTerminalClose(_) => "project_terminal.close",
            Self::ProjectTerminalCreated(_) => "project_terminal.created",
            Self::ProjectTerminalDismiss(_) => "project_terminal.dismiss",
            Self::ProjectAgentOpen(_) => "project_agent.open",
            Self::ProjectAgentList(_) => "project_agent.list",
            Self::ProjectAgentClose(_) => "project_agent.close",
            Self::ProjectAgentCloseEnded(_) => "project_agent.close_ended",
            Self::ProjectAgentResume(_) => "project_agent.resume",
            Self::ProjectAgentRename(_) => "project_agent.rename",
            Self::AgentName(_) => "agent.name",
            Self::AgentAsk(_) => "agent.ask",
            Self::AgentAskHuman(_) => "agent.ask_human",
            Self::ProjectAgentAnswer(_) => "project_agent.answer",
            Self::ProjectAgentPrime(_) => "project_agent.prime",
            Self::ProjectAgentCreated(_) => "project_agent.created",
            Self::LeadOpen(_) => "lead.open",
            Self::LeadResume(_) => "lead.resume",
            Self::LeadSuggest(_) => "lead.suggest",
            Self::SuggestionConfirm(_) => "suggestion.confirm",
            Self::SuggestionDismiss(_) => "suggestion.dismiss",
            Self::LeadList(_) => "lead.list",
            Self::LeadPassOn(_) => "lead.pass_on",
            Self::LeadQuestions => "lead.questions",
            Self::LeadPrime(_) => "lead.prime",
            Self::LeadNote(_) => "lead.note",
            Self::TicketNotes(_) => "ticket.notes",
            Self::TicketLeadLog(_) => "ticket.lead_log",
            Self::TicketTerminal(_) => "ticket.terminal",
            Self::AgentsList => "agents.list",
            Self::AgentsAdd(_) => "agents.add",
            Self::AgentsEdit(_) => "agents.edit",
            Self::AgentsDuplicate(_) => "agents.duplicate",
            Self::AgentsDelete(_) => "agents.delete",
            Self::AgentsInUse(_) => "agents.in_use",
            Self::AgentsSetDefault(_) => "agents.set_default",
            Self::ProjectSetAgentDefault(_) => "project.set_agent_default",
            Self::ProjectAgentDefaults(_) => "project.agent_defaults",
            Self::TicketAgentRow(_) => "ticket.agent_row",
            Self::TicketAgents(_) => "ticket.agents",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply {
    Ok,
    Pong,
    ServiceStatus(ServiceStatus),
    StartupReport(Box<crate::recovery::StartupReport>),
    ToolsCheck(ToolsReport),
    OnboardingState(OnboardingState),
    Projects(Vec<ProjectInfo>),
    Project(ProjectInfo),
    /// The answer to `project.add`: the project, and whether the agent tools' trust was recorded for its folder
    /// (PRJ-06). No `trust` when no tool's config file was located. A failed trust never fails the add.
    ProjectAdded {
        project: ProjectInfo,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trust: Option<crate::agent_tools::SetupOutcome>,
    },
    FolderProbe(FolderProbe),
    Branches(Branches),
    PrProvider(crate::pr::provider::PrProvider),
    PrOutcome(crate::pr::service::PrOutcome),
    PrFlow(Box<crate::pr::flow::PrFlowView>),
    BranchCheck(crate::pr::flow::BranchCheck),
    MarkedMerged(crate::review::pr_state::MarkedMerged),
    Message(Box<crate::messages::Message>),
    Messages(Vec<crate::messages::Message>),
    HumanAnswers(Vec<crate::board::questions::HumanAnswer>),
    Decision(Box<crate::decisions::Decision>),
    Decisions(Vec<crate::decisions::Decision>),
    DecisionChanged(crate::decisions::ChangeOutcome),
    Suggestion(Box<crate::epic_lead::suggest::Suggestion>),
    LeadQuestion(Box<crate::epic_lead::questions::LeadQuestion>),
    LeadQuestions(Vec<crate::epic_lead::questions::LeadQuestion>),
    EpicPrime(Box<crate::epic_lead::prime::EpicPrime>),
    LeadNote(Box<crate::epic_lead::notes::LeadNote>),
    LeadNotes(Vec<crate::epic_lead::notes::LeadNote>),
    LeadLog(Box<crate::epic_lead::log::LeadLog>),
    Board(BoardSnapshot),
    Ticket(Box<Ticket>),
    Validations(Vec<crate::board::validation::ValidationRecord>),
    Tickets(Vec<Ticket>),
    Readiness(Vec<Readiness>),
    SchedulerStatus(SchedulerStatus),
    Usage(Box<crate::usage::UsageSummary>),
    SessionUsage(Vec<crate::usage::SessionUsage>),
    Activity(Box<crate::usage::activity::Activity>),
    AgentEdits(AgentEdits),
    ClosePlan(ClosePlan),
    EpicQueued(EpicQueueOutcome),
    EpicTakenBack(TakeBackPlan),
    EpicQueueUndone(UndoQueueOutcome),
    DeletePreview(DeletePreview),
    DepTree(DepTree),
    /// Dependency loops, each starting and ending on the same ticket; empty on a valid database.
    Cycles(Vec<Vec<TicketKey>>),
    GlobalSettings(Box<GlobalSettingsReply>),
    ProjectSettings(Box<ProjectSettingsReply>),
    Sessions(Vec<SessionInfo>),
    SessionAttached(Box<SessionAttach>),
    Delivery(Delivery),
    History(Vec<HistoryEvent>),
    Prime(Box<PrimeContext>),
    PrimeValidate(Box<crate::prime::ValidatePrime>),
    DiffSummary(Box<DiffSummary>),
    FileDiff(Box<FileDiff>),
    ReviewComment(ReviewComment),
    ReviewComments(ReviewComments),
    Returned(ReturnOutcome),
    ReviewFeedback(ReviewFeedback),
    MergeProposal(MergeProposal),
    LocalMerge(LocalMergeOutcome),
    ApproveCheck(ApproveCheck),
    Conflict(Option<Conflict>),
    /// The tickets Resume all resumed, in key order.
    Resumed(Vec<TicketKey>),
    /// The tickets "This is the usage limit" folded into the banner.
    LimitMarked(Vec<TicketKey>),
    BeforeRestart(Box<crate::recovery::BeforeRestart>),
    Inbox(Box<crate::attention::inbox::InboxList>),
    InboxCounts(crate::attention::inbox::InboxCounts),
    CleanupScan(Vec<crate::recovery::cleanup::CleanupItem>),
    CleanupOutcome(crate::recovery::cleanup::CleanupOutcome),
    WarmList(Box<crate::warm::store::WarmList>),
    WarmClear(crate::warm::store::ClearOutcome),
    Run(Box<crate::terminals::run::RunInfo>),
    Runs(Vec<crate::terminals::run::RunInfo>),
    Session(Box<SessionInfo>),
    CreatedHere(Box<crate::board::created::CreatedHere>),
    ProjectTerminal(Box<crate::terminals::shell::ProjectTerminal>),
    ProjectTerminals(Vec<crate::terminals::shell::ProjectTerminal>),
    ProjectAgent(Box<crate::project_agents::ProjectAgent>),
    ProjectAgents(Box<crate::project_agents::ProjectAgentList>),
    ProjectPrime(Box<crate::project_agents::prime::ProjectPrime>),
    CreatedAndChanged(Box<crate::project_agents::CreatedAndChanged>),
    Lead(Box<crate::project_agents::lead::Lead>),
    Leads(Vec<crate::project_agents::lead::Lead>),
    TicketTerminal(Box<TicketTerminal>),
    /// A tool's model list as Otaq's copy has it; `None` before it was first read.
    Models(Option<Box<crate::agent_tools::models::ModelCopy>>),
    Agents(Box<crate::named_agents::AgentList>),
    Agent(Box<crate::named_agents::NamedAgent>),
    AgentDraft(Box<crate::named_agents::AgentFields>),
    TicketAgents(Box<crate::named_agents::rows::TicketAgents>),
    AgentDefaults(Box<crate::named_agents::RoleDefaults>),
    AgentInUse(Box<crate::named_agents::InUse>),
}

/// The global settings with any values flagged as invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalSettingsReply {
    pub settings: GlobalSettings,
    pub problems: Vec<SettingProblem>,
}

/// A project's settings with any values flagged as invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSettingsReply {
    pub settings: ProjectSettings,
    pub problems: Vec<SettingProblem>,
    /// What the project uses after falling back to the global settings, e.g. that Auto-merge does nothing
    /// while validation is off (d0064).
    pub effective: EffectiveSettings,
}

/// The answer to `onboarding.state`: the first run, and the banners kept by the service. A change to the folder
/// trust fields sends [`Event::TrustChanged`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnboardingState {
    /// First run means no project is stored yet (UI-08).
    pub has_projects: bool,
    pub permission_warning_dismissed: bool,
    /// The one-time note that Otaq recorded Claude Code's trust for projects added earlier (PRJ-08): shown until
    /// `notices.dismiss_trust_note`.
    #[serde(default)]
    pub trust_note: bool,
    /// Stored projects whose folder trust couldn't be recorded, by key (PRJ-08): one warning banner. A project
    /// leaves the list when a later startup records its trust.
    #[serde(default)]
    pub trust_failed: Vec<crate::agent_tools::setup::TrustFailure>,
}

/// The service's answer to `service.status`. Version-independent: fields may be added (with defaults), never
/// renamed or removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub pid: u32,
    pub build_id: String,
    pub protocol_version: u32,
    /// When the service started, in Unix milliseconds.
    pub started_at: i64,
    pub uptime_secs: u64,
    /// Agents working right now, for "Restart now (3 agents working, 2 waiting)" (d0022).
    #[serde(default)]
    pub agents_working: u32,
    /// Agents waiting for the user (the needs-you family, d0042).
    #[serde(default)]
    pub agents_waiting: u32,
    /// This run of the service followed a crash or a reboot (its previous run didn't stop cleanly; REC-06).
    #[serde(default)]
    pub unclean_start: bool,
    /// Tickets the startup reconcile set to Interrupted ("4 sessions were interrupted", REC-01, REC-06).
    #[serde(default)]
    pub interrupted_at_start: u32,
}

/// Something that happened in the service, pushed to subscribers (SDD §5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Only used by tests, so the event stream is testable before real events exist.
    Test { seq: u64 },
    /// A ticket or epic changed in any way (fields, column, links, pin, key). Refetch it or the board.
    TicketChanged {
        project: String,
        id: String,
        key: TicketKey,
    },
    /// A ticket's status changed (sent with [`Event::TicketChanged`]).
    CardStatusChanged {
        project: String,
        id: String,
        key: TicketKey,
        status: Status,
    },
    /// A ticket is gone (deleted, or its project removed).
    TicketDeleted {
        project: String,
        id: String,
        key: TicketKey,
    },
    /// A project was added, changed or removed. Refetch `project.list`.
    ProjectsChanged,
    /// Output of a session this connection attached to (only sent to that connection). `data` is base64.
    SessionOutput {
        session: String,
        /// Where `data` starts in the session's whole output.
        offset: u64,
        data: String,
    },
    /// A session started or ended.
    SessionState { session: SessionInfo },
    /// How many automatic deliveries wait for a session ("1 message waiting · Deliver now", SES-06).
    MessagesWaiting {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ticket: Option<TicketKey>,
        count: u32,
    },
    /// A client's terminal for a session gained or lost focus.
    SessionFocus {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ticket: Option<TicketKey>,
        focused: bool,
    },
    /// The needs-you items changed (an item came, went or changed group or reason): refetch `inbox.list`. Carries
    /// the new counts for the header and the project switcher (ATT-10). Not sent when only ages grew.
    InboxChanged {
        counts: crate::attention::inbox::InboxCounts,
    },
    /// An OS notification was clicked (ATT-11): bring the app forward on this ticket with its terminal focused,
    /// or on the inbox for a grouped notification (`key` absent).
    OpenFromNotification {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<TicketKey>,
    },
    /// Token counts changed (USG-01): refetch `usage.summary`.
    UsageChanged,
    /// A ticket's Run changed state (RUN-01): refetch `run.status` / `run.list`.
    RunChanged { project: String, key: TicketKey },
    /// Ask Claude's progress and outcome for a project's run command (RUN-02).
    RunSuggestion {
        project: String,
        update: crate::terminals::suggest::SuggestUpdate,
    },
    /// A Run was stopped because its ticket reached Done or was closed: a toast ("Run stopped · SHOP-19 is done").
    RunStopped {
        project: String,
        key: TicketKey,
        message: String,
    },
    /// After a push, the PR branch's content differs from what the user reviewed (REV-06, T-0145): a toast
    /// ("SHOP-18 changed since you reviewed it · the pushed content differs in 2 files").
    PushedContentDiffers {
        project: String,
        key: TicketKey,
        message: String,
    },
    /// A toast (UI-07, d0064): Auto-merge merged a validated ticket ("SHOP-40.1 merged automatically into the
    /// Wishlist branch" · "Validated · round 1 · local, not pushed · SHOP-40.2 can start"). No notification.
    MergedAutomatically {
        project: String,
        key: TicketKey,
        title: String,
        message: String,
    },
    /// A toast (UI-07, d0063): a validation was cancelled because the code or the description changed
    /// ("Validation cancelled: SHOP-29’s code changed" · "Round not counted · the agent was told to submit again").
    ValidationCancelled {
        project: String,
        key: TicketKey,
        title: String,
        message: String,
    },
    /// A question or answer between agents was typed into the other agent's session (MSG-02, MSG-06): the
    /// receiving ticket's key; both tickets' cards and Messages refetch.
    MessageDelivered {
        project: String,
        key: TicketKey,
        message: String,
    },
    /// A Project agent was opened, named, closed, resumed, or its turn or state changed (d0068): refetch
    /// `project_agent.list`.
    ProjectAgentChanged { project: String, id: String },
    /// An epic's lead was opened, resumed or ended, or its status changed (d0083): refetch the epic's lead.
    LeadChanged { project: String, epic: TicketKey },
    /// The global settings were saved (`settings.set`): refetch `settings.get`. Every window applies a new theme
    /// at once (UIT-07).
    SettingsChanged,
    /// The named agents or the role defaults (app-wide or a project's) changed (AGT-01…AGT-03): refetch
    /// `agents.list` (and the project's settings).
    AgentsChanged,
    /// The folder trust notices changed (the one-time note, the projects whose trust couldn't be recorded;
    /// PRJ-08): refetch `onboarding.state`.
    TrustChanged,
    /// A project's warm build was kept, replaced or cleared, or a Clear started waiting for a copy (WRM-06):
    /// refetch `warm.list`. `repo` is its folder in `warm/`; `project` the key of the project it belongs to.
    WarmBuildChanged {
        repo: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
    },
    /// Enter was pressed in a session's terminal (clears needs-you signals from M4, ATT-02).
    SessionEnter {
        session: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ticket: Option<TicketKey>,
    },
}

/// The `{"error": {...}}` body, used on the socket and printed by the CLI with `--json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorBody {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            data: None,
        }
    }
}

impl fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The peer speaks another protocol version ("Restart required", d0022).
    IncompatibleProtocol,
    /// The message couldn't be parsed or isn't a valid request.
    BadRequest,
    /// The request was valid but the service failed to carry it out.
    Internal,
    /// A board rule refused the request; `data.refusal` says which (for example `cycle`, `cross_base`).
    Refused,
    /// No such project or ticket.
    NotFound,
    /// The background service isn't reachable.
    Disconnected,
    /// Wrong command-line usage.
    Usage,
    /// A kind this build doesn't know (sent by a newer peer).
    #[serde(other)]
    Unknown,
}

/// Why a frame couldn't be turned into a [`Message`].
#[derive(Debug)]
pub enum DecodeError {
    /// Not JSON, or not a message this build understands.
    Malformed(serde_json::Error),
    /// A well-formed message from a peer with another protocol version.
    IncompatibleVersion(ForeignMessage),
}

impl DecodeError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Malformed(_) => ErrorKind::BadRequest,
            Self::IncompatibleVersion(_) => ErrorKind::IncompatibleProtocol,
        }
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "malformed message: {e}"),
            Self::IncompatibleVersion(m) => write!(
                f,
                "incompatible protocol version {} (this build speaks {PROTOCOL_VERSION})",
                m.protocol_version
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A message from a peer speaking another protocol version, kept raw. Only its envelope and the
/// [`VERSION_INDEPENDENT_METHODS`] can be relied on.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignMessage {
    pub protocol_version: u32,
    pub id: Option<u64>,
    pub raw: Value,
}

impl ForeignMessage {
    /// The request method, if this is a request.
    pub fn method(&self) -> Option<&str> {
        self.raw.get("request")?.get("method")?.as_str()
    }

    /// Whether this is a request every service answers regardless of version.
    pub fn is_version_independent_request(&self) -> bool {
        self.method()
            .is_some_and(|m| VERSION_INDEPENDENT_METHODS.contains(&m))
    }

    /// Reads the message with this build's types. Works for the version-independent requests and replies.
    pub fn read_as_current(&self) -> Option<Message> {
        let mut raw = self.raw.clone();
        raw.as_object_mut()?
            .insert("protocol_version".into(), PROTOCOL_VERSION.into());
        serde_json::from_value(raw).ok()
    }
}

/// Serializes a message as one frame: compact JSON followed by `\n`.
pub fn encode(message: &Message) -> Vec<u8> {
    let mut frame = serde_json::to_vec(message).expect("messages always serialize");
    frame.push(b'\n');
    frame
}

/// Parses one frame (with or without its trailing newline). The protocol version is checked before the body,
/// so a peer with another version is reported as such even if its messages no longer parse.
pub fn decode(frame: &[u8]) -> Result<Message, DecodeError> {
    let raw: Value = serde_json::from_slice(frame).map_err(DecodeError::Malformed)?;
    let version = raw
        .get("protocol_version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    match version {
        Some(v) if v != PROTOCOL_VERSION => Err(DecodeError::IncompatibleVersion(ForeignMessage {
            protocol_version: v,
            id: raw.get("id").and_then(Value::as_u64),
            raw,
        })),
        _ => serde_json::from_value(raw).map_err(DecodeError::Malformed),
    }
}

/// Reads frames from a byte stream. Handles partial reads; a frame may arrive in any number of pieces.
pub struct FrameReader<R> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: AsyncBufRead + Unpin> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
        }
    }

    /// The next frame without its newline, or `None` at a clean end of stream.
    pub async fn next_frame(&mut self) -> io::Result<Option<&[u8]>> {
        self.buf.clear();
        let limit = MAX_FRAME_BYTES as u64;
        let n = (&mut self.inner)
            .take(limit)
            .read_until(b'\n', &mut self.buf)
            .await?;
        if n == 0 {
            return Ok(None);
        }
        if self.buf.last() != Some(&b'\n') {
            let error = if self.buf.len() >= MAX_FRAME_BYTES {
                io::Error::new(io::ErrorKind::InvalidData, "frame too large")
            } else {
                io::Error::new(io::ErrorKind::UnexpectedEof, "stream ended mid-frame")
            };
            return Err(error);
        }
        self.buf.pop();
        Ok(Some(&self.buf))
    }
}

/// Writes one message as a frame and flushes it.
pub async fn write_message<W: AsyncWrite + Unpin>(w: &mut W, message: &Message) -> io::Result<()> {
    w.write_all(&encode(message)).await?;
    w.flush().await
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use serde_json::json;
    use tokio::io::{AsyncRead, BufReader, ReadBuf};

    use super::*;

    fn status() -> ServiceStatus {
        ServiceStatus {
            pid: 4120,
            build_id: "abc.1".into(),
            protocol_version: PROTOCOL_VERSION,
            started_at: 1_791_504_000_000,
            uptime_secs: 42,
            agents_working: 3,
            agents_waiting: 2,
            ..Default::default()
        }
    }

    fn samples() -> Vec<Message> {
        vec![
            Message::request(1, Request::Ping),
            Message::request(2, Request::ServiceStatus),
            Message::request(3, Request::ServiceStop),
            Message::request(6, Request::Subscribe),
            Message::request(7, Request::ToolsCheck),
            Message::request(8, Request::OnboardingState),
            Message::request(9, Request::DismissPermissionWarning),
            Message::request(9, Request::DismissTrustNote),
            Message::reply(
                8,
                Reply::OnboardingState(OnboardingState {
                    has_projects: false,
                    permission_warning_dismissed: true,
                    trust_note: true,
                    trust_failed: vec![crate::agent_tools::setup::TrustFailure {
                        project_key: "NOTE".into(),
                        tool: crate::agent_tools::Tool::Claude,
                        reason: "Claude Code's config file couldn't be written".into(),
                    }],
                }),
            ),
            Message::reply(1, Reply::Pong),
            Message::reply(2, Reply::ServiceStatus(status())),
            Message::reply(3, Reply::Ok),
            Message::event(Event::Test { seq: 9 }),
            Message::request(
                10,
                Request::TicketMove(MoveParams {
                    key: "SHOP-18".into(),
                    target: DropTarget::ReadyForDev,
                    confirmed: true,
                }),
            ),
            Message::request(
                11,
                Request::TicketUpdate(TicketUpdateParams {
                    key: "SHOP-18".into(),
                    update: TicketUpdate {
                        title: Some("Checkout".into()),
                        ..Default::default()
                    },
                }),
            ),
            Message::request(12, Request::ProjectList),
            Message::event(Event::CardStatusChanged {
                project: "SHOP".into(),
                id: "id".into(),
                key: "SHOP-40.1".parse().unwrap(),
                status: Status::Queued,
            }),
            Message::event(Event::ProjectsChanged),
            Message::event(Event::TrustChanged),
            Message::error(
                Some(4),
                ErrorBody::new(ErrorKind::BadRequest, "unknown method"),
            ),
            Message {
                caller: Some("agent:SHOP-18".into()),
                ..Message::request(5, Request::Ping)
            },
        ]
    }

    #[test]
    fn round_trips() {
        for message in samples() {
            let frame = encode(&message);
            assert_eq!(frame.last(), Some(&b'\n'));
            assert_eq!(
                frame.iter().filter(|b| **b == b'\n').count(),
                1,
                "one line per message"
            );
            assert_eq!(decode(&frame).unwrap(), message);
        }
    }

    #[test]
    fn wire_shapes() {
        let as_json = |m: &Message| serde_json::to_value(m).unwrap();
        assert_eq!(
            as_json(&Message::request(7, Request::ServiceStatus)),
            json!({"protocol_version": 1, "id": 7, "pid": std::process::id(), "request": {"method": "service.status"}})
        );
        assert_eq!(
            as_json(&Message::error(
                Some(8),
                ErrorBody::new(ErrorKind::IncompatibleProtocol, "x")
            )),
            json!({"protocol_version": 1, "id": 8, "error": {"kind": "incompatible_protocol", "message": "x"}})
        );
        assert_eq!(
            as_json(&Message::request(
                4,
                Request::DepAdd(DepParams {
                    child: "SHOP-20".into(),
                    parent: "SHOP-18".into()
                })
            )),
            json!({"protocol_version": 1, "id": 4, "pid": std::process::id(), "request": {"method": "dep.add", "params": {"child": "SHOP-20", "parent": "SHOP-18"}}})
        );
        assert_eq!(
            as_json(&Message::event(Event::Test { seq: 3 })),
            json!({"protocol_version": 1, "event": {"kind": "test", "seq": 3}})
        );
        for m in VERSION_INDEPENDENT_METHODS {
            assert!(
                [Request::Ping, Request::ServiceStatus, Request::ServiceStop]
                    .iter()
                    .any(|r| r.method() == *m)
            );
        }
    }

    #[test]
    fn unknown_error_kinds_are_tolerated() {
        let m = decode(
            br#"{"protocol_version":1,"id":1,"error":{"kind":"from_the_future","message":"m"}}"#,
        )
        .unwrap();
        let Body::Error(e) = m.body else {
            panic!("{m:?}")
        };
        assert_eq!(e.kind, ErrorKind::Unknown);
    }

    #[test]
    fn version_mismatch_is_a_distinct_error() {
        let frame = br#"{"protocol_version":2,"id":5,"request":{"method":"tickets.frobnicate","params":{"x":1}}}"#;
        let err = decode(frame).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::IncompatibleProtocol);
        let DecodeError::IncompatibleVersion(foreign) = err else {
            panic!()
        };
        assert_eq!(foreign.protocol_version, 2);
        assert_eq!(foreign.id, Some(5));
        assert_eq!(foreign.method(), Some("tickets.frobnicate"));
        assert!(!foreign.is_version_independent_request());
    }

    #[test]
    fn version_independent_messages_read_across_versions() {
        let request = br#"{"protocol_version":9,"id":1,"request":{"method":"service.status"}}"#;
        let DecodeError::IncompatibleVersion(foreign) = decode(request).unwrap_err() else {
            panic!()
        };
        assert!(foreign.is_version_independent_request());
        assert_eq!(
            foreign.read_as_current().unwrap().body,
            Body::Request(Request::ServiceStatus)
        );

        // A newer service's status reply, with a field this build doesn't know.
        let mut reply =
            serde_json::to_value(Message::reply(1, Reply::ServiceStatus(status()))).unwrap();
        reply["protocol_version"] = json!(9);
        reply["reply"]["service_status"]["new_field"] = json!(true);
        let DecodeError::IncompatibleVersion(foreign) =
            decode(&serde_json::to_vec(&reply).unwrap()).unwrap_err()
        else {
            panic!()
        };
        assert_eq!(
            foreign.read_as_current().unwrap().body,
            Body::Reply(Reply::ServiceStatus(status()))
        );
    }

    #[test]
    fn malformed_frames() {
        for frame in [
            &b"not json"[..],
            br#"{"protocol_version":1}"#,
            br#"{"id":1,"request":{"method":"ping"}}"#,
        ] {
            let err = decode(frame).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::BadRequest, "{frame:?}");
        }
    }

    #[test]
    fn build_id_has_a_hash_and_a_time() {
        let id = build_id();
        let (hash, secs) = id.rsplit_once('.').expect(id);
        assert!(!hash.is_empty(), "{id}");
        assert!(secs.parse::<u64>().unwrap() > 0, "{id}");
    }

    /// Yields at most `chunk` bytes per read, to exercise partial reads.
    struct Trickle {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }

    impl AsyncRead for Trickle {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let end = (self.pos + self.chunk)
                .min(self.data.len())
                .min(self.pos + buf.remaining());
            let piece = self.data[self.pos..end].to_vec();
            buf.put_slice(&piece);
            self.pos = end;
            Poll::Ready(Ok(()))
        }
    }

    fn trickle(data: Vec<u8>, chunk: usize) -> FrameReader<BufReader<Trickle>> {
        FrameReader::new(BufReader::with_capacity(
            4,
            Trickle {
                data,
                pos: 0,
                chunk,
            },
        ))
    }

    #[tokio::test]
    async fn framing_with_partial_reads() {
        let messages = samples();
        let stream: Vec<u8> = messages.iter().flat_map(encode).collect();
        for chunk in [1, 3, 7, 1000] {
            let mut reader = trickle(stream.clone(), chunk);
            let mut got = Vec::new();
            while let Some(frame) = reader.next_frame().await.unwrap() {
                got.push(decode(frame).unwrap());
            }
            assert_eq!(got, messages, "chunk size {chunk}");
        }
    }

    #[tokio::test]
    async fn stream_ending_mid_frame_is_an_error() {
        let mut stream = encode(&Message::request(1, Request::Ping));
        stream.extend_from_slice(br#"{"protocol_version":1,"#);
        let mut reader = trickle(stream, 5);
        assert!(reader.next_frame().await.unwrap().is_some());
        let err = reader.next_frame().await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn write_then_read() {
        let (client, server) = tokio::io::duplex(64);
        let (_, mut write_half) = tokio::io::split(client);
        let (read_half, _) = tokio::io::split(server);
        let messages = samples();
        let sent = messages.clone();
        let writer = tokio::spawn(async move {
            for m in &sent {
                write_message(&mut write_half, m).await.unwrap();
            }
        });
        let mut reader = FrameReader::new(BufReader::new(read_half));
        for expected in &messages {
            let frame = reader.next_frame().await.unwrap().unwrap();
            assert_eq!(&decode(frame).unwrap(), expected);
        }
        writer.await.unwrap();
    }
}
