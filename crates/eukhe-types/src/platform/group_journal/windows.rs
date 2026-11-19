//! Windows: no Unix process groups, so no journal of them. A command's
//! tree there needs a job object, a Win32 handle safe Rust cannot make.

use std::path::Path;

/// The refusal of a journal of process groups on Windows.
fn no_process_groups() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the process group journal needs Unix process groups, which Windows lacks",
    )
}

/// Refuses: Windows has no process groups to journal.
///
/// # Errors
///
/// Always [`std::io::ErrorKind::Unsupported`].
pub fn open(_path: &Path) -> std::io::Result<()> {
    Err(no_process_groups())
}

/// A process group recorded running: on Windows no journal opens, so a
/// guard records nothing (as on Unix before [`open`]).
#[must_use = "the group is recorded only while the guard lives"]
pub struct RunningGroup;

impl RunningGroup {
    /// Records nothing: no journal is open on Windows.
    pub fn record(_pgid: u32) -> Self {
        Self
    }
}

/// A missing journal records nothing; a journal found at `path` refuses,
/// since Windows cannot kill the Unix process groups it names.
///
/// # Errors
///
/// [`std::io::ErrorKind::Unsupported`] when a journal is at `path`; the
/// error of the look otherwise.
pub fn reap(path: &Path) -> std::io::Result<Vec<u32>> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(no_process_groups()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_journal_refuses_and_a_missing_one_records_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.groups.jsonl");
        let _group = RunningGroup::record(7);
        assert_eq!(
            open(&path).map_err(|error| error.kind()),
            Err(std::io::ErrorKind::Unsupported)
        );
        assert_eq!(reap(&path).map_err(|error| error.kind()), Ok(Vec::new()));
        std::fs::write(&path, "{\"pgid\":7,\"running\":true}\n").unwrap();
        assert_eq!(
            reap(&path).map_err(|error| error.kind()),
            Err(std::io::ErrorKind::Unsupported)
        );
    }
}
