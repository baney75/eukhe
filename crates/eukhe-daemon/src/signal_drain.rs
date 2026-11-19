//! The supervisor's OS-signal drain loop (SIGTERM/SIGINT): the Codex
//! app-server's `shutdown_signal` + `ShutdownState` structure
//! (codex-rs/app-server/src/lib.rs) adapted to the multi-worker supervisor.
//!
//! [`Supervisor::begin_signal_drain`] is the state machine: the first
//! signal enters the graceful drain (new work refused at the gates, every
//! client told the daemon is closing, the running turns settled by each
//! worker's routed `shutdown` flush barrier, the workers retired so
//! nothing lingers in the supervisor-lost window); a later signal - or one
//! that finds a client-command shutdown, a committed update stop, or an
//! update exit already in flight - is the force request, and the forced
//! exit skips teardown entirely: runtime teardown can wait forever on
//! blocked worker I/O (the Codex `AppServerExit::Forced` rationale,
//! app-server/src/main.rs).
//!
//! [`install`] registers the handlers synchronously (the run loop calls it
//! directly before the socket binds) and returns the loop that serves
//! them: a signal cannot land in a spawn-to-first-poll window, or after a
//! client could reach the socket, with the default disposition still
//! active. A handler that fails to register is logged and dropped while
//! the other signal keeps draining; dropping a registered listener would
//! strand its signal (tokio keeps the replacement disposition installed
//! after the listener is gone, so a dropped stream swallows every later
//! delivery).
//!
//! On Windows the pair is Ctrl+Break (the terminate a console sends a
//! process group, `GenerateConsoleCtrlEvent`) and Ctrl+C. A daemon started
//! in its own process group ignores Ctrl+C from the launcher's console, so
//! the usual stop there is the `shutdown` request.

use std::sync::Arc;
#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

/// The terminate handler (SIGTERM; Windows: Ctrl+Break).
#[cfg(unix)]
fn terminate_handler() -> std::io::Result<tokio::signal::unix::Signal> {
    signal(SignalKind::terminate())
}

/// The interrupt handler (SIGINT; Windows: Ctrl+C).
#[cfg(unix)]
fn interrupt_handler() -> std::io::Result<tokio::signal::unix::Signal> {
    signal(SignalKind::interrupt())
}

/// The terminate handler (SIGTERM; Windows: Ctrl+Break).
#[cfg(windows)]
fn terminate_handler() -> std::io::Result<tokio::signal::windows::CtrlBreak> {
    tokio::signal::windows::ctrl_break()
}

/// The interrupt handler (SIGINT; Windows: Ctrl+C).
#[cfg(windows)]
fn interrupt_handler() -> std::io::Result<tokio::signal::windows::CtrlC> {
    tokio::signal::windows::ctrl_c()
}

/// A signal stream: one wait per delivery.
trait Delivery {
    async fn delivered(&mut self);
}

#[cfg(unix)]
impl Delivery for tokio::signal::unix::Signal {
    async fn delivered(&mut self) {
        let _ = self.recv().await;
    }
}

#[cfg(windows)]
impl Delivery for tokio::signal::windows::CtrlBreak {
    async fn delivered(&mut self) {
        let _ = self.recv().await;
    }
}

#[cfg(windows)]
impl Delivery for tokio::signal::windows::CtrlC {
    async fn delivered(&mut self) {
        let _ = self.recv().await;
    }
}

use crate::supervisor::Supervisor;

/// Install the SIGTERM/SIGINT handlers and return the drain loop that
/// serves them ([`Supervisor::run`] spawns it). Returns a loop that exits
/// immediately when neither handler could register: with no listener, the
/// signals keep their default disposition.
pub(crate) fn install(supervisor: Arc<Supervisor>) -> impl std::future::Future<Output = ()> + Send {
    let mut terminate = match terminate_handler() {
        Ok(terminate) => Some(terminate),
        Err(error) => {
            supervisor.log_line(&format!(
                "signal drain could not install the SIGTERM handler ({error}); SIGTERM keeps the default disposition"
            ));
            None
        }
    };
    let mut interrupt = match interrupt_handler() {
        Ok(interrupt) => Some(interrupt),
        Err(error) => {
            supervisor.log_line(&format!(
                "signal drain could not install the SIGINT handler ({error}); SIGINT keeps the default disposition"
            ));
            None
        }
    };
    async move {
        if terminate.is_none() && interrupt.is_none() {
            return;
        }
        loop {
            tokio::select! {
                () = recv_opt(terminate.as_mut()) => {}
                () = recv_opt(interrupt.as_mut()) => {}
            }
            if !supervisor.begin_signal_drain() {
                supervisor
                    .log_line("received shutdown signal while already shutting down; forcing exit");
                // Forced exit skips teardown: a worker wedged in its settle
                // must not hold the operator's second demand hostage.
                std::process::exit(0);
            }
        }
    }
}

/// Wait on one optional signal stream: an absent stream (a registration
/// failure) parks forever instead of spinning the loop.
async fn recv_opt(stream: Option<&mut impl Delivery>) {
    match stream {
        Some(stream) => stream.delivered().await,
        None => std::future::pending::<()>().await,
    }
}
