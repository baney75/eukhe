//! The bash every Windows shell call site runs, shared by eukhe-core (the
//! bash tool, gates, checks) and eukhe-durable (the native execution
//! environment), so both pick the same one.

/// Git Bash under `%ProgramFiles%` or `%ProgramFiles(x86)%`, then the
/// first `bash.exe` on `PATH` outside the Windows directory (whose
/// `System32\bash.exe` launches WSL, not a Windows bash).
#[cfg(windows)]
#[must_use]
pub fn find_windows_bash() -> Option<std::path::PathBuf> {
    let installed = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|root| {
            std::path::Path::new(&root)
                .join("Git")
                .join("bin")
                .join("bash.exe")
        });
    let windows_dir =
        std::env::var_os("SystemRoot").map(|dir| dir.to_string_lossy().to_lowercase());
    let on_path = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|dir| {
            windows_dir
                .as_ref()
                .is_none_or(|windows| !dir.to_string_lossy().to_lowercase().starts_with(windows))
        })
        .map(|dir| dir.join("bash.exe"));
    installed
        .chain(on_path)
        .find(|candidate| candidate.is_file())
}
