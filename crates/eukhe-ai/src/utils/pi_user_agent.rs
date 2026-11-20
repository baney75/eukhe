//! The `pi (<platform> <release>; <arch>)` user agent.

use std::sync::LazyLock;

/// Node `os.platform()` for the supported platforms.
fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// Node `os.arch()` for the supported platforms.
fn node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// Node `os.release()`, read once per process.
pub(crate) fn node_release() -> &'static str {
    static RELEASE: LazyLock<String> = LazyLock::new(read_node_release);
    &RELEASE
}

/// Node `os.release()`: the kernel release (`uname -r`).
#[cfg(unix)]
fn read_node_release() -> String {
    rustix::system::uname()
        .release()
        .to_string_lossy()
        .into_owned()
}

/// Node `os.release()` on Windows: `<major>.<minor>.<build>`, read from
/// the `[Version 10.0.26100.1234]` line `cmd /c ver` prints (safe Rust
/// has no version query); empty when `ver` cannot run.
#[cfg(windows)]
fn read_node_release() -> String {
    std::process::Command::new("cmd")
        .args(["/d", "/c", "ver"])
        .output()
        .map(|output| windows_release(&String::from_utf8_lossy(&output.stdout)))
        .unwrap_or_default()
}

/// The `<major>.<minor>.<build>` of a `ver` banner: the first dotted
/// number in its brackets, whatever word the locale prints before it.
#[cfg(any(windows, test))]
fn windows_release(banner: &str) -> String {
    banner
        .split('[')
        .nth(1)
        .and_then(|inside| inside.split(']').next())
        .and_then(|inside| {
            inside.split_whitespace().find(|word| {
                word.contains('.') && word.chars().all(|c| c.is_ascii_digit() || c == '.')
            })
        })
        .map(|version| version.split('.').take(3).collect::<Vec<_>>().join("."))
        .unwrap_or_default()
}

/// The user agent, computed once per process.
static USER_AGENT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "pi ({} {}; {})",
        node_platform(),
        node_release(),
        node_arch()
    )
});

/// TS `getPiUserAgent()` in a Node runtime.
#[must_use]
pub fn get_pi_user_agent() -> &'static str {
    &USER_AGENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_platform_release_and_arch() {
        let agent = get_pi_user_agent();
        assert!(agent.starts_with(&format!("pi ({} ", node_platform())));
        assert!(agent.ends_with(&format!("; {})", node_arch())));
        assert!(!node_release().is_empty());
    }

    #[test]
    fn windows_release_is_major_minor_build() {
        assert_eq!(
            windows_release("\r\nMicrosoft Windows [Version 10.0.26100.4652]\r\n"),
            "10.0.26100"
        );
        assert_eq!(
            windows_release("\r\nMicrosoft Windows [version 10.0.19045.3803]\r\n"),
            "10.0.19045"
        );
    }
}
