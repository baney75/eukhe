//! Shell selection for the bash tool and the kernel's `bash()`.
//!
//! Explicit path, then on Unix `/bin/bash`, `which bash`, `sh`; on
//! Windows Git Bash under Program Files, then `bash.exe` on `PATH`
//! (not the System32 WSL launcher), else an error.

use std::path::Path;

/// Shell program plus the fixed argument list used to run a command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellConfig {
    pub shell: String,
    pub args: Vec<String>,
}

/// Resolve the shell to run commands with, honoring an explicit custom path.
///
/// # Errors
///
/// Returns an error when the explicit custom shell path is not absolute (a
/// relative path would resolve against whatever directory the command
/// runs in) or does not exist. Built-in resolution never fails on Unix (a
/// missing bash falls back to `sh`); on Windows it fails when no bash is
/// installed, since Windows has no `sh` to fall back to.
pub fn get_shell_config(custom_shell_path: Option<&str>) -> anyhow::Result<ShellConfig> {
    if let Some(path) = custom_shell_path {
        if !Path::new(path).is_absolute() {
            return Err(anyhow::anyhow!(
                "shellPath must be an absolute path, got: {path}"
            ));
        }
        if Path::new(path).exists() {
            return Ok(ShellConfig {
                shell: path.to_string(),
                args: vec!["-c".to_string()],
            });
        }
        return Err(anyhow::anyhow!("Custom shell path not found: {path}"));
    }
    default_shell_config()
}

#[cfg(windows)]
fn default_shell_config() -> anyhow::Result<ShellConfig> {
    let bash = find_windows_bash().ok_or_else(|| {
        anyhow::anyhow!("no bash found: install Git for Windows or set shellPath to a bash.exe")
    })?;
    Ok(ShellConfig {
        shell: bash,
        args: vec!["-c".to_string()],
    })
}

/// Git Bash under `%ProgramFiles%` or `%ProgramFiles(x86)%`, then the
/// first `bash.exe` on `PATH` outside the Windows directory (whose
/// `System32\bash.exe` launches WSL, not a Windows bash).
#[cfg(windows)]
fn find_windows_bash() -> Option<String> {
    let installed = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|root| Path::new(&root).join("Git").join("bin").join("bash.exe"));
    let windows_dir = std::env::var_os("SystemRoot").map(std::path::PathBuf::from);
    let on_path = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|dir| {
            windows_dir.as_ref().is_none_or(|windows| {
                !dir.to_string_lossy()
                    .to_lowercase()
                    .starts_with(&windows.to_string_lossy().to_lowercase())
            })
        })
        .map(|dir| dir.join("bash.exe"));
    installed
        .chain(on_path)
        .find(|candidate| candidate.is_file())
        .map(|bash| bash.to_string_lossy().into_owned())
}

// Never an error on Unix (sh is the last resort); the Result is for Windows.
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn default_shell_config() -> anyhow::Result<ShellConfig> {
    if Path::new("/bin/bash").exists() {
        return Ok(ShellConfig {
            shell: "/bin/bash".to_string(),
            args: vec!["-c".to_string()],
        });
    }

    if let Some(bash) = find_bash_on_path() {
        return Ok(ShellConfig {
            shell: bash,
            args: vec!["-c".to_string()],
        });
    }

    Ok(ShellConfig {
        shell: "sh".to_string(),
        args: vec!["-c".to_string()],
    })
}

#[cfg(unix)]
fn find_bash_on_path() -> Option<String> {
    let out = std::process::Command::new("which")
        .arg("bash")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.trim().lines().next()?;
    (!first.is_empty()).then(|| first.to_string())
}

/// Absolute default shell for the kernel's `bash()`: explicit path wins,
/// else `/bin/bash`, else `/bin/sh`.
pub fn resolve_kernel_bash_shell(custom_shell_path: Option<&str>) -> String {
    if let Some(explicit) = custom_shell_path.map(str::trim).filter(|s| !s.is_empty()) {
        return explicit.to_string();
    }
    if Path::new("/bin/bash").exists() {
        "/bin/bash".to_string()
    } else {
        "/bin/sh".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relative `shellPath` is refused even when it names an existing
    /// file relative to the current directory (the crate root's
    /// `Cargo.toml` here): it would resolve against whatever directory the
    /// command runs in, which a cloned repository controls.
    #[test]
    fn relative_custom_shell_path_is_rejected() {
        assert!(Path::new("Cargo.toml").exists());
        let error = get_shell_config(Some("Cargo.toml")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "shellPath must be an absolute path, got: Cargo.toml"
        );
    }
}
