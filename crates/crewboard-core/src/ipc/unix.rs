//! macOS and Linux: a Unix domain socket in the data folder, `0600`, with a peer-UID check on every connection.

use std::io;
use std::path::Path;

use tokio::net::{UnixListener, UnixStream};

pub type ServerStream = UnixStream;
pub type ClientStream = UnixStream;
/// An OS account: the UID.
pub type UserId = u32;

/// The service's listening socket.
pub struct Listener(UnixListener);

impl Listener {
    /// Listens at `path`, readable and writable only by the current user. The caller holds the service lock, so a
    /// socket file already there was left by a service that crashed and is replaced.
    pub fn bind(path: &Path) -> io::Result<Self> {
        if path.exists() {
            tracing::warn!(path = %path.display(), "replacing a stale socket");
            std::fs::remove_file(path)?;
        }
        let listener = UnixListener::bind(path)?;
        crate::service::access::restrict(path, 0o600)?;
        Ok(Self(listener))
    }

    pub async fn accept(&mut self) -> io::Result<ServerStream> {
        self.0.accept().await.map(|(stream, _)| stream)
    }
}

pub async fn connect(path: &Path) -> io::Result<ClientStream> {
    UnixStream::connect(path).await
}

/// The kernel's word on which process connected (CLI-01).
pub fn peer_pid(stream: &ServerStream) -> Option<u32> {
    stream
        .peer_cred()
        .ok()
        .and_then(|c| c.pid())
        .and_then(|p| u32::try_from(p).ok())
}

/// The account on the other end (`getpeereid` on macOS, `SO_PEERCRED` on Linux).
pub fn peer_user(stream: &ServerStream) -> io::Result<UserId> {
    Ok(stream.peer_cred()?.uid())
}

/// The account this process runs as.
pub fn current_user() -> UserId {
    rustix::process::getuid().as_raw()
}
