//! Browser launch for OAuth login URLs: the platform opener (the TS
//! login dialog's command table — macOS `open`, Linux `xdg-open`).

use std::process::{Command, Stdio};

/// The opener program for one URL.
#[cfg(target_os = "macos")]
const OPENER: &str = "open";

/// The opener program for one URL.
#[cfg(target_os = "linux")]
const OPENER: &str = "xdg-open";

/// The opener program for one URL: the URL protocol handler of
/// `url.dll`, which takes the URL as one argument (no shell parses its
/// `&`, as `cmd /c start` would).
#[cfg(windows)]
const OPENER: &str = "rundll32.exe";

/// The opener's arguments before the URL.
#[cfg(windows)]
const OPENER_ARGS: &[&str] = &["url.dll,FileProtocolHandler"];

/// The opener's arguments before the URL.
#[cfg(not(windows))]
const OPENER_ARGS: &[&str] = &[];

/// Open `url` in the user's browser. Fire-and-forget like the TS dialog
/// (`execFileHidden` with a swallowed callback): the caller also shows the
/// URL itself, so a failed launch (no desktop session, no opener) never
/// fails the login. The spawn result is deliberately not an error surface.
pub fn open_in_browser(url: &str) {
    let _ = Command::new(OPENER)
        .args(OPENER_ARGS)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}
