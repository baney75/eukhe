//! Windows: no native watch backend. Safe Rust reaches no
//! `ReadDirectoryChangesW`, and no watcher crate is in the graph, so every
//! watcher compares snapshots ([`super::scan::any_unreliable`] answers
//! true). A watcher forced to native mode finds no watch to install and
//! falls back to snapshots, reporting the coverage as uncertain.

use std::io;

use super::{HandleId, Listener, Sink};

/// The watch backend that installs nothing.
pub(super) struct Backend;

impl Backend {
    pub(super) fn new(_sink: Sink) -> Self {
        Self
    }

    /// Refuses: Windows has no native watch here.
    pub(super) fn add(&mut self, _path: &str, _listener: Listener) -> io::Result<HandleId> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native file watching is not available on Windows; snapshots are compared",
        ))
    }

    /// Nothing is installed, so nothing is removed.
    pub(super) fn remove(&mut self, _handle: HandleId) {}

    /// Nothing is installed, so nothing closes.
    pub(super) fn close(&mut self) {}
}
