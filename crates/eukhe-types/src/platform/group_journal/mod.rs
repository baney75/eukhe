//! The process groups a session worker's commands run in, journaled while
//! they run so they die with the worker: Unix process groups (`unix`).
//! Windows has no process groups to journal or kill (`windows`): the
//! journal refuses to open, and a journal found there refuses to reap.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{open, reap, RunningGroup};
#[cfg(windows)]
pub use windows::{open, reap, RunningGroup};
