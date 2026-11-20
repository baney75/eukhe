//! Windows process control. No signals, process groups or pidfds there:
//! a stop ends the process at once (`taskkill /F`; a console process has
//! no graceful stop a safe call can send), a process group is named by its
//! leader's pid (a child started with `CREATE_NEW_PROCESS_GROUP`) and
//! killed as that leader's process tree (`taskkill /T`), and an exit wait
//! polls liveness with the start identity, so a reused pid never passes
//! for the waited process.

use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::Duration;

use eukhe_types::platform::process::{
    is_process_alive, kill_process, kill_process_tree, process_start_id, CREATE_NEW_PROCESS_GROUP,
};

use super::{PidfdSignal, Signal, STARTUP_STDERR};

/// How often [`wait_for_exit`] looks again: Windows gives safe Rust no
/// exit notification for a process it did not start.
const EXIT_POLL: Duration = Duration::from_millis(250);

/// A kernel-held process handle: none exists on Windows ([`open_pidfd`]
/// always refuses), so no value of this type is ever made.
#[derive(Debug)]
pub enum ProcessHandle {}

/// Start the child in its own process group, so a console Ctrl+C never
/// reaches it and a group kill ([`signal_process_group`]) ends its tree.
pub fn set_new_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

/// The child half of
/// [`request_new_session`](eukhe_types::platform::process::request_new_session):
/// on Windows the launcher's creation flags already gave the child its own
/// group and hidden console, so nothing is left to do.
///
/// # Errors
///
/// None on Windows.
pub fn enter_requested_session() -> std::io::Result<()> {
    Ok(())
}

/// Point stderr at `NUL` when it is the launcher's startup pipe
/// ([`super::claim_startup_stderr`]); otherwise do nothing. std writes
/// stderr through the process's standard handle, which the override
/// replaces for the process's life. The pipe's handle stays open (the
/// override keeps it), so the launcher's reader ends at the daemon's exit.
///
/// # Errors
///
/// Opening `NUL` or replacing the standard handle failed.
pub fn release_startup_stderr() -> std::io::Result<()> {
    if !STARTUP_STDERR.swap(false, Ordering::SeqCst) {
        return Ok(());
    }
    let null = std::fs::OpenOptions::new().write(true).open("NUL")?;
    let replaced = stdio_override::StderrOverride::from_io(null)?;
    // The override holds for the process's life: its drop would put the
    // closed pipe back.
    std::mem::forget(replaced);
    Ok(())
}

/// Windows sets no soft open-handle limit to raise.
///
/// # Errors
///
/// Always [`std::io::ErrorKind::Unsupported`].
pub fn raise_open_file_limit() -> std::io::Result<u64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows has no open-file limit to raise",
    ))
}

/// End a single pid (both signals end it at once). True only when the
/// kill succeeded, proving the pid was alive.
#[must_use]
pub fn kill_pid(pid: i32, signal: Signal) -> bool {
    match signal {
        Signal::Term | Signal::Kill => {}
    }
    u32::try_from(pid)
        .ok()
        .filter(|pid| *pid > 0)
        .is_some_and(|pid| kill_process(pid).is_ok())
}

/// End the group led by `pgid`: the leader's process tree (both signals
/// end it at once). True only when the kill succeeded.
#[must_use]
pub fn signal_process_group(pgid: i32, signal: Signal) -> bool {
    match signal {
        Signal::Term | Signal::Kill => {}
    }
    u32::try_from(pgid)
        .ok()
        .filter(|pgid| *pgid > 0)
        .is_some_and(|pgid| kill_process_tree(pgid).is_ok())
}

/// The group a live pid names: its own pid (a group on Windows is named
/// by its leader, and the tree kill reaches what it started), or `None`
/// when the pid is gone or not observable.
#[must_use]
pub fn process_group_id(pid: u32) -> Option<i32> {
    let raw = i32::try_from(pid).ok().filter(|raw| *raw > 0)?;
    pid_exists(pid).then_some(raw)
}

/// End a process and its tree. True when the kill succeeded.
#[must_use]
pub fn kill_process_group_or_pid(pid: i32) -> bool {
    signal_process_group(pid, Signal::Kill)
}

/// Whether a process has the pid (`tasklist`); a failed look counts as no.
#[must_use]
pub fn pid_exists(pid: u32) -> bool {
    is_process_alive(pid).unwrap_or(false)
}

