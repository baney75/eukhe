//! Node-compatible failures of native file system calls: the messages and
//! codes `node:fs` puts on its errors, and `toFileError` of `env/node.ts`,
//! which maps them to [`FileError`] codes.

use std::io;
use std::sync::Arc;

#[cfg(unix)]
use nix::errno::Errno;

use super::{FileError, FileErrorCode};

/// libuv's name and description of an OS error, as Node formats it
/// (`uv_err_name`, `uv_strerror`).
#[cfg(unix)]
pub(crate) fn uv_error_name(error: &io::Error) -> (&'static str, &'static str) {
    let Some(raw) = error.raw_os_error() else {
        return ("UNKNOWN", "unknown error");
    };
    match Errno::from_raw(raw) {
        Errno::E2BIG => ("E2BIG", "argument list too long"),
        Errno::EACCES => ("EACCES", "permission denied"),
        Errno::EAGAIN => ("EAGAIN", "resource temporarily unavailable"),
        Errno::EBADF => ("EBADF", "bad file descriptor"),
        Errno::EBUSY => ("EBUSY", "resource busy or locked"),
        Errno::ECANCELED => ("ECANCELED", "operation canceled"),
        Errno::EEXIST => ("EEXIST", "file already exists"),
        Errno::EFAULT => ("EFAULT", "bad address in system call argument"),
        Errno::EFBIG => ("EFBIG", "file too large"),
        Errno::EINTR => ("EINTR", "interrupted system call"),
        Errno::EINVAL => ("EINVAL", "invalid argument"),
        Errno::EIO => ("EIO", "i/o error"),
        Errno::EISDIR => ("EISDIR", "illegal operation on a directory"),
        Errno::ELOOP => ("ELOOP", "too many symbolic links encountered"),
        Errno::EMFILE => ("EMFILE", "too many open files"),
        Errno::EMLINK => ("EMLINK", "too many links"),
        Errno::ENAMETOOLONG => ("ENAMETOOLONG", "name too long"),
        Errno::ENFILE => ("ENFILE", "file table overflow"),
        Errno::ENODEV => ("ENODEV", "no such device"),
        Errno::ENOENT => ("ENOENT", "no such file or directory"),
        Errno::ENOEXEC => ("ENOEXEC", "exec format error"),
        Errno::ENOMEM => ("ENOMEM", "not enough memory"),
        Errno::ENOSPC => ("ENOSPC", "no space left on device"),
        Errno::ENOSYS => ("ENOSYS", "function not implemented"),
        Errno::ENOTDIR => ("ENOTDIR", "not a directory"),
        Errno::ENOTEMPTY => ("ENOTEMPTY", "directory not empty"),
        Errno::ENOTSUP => ("ENOTSUP", "operation not supported on socket"),
        Errno::ENXIO => ("ENXIO", "no such device or address"),
        Errno::EOVERFLOW => ("EOVERFLOW", "value too large for defined data type"),
        Errno::EPERM => ("EPERM", "operation not permitted"),
        Errno::EPIPE => ("EPIPE", "broken pipe"),
        Errno::EROFS => ("EROFS", "read-only file system"),
        Errno::ESPIPE => ("ESPIPE", "invalid seek"),
        Errno::ESRCH => ("ESRCH", "no such process"),
        Errno::ETIMEDOUT => ("ETIMEDOUT", "connection timed out"),
        Errno::ETXTBSY => ("ETXTBSY", "text file is busy"),
        Errno::EXDEV => ("EXDEV", "cross-device link not permitted"),
        _ => ("UNKNOWN", "unknown error"),
    }
}

/// `ERROR_CANT_RESOLVE_FILENAME`, which libuv reads as ELOOP.
#[cfg(windows)]
pub(crate) const ERROR_CANT_RESOLVE_FILENAME: i32 = 1921;

