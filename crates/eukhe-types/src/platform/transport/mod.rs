//! Transport contract for eukhe's sockets and streams.
//!
//! Callers bind/connect through these functions and never name a concrete
//! socket type: `AF_UNIX` sockets on Unix (`unix`), named pipes on Windows
//! (`windows`). An endpoint is named by a path:
//!
//! - On Linux eukhe binds it as an abstract socket whose name is the path's
//!   bytes: no file, so no grant on a directory reaches it, and the kernel
//!   frees the name when the listener closes (`/proc/net/unix` shows it as
//!   `@<path>`). A Landlock domain scoped for abstract sockets cannot
//!   connect to it.
//! - On Windows a file at the path names the listener's pipe, a name of
//!   128 random bits.
//! - Elsewhere it is a socket file at the path.
//!
//! A connect reaches the abstract socket first (Linux) and, when none
//! listens, a socket file at the path: an earlier release's daemon, or
//! another program's socket.
//!
//! The Unix peer rule: both ends apply the compiled core's rules to the
//! other end. A listener drops a peer of another uid before a frame is
//! read. A client refuses a socket file's listener of another uid (the
//! file's private dir keeps other users out), and an abstract endpoint's
//! listener unless it runs as this uid and is the process recorded for the
//! endpoint: any user can bind a free abstract name first, so the listener
//! records its pid beside the name's path (`record_path`) once bound.
//!
//! The Windows peer rule: the pipe's default DACL. Only its creator,
//! Administrators and SYSTEM open it read-write; Everyone else may open it
//! read-only, so sends no frame and reads only what the listener writes
//! before a request. The pipe's name is in a file only this user reads,
//! and the listener holds the first instance of it, so no one squats on
//! it. Safe Rust can neither set a pipe's DACL nor ask a pipe for its
//! peer's process, so the compiled core's uid rules do not run there.

use std::future::Future;
use std::pin::Pin;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

/// A full-duplex stream between a client and a daemon endpoint.
///
/// `split` consumes the boxed stream into its owned halves; concrete socket
/// types implement this, and callers hold only the erased halves.
pub trait TransportStream: Send + Sync {
    fn split(self: Box<Self>) -> (Box<dyn AsyncReadHalf>, Box<dyn AsyncWriteHalf>);
}

/// Owned read half of a [`TransportStream`]; blanket-implemented.
pub trait AsyncReadHalf: tokio::io::AsyncRead + Unpin + Send {}
impl<T> AsyncReadHalf for T where T: tokio::io::AsyncRead + Unpin + Send {}

/// Owned write half of a [`TransportStream`]; blanket-implemented.
pub trait AsyncWriteHalf: tokio::io::AsyncWrite + Unpin + Send {}
impl<T> AsyncWriteHalf for T where T: tokio::io::AsyncWrite + Unpin + Send {}

/// Future returned by [`TransportListener::accept`].
pub type AcceptFuture<'a> =
    Pin<Box<dyn Future<Output = std::io::Result<Box<dyn TransportStream>>> + Send + 'a>>;

/// A bound transport endpoint that hands out connected streams.
pub trait TransportListener: Send + Sync {
    /// Future-boxed so the trait stays dyn-compatible (RPITIT methods are not);
    /// the future borrows the listener for the duration of the accept.
    fn accept(&self) -> AcceptFuture<'_>;
}

/// A blocking full-duplex stream, for the CLI's one-shot command client.
pub trait BlockingTransportStream:
    std::fmt::Debug + std::io::Read + std::io::Write + Send + Sync
{
    /// Duplicate the underlying handle so reads and writes can proceed on
    /// separate owned halves.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying handle cannot be duplicated.
    fn try_clone_box(&self) -> std::io::Result<Box<dyn BlockingTransportStream>>;
    /// Deadline a pending read (poll granularity for deadline-driven waits).
    ///
    /// # Errors
    ///
    /// Returns an error if setting the read deadline on the underlying
    /// stream fails.
    fn set_read_timeout(&self, timeout: std::time::Duration) -> std::io::Result<()>;
}
