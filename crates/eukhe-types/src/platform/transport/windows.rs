//! The named-pipe transport (Windows).
//!
//! A listener creates `\\.\pipe\eukhe-<32 hex digits>` (128 random bits)
//! as the first instance of that name, refusing remote clients, and writes
//! the name into the endpoint file at the endpoint's path (eukhe's run dir
//! in `%LOCALAPPDATA%`, which only this user reads). A client reads the
//! name there and opens the pipe. No other user can learn the name before
//! the pipe exists, and once it exists no one else can create an instance
//! of it, so no one can squat on an endpoint.
//!
//! The peer rule is the pipe's default DACL (the transport module doc).
//!
//! A listener's accept creates the next instance before it hands out the
//! connected one: a client that connects meanwhile waits for it as for a
//! busy pipe. The endpoint file goes when the listener closes, if it still
//! names that listener's pipe.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::Result;
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use super::{
    AcceptFuture, AsyncReadHalf, AsyncWriteHalf, BlockingTransportStream, TransportListener,
    TransportStream,
};
use crate::platform::access::{PeerRule, PeerRules};

/// Whether eukhe's endpoints are files: on Windows a file at the path names
/// the endpoint's pipe.
pub const ENDPOINTS_ARE_FILES: bool = true;

/// The start of every pipe name eukhe creates; 32 lowercase hex digits
/// follow.
const PIPE_PREFIX: &str = r"\\.\pipe\eukhe-";

/// The Win32 errors a pipe's open and connect report (`winerror.h`).
const ERROR_PIPE_BUSY: i32 = 231;
const ERROR_NO_DATA: i32 = 232;
/// A first instance's create while the name has instances (`winerror.h`).
const ERROR_ACCESS_DENIED: i32 = 5;

/// How long an open waits for a busy pipe's next instance, and how often
/// it tries again.
const BUSY_WAIT: Duration = Duration::from_secs(2);
const BUSY_POLL: Duration = Duration::from_millis(10);

impl TransportStream for NamedPipeServer {
    fn split(self: Box<Self>) -> (Box<dyn AsyncReadHalf>, Box<dyn AsyncWriteHalf>) {
        split_pipe(*self)
    }
}

impl TransportStream for NamedPipeClient {
    fn split(self: Box<Self>) -> (Box<dyn AsyncReadHalf>, Box<dyn AsyncWriteHalf>) {
        split_pipe(*self)
    }
}

/// The two halves of one pipe end. A pipe has no half-close, so the
/// write half's shutdown or drop (a Unix write half's `shutdown(Write)`)
/// closes the pipe: the peer reads its end, and so does this read half,
/// at once.
fn split_pipe<P>(pipe: P) -> (Box<dyn AsyncReadHalf>, Box<dyn AsyncWriteHalf>)
where
    P: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let shared = Arc::new(Mutex::new(PipeEnd {
        pipe: Some(pipe),
        reader: None,
    }));
    (
        Box::new(PipeReadHalf(Arc::clone(&shared))),
        Box::new(PipeWriteHalf(shared)),
    )
}

/// A pipe end its halves share: the pipe until the write half closes it,
/// and the waker of a read waiting on it.
struct PipeEnd<P> {
    pipe: Option<P>,
    reader: Option<std::task::Waker>,
}

struct PipeReadHalf<P>(Arc<Mutex<PipeEnd<P>>>);

struct PipeWriteHalf<P>(Arc<Mutex<PipeEnd<P>>>);

impl<P> PipeWriteHalf<P> {
    /// Close the pipe and wake a waiting read, which then reads the end.
    fn close(&self) {
        let (pipe, reader) = {
            let mut end = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            (end.pipe.take(), end.reader.take())
        };
        drop(pipe);
        if let Some(reader) = reader {
            reader.wake();
        }
    }
}

impl<P> Drop for PipeWriteHalf<P> {
    fn drop(&mut self) {
        self.close();
    }
}

