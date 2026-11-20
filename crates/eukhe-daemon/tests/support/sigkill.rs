//! The test harness's hard kill of an arbitrary pid (a worker the wire
//! names): `kill -9` on Unix, `taskkill /F` through the platform helper on
//! Windows, where no `kill` reaches a native process.

/// Kill `pid` at once; panics when the kill fails.
#[cfg(unix)]
pub fn sigkill(pid: u32) {
    let status = std::process::Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .status()
        .expect("run kill");
    assert!(status.success(), "kill -9 {pid} failed: {status}");
}

/// Kill `pid` at once; panics when the kill fails.
#[cfg(windows)]
pub fn sigkill(pid: u32) {
    eukhe_types::platform::process::kill_process(pid)
        .unwrap_or_else(|error| panic!("taskkill /F {pid} failed: {error}"));
}
