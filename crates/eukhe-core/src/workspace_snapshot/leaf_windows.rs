//! The leaf open of a snapshot capture on Windows. std has no `openat`
//! there, so the walk pins each ancestor instead: the root and every ancestor directory
//! is opened without following a reparse point and without
//! `FILE_SHARE_DELETE`, so while the capture holds it no one can rename it,
//! delete it or swap a link in at its name. Each held handle is checked to
//! be a real directory, and the leaf is opened without following a reparse
//! point and checked through its own handle, so a link swapped in after a
//! look is refused, never followed.

use std::fs::{File, Metadata, OpenOptions};
use std::io::Read as _;
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::Path;

use super::OpenLeaf;

/// Opens a directory handle (`CreateFileW` needs it for any directory).
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
/// Opens a symbolic link or junction itself, never its target.
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_SHARE_DELETE: u32 = 0x4;

/// Open `path` beneath `root` without following a symbolic link at any
/// step: an ancestor link answers [`OpenLeaf::AncestorSymlink`], a leaf
/// link its target (with `/` separators, as git records it). No file has
/// an execute bit on Windows: `executable` is false, and a clean leaf
/// takes HEAD's mode.
pub(super) fn open_leaf(
    root: &Path,
    path: &str,
    max_bytes: usize,
    classify_only: bool,
) -> std::io::Result<OpenLeaf> {
    let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
    let Some((leaf, ancestors)) = components.split_last() else {
        return Ok(OpenLeaf::Missing);
    };
    // Held until the leaf is read: each pins its directory's name. The
    // root is pinned too, and like Unix's O_NOFOLLOW open of it, a link
    // there fails the capture.
    let mut pinned = Vec::with_capacity(ancestors.len() + 1);
    let held_root = open_no_follow(root, FILE_SHARE_READ | FILE_SHARE_WRITE)?;
    let meta = held_root.metadata()?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(std::io::Error::other(format!(
            "the snapshot root {} is not a real directory",
            root.display()
        )));
    }
    pinned.push(held_root);
    let mut dir = root.to_path_buf();
    for component in ancestors {
        dir.push(component);
        let handle = match open_no_follow(&dir, FILE_SHARE_READ | FILE_SHARE_WRITE) {
            Ok(handle) => handle,
            Err(error) if is_missing(&error) => return Ok(OpenLeaf::Missing),
            Err(error) => return Err(error),
        };
        let meta = handle.metadata()?;
        if meta.file_type().is_symlink() {
            return Ok(OpenLeaf::AncestorSymlink);
        }
        if !meta.is_dir() {
            return Ok(OpenLeaf::Missing);
        }
        pinned.push(handle);
    }
    let leaf = dir.join(leaf);
    let file = match open_no_follow(
        &leaf,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
    ) {
        Ok(file) => file,
        Err(error) if is_missing(&error) => return Ok(OpenLeaf::Missing),
        Err(error) => return Err(error),
    };
    let meta: Metadata = file.metadata()?;
    if meta.file_type().is_symlink() {
        drop(file);
        let target = std::fs::read_link(&leaf)?;
        let target = target.into_os_string().into_string().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "non-UTF-8 link target")
        })?;
        return Ok(OpenLeaf::Symlink {
            target: target.replace('\\', "/"),
        });
    }
    if !meta.is_file() {
        return Ok(OpenLeaf::NotRegularFile);
    }
    if classify_only {
        return Ok(OpenLeaf::File {
            content: Vec::new(),
            executable: false,
        });
    }
    // One byte past the cap: the size check then errors on the oversized
    // capture instead of buffering an unbounded raced-grown file.
    let limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut content = Vec::new();
    file.take(limit).read_to_end(&mut content)?;
    drop(pinned);
    Ok(OpenLeaf::File {
        content,
        executable: false,
    })
}

/// Open `path` for reading as itself: a link or junction opens as the
/// link, a directory as a directory.
fn open_no_follow(path: &Path, share: u32) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .share_mode(share)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

/// A missing step: the name is gone (`NotFound`), or a parent in the path
/// is not a directory (`ERROR_DIRECTORY` and kin read as `NotADirectory`).
fn is_missing(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A held ancestor cannot be renamed away, so no link can take its
    /// name while the leaf is read.
    #[test]
    fn a_pinned_ancestor_cannot_be_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let ancestor = dir.path().join("a");
        std::fs::create_dir(&ancestor).unwrap();
        let held = open_no_follow(&ancestor, FILE_SHARE_READ | FILE_SHARE_WRITE).unwrap();
        assert!(std::fs::rename(&ancestor, dir.path().join("moved")).is_err());
        drop(held);
        std::fs::rename(&ancestor, dir.path().join("moved")).unwrap();
    }

    /// A junction at an ancestor's name answers `AncestorSymlink`; the
    /// capture never reads through it.
    #[test]
    fn a_junction_ancestor_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret"), b"outside").unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(root.join("a"))
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(matches!(
            open_leaf(&root, "a/secret", 1024, false).unwrap(),
            OpenLeaf::AncestorSymlink
        ));
    }

    /// A root that is a junction fails the capture: a root-level leaf has
    /// no ancestor to pin, so the root itself is checked.
    #[test]
    fn a_junction_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret"), b"outside").unwrap();
        let root = dir.path().join("root");
        let status = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&root)
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success());
        let Err(error) = open_leaf(&root, "secret", 1024, false) else {
            panic!("a junction root must fail the capture");
        };
        assert!(
            error.to_string().contains("is not a real directory"),
            "{error}"
        );
    }
}