impl<P: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for PipeReadHalf<P> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let mut end = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let end = &mut *end;
        match end.pipe.as_mut() {
            None => std::task::Poll::Ready(Ok(())),
            Some(pipe) => {
                end.reader = Some(cx.waker().clone());
                std::pin::Pin::new(pipe).poll_read(cx, buf)
            }
        }
    }
}

impl<P: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for PipeWriteHalf<P> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let mut end = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match end.pipe.as_mut() {
            None => std::task::Poll::Ready(Err(closed_pipe())),
            Some(pipe) => std::pin::Pin::new(pipe).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let mut end = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match end.pipe.as_mut() {
            None => std::task::Poll::Ready(Err(closed_pipe())),
            Some(pipe) => std::pin::Pin::new(pipe).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.close();
        std::task::Poll::Ready(Ok(()))
    }
}

/// The error of a write after the write half closed its pipe.
fn closed_pipe() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "the write half closed this pipe",
    )
}

/// A new pipe name: [`PIPE_PREFIX`] and 128 random bits in hex.
fn new_pipe_name() -> std::io::Result<String> {
    use std::fmt::Write as _;
    let mut bits = [0u8; 16];
    getrandom::fill(&mut bits).map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut name = PIPE_PREFIX.to_owned();
    for byte in bits {
        // A write into a String cannot fail.
        let _ = write!(name, "{byte:02x}");
    }
    Ok(name)
}

/// The two 64-bit halves of the random part of a pipe name eukhe made;
/// `None` for any other text.
fn pipe_bits(name: &str) -> Option<(u64, u64)> {
    let digits = name.strip_prefix(PIPE_PREFIX)?;
    let lowercase_hex = digits
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if digits.len() != 32 || !lowercase_hex {
        return None;
    }
    let (high, low) = digits.split_at(16);
    Some((
        u64::from_str_radix(high, 16).ok()?,
        u64::from_str_radix(low, 16).ok()?,
    ))
}

/// The pipe the endpoint file at `path` names.
///
/// # Errors
///
/// The file does not read (`NotFound` when there is none), or names no
/// pipe eukhe made (`InvalidData`).
fn read_pipe_name(path: &Path) -> std::io::Result<String> {
    let text = std::fs::read_to_string(path)?;
    let name = text.trim_end();
    if pipe_bits(name).is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} names no Eukhe pipe", path.display()),
        ));
    }
    Ok(name.to_owned())
}

/// The random bits of the pipe the endpoint file at `path` names (an
/// endpoint's identity): `None` when no file there names one.
#[must_use]
pub(crate) fn endpoint_bits(path: &Path) -> Option<(u64, u64)> {
    read_pipe_name(path).ok().as_deref().and_then(pipe_bits)
}

/// Whether the file at `path` names a pipe eukhe made (live or left by a
/// crashed listener).
#[must_use]
pub(crate) fn is_endpoint_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
        && endpoint_bits(path).is_some()
}

/// Every path can name an endpoint file: Windows sets no address-length
/// limit on one.
#[must_use]
pub fn is_addressable(_path: &Path) -> bool {
    true
}

/// The endpoint `path` names: `path` itself (every path is addressable).
#[must_use]
pub fn endpoint_behind(path: &Path) -> std::borrow::Cow<'_, Path> {
    std::borrow::Cow::Borrowed(path)
}

/// The options of every instance of a listener's pipe: duplex bytes, local
/// clients only.
fn instance_options() -> ServerOptions {
    let mut options = ServerOptions::new();
    options.reject_remote_clients(true);
    options
}

/// A bound pipe endpoint: the instance waiting for the next client, and
/// the endpoint file naming the pipe.
#[derive(Debug)]
pub struct PipeListener {
    endpoint: PathBuf,
    name: String,
    /// The instance the next client connects to; the accept replaces it.
    pending: tokio::sync::Mutex<NamedPipeServer>,
}

