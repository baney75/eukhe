//! A worker's socket on each platform: a Unix socket, or on Windows the
//! named pipe eukhe-types serves behind an endpoint file at the same path.

use std::io;
use std::path::Path;

/// The client's read half of a worker connection.
#[cfg(unix)]
pub(crate) type ClientRead = tokio::net::unix::OwnedReadHalf;
/// The client's write half of a worker connection.
#[cfg(unix)]
pub(crate) type ClientWrite = tokio::net::unix::OwnedWriteHalf;
/// The client's read half of a worker connection.
#[cfg(windows)]
pub(crate) type ClientRead = tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeClient>;
/// The client's write half of a worker connection.
#[cfg(windows)]
pub(crate) type ClientWrite =
    tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeClient>;

/// A bound worker socket.
#[cfg(unix)]
pub(crate) type Listener = tokio::net::UnixListener;
/// A bound worker socket.
#[cfg(windows)]
pub(crate) type Listener = eukhe_types::platform::transport::PipeListener;

/// The worker's side of one accepted connection.
#[cfg(unix)]
pub(crate) type Served = tokio::net::UnixStream;
/// The worker's side of one accepted connection.
#[cfg(windows)]
pub(crate) type Served = tokio::net::windows::named_pipe::NamedPipeServer;

/// Connects to the worker socket at `path`, split into its halves.
///
/// # Errors
///
/// The connect fails (no worker: not found or refused).
pub(crate) async fn connect(path: &Path) -> io::Result<(ClientRead, ClientWrite)> {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(path)
            .await
            .map(tokio::net::UnixStream::into_split)
    }
    #[cfg(windows)]
    {
        eukhe_types::platform::transport::connect_tokio(path)
            .await
            .map(tokio::io::split)
    }
}

/// The next connection to `listener`.
///
/// # Errors
///
/// The accept fails.
pub(crate) async fn accept(listener: &Listener) -> io::Result<Served> {
    #[cfg(unix)]
    {
        listener.accept().await.map(|(stream, _peer)| stream)
    }
    #[cfg(windows)]
    {
        listener.accept().await
    }
}

/// The read half of a served connection.
#[cfg(unix)]
pub(crate) type ServedRead = tokio::net::unix::OwnedReadHalf;
/// The write half of a served connection.
#[cfg(unix)]
pub(crate) type ServedWrite = tokio::net::unix::OwnedWriteHalf;
/// The read half of a served connection.
#[cfg(windows)]
pub(crate) type ServedRead = tokio::io::ReadHalf<Served>;
/// The write half of a served connection.
#[cfg(windows)]
pub(crate) type ServedWrite = tokio::io::WriteHalf<Served>;

/// A served connection's halves.
pub(crate) fn split(stream: Served) -> (ServedRead, ServedWrite) {
    #[cfg(unix)]
    {
        stream.into_split()
    }
    #[cfg(windows)]
    {
        tokio::io::split(stream)
    }
}
