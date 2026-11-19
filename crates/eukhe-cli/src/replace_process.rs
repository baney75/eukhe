//! Start a program in place of this process: `exec` on Unix (same pid,
//! terminal and environment). Windows has no `exec`: the program runs as
//! a child that inherits this process's console and handles, and this
//! process exits with its exit code once it ends, so a caller that waits
//! on this pid still sees the replacement's end and code.

use std::process::Command;

/// Run `command` in place of this process. Returns only when the start
/// failed.
#[cfg(unix)]
pub(crate) fn replace_process(command: &mut Command) -> std::io::Error {
    use std::os::unix::process::CommandExt as _;
    command.exec()
}

/// Run `command` in place of this process (module doc: a child this
/// process waits on, then exits with its code; 1 when it ended without
/// one). Returns only when the start failed.
#[cfg(windows)]
pub(crate) fn replace_process(command: &mut Command) -> std::io::Error {
    match command.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => error,
    }
}
