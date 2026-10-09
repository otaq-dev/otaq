//! The socket is locked to the current OS user (SVC-02, d0027): it can move tickets and type into agent
//! sessions, so no other account may connect.
//!
//! On macOS and Linux this takes two layers: the data folder holding the socket is `0700` and the socket file
//! `0600` (both enforced at every start, also when they already exist with looser permissions), and every
//! accepted connection's peer UID is compared with the service's own. (Windows uses an owner-only ACL on the
//! named pipe instead, T-0155.)

use std::io;
use std::path::Path;

use tokio::net::UnixStream;

/// Who is on the other end of a connection. Injectable so tests can pose as another user.
pub trait PeerCredentials: Send + Sync {
    fn peer_uid(&self, stream: &UnixStream) -> io::Result<u32>;
}

/// The OS's answer (`getpeereid` on macOS, `SO_PEERCRED` on Linux).
pub struct OsPeerCredentials;

impl PeerCredentials for OsPeerCredentials {
    fn peer_uid(&self, stream: &UnixStream) -> io::Result<u32> {
        Ok(stream.peer_cred()?.uid())
    }
}

/// The UID this process runs as.
pub fn current_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Whether a connection may stay open. Logs a warning when it may not.
pub fn peer_allowed(owner_uid: u32, peer_uid: io::Result<u32>) -> bool {
    match peer_uid {
        Ok(uid) if uid == owner_uid => true,
        Ok(uid) => {
            tracing::warn!(
                peer_uid = uid,
                owner_uid,
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
pub fn restrict(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_owner_is_allowed() {
        assert!(peer_allowed(501, Ok(501)));
        assert!(!peer_allowed(501, Ok(502)));
        assert!(!peer_allowed(501, Ok(0)), "not even root");
        assert!(!peer_allowed(501, Err(io::Error::other("no creds"))));
    }

    #[test]
    fn current_uid_matches_owned_files() {
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(std::fs::metadata(tmp.path()).unwrap().uid(), current_uid());
    }
}