impl PipeListener {
    /// Create a pipe of a new name and publish it at `path` ([`bind_tokio`]).
    ///
    /// # Errors
    ///
    /// The pipe cannot be created, a live listener's file is at `path`
    /// (`AddrInUse`), or the endpoint file cannot be written.
    pub fn bind(path: &Path) -> std::io::Result<Self> {
        let name = new_pipe_name()?;
        let first = instance_options().first_pipe_instance(true).create(&name)?;
        publish(path, &name)?;
        Ok(Self {
            endpoint: path.to_path_buf(),
            name,
            pending: tokio::sync::Mutex::new(first),
        })
    }

    /// The next connected client. A client that connected and left before
    /// the accept leaves its instance for the next one.
    ///
    /// # Errors
    ///
    /// The connect wait fails, or the next instance cannot be created (the
    /// connected one then waits for the next accept).
    pub async fn accept(&self) -> std::io::Result<NamedPipeServer> {
        let mut pending = self.pending.lock().await;
        loop {
            match pending.connect().await {
                Ok(()) => break,
                Err(error) if error.raw_os_error() == Some(ERROR_NO_DATA) => {
                    pending.disconnect()?;
                }
                Err(error) => return Err(error),
            }
        }
        let next = instance_options().create(&self.name)?;
        Ok(std::mem::replace(&mut *pending, next))
    }
}

impl Drop for PipeListener {
    fn drop(&mut self) {
        if read_pipe_name(&self.endpoint).is_ok_and(|name| name == self.name) {
            let _ = std::fs::remove_file(&self.endpoint);
        }
    }
}

/// Write `name` into the endpoint file at `path`, whole or not at all: a
/// staged file, hard-linked to `path`. A file already there that names a
/// live pipe refuses the bind; one whose pipe is gone (a crashed
/// listener's) is replaced.
fn publish(path: &Path, name: &str) -> std::io::Result<()> {
    let mut staged_name = path.file_name().unwrap_or_default().to_os_string();
    staged_name.push(format!(".{}.tmp", std::process::id()));
    let staged = path.with_file_name(staged_name);
    std::fs::write(&staged, format!("{name}\n"))?;
    let linked = std::fs::hard_link(&staged, path).or_else(|error| {
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error);
        }
        if endpoint_listens(path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                format!("a live Eukhe endpoint is at {}", path.display()),
            ));
        }
        std::fs::remove_file(path)?;
        std::fs::hard_link(&staged, path)
    });
    let removed = std::fs::remove_file(&staged);
    linked.and(removed)
}

/// Whether the endpoint file at `path` names a pipe a listener serves: a
/// first instance of its name is refused (access denied) while any
/// instance exists. The probe opens no connection, so the listener's
/// waiting instance never holds a client that already left.
fn endpoint_listens(path: &Path) -> bool {
    read_pipe_name(path).is_ok_and(|name| {
        match instance_options().first_pipe_instance(true).create(&name) {
            Ok(_) => false,
            Err(error) => error.raw_os_error() == Some(ERROR_ACCESS_DENIED),
        }
    })
}

/// A bound pipe whose accept hands out every client the pipe's DACL
/// admits.
struct PeerListener {
    listener: PipeListener,
}

impl TransportListener for PeerListener {
    fn accept(&self) -> AcceptFuture<'_> {
        Box::pin(async move {
            let stream = self.listener.accept().await?;
            Ok(Box::new(stream) as Box<dyn TransportStream>)
        })
    }
}

/// Bind a listening endpoint at `path`. The pipe's DACL takes the place of
/// `peers` (module doc).
///
/// # Errors
///
/// The pipe cannot be created, a live listener's file is at `path`, or
/// the endpoint file cannot be written.
pub async fn bind_transport(path: &Path, _peers: PeerRule) -> Result<Box<dyn TransportListener>> {
    let listener = bind_tokio(path)?;
    Ok(Box::new(PeerListener { listener }))
}

/// Bind the endpoint at `path` as a bare pipe listener (a test's stand-in
/// daemon); inside a tokio runtime.
///
/// # Errors
///
/// The pipe cannot be created, a live listener's file is at `path`, or
/// the endpoint file cannot be written.
pub fn bind_tokio(path: &Path) -> std::io::Result<PipeListener> {
    PipeListener::bind(path)
}

