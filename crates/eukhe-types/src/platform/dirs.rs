//! Home-directory resolution shared by every crate that expands `~`.
//!
//! The TS product resolves the home directory with Node's `os.homedir()`
//! (or `process.env.HOME || homedir()` in the package manager, which keeps
//! the same first step): `HOME` when set. When nothing resolves it
//! throws - this port returns `None` and each caller owns its fallback
//! (an explicit error in the daemon, a documented degraded value elsewhere),
//! so the decision stays visible at the point of use instead of a shared
//! silent default.
//!
//! Windows: the home is `USERPROFILE` (Node's `os.homedir()` there), and
//! the agent state directory is `eukhe` in `%LOCALAPPDATA%`, the user's
//! private local data dir.

use std::path::{Path, PathBuf};

/// The variable that names the home directory.
#[cfg(unix)]
const HOME_ENV: &str = "HOME";
#[cfg(windows)]
const HOME_ENV: &str = "USERPROFILE";

/// The user's home directory, Node `os.homedir()` semantics.
/// `HOME` (Windows: `USERPROFILE`) when set and non-empty (matching the TS
/// `process.env.HOME || homedir()` order). `None` means no home resolved;
/// the per-call-site fallback replaces the TS `os.homedir()` throw.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(HOME_ENV)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// `EUKHE_CODING_AGENT_DIR`: the agent state directory override (a
/// wire-internal identifier kept byte-compatible with the TS product).
pub const ENV_AGENT_DIR: &str = "EUKHE_CODING_AGENT_DIR";

/// The user's agent state directory under the home directory. eukhe keeps
/// its state apart from an installed Eukhe (`.eukhe`): its own
/// settings, sessions, daemon, kernel venv, and chat memory. Project-local
/// configuration stays `<project>/.eukhe`.
pub const AGENT_DIR_NAME: &str = ".eukhe";

/// The agent state directory's name in `%LOCALAPPDATA%` (Windows).
#[cfg(windows)]
pub const WINDOWS_AGENT_DIR_NAME: &str = "eukhe";

/// The directory under the agent dir that holds the daemon's and workers' sockets (macOS) and their locks.
pub const RUN_DIR_NAME: &str = "run";

/// The agent state directory (TS `getAgentDir`): the env override with a
/// leading `~`/`~/` expanded against [`home_dir`], else `<home>/.eukhe`
/// (Windows: `%LOCALAPPDATA%\eukhe`). `None` when no override is set and
/// the home directory (Windows: `LOCALAPPDATA`) does not resolve (each
/// caller owns its fallback).
#[must_use]
pub fn agent_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(ENV_AGENT_DIR).filter(|dir| !dir.is_empty()) {
        return Some(expand_tilde(&dir.to_string_lossy()));
    }
    #[cfg(unix)]
    {
        home_dir().map(|home| home.join(AGENT_DIR_NAME))
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .filter(|dir| !dir.is_empty())
            .map(|dir| PathBuf::from(dir).join(WINDOWS_AGENT_DIR_NAME))
    }
}

/// The work root of `agent_dir`: copies and snapshots of agents' work (child workspaces, check copies, kernel snapshots), beside the agent state directory and outside it (`<agent dir>-work`, `~/.eukhe-work` by default). `None` when `agent_dir` has no file name.
#[must_use]
pub fn work_root_of(agent_dir: &Path) -> Option<PathBuf> {
    let mut name = agent_dir.file_name()?.to_os_string();
    name.push("-work");
    Some(agent_dir.with_file_name(name))
}

/// [`work_root_of`] the [`agent_dir`]. `None` when [`agent_dir`] is `None` or has no file name.
#[must_use]
pub fn work_root() -> Option<PathBuf> {
    work_root_of(&agent_dir()?)
}

/// Expand a leading `~`/`~/` against [`home_dir`]; other values pass
/// through (TS `expandTildePath`: `~foo` is not an expansion).
#[must_use]
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().map_or_else(|| PathBuf::from(path), |home| home.join(rest));
    }
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    PathBuf::from(path)
}

