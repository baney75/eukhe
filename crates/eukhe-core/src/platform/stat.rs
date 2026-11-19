//! The stat fields eukhe compares to see that a file changed: device,
//! inode, mode, modification time and change time, read the same way on
//! each platform.
//!
//! Windows shows no device or inode number through std (the volume serial
//! and file index are behind an unstable API), no inode change time and
//! no mode: there `dev` and `ino` read 0, `mode` reads 1 for a read-only
//! file else 0, and `changed` is the creation time. A rename that
//! replaces a file there is seen through its new modification time and
//! length, and an attribute change not at all (with the creation time,
//! tunneling can hand a replacement made within 15 seconds the old
//! name's creation time).

use std::fs::Metadata;
#[cfg(any(windows, test))]
use std::time::{SystemTime, UNIX_EPOCH};

/// One file's stat fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatFields {
    /// The device number (0 on Windows).
    pub dev: u64,
    /// The inode number (0 on Windows).
    pub ino: u64,
    /// The mode bits (Windows: 1 when read-only, else 0).
    pub mode: u32,
    /// The modification time: seconds and nanoseconds since the epoch.
    pub modified: (i64, i64),
    /// The inode change time (Windows: the creation time), as `modified`.
    pub changed: (i64, i64),
}

/// The stat fields of `metadata`.
#[cfg(unix)]
#[must_use]
pub fn stat_fields(metadata: &Metadata) -> StatFields {
    use std::os::unix::fs::MetadataExt;
    StatFields {
        dev: metadata.dev(),
        ino: metadata.ino(),
        mode: metadata.mode(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    }
}

/// The stat fields of `metadata`.
#[cfg(windows)]
#[must_use]
pub fn stat_fields(metadata: &Metadata) -> StatFields {
    StatFields {
        dev: 0,
        ino: 0,
        mode: u32::from(metadata.permissions().readonly()),
        modified: since_epoch(metadata.modified().ok()),
        changed: since_epoch(metadata.created().ok()),
    }
}

/// Seconds and nanoseconds of `time` from the epoch, negative before it;
/// `(0, 0)` without a time.
#[cfg(any(windows, test))]
fn since_epoch(time: Option<SystemTime>) -> (i64, i64) {
    let Some(time) = time else {
        return (0, 0);
    };
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => (
            i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
            i64::from(after.subsec_nanos()),
        ),
        Err(before) => {
            let before = before.duration();
            let seconds = i64::try_from(before.as_secs()).unwrap_or(i64::MAX);
            match before.subsec_nanos() {
                0 => (-seconds, 0),
                nanos => (-seconds - 1, 1_000_000_000 - i64::from(nanos)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_before_the_epoch_reads_as_whole_seconds_down_and_nanoseconds_up() {
        let before = UNIX_EPOCH - std::time::Duration::new(1, 250_000_000);
        assert_eq!(since_epoch(Some(before)), (-2, 750_000_000));
        let after = UNIX_EPOCH + std::time::Duration::new(3, 5);
        assert_eq!(since_epoch(Some(after)), (3, 5));
        assert_eq!(since_epoch(None), (0, 0));
    }

    #[test]
    fn a_rewrite_changes_the_stat_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("f");
        std::fs::write(&path, "one").expect("write");
        let first = stat_fields(&std::fs::metadata(&path).expect("metadata"));
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&path, "two!").expect("rewrite");
        let second = stat_fields(&std::fs::metadata(&path).expect("metadata"));
        assert_ne!(first, second);
    }
}