/// libuv's name and description of a Windows error, as Node formats it:
/// the Win32 code through libuv's `uv_translate_sys_error` table (the
/// codes file calls meet), then `uv_err_name`, `uv_strerror`.
#[cfg(windows)]
pub(crate) fn uv_error_name(error: &io::Error) -> (&'static str, &'static str) {
    match error.raw_os_error() {
        Some(1) => ("EISDIR", "illegal operation on a directory"),
        Some(2 | 3 | 123 | 161) => ("ENOENT", "no such file or directory"),
        Some(4) => ("EMFILE", "too many open files"),
        Some(5) => ("EPERM", "operation not permitted"),
        Some(6) => ("EBADF", "bad file descriptor"),
        Some(8 | 14) => ("ENOMEM", "not enough memory"),
        Some(17) => ("EXDEV", "cross-device link not permitted"),
        Some(19) => ("EROFS", "read-only file system"),
        Some(32 | 33) => ("EBUSY", "resource busy or locked"),
        Some(39 | 112) => ("ENOSPC", "no space left on device"),
        Some(50) => ("ENOTSUP", "operation not supported on socket"),
        Some(80 | 183) => ("EEXIST", "file already exists"),
        Some(87) => ("EINVAL", "invalid argument"),
        Some(109) => ("EOF", "end of file"),
        Some(121) => ("ETIMEDOUT", "connection timed out"),
        Some(145) => ("ENOTEMPTY", "directory not empty"),
        Some(206) => ("ENAMETOOLONG", "name too long"),
        Some(232) => ("EPIPE", "broken pipe"),
        Some(267) => ("ENOTDIR", "not a directory"),
        Some(995) => ("ECANCELED", "operation canceled"),
        Some(998 | 1920) => ("EACCES", "permission denied"),
        Some(ERROR_CANT_RESOLVE_FILENAME) => ("ELOOP", "too many symbolic links encountered"),
        Some(_) | None => ("UNKNOWN", "unknown error"),
    }
}

/// The Node error code of an OS error, e.g. `"ENOENT"`.
pub(crate) fn uv_code(error: &io::Error) -> &'static str {
    uv_error_name(error).0
}

/// The `FileErrorCode` `toFileError` gives a Node error with `code`.
fn file_error_code(code: &str) -> FileErrorCode {
    match code {
        "ABORT_ERR" => FileErrorCode::Aborted,
        "ENOENT" => FileErrorCode::NotFound,
        "EACCES" | "EPERM" => FileErrorCode::PermissionDenied,
        "ENOTDIR" => FileErrorCode::NotDirectory,
        "EISDIR" => FileErrorCode::IsDirectory,
        "EINVAL" => FileErrorCode::Invalid,
        _ => FileErrorCode::Unknown,
    }
}

/// `toFileError` of the error Node throws when `syscall` on `path` fails:
/// message `CODE: description, syscall 'path'`, `error.path` = `path`.
pub(crate) fn fs_error(error: io::Error, syscall: &str, path: &str) -> FileError {
    let (code, description) = uv_error_name(&error);
    let message = format!("{code}: {description}, {syscall} '{path}'");
    FileError::new(file_error_code(code), message, Some(path.to_owned()))
        .with_cause(Arc::new(error))
}

/// [`fs_error`] of a two-path call: message `CODE: description, syscall
/// 'path' -> 'dest'`, `error.path` = `path`.
pub(crate) fn fs_error_dest(error: io::Error, syscall: &str, path: &str, dest: &str) -> FileError {
    let (code, description) = uv_error_name(&error);
    let message = format!("{code}: {description}, {syscall} '{path}' -> '{dest}'");
    FileError::new(file_error_code(code), message, Some(path.to_owned()))
        .with_cause(Arc::new(error))
}

/// `toFileError(error, fallbackPath)` of a failed call on an open file
/// descriptor (`read`, `fstat`, `fsync`, ...), whose Node message names no
/// path: `CODE: description, syscall`.
pub(crate) fn fd_error(error: io::Error, syscall: &str, fallback_path: Option<&str>) -> FileError {
    let (code, description) = uv_error_name(&error);
    let message = format!("{code}: {description}, {syscall}");
    FileError::new(
        file_error_code(code),
        message,
        fallback_path.map(str::to_owned),
    )
    .with_cause(Arc::new(error))
}
