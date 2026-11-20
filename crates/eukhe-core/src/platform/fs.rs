//! Sync primitives matched to each file's durability class: plain `fsync`
//! for append-only rows, and the directory sync a durable rename needs.

use std::fs::File;
use std::io;
use std::path::Path;

/// fsync a directory so a completed rename inside it survives a crash.
///
/// # Errors
///
/// Returns an error when the directory cannot be opened or synced.
#[cfg(unix)]
pub fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// A documented no-op on Windows: a directory opens as a file only with
/// backup semantics, and NTFS writes a rename to its own journal, so no
/// directory sync exists to call.
///
/// # Errors
///
/// None on Windows.
#[cfg(windows)]
pub fn sync_directory(path: &Path) -> io::Result<()> {
    let _ = path;
    Ok(())
}

/// Flush one file's written bytes with a plain `fsync(2)`; std's
/// `sync_data`/`sync_all` take the `F_FULLFSYNC` barrier on Apple, which
/// costs a drive-cache flush per append row.
///
/// # Errors
///
/// Returns the OS error when the sync fails.
#[cfg(unix)]
pub fn fsync(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    nix::unistd::fsync(file.as_raw_fd()).map_err(io::Error::from)
}

/// Flush one file's written bytes (`FlushFileBuffers` through std's
/// `sync_data`; Windows has no cheaper barrier to pick).
///
/// # Errors
///
/// Returns the OS error when the sync fails.
#[cfg(windows)]
pub fn fsync(file: &File) -> io::Result<()> {
    file.sync_data()
}

/// Commit one file's content, opened by path: POSIX fsyncs a read-only
/// descriptor.
///
/// # Errors
///
/// Returns the OS error of the open or the sync.
#[cfg(unix)]
pub fn sync_file(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// Commit one file's content, opened by path: `FlushFileBuffers` needs a
/// handle with write access, so the file opens for writing (appending
/// nothing). A read-only file (`fs::copy` carries the attribute over)
/// loses the attribute for the flush and gets it back after.
///
/// # Errors
///
/// Returns the OS error of the open, the sync or an attribute change.
#[cfg(windows)]
pub fn sync_file(path: &Path) -> io::Result<()> {
    let flush = || {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .sync_all()
    };
    let mut permissions = std::fs::metadata(path)?.permissions();
    if !permissions.readonly() {
        return flush();
    }
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions.clone())?;
    let flushed = flush();
    permissions.set_readonly(true);
    let restored = std::fs::set_permissions(path, permissions);
    flushed.and(restored)
}

/// Make `link` a symbolic link to the directory `target`, which need not
/// exist yet (Windows keeps directory and file links apart).
///
/// # Errors
///
/// Returns the OS error of the link.
pub fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

/// Make `link` a symbolic link to `target`.
///
/// # Errors
///
/// Returns the OS error of the link.
#[cfg(unix)]
pub fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Make `link` a symbolic link to `target`: a directory link when
/// `target` (read from `link`'s dir when relative) is a directory, else a
/// file link, as Windows keeps the two kinds apart. Making one needs the
/// symbolic link privilege or Developer Mode; without it the OS error
/// comes back.
///
/// # Errors
///
/// Returns the OS error of the link.
#[cfg(windows)]
pub fn symlink(target: &Path, link: &Path) -> io::Result<()> {
    let resolved = match link.parent() {
        Some(dir) if target.is_relative() => dir.join(target),
        _ => target.to_path_buf(),
    };
    if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

/// Rename `from` to `to` only when nothing is at `to`: the check and the
/// rename are one step of the kernel (`renameat2` `RENAME_NOREPLACE` on
/// Linux, `renameatx_np` `RENAME_EXCL` on macOS), so a rename never
/// replaces an entry, even an empty directory that appeared meanwhile.
///
/// # Errors
///
/// `AlreadyExists` when an entry is at `to`; else the OS error (also on a
/// file system that cannot rename without replacement).
#[cfg(unix)]
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
}

/// Rename `from` to `to` only when nothing is at `to`, on Windows. A file
/// moves as a hard link made at `to` (which fails on any entry there, in
/// one step) and then the old name's removal. A directory takes a look
/// for `to` and then the rename: std's rename can replace no directory,
/// so an entry that appears between the two fails the rename with the OS
/// error, never a replacement.
///
/// # Errors
///
/// `AlreadyExists` when an entry is at `to`; else the OS error (also on a
/// file system without hard links, such as FAT).
#[cfg(windows)]
pub fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    let source = std::fs::symlink_metadata(from)?;
    if source.is_dir() {
        if std::fs::symlink_metadata(to).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} already exists", to.display()),
            ));
        }
        return std::fs::rename(from, to);
    }
    std::fs::hard_link(from, to)?;
    // A source that cannot go (read-only, or open without delete sharing)
    // takes its new name back, so a retry finds `to` free again.
    std::fs::remove_file(from).inspect_err(|_| {
        let _ = std::fs::remove_file(to);
    })
}
