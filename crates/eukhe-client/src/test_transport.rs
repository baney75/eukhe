//! The daemon and worker side of the transport in this crate's tests: a
//! listener at a socket path, its accepted stream, and a stale endpoint.
//!
//! On Unix the listener is tokio's Unix listener itself. On Windows it is
//! the named-pipe listener of `eukhe_types::platform::transport`, which
//! publishes its pipe in an endpoint file at the path, behind the same
//! `bind`, `accept` and `into_split` the tests call.

#[cfg(unix)]
pub(crate) use tokio::net::UnixListener as Listener;

/// Leave a stale endpoint at `path`: one a client can find and identify
/// but nobody serves (a bound, then closed, Unix socket).
#[cfg(unix)]
pub(crate) fn stale_endpoint(path: &std::path::Path) {
    std::os::unix::net::UnixListener::bind(path).expect("bind a stale socket");
}

#[cfg(windows)]
pub(crate) use self::windows::{stale_endpoint, Listener};

#[cfg(windows)]
mod windows {
    use std::io;
    use std::path::Path;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::task::{Context, Poll};

    use eukhe_types::platform::transport::PipeListener;
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, WriteHalf};
    use tokio::net::windows::named_pipe::NamedPipeServer;

    /// A pipe listener with the Unix listener's test surface.
    pub(crate) struct Listener(PipeListener);

    impl Listener {
        /// Create a pipe and publish it at `path`.
        pub(crate) fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
            PipeListener::bind(path.as_ref()).map(Self)
        }

        /// The next client, with its peer address: a pipe client has none
        /// (as a Unix accept's unnamed one).
        pub(crate) async fn accept(&self) -> io::Result<(Stream, NoAddress)> {
            self.0
                .accept()
                .await
                .map(|server| (Stream(server), NoAddress))
        }
    }

    /// A pipe client's peer address: none.
    pub(crate) struct NoAddress;

    /// One accepted client.
    pub(crate) struct Stream(NamedPipeServer);

    impl Stream {
        /// Its read and write halves.
        pub(crate) fn into_split(self) -> (ReadHalf<Self>, WriteHalf<Self>) {
            tokio::io::split(self)
        }
    }

    impl AsyncRead for Stream {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for Stream {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Pin::new(&mut self.0).poll_write(cx, buf)
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_flush(cx)
        }

        fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Pin::new(&mut self.0).poll_shutdown(cx)
        }
    }

    /// Leave a stale endpoint at `path`: an endpoint file naming a pipe
    /// nobody serves (a crashed listener's). Each call names its own pipe
    /// (the process id, then a count with its top bit set), so no two
    /// share an identity.
    pub(crate) fn stale_endpoint(path: &Path) {
        static COUNT: AtomicU64 = AtomicU64::new(1 << 63);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let name = format!(r"\\.\pipe\eukhe-{pid:016x}{count:016x}");
        std::fs::write(path, format!("{name}\n")).expect("write a stale endpoint file");
    }
}
