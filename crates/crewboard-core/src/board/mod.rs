//! The board's rules: projects, tickets, epics, dependencies, moves and pins (SDD §6, §7). The service calls
//! these for the app and the CLI alike, so every rule is enforced in one place (d0055). No UI, no sockets.

pub mod agent_edits;
pub mod batch;
pub mod change_note;
pub mod delete;
pub mod deps;
pub mod diff;
pub mod epics;
pub mod moves;
pub mod needs_you;
pub mod order;
pub mod projects;
pub mod questions;
pub mod readiness;
pub mod review;
pub mod settings;
pub mod snapshot;
pub mod stack;
pub mod tickets;

use std::fmt;

use crate::domain::stored_enum;
use crate::git::{Git, GitError};
use crate::store::{Store, StoreError};

/// The store plus the user's git. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Board {
    store: Store,
    /// `None` when git isn't installed; git work then fails with [`GitError::NotFound`].
    git: Option<Git>,
}

impl Board {
    pub fn new(store: Store, git: Option<Git>) -> Self {
        Self { store, git }
    }

    /// A board using the `git` found on `PATH`, if any.
    pub fn with_path_git(store: Store) -> Self {
        Self::new(store, Git::find().ok())
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub(crate) fn git(&self) -> Result<&Git, BoardError> {
        self.git.as_ref().ok_or(BoardError::Git(GitError::NotFound))
    }
}

stored_enum! {
    /// Why a request was refused, so a client can place the message (for example next to a form field).
    pub enum Refusal ("refusal") {
        /// The folder doesn't exist or isn't inside a git repository.
        NotAGitRepo = "not_a_git_repo",
        /// The repository is already a project.
        AlreadyAdded = "already_added",
        /// The project key's format is wrong.
        BadKey = "bad_key",
        /// Another project uses the key.
        KeyTaken = "key_taken",
        /// The project key can't change once added (d0029).
        KeyLocked = "key_locked",
        /// No base branch could be detected or the given one is empty.
        NoBaseBranch = "no_base_branch",
        /// A required field (a name, a title) is empty.
        EmptyField = "empty_field",
        /// Epic children have no base branch of their own: they use the epic's branch (d0051, d0059).
        BaseFromEpic = "base_from_epic",
        /// The ticket has started; its base branch is locked (d0051).
        BaseLocked = "base_locked",
        /// Epic children have no External ID (d0059).
        ExternalIdOnChild = "external_id_on_child",
        /// The ticket named as an epic isn't one.
        NotAnEpic = "not_an_epic",
        /// Epics can't be nested: one level only (d0039).
        EpicInEpic = "epic_in_epic",
        /// A "Done when" item that doesn't exist.
        NoSuchItem = "no_such_item",
        /// The tickets belong to different projects (d0024).
        OtherProject = "other_project",
        /// A ticket can't depend on itself.
        SelfDependency = "self_dependency",
        /// The link exists already.
        DuplicateDependency = "duplicate_dependency",
        /// An epic child can't depend on its own epic.
        DependsOnOwnEpic = "depends_on_own_epic",
        /// The link would close a loop (DEP-01).
        Cycle = "cycle",
        /// Linked tickets must use the same base branch (DEP-02, d0051).
        CrossBase = "cross_base",
        /// The ticket is already an epic child; a child can't move to another epic (d0039).
        AlreadyInEpic = "already_in_epic",
        /// The ticket has been to Dev; only never-started tickets may do this (d0044, d0060).
        Started = "started",
        /// The drag rules don't allow this move (SDD §7.4); the message says why.
        MoveRefused = "move_refused",
        /// Starting a ticket with an empty "Done when" needs "Start anyway" (TKT-04).
        EmptyDoneWhen = "empty_done_when",
        /// The move needs the user's confirmation or choice first.
        NeedsConfirmation = "needs_confirmation",
        /// Allowed by the rules, but the feature that carries it out isn't built yet.
        NotAvailableYet = "not_available_yet",
        /// The caller may not do this (an agent deleting tickets, CLI-02).
        NotAllowed = "not_allowed",
        /// The ticket already has a running agent session (SES-04).
        AlreadyRunning = "already_running",
        /// The session has ended: it takes no input or resize.
        SessionEnded = "session_ended",
        /// The worktree has uncommitted files (REV-07, d0050).
        Uncommitted = "uncommitted",
        /// A question or answer is empty or malformed.
        BadQuestion = "bad_question",
        /// The ticket has no branch yet (it never started), so it has no diff.
        NoBranch = "no_branch",
    }
}

#[derive(Debug)]
pub enum BoardError {
    /// A rule refused the request. The message is written for people.
    Refused {
        code: Refusal,
        message: String,
    },
    /// No such project or ticket.
    NotFound(String),
    Git(GitError),
    Store(StoreError),
}

impl BoardError {
    pub fn refused(code: Refusal, message: impl Into<String>) -> Self {
        Self::Refused {
            code,
            message: message.into(),
        }
    }

    /// The refusal code, if a rule refused the request.
    pub fn refusal(&self) -> Option<Refusal> {
        match self {
            Self::Refused { code, .. } => Some(*code),
            _ => None,
        }
    }
}

impl fmt::Display for BoardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused { message, .. } => f.write_str(message),
            Self::NotFound(what) => f.write_str(what),
            Self::Git(e) => write!(f, "{e}"),
            Self::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BoardError {}

impl From<StoreError> for BoardError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl From<sqlx::Error> for BoardError {
    fn from(e: sqlx::Error) -> Self {
        Self::Store(StoreError::Database(e))
    }
}

impl From<GitError> for BoardError {
    fn from(e: GitError) -> Self {
        Self::Git(e)
    }
}