/// Environment mutations are process-global: every test that sets one
/// takes this lock and restores the previous values on exit, and a read of
/// the profile ([`is_in_own_profile`](super::access::is_in_own_profile))
/// takes it too, so no test sees another's stand-in home.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `body` with the named variables set to the given values (`None`
    /// removes them), restoring the previous state afterwards.
    fn with_env(names: &[(&str, Option<&str>)], body: impl FnOnce()) {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous: Vec<(&str, Option<std::ffi::OsString>)> = names
            .iter()
            .map(|(name, _)| (*name, std::env::var_os(name)))
            .collect();
        for (name, value) in names {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        body();
        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    #[test]
    fn agent_dir_env_override_expands_tilde() {
        with_env(
            &[
                (HOME_ENV, Some("/home/tester")),
                ("EUKHE_CODING_AGENT_DIR", Some("~/state")),
            ],
            || {
                assert_eq!(agent_dir(), Some(PathBuf::from("/home/tester/state")));
            },
        );
    }

    #[test]
    fn agent_dir_env_override_passthrough() {
        with_env(
            &[
                (HOME_ENV, None),
                ("EUKHE_CODING_AGENT_DIR", Some("/opt/state")),
            ],
            || {
                assert_eq!(agent_dir(), Some(PathBuf::from("/opt/state")));
            },
        );
    }

    #[test]
    fn work_root_is_the_agent_dir_sibling() {
        assert_eq!(
            [
                work_root_of(Path::new("/home/tester/.eukhe")),
                work_root_of(Path::new("/opt/state/")),
                work_root_of(Path::new("/")),
            ],
            [
                Some(PathBuf::from("/home/tester/.eukhe-work")),
                Some(PathBuf::from("/opt/state-work")),
                None,
            ]
        );
    }

    #[test]
    fn work_root_follows_the_agent_dir_override() {
        with_env(
            &[
                (HOME_ENV, Some("/home/tester")),
                ("EUKHE_CODING_AGENT_DIR", Some("~/state")),
            ],
            || {
                assert_eq!(work_root(), Some(PathBuf::from("/home/tester/state-work")));
            },
        );
    }

    #[cfg(unix)]
    // Unix only: its expected values are Unix paths.
    #[test]
    fn agent_dir_default_under_home() {
        with_env(
            &[
                (HOME_ENV, Some("/home/tester")),
                ("EUKHE_CODING_AGENT_DIR", None),
            ],
            || {
                assert_eq!(agent_dir(), Some(PathBuf::from("/home/tester/.eukhe")));
            },
        );
    }

    #[cfg(unix)]
    // Unix only: on Windows the agent dir is under %LOCALAPPDATA%, not the home.
    #[test]
    fn agent_dir_without_home_is_none() {
        with_env(
            &[(HOME_ENV, None), ("EUKHE_CODING_AGENT_DIR", None)],
            || {
                assert_eq!(agent_dir(), None);
            },
        );
    }

    #[test]
    fn home_dir_follows_home() {
        with_env(&[(HOME_ENV, Some("/home/tester"))], || {
            assert_eq!(home_dir(), Some(PathBuf::from("/home/tester")));
        });
    }

    #[test]
    fn home_dir_unset_is_none() {
        with_env(&[(HOME_ENV, None)], || {
            assert_eq!(home_dir(), None);
        });
    }

    #[test]
    fn home_dir_empty_is_none() {
        with_env(&[(HOME_ENV, Some(""))], || {
            assert_eq!(home_dir(), None);
        });
    }

    #[cfg(windows)]
    #[test]
    fn agent_dir_default_under_local_app_data() {
        with_env(
            &[
                ("LOCALAPPDATA", Some(r"C:\Users\tester\AppData\Local")),
                ("EUKHE_CODING_AGENT_DIR", None),
            ],
            || {
                assert_eq!(
                    agent_dir(),
                    Some(PathBuf::from(r"C:\Users\tester\AppData\Local\eukhe"))
                );
            },
        );
    }

    #[cfg(windows)]
    #[test]
    fn agent_dir_without_local_app_data_is_none() {
        with_env(
            &[("LOCALAPPDATA", None), ("EUKHE_CODING_AGENT_DIR", None)],
            || {
                assert_eq!(agent_dir(), None);
            },
        );
    }
}
