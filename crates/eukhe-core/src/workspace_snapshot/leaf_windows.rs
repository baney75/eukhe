//! The leaf open of a snapshot capture on Windows. std has no `openat`
//! there, so each component is looked at by path with `symlink_metadata`
//! (a symbolic link or junction answers `is_symlink`), and the leaf is
//! opened after its look. Unlike the Unix walk, whose opened directory
//! handles pin each step, a link swapped in between a look and the next
//! open can redirect the read: a concurrent writer inside the worktree is
//! needed for that, and the object id check against HEAD still fails a
//! changed baseline file.

use std::io::Read as _;
use std::path::Path;

use super::OpenLeaf;

/// Open `path` beneath `root` without following a symbolic link at any
/// step it looks at: an ancestor link answers
/// [`OpenLeaf::AncestorSymlink`], a leaf link its target (with `/`
/// separators, as git records it). No file has an execute bit on Windows:
/// `executable` is false, and a clean leaf takes HEAD's mode.
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
    let mut dir = root.to_path_buf();
    for component in ancestors {
        dir.push(component);
        match std::fs::symlink_metadata(&dir) {
            Ok(meta) if meta.file_type().is_symlink() => return Ok(OpenLeaf::AncestorSymlink),
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Ok(OpenLeaf::Missing),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(OpenLeaf::Missing)
            }
            Err(error) => return Err(error),
        }
    }
    let leaf = dir.join(leaf);
    let meta = match std::fs::symlink_metadata(&leaf) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(OpenLeaf::Missing),
        Err(error) => return Err(error),
    };
    if meta.file_type().is_symlink() {
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
    std::fs::File::open(&leaf)?
        .take(limit)
        .read_to_end(&mut content)?;
    Ok(OpenLeaf::File {
        content,
        executable: false,
    })
}
