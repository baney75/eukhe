//! RLM kernel layer: persistent Python REPL lifecycle.
//!
//! The kernel is a JSON-lines subprocess (`python -m rlm.repl`): requests on
//! stdin, events on stdout, stderr kept as a diagnostics tail. The protocol is
//! documented in `eukhe-runtime/src/rlm/repl.md` (protocol version 3).
//!
//! Ported from the TypeScript product's `core/kernel/` (repl-manager.ts,
//! bootstrap.ts, shared.ts, state-snapshot.ts, boot-gate.ts) and
//! `core/rlm-runtime.ts`.

pub mod bootstrap;
pub mod cancellation;
pub mod live_kernels;
pub mod manager;
pub mod orphan_journal;
pub mod protocol;
pub mod provisioner;
pub mod python_skills;
pub mod rlm_runtime;
pub mod shared;
pub mod state_snapshot;
pub mod transport;

pub use bootstrap::{
    build_rlm_bootstrap_code, ensure_kernel_python, kernel_state, kernel_unavailable,
    lock_project_packages, pinned_python_version, pinned_uv_version, unused_kernel_venv,
    EnsureKernelPythonOptions, KernelBootstrapProgressHandler, KernelFetchState, KernelPackages,
    KernelPython, KernelPythonSkill, KernelUnavailable, ProjectLockChange, KERNEL_INSTALL_COMMAND,
    LOCKED_PACKAGES, PYTHON_SKILL_IMPORT_ERROR_REPORT_MARKER,
};
pub use cancellation::AbortSignal;
pub use manager::ReplKernelManager;
pub use provisioner::IpythonKernelProvisioner;
pub use rlm_runtime::{
    create_default_rlm_subagent_session_name, find_rlm_model_matches,
    find_unique_rlm_short_form_model_match, format_rlm_model_unavailable_error,
    normalize_requested_rlm_subagent_model, normalize_requested_rlm_subagent_session_name,
    normalize_requested_rlm_subagent_thinking_level, RlmModelInfo, RlmModelMatch, THINKING_LEVELS,
};
pub use shared::{
    HostRequestHandler, HostRequestHandlers, HostRequestPayload, KernelManagerOptions,
    KernelSentAgentMessage, KernelShutdownOptions, KernelSnapshotConfig, StreamName,
    EXECUTE_STATUS_ABORTED, EXECUTE_STATUS_ERROR, EXECUTE_STATUS_OK,
};
pub use state_snapshot::{manifest_path_in, snapshot_path_in, RestoreResult, SnapshotResult};
pub use transport::KernelTransport;

/// The refusal of a kernel start on a platform the RLM kernel runtime does
/// not run on. The Python runtime (`eukhe-runtime/src/rlm`) drives its
/// cells through POSIX facilities (`fcntl`, `termios`, process groups and
/// thread signals), so Windows has no kernel yet. A caller can downcast a
/// start error to this type to tell the platform refusal from a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the RLM kernel is not supported on {platform}: its runtime needs POSIX process control")]
pub struct KernelUnsupportedPlatform {
    /// The platform name, as `std::env::consts::OS` reports it.
    pub platform: &'static str,
}

impl KernelUnsupportedPlatform {
    /// The refusal on this platform: `Some` on Windows, `None` where the
    /// runtime runs.
    #[must_use]
    pub fn here() -> Option<Self> {
        cfg!(windows).then_some(Self {
            platform: std::env::consts::OS,
        })
    }
}

#[cfg(test)]
mod platform_tests {
    use super::KernelUnsupportedPlatform;

    /// The kernel refuses on Windows alone, with an error that names it.
    #[test]
    fn the_kernel_refuses_only_where_its_runtime_does_not_run() {
        let refusal = KernelUnsupportedPlatform::here();
        assert_eq!(refusal.is_some(), cfg!(windows));
        let named = KernelUnsupportedPlatform {
            platform: "windows",
        };
        let error = anyhow::Error::from(named);
        assert_eq!(
            error.downcast_ref::<KernelUnsupportedPlatform>(),
            Some(&named)
        );
        assert!(error.to_string().contains("not supported on windows"));
    }
}
