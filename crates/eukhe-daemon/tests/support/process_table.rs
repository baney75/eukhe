//! Portable process-table probes for the e2e tests: liveness, parent links
//! and command lines, the same on Linux, macOS and Windows. The census
//! runs `ps` on Unix (`-A -ww -o pid=,ppid=,args=` means the same to procps
//! and BSD `ps`), so no test reads `/proc` directly; Windows has no `ps`,
//! so there it reads the process table through sysinfo.

#![allow(dead_code)] // each including test binary uses its own subset

#[cfg(unix)]
use std::process::Command;

/// One process of the census.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub ppid: u32,
    /// The command line, arguments joined by spaces.
    pub args: String,
}

impl ProcessRow {
    /// Whether the command line holds `words` as consecutive arguments.
    pub fn runs(&self, words: &[&str]) -> bool {
        let args: Vec<&str> = self.args.split_whitespace().collect();
        args.windows(words.len()).any(|window| window == words)
    }
}

/// Every process the host lists.
#[cfg(unix)]
pub fn snapshot() -> Vec<ProcessRow> {
    let output = Command::new("ps")
        .args(["-A", "-ww", "-o", "pid=,ppid=,args="])
        .output()
        .expect("run ps");
    assert!(output.status.success(), "ps failed: {output:?}");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let parent = fields.next()?.parse().ok()?;
            Some(ProcessRow {
                pid,
                ppid: parent,
                args: fields.collect::<Vec<_>>().join(" "),
            })
        })
        .collect()
}

/// Every process the host lists, with the command line each one may be
/// read for (another user's stays empty, as `ps` leaves it unreadable).
#[cfg(windows)]
pub fn snapshot() -> Vec<ProcessRow> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    system
        .processes()
        .iter()
        .map(|(pid, process)| ProcessRow {
            pid: pid.as_u32(),
            ppid: process.parent().map_or(0, sysinfo::Pid::as_u32),
            args: process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect()
}

/// Pids whose parent is `ppid`.
pub fn child_pids_of(ppid: u32) -> Vec<u32> {
    snapshot()
        .into_iter()
        .filter(|row| row.ppid == ppid)
        .map(|row| row.pid)
        .collect()
}

/// Whether `pid` runs under `ancestor` in `table` (its parent chain
/// reaches it).
pub fn descends_from(table: &[ProcessRow], mut pid: u32, ancestor: u32) -> bool {
    while pid > 1 {
        let Some(row) = table.iter().find(|row| row.pid == pid) else {
            return false;
        };
        if row.ppid == ancestor {
            return true;
        }
        pid = row.ppid;
    }
    false
}

/// Liveness that ignores zombies (a child nobody reaps yet keeps its
/// process-table entry until its status is collected).
pub fn process_alive(pid: u32) -> bool {
    eukhe_types::platform::process::is_process_alive(pid).expect("liveness probe")
}
