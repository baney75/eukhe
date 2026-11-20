//! The directory sync a durable create or rename needs, on every platform.

use std::io;
use std::path::Path;

/// fsync the directory `path`, so an entry made or renamed in it survives
/// a crash.
///
/// # Errors
///
/// The OS error when the directory cannot be opened or synced.
#[cfg(unix)]
pub fn sync_directory(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

/// A documented no-op on Windows: a directory opens as a file only with
/// backup semantics, and NTFS writes a create or rename to its own
/// journal, so no directory sync exists to call.
///
/// # Errors
///
/// None on Windows.
#[cfg(windows)]
pub fn sync_directory(path: &Path) -> io::Result<()> {
    let _ = path;
    Ok(())
}
