//! File permission policy: owner-only mode bits (0o600 files, 0o700 dirs,
//! the constants of `eukhe_types::platform::access`).

use std::fs::OpenOptions;
use std::path::Path;

use eukhe_types::platform::access::{PRIVATE_DIR_MODE, PRIVATE_FILE_MODE};
use eukhe_types::platform::mode::{mode_of, set_file_mode, set_mode, CreationMode};

/// Make a file owner-readable/writable only (`chmod 0o600`). Best-effort:
/// callers decide whether a failure is fatal.
///
/// # Errors
///
/// Returns the underlying I/O error when the permission change fails.
pub fn restrict_file(path: &Path) -> std::io::Result<()> {
    set_mode(path, PRIVATE_FILE_MODE)
}

/// Make a directory owner-accessible only (`chmod 0o700`).
///
/// # Errors
///
/// Returns the underlying I/O error when the permission change fails.
pub fn restrict_dir(path: &Path) -> std::io::Result<()> {
    set_mode(path, PRIVATE_DIR_MODE)
}

/// Set the private mode on files created through these options.
pub fn set_private_mode(options: &mut OpenOptions) {
    options.creation_mode(PRIVATE_FILE_MODE);
}

/// The file's mode bits (`mode & 0o777`); None when the metadata read fails
/// (and always on Windows, which has no mode bits).
#[must_use]
pub fn file_mode(path: &Path) -> Option<u32> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| mode_of(&m))
        .map(|mode| mode & 0o777)
}

/// True when the path is an executable file (any execute bit).
#[cfg(unix)]
#[must_use]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

/// True when the path is a file Windows runs by name: no execute bit
/// exists there, so the extension decides (`.exe`, `.com`, `.bat`,
/// `.cmd`, the default `PATHEXT` launchers).
#[cfg(windows)]
#[must_use]
pub fn is_executable(path: &Path) -> bool {
    let launchable = path.extension().is_some_and(|ext| {
        ["exe", "com", "bat", "cmd"]
            .iter()
            .any(|known| ext.eq_ignore_ascii_case(known))
    });
    launchable && std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// True when the current process may read and write the file (access(2)
/// semantics: real/effective uid checks, not just the file mode).
#[cfg(unix)]
#[must_use]
pub fn is_readable_writable(path: &Path) -> bool {
    nix::unistd::access(
        path,
        nix::unistd::AccessFlags::R_OK | nix::unistd::AccessFlags::W_OK,
    )
    .is_ok()
}

/// True when the file exists and is not read-only. Windows has no
/// `access(2)`: the read-only attribute is the one write bar a metadata
/// read shows (an ACL that denies the user still fails the later open).
#[cfg(windows)]
#[must_use]
pub fn is_readable_writable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| !m.permissions().readonly())
}

/// Set the private mode on an already-open file (`fchmod`): exact bits despite
/// the umask, and tightens a pre-existing loose file. Callers decide whether a
/// failure is fatal.
///
/// # Errors
///
/// Returns the underlying I/O error when the permission bits cannot be set.
pub fn restrict_open_file(file: &std::fs::File) -> std::io::Result<()> {
    set_file_mode(file, PRIVATE_FILE_MODE)
}

/// Create directories recursively with the private dir mode; existing
/// directories are left untouched (mkdir semantics).
///
/// # Errors
///
/// Returns the underlying I/O error when a directory cannot be created.
pub fn create_dir_all_private(path: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .creation_mode(PRIVATE_DIR_MODE)
        .recursive(true)
        .create(path)
}
