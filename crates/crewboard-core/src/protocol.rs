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
//! it is trivial to read in logs and to poke at by hand (`nc -U ~/.crewboard/crewboard.sock`).
//!
//! # Versions
//!
//! A peer with another `protocol_version` is reported as [`DecodeError::IncompatibleVersion`]
//! ([`ErrorKind::IncompatibleProtocol`]), which drives "Restart required" (d0022). The envelope fields and the
//! [`VERSION_INDEPENDENT_METHODS`] (`ping`, `service.status`, `service.stop`) never change incompatibly, so an
//! app can always learn the old service's status and restart it.

use std::fmt;
use std::io;

use crate::board::agent_edits::AgentEdits;
use crate::board::batch::BatchTicket;
use crate::board::delete::DeletePreview;
use crate::board::deps::DepTree;
use crate::board::diff::DiffSummary;
use crate::board::epics::ClosePlan;
use crate::board::moves::DropTarget;
use crate::board::projects::{AddProject, Branches, FolderProbe, ProjectUpdate};
use crate::board::readiness::Readiness;
use crate::board::settings::{GlobalSettings, ProjectSettings, SettingProblem};
use crate::board::snapshot::{BoardSnapshot, ProjectInfo};
use crate::board::tickets::{HistoryEvent, NewTicketInput, Ticket, TicketUpdate};
use crate::domain::{Status, TicketKey};
use crate::git::diff::FileDiff;
use crate::prime::PrimeContext;
use crate::scheduler::SchedulerStatus;
use crate::session::deliveries::Delivery;
use crate::session::{SessionInfo, TerminalSize};
use crate::signals::HookEvent;
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

    // Projects (PRJ-01…04). Answered with [`Reply::Projects`], [`Reply::FolderProbe`] or [`Reply::Project`].
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
    /// Creates several tickets with links between them, all or nothing (`crewboard create -f`, `--deps`).
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
    /// The context an agent reads first (`crewboard prime`, PLG-02). Answered with [`Reply::Prime`].
    #[serde(rename = "ticket.prime")]
    TicketPrime(KeyParams),
    /// `crewboard ask human`: the ticket shows Asks you with the question (ATT-01). Answered with
    /// [`Reply::Ticket`].
    #[serde(rename = "ticket.ask_human")]
    TicketAskHuman(AskHumanParams),
    /// `crewboard review submit`: Ready for Review with the summary; refused with uncommitted files (REV-07).
    /// Answered with [`Reply::Ticket`].
    #[serde(rename = "ticket.review_submit")]
    TicketReviewSubmit(ReviewSubmitParams),
    /// The ticket's events, newest first (History tab). Answered with [`Reply::History`].
    #[serde(rename = "ticket.history")]
    TicketHistory(KeyParams),

    // Review (REV-01…REV-03).
    /// The ticket's changed files since its `base_sha` (epics: also by child). Answered with
    /// [`Reply::DiffSummary`].
    #[serde(rename = "diff.files")]
    DiffFiles(KeyParams),
    /// One file's hunks; answered with [`Reply::FileDiff`].
    #[serde(rename = "diff.file")]
    DiffFile(DiffFileParams),

    // Epics.
    #[serde(rename = "epic.move_to")]
    EpicMoveTo(MoveToEpicParams),
    /// Answered with [`Reply::ClosePlan`].
    #[serde(rename = "epic.close_plan")]
    EpicClosePlan(KeyParams),
    /// Answered with [`Reply::ClosePlan`] (what was closed).
    #[serde(rename = "epic.close")]
    EpicClose(KeyParams),

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
    /// Each unfinished ticket's dependency state (`crewboard ready` / `blocked`); answered with
    /// [`Reply::Readiness`].
    #[serde(rename = "dep.readiness")]
    DepReadiness(ProjectParams),
    /// The agent limits as the scheduler sees them now (the header's "3 / 5 agents", Settings' "Right now");
    /// answered with [`Reply::SchedulerStatus`].
    #[serde(rename = "scheduler.status")]
    SchedulerStatus,

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
    /// Answered with [`Reply::SessionAttached`].
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

    // Agent signals (SDD §11.4).
    /// A Claude Code hook of the caller's agent session (`crewboard hook <event>`, PLG-04). Answered with
    /// [`Reply::Ok`]; from outside an agent session it changes nothing.
    #[serde(rename = "hook")]
    Hook(HookParams),
}

