//! Windows: a console keeps its modes per handle and has no termios line
//! discipline, so there is no raw termios state to verify and no Ctrl+S
//! output stop of a line discipline to lift.

use super::TtyCooked;

/// No termios output stop exists on Windows: nothing to lift (the Unix
/// contract's "no tty" case).
pub fn restart_output() {}

/// No termios access on Windows: [`TtyCooked::Unavailable`].
#[must_use]
pub fn ensure_cooked_tty() -> TtyCooked {
    TtyCooked::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_windows_console_has_no_termios_to_verify() {
        restart_output();
        assert_eq!(ensure_cooked_tty(), TtyCooked::Unavailable);
    }
}
