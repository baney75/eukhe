//! Tty line-discipline verification (the platform wall for the dying-
//! surface restore): termios on Unix (`unix`); Windows consoles have no
//! termios (`windows`).

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{ensure_cooked_tty, restart_output};
#[cfg(windows)]
pub use windows::{ensure_cooked_tty, restart_output};

/// The outcome of a cooked-tty verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtyCooked {
    /// The tty already reads cooked; nothing changed.
    AlreadyCooked,
    /// The tty read raw and the sane reconstruction was applied.
    Repaired,
    /// No tty or no termios access: nothing to verify (the headless
    /// harness, a dead terminal, a Windows console).
    Unavailable,
}
