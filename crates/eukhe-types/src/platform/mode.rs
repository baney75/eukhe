//! File modes: the Unix permission bits eukhe sets and reads, behind calls
//! every platform has. Windows has no mode bits: a new file or directory
//! takes the ACL its parent passes down (under the user's profile, the
//! owner's alone), so setting a mode there changes nothing and reading one
//! answers `None`.

use std::path::Path;

/// The mode a created file or directory gets.
pub trait CreationMode {
    /// Sets `mode` as the creation mode (Unix `mode(2)` bits, before the
    /// umask). Windows: returns the builder unchanged.
    fn creation_mode(&mut self, mode: u32) -> &mut Self;
}

impl CreationMode for std::fs::OpenOptions {
    fn creation_mode(&mut self, mode: u32) -> &mut Self {
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(self, mode);
        #[cfg(windows)]
        let _ = mode;
        self
    }
}

impl CreationMode for std::fs::DirBuilder {
    fn creation_mode(&mut self, mode: u32) -> &mut Self {
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(self, mode);
        #[cfg(windows)]
        let _ = mode;
        self
    }
}

impl CreationMode for tokio::fs::OpenOptions {
    fn creation_mode(&mut self, mode: u32) -> &mut Self {
        #[cfg(unix)]
        self.mode(mode);
        #[cfg(windows)]
        let _ = mode;
        self
    }
}

impl CreationMode for tokio::fs::DirBuilder {
    fn creation_mode(&mut self, mode: u32) -> &mut Self {
        #[cfg(unix)]
        self.mode(mode);
        #[cfg(windows)]
        let _ = mode;
        self
    }
}

/// Sets the mode bits of `path` (`chmod`). Windows: `Ok(())`, nothing
/// changes.
///
/// # Errors
///
/// Returns the I/O error of the permission change.
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
    #[cfg(windows)]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Sets the mode bits of an open file (`fchmod`). Windows: `Ok(())`,
/// nothing changes.
///
/// # Errors
///
/// Returns the I/O error of the permission change.
pub fn set_file_mode(file: &std::fs::File, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(std::fs::Permissions::from_mode(mode))
    }
    #[cfg(windows)]
    {
        let _ = (file, mode);
        Ok(())
    }
}

/// The mode of a file's metadata (`st_mode`, type bits included).
/// Windows: `None`.
#[must_use]
pub fn mode_of(metadata: &std::fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        Some(metadata.permissions().mode())
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        None
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_created_file_takes_the_creation_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("f");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .creation_mode(0o600)
            .open(&path)
            .expect("create");
        let mode = mode_of(&std::fs::metadata(&path).expect("metadata"));
        assert_eq!(mode.map(|mode| mode & 0o777), Some(0o600));
        set_mode(&path, 0o640).expect("chmod");
        let mode = mode_of(&std::fs::metadata(&path).expect("metadata"));
        assert_eq!(mode.map(|mode| mode & 0o777), Some(0o640));
    }
}