/// `crewboard hook Stop`: the event and the JSON Claude Code passed on stdin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookParams {
    pub event: HookEvent,
    #[serde(default)]
    pub payload: Value,
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

/// `{"key": "SHOP-18", "path": "lib/pricing.ts"}`; `child` (epics) limits it to that child's squash commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffFileParams {
    pub key: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
}

/// `{"key": "SHOP-18", "summary": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSubmitParams {
    pub key: String,
    #[serde(default)]
    pub summary: String,
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

/// `{"key": "SHOP-18"}` (a ticket) or `{"key": "SHOP"}` (a project).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyParams {
    pub key: String,
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
    pub settings: ProjectSettings,
}

/// `{"key": "SHOP-18"}` for a ticket's sessions, `{}` for every session in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// A session by id (`{"session": "…"}`), or a ticket's agent session by key (`{"key": "SHOP-18"}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
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
            Self::Subscribe => "events.subscribe",
            Self::ToolsCheck => "tools.check",
            Self::OnboardingState => "onboarding.state",
            Self::DismissPermissionWarning => "onboarding.dismiss_permission_warning",
            Self::ProjectList => "project.list",
            Self::ProjectProbe(_) => "project.probe",
            Self::ProjectAdd(_) => "project.add",
            Self::ProjectUpdate(_) => "project.update",
            Self::ProjectRemove(_) => "project.remove",
            Self::ProjectBranches(_) => "project.branches",
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
            Self::DiffFiles(_) => "diff.files",
            Self::DiffFile(_) => "diff.file",
            Self::TicketPrime(_) => "ticket.prime",
            Self::TicketAskHuman(_) => "ticket.ask_human",
            Self::TicketReviewSubmit(_) => "ticket.review_submit",
            Self::EpicMoveTo(_) => "epic.move_to",
            Self::EpicClosePlan(_) => "epic.close_plan",
            Self::EpicClose(_) => "epic.close",
            Self::DepAdd(_) => "dep.add",
            Self::DepRemove(_) => "dep.remove",
            Self::DepTree(_) => "dep.tree",
            Self::DepCycles(_) => "dep.cycles",
            Self::DepReadiness(_) => "dep.readiness",
            Self::SchedulerStatus => "scheduler.status",
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
            Self::Hook(_) => "hook",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply {
    Ok,
    Pong,
    ServiceStatus(ServiceStatus),
    ToolsCheck(ToolsReport),
    OnboardingState(OnboardingState),
    Projects(Vec<ProjectInfo>),
    Project(ProjectInfo),
    FolderProbe(FolderProbe),
    Branches(Branches),
    Board(BoardSnapshot),
    Ticket(Box<Ticket>),
    Tickets(Vec<Ticket>),
    Readiness(Vec<Readiness>),
    SchedulerStatus(SchedulerStatus),
    AgentEdits(AgentEdits),
    ClosePlan(ClosePlan),
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
    DiffSummary(Box<DiffSummary>),
    FileDiff(Box<FileDiff>),
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
}

/// The answer to `onboarding.state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnboardingState {
    /// First run means no project is stored yet (UI-08).
    pub has_projects: bool,
    pub permission_warning_dismissed: bool,
}

/// The service's answer to `service.status`. Version-independent: fields may be added (with defaults), never
/// renamed or removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Agents waiting for the user (the orange family, d0042).
    #[serde(default)]
    pub agents_waiting: u32,
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
    /// Enter was pressed in a session's terminal (clears orange signals from M4, ATT-02).
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
            Message::reply(
                8,
                Reply::OnboardingState(OnboardingState {
                    has_projects: false,
                    permission_warning_dismissed: true,
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
                    target: DropTarget::DevQueued,
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
