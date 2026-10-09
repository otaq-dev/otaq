//! The board's rules: projects, tickets, epics, dependencies, moves and pins (SDD §6, §7). The service calls
//! these for the app and the CLI alike, so every rule is enforced in one place (d0055). No UI, no sockets.

pub mod projects;

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

    fn git(&self) -> Result<&Git, BoardError> {
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
