//! Cross-crate platform contracts: transport, process identity, local
//! access (own uid, private modes, socket peers), and home-dir resolution.
//!
//! eukhe-types is the only crate every platform consumer can depend on
//! (eukhe-tui depends on eukhe-types alone; eukhe-daemon, eukhe-cli, eukhe-core all sit
//! above it), so the shared platform traits live here: Unix sockets and
//! `/proc`/libproc process queries, with call sites never branching on
//! `cfg` themselves.

pub mod access;
pub mod daemon_socket;
pub mod dirs;
#[cfg(unix)]
pub mod endpoints;
pub mod group_journal;
#[cfg(unix)]
pub mod identity;
pub mod kill_point;
pub mod process;
pub mod terminal;
#[cfg(unix)]
pub mod transport;

pub use dirs::{agent_dir, home_dir};
#[cfg(unix)]
pub use identity::socket_identity;
pub use process::{
    ignore_sigint_for_suspend, is_process_alive, process_start_id, restore_default_sigint,
    stop_own_process_group,
};
#[cfg(unix)]
pub use transport::{
    bind_transport, connect_blocking, connect_transport, BlockingTransportStream,
    TransportListener, TransportStream,
};