/// Connect to the endpoint at `path`: the pipe its file names, waiting up
/// to [`BUSY_WAIT`] while it is busy. No rule: the pipe's DACL decides who
/// serves it.
///
/// # Errors
///
/// No endpoint file is at `path` or its pipe is gone (`NotFound`), the
/// file names no Eukhe pipe, or the open fails.
pub async fn connect_tokio(path: &Path) -> std::io::Result<NamedPipeClient> {
    let name = read_pipe_name(path)?;
    let deadline = tokio::time::Instant::now() + BUSY_WAIT;
    loop {
        match ClientOptions::new().open(&name) {
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY)
                    && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(BUSY_POLL).await;
            }
            opened => return opened,
        }
    }
}

/// Connect to the endpoint at `path` asynchronously. The endpoint file in
/// this user's private dir and the pipe's DACL take the place of `rules`
/// (module doc).
///
/// # Errors
///
/// Nothing listens at `path`, or the open fails.
pub async fn connect_transport(path: &Path, _rules: PeerRules) -> Result<Box<dyn TransportStream>> {
    Ok(Box::new(connect_tokio(path).await?))
}

/// The pid of the process listening at the endpoint `path`: 0, as Windows
/// does not report a pipe server's pid to safe Rust; `None` when nothing
/// listens there.
///
/// # Errors
///
/// The open fails other than for want of a listener.
pub async fn listener_pid(path: &Path, _rules: PeerRules) -> std::io::Result<Option<u32>> {
    match connect_tokio(path).await {
        Ok(_stream) => Ok(Some(0)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// A blocking client stream over a pipe. A synchronous pipe handle
/// serializes its reads and writes (a pending read holds back every
/// write), so the stream drives an overlapped client on a runtime of its
/// own; clones share the client, the runtime and the read deadline, as
/// clones of a socket do.
#[derive(Debug, Clone)]
struct BlockingPipe {
    /// Declared before the runtime: it drops first, while its runtime runs.
    client: Arc<NamedPipeClient>,
    runtime: Arc<tokio::runtime::Runtime>,
    read_timeout: Arc<Mutex<Option<Duration>>>,
}

/// The refusal of a blocking pipe call made on a tokio runtime's thread,
/// where it cannot wait.
fn on_runtime_thread() -> std::io::Error {
    std::io::Error::other("a blocking pipe stream cannot wait on a tokio runtime's thread")
}

impl BlockingPipe {
    /// Run `io` on the stream's runtime until it completes.
    fn drive<T>(
        &self,
        io: impl std::future::Future<Output = std::io::Result<T>>,
    ) -> std::io::Result<T> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(on_runtime_thread());
        }
        self.runtime.block_on(io)
    }
}

impl std::io::Read for BlockingPipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let timeout = *self
            .read_timeout
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let client = Arc::clone(&self.client);
        self.drive(async move {
            let read = async {
                loop {
                    client.readable().await?;
                    match client.try_read(buf) {
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        read => return read,
                    }
                }
            };
            match timeout {
                None => read.await,
                // A read past its deadline fails as a socket's does.
                Some(timeout) => tokio::time::timeout(timeout, read)
                    .await
                    .unwrap_or_else(|_| Err(std::io::ErrorKind::WouldBlock.into())),
            }
        })
    }
}

impl std::io::Write for BlockingPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let client = Arc::clone(&self.client);
        self.drive(async move {
            loop {
                client.writable().await?;
                match client.try_write(buf) {
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    written => return written,
                }
            }
        })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl BlockingTransportStream for BlockingPipe {
    fn try_clone_box(&self) -> std::io::Result<Box<dyn BlockingTransportStream>> {
        Ok(Box::new(self.clone()))
    }

    fn set_read_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        if timeout.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "a zero read timeout",
            ));
        }
        *self
            .read_timeout
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(timeout);
        Ok(())
    }
}

