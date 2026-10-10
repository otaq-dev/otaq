//! The socket is locked to the current OS user (SVC-02, d0027): it can move tickets and type into agent
//! sessions, so no other account may connect.
//!
//! On macOS and Linux this takes two layers: the data folder holding the socket is `0700` and the socket file
//! `0600` (both enforced at every start, also when they already exist with looser permissions), and every
//! accepted connection's peer UID is compared with the service's own. On Windows the named pipe carries an
//! owner-only ACL ([`crate::ipc`]), and every connection's account (SID) is compared the same way.

use std::io;
use std::path::Path;

use crate::ipc::{self, ServerStream, UserId};

/// Who is on the other end of a connection. Injectable so tests can pose as another user.
pub trait PeerCredentials: Send + Sync {
    fn peer_user(&self, stream: &ServerStream) -> io::Result<UserId>;
}

/// The OS's answer (`getpeereid` on macOS, `SO_PEERCRED` on Linux, the client process's token on Windows).
pub struct OsPeerCredentials;

impl PeerCredentials for OsPeerCredentials {
    fn peer_user(&self, stream: &ServerStream) -> io::Result<UserId> {
        ipc::peer_user(stream)
    }
}

/// The account this process runs as.
pub fn current_user() -> UserId {
    ipc::current_user()
}

/// Whether a connection may stay open. Logs a warning when it may not.
pub fn peer_allowed(owner: &UserId, peer: io::Result<UserId>) -> bool {
    match peer {
        Ok(user) if user == *owner => true,
        Ok(user) => {
            tracing::warn!(
                peer = %user,
                owner = %owner,
                "connection from another user refused"
            );
            false
        }
        Err(e) => {
            tracing::warn!(error = %e, "connection refused: peer credentials unavailable");
            false
        }
    }
}

/// Makes `path` readable and writable only by its owner (`mode` is `0o700` for folders, `0o600` for files).
/// On Windows the data folder sits in the user's profile, whose inherited ACL is already the user's (plus
/// SYSTEM and Administrators); the pipe's own ACL is the lock there, so this does nothing.
pub fn restrict(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn only_the_owner_is_allowed() {
        assert!(peer_allowed(&501, Ok(501)));
        assert!(!peer_allowed(&501, Ok(502)));
        assert!(!peer_allowed(&501, Ok(0)), "not even root");
        assert!(!peer_allowed(&501, Err(io::Error::other("no creds"))));
    }

    #[cfg(windows)]
    #[test]
    fn only_the_owner_is_allowed() {
        let me = "S-1-5-21-1-2-3-1001".to_owned();
        assert!(peer_allowed(&me, Ok(me.clone())));
        assert!(!peer_allowed(&me, Ok("S-1-5-21-1-2-3-1002".into())));
        assert!(!peer_allowed(&me, Ok("S-1-5-18".into())), "not even SYSTEM");
        assert!(!peer_allowed(&me, Err(io::Error::other("no token"))));
    }

    #[cfg(unix)]
    #[test]
    fn current_user_matches_owned_files() {
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(std::fs::metadata(tmp.path()).unwrap().uid(), current_user());
    }
}