/// Windows has no pidfd: the handle is unobtainable.
///
/// # Errors
///
/// Always `Unsupported`.
pub fn open_pidfd(_pid: u32) -> std::io::Result<ProcessHandle> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// A signal through a handle: no handle exists on Windows.
#[must_use]
pub fn pidfd_signal(handle: &ProcessHandle, _signal: Signal) -> PidfdSignal {
    match *handle {}
}

/// Resolve once the process behind `pid` has exited: liveness polled
/// every 250 ms (no safe exit notification), ended early by a start
/// identity that changed (the pid now names another process). A pid that
/// names no process resolves at once.
///
/// # Errors
///
/// None on Windows: a liveness look that fails counts as an exit.
pub async fn wait_for_exit(pid: u32) -> std::io::Result<()> {
    let identity = process_start_id(pid);
    loop {
        let expected = identity.clone();
        let alive = tokio::task::spawn_blocking(move || {
            pid_exists(pid) && process_start_id(pid) == expected
        })
        .await
        .unwrap_or(false);
        if !alive {
            return Ok(());
        }
        tokio::time::sleep(EXIT_POLL).await;
    }
}

/// A Windows exit carries no signal.
#[must_use]
pub fn termination_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_killed_group_is_gone_and_its_exit_wait_resolves() {
        let mut command = Command::new("cmd");
        command
            .args(["/C", "pause"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null());
        set_new_process_group(&mut command);
        let mut child = command.spawn().expect("spawn cmd");
        let pid = child.id();
        let group = process_group_id(pid).expect("a live leader names its group");
        assert!(signal_process_group(group, Signal::Term));
        child.wait().expect("wait");
        drop(child);
        wait_for_exit(pid).await.expect("the exit wait");
        assert_eq!((pid_exists(pid), process_group_id(pid)), (false, None));
    }

    /// The bash tool's abort: a group kill of a Git Bash command ends the
    /// command bash started too, so its stdout pipe closes and the wait
    /// returns, well before the command's own end.
    #[test]
    fn a_group_kill_of_bash_closes_its_pipe() {
        use std::io::Read as _;
        let bash = crate::platform::shell::posix_shell_program("bash").expect("Git Bash");
        let mut command = Command::new(bash);
        command
            .args(["-c", "echo started; sleep 30"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped());
        set_new_process_group(&mut command);
        let mut child = command.spawn().expect("spawn bash");
        let mut stdout = child.stdout.take().expect("stdout");
        let mut started = [0u8; 8];
        stdout.read_exact(&mut started).expect("bash started");
        let begun = std::time::Instant::now();
        let pid = i32::try_from(child.id()).expect("pid");
        assert!(kill_process_group_or_pid(pid), "the group kill");
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut rest = Vec::new();
            let _ = sender.send(stdout.read_to_end(&mut rest).map(|_| rest));
        });
        let ended = receiver.recv_timeout(Duration::from_secs(10));
        assert!(
            ended.is_ok(),
            "the pipe stayed open {:?} after the kill",
            begun.elapsed()
        );
        child.wait().expect("wait");
    }
}

/// A child's process tree, held in a Job Object that ends every process in
/// it when the job closes: the child is assigned right after its spawn, and
/// whatever it starts joins the job too. Ending it ([`ProcessJob::end`], or
/// a drop) kills the tree even when its root is already gone, which a
/// `taskkill /T` of the root cannot reach.
#[derive(Debug)]
pub struct ProcessJob(Option<win32job::Job>);

impl ProcessJob {
    /// Put `child` in a new kill-on-close job. A child the job cannot take
    /// (already gone, or a job error) gets an empty guard: the caller's
    /// own kill still runs.
    #[must_use]
    pub fn contain(child: &tokio::process::Child) -> Self {
        let job = child.raw_handle().and_then(|handle| {
            let mut limits = win32job::ExtendedLimitInfo::new();
            limits.limit_kill_on_job_close();
            let job = win32job::Job::create_with_limit_info(&limits).ok()?;
            // The handle is the child's own, borrowed for the call.
            job.assign_process(handle as isize).ok()?;
            Some(job)
        });
        Self(job)
    }

    /// End every process in the job now.
    pub fn end(&mut self) {
        self.0.take();
    }

    /// Let the tree outlive the guard, as a process group outlives its
    /// leader's wait on Unix: the job stops killing on close first.
    pub fn release(mut self) {
        if let Some(job) = self.0.take() {
            // A job whose limit stays set still closes: the tree then ends,
            // the safe side of a failed release.
            let _ = job.set_extended_limit_info(&win32job::ExtendedLimitInfo::new());
        }
    }
}