/// Connect to the endpoint at `path`, blocking until connected. The
/// endpoint file and the pipe's DACL take the place of `rules` (module
/// doc). Off any tokio runtime's thread: the stream waits on a runtime of
/// its own.
///
/// # Errors
///
/// Nothing listens at `path`, the open fails, the stream's runtime cannot
/// start, or the caller runs on a tokio runtime's thread.
pub fn connect_blocking(
    path: &Path,
    _rules: PeerRules,
) -> std::io::Result<Box<dyn BlockingTransportStream>> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return Err(on_runtime_thread());
    }
    let name = read_pipe_name(path)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_io()
        .enable_time()
        .build()?;
    let client = {
        let _entered = runtime.enter();
        let deadline = std::time::Instant::now() + BUSY_WAIT;
        loop {
            match ClientOptions::new().open(&name) {
                Err(error)
                    if error.raw_os_error() == Some(ERROR_PIPE_BUSY)
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(BUSY_POLL);
                }
                opened => break opened?,
            }
        }
    };
    Ok(Box::new(BlockingPipe {
        client: Arc::new(client),
        runtime: Arc::new(runtime),
        read_timeout: Arc::new(Mutex::new(None)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::access::{PeerVerdict, ServerVerdict};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Dropping a write half closes the pipe: the peer reads its end, and
    /// so does the dropping side's read half.
    #[tokio::test]
    async fn a_dropped_write_half_ends_the_pipe_both_ways() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let endpoint = dir.path().join("d.sock");
        let listener = bind_tokio(&endpoint).expect("bind");
        let (server, client) = tokio::join!(listener.accept(), connect_tokio(&endpoint));
        let (mut server_reader, _server_writer) = Box::new(server.expect("accept")).split();
        let (mut client_reader, client_writer) = Box::new(client.expect("connect")).split();
        drop(client_writer);
        let mut seen = Vec::new();
        let peer = server_reader
            .read_to_end(&mut seen)
            .await
            .expect("peer read");
        let own = client_reader
            .read_to_end(&mut seen)
            .await
            .expect("own read");
        assert_eq!((peer, own), (0, 0));
    }

    fn any_peer(_: u32, _: u32) -> PeerVerdict {
        PeerVerdict::Accept
    }

    fn any_server(_: u32, _: u32, _: u32, _: u32) -> ServerVerdict {
        ServerVerdict::Served
    }

    /// Rules the pipe transport never consults.
    const ANY: PeerRules = PeerRules {
        peer: any_peer,
        server: any_server,
    };

    /// One frame from `from` arrives at `to`.
    async fn round_trip(
        from: Box<dyn TransportStream>,
        to: Box<dyn TransportStream>,
        frame: &[u8],
    ) {
        let (_from_reader, mut writer) = from.split();
        let (mut reader, _to_writer) = to.split();
        writer
            .write_all(frame)
            .await
            .unwrap_or_else(|error| panic!("write {frame:?}: {error}"));
        let mut buf = vec![0u8; frame.len()];
        reader
            .read_exact(&mut buf)
            .await
            .unwrap_or_else(|error| panic!("read {frame:?}: {error}"));
        assert_eq!(buf, frame);
    }

    /// A bound endpoint is a file naming a fresh pipe; a client reaches
    /// the listener and frames cross both ways; a second bind at the path
    /// is refused while the first listens, and the first still serves
    /// after the refusal's probe.
    #[tokio::test]
    async fn an_endpoint_binds_and_a_client_round_trips_a_frame() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let listener = bind_transport(&socket, any_peer).await.expect("bind");
        let name = read_pipe_name(&socket).expect("the endpoint file names the pipe");
        assert!(name.starts_with(PIPE_PREFIX), "{name}");
        let client = connect_transport(&socket, ANY).await.expect("connect");
        let server = listener.accept().await.expect("accept");
        round_trip(server, client, b"ping").await;
        let refused = bind_transport(&socket, any_peer)
            .await
            .err()
            .expect("refused");
        assert_eq!(
            refused
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::AddrInUse)
        );
        // The refusal's probe opened no connection: the first still
        // serves its next client.
        let (client, server) = tokio::join!(connect_transport(&socket, ANY), listener.accept());
        round_trip(client.expect("connect"), server.expect("accept"), b"pong").await;
    }

    /// A second client connects while the first is connected, and each
    /// keeps its own stream.
    #[tokio::test]
    async fn a_second_client_connects_while_the_first_is_connected() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let listener = bind_tokio(&socket).expect("bind");
        let first = connect_tokio(&socket).await.expect("first connect");
        let first_server = listener.accept().await.expect("first accept");
        let second = connect_tokio(&socket).await.expect("second connect");
        let second_server = listener.accept().await.expect("second accept");
        round_trip(Box::new(second_server), Box::new(second), b"two").await;
        round_trip(Box::new(first), Box::new(first_server), b"one").await;
    }

    /// A closed listener takes its endpoint file away, but not a file that
    /// names another listener's pipe; a file left by a crashed listener is
    /// nothing to connect to and gives way to the next bind.
    #[tokio::test]
    async fn the_endpoint_file_goes_with_its_listener_only() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let listener = bind_tokio(&socket).expect("bind");
        drop(listener);
        assert!(!socket.exists());

        let other = dir.path().join("other.sock");
        let other_listener = bind_tokio(&other).expect("bind other");
        let listener = bind_tokio(&socket).expect("rebind");
        std::fs::copy(&other, &socket).expect("name the other pipe");
        drop(listener);
        assert_eq!(
            read_pipe_name(&socket).ok(),
            read_pipe_name(&other).ok(),
            "the file names the other listener: it stays"
        );
        drop(other_listener);

        let stale = new_pipe_name().expect("name");
        std::fs::write(&socket, format!("{stale}\n")).expect("a crashed listener's file");
        assert_eq!(
            (
                connect_tokio(&socket).await.map(drop).map_err(|e| e.kind()),
                listener_pid(&socket, ANY).await.map_err(|e| e.kind()),
            ),
            (Err(std::io::ErrorKind::NotFound), Ok(None))
        );
        let listener = bind_tokio(&socket).expect("the stale file gives way");
        assert_ne!(read_pipe_name(&socket).ok(), Some(stale));
        drop(listener);
        assert!(!socket.exists());
    }

    /// A blocking client writes and reads through the pipe, its clone
    /// writes while it waits, and a read past its deadline fails as a
    /// socket's does.
    #[test]
    fn a_blocking_client_round_trips_and_honors_its_read_deadline() {
        use std::io::{Read, Write};
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let dir = tempfile::TempDir::new().expect("temp dir");
        let socket = dir.path().join("daemon.sock");
        let listener = runtime
            .block_on(async { bind_tokio(&socket) })
            .expect("bind");
        let echo = runtime.spawn(async move {
            let mut server = listener.accept().await.expect("accept");
            let mut buf = [0u8; 4];
            server.read_exact(&mut buf).await.expect("read");
            server.write_all(&buf).await.expect("echo");
            // Hold the pipe open, writing nothing more.
            let mut rest = Vec::new();
            server
                .read_to_end(&mut rest)
                .await
                .expect("read to the end");
        });
        let mut client = connect_blocking(&socket, ANY).expect("connect");
        let mut writer = client.try_clone_box().expect("clone");
        writer.write_all(b"echo").expect("write");
        let mut buf = [0u8; 4];
        client.read_exact(&mut buf).expect("read");
        assert_eq!(&buf, b"echo");
        client
            .set_read_timeout(Duration::from_millis(50))
            .expect("deadline");
        assert_eq!(
            client.read(&mut buf).map_err(|e| e.kind()),
            Err(std::io::ErrorKind::WouldBlock)
        );
        drop((client, writer));
        runtime.block_on(echo).expect("echo task");
    }
}
