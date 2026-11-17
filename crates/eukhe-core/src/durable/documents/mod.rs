//! `eukhe.documents`: the session's documents (`eukhe_docs::Documents`,
//! one edit store per session) behind the wire `read`, `edit`, `grep`, and
//! `glob` tools and the kernel's `fs.*` host requests. Both entry points
//! decode through one contract per function and share the store, so a
//! `read()` in the kernel makes its lines anchorable for the `edit` tool and
//! the other way round. `read` also serves the internal URLs (`urls`), and
//! `ssh://` through the user's own `ssh` program (`ssh`). A durable
//! session keeps its seen lines in its storage (`seen_log`), so the next
//! process of the session anchors edits to the lines the model saw.

mod contract;
mod probe;
mod requests;
mod seen_log;
#[cfg(test)]
mod seen_tests;
mod ssh;
mod tool;
mod urls;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Weak};

use eukhe_chord::context::BACKGROUND_CONTEXT;
use eukhe_docs::Documents;
use eukhe_durable::harness::define::define_extension;
use eukhe_durable::harness::types::{Extension, ToolExecutionApi};

use self::contract::Function;
use self::requests::{perform, Caller};
use super::path_policy::{session_policy, Allowed, DocumentsGate, Rerun};
use super::settings::EukheSettings;
use super::waiting::Recorder;
use super::{HostDeps, ShellApprovalSink};
use crate::memory::Memory;
use crate::resources::LoadedResources;

/// The extension's name.
pub const DOCUMENTS_EXTENSION: &str = "eukhe.documents";

/// The wire `edit` tool's name.
pub const EDIT_TOOL_NAME: &str = Function::Edit.name();

/// What the requests and the tools share.
pub(crate) struct DocumentsHost {
    documents: Documents,
    /// The root of `repo://`: the session's working directory.
    workspace: PathBuf,
    /// The skills `skill://` serves.
    resources: Arc<LoadedResources>,
    /// The chat memory `memory://` serves; `None` without one.
    memory: Option<Memory>,
    /// The session (its MCP servers and kernels) `mcp://` reads through;
    /// weak: the session's host requests own this host.
    deps: Weak<HostDeps>,
    /// The gate's inputs: the agent dir and settings the path policy
    /// derives its protected set from, and the session's approval sink.
    gate: GateInputs,
}

/// What the gate of each call reads.
pub(crate) struct GateInputs {
    pub(crate) agent_dir: PathBuf,
    pub(crate) settings: Arc<EukheSettings>,
    pub(crate) sink: Option<ShellApprovalSink>,
    pub(crate) recorder: Recorder,
}

impl DocumentsHost {
    /// The gate of one call working in `workspace`: the session's path
    /// policy, its auto set, approval mode and sink; each question's id
    /// starts with the id of `api`'s tool call (a kernel request without
    /// one: a fresh id). An access that waits is recorded with `rerun`.
    fn gate(
        &self,
        workspace: &Path,
        api: Option<&dyn ToolExecutionApi>,
        rerun: Rerun,
        allowed: Allowed,
    ) -> DocumentsGate {
        DocumentsGate {
            policy: session_policy(&self.gate.agent_dir, &self.gate.settings, workspace),
            auto: super::shell::auto_set(&self.gate.settings),
            mode: self.gate.settings.manager().get_approval_mode(),
            sink: self.gate.sink.clone(),
            recorder: self.gate.recorder.clone(),
            call: api.map_or_else(
                || format!("fs-{}", uuid::Uuid::new_v4().simple()),
                |api| api.task_id().to_string(),
            ),
            rerun,
            allowed,
            asked: AtomicU64::new(0),
            runtime: tokio::runtime::Handle::current(),
        }
    }
}

/// Why an approved document access did not run again.
#[derive(Debug, thiserror::Error)]
pub enum RerunError {
    #[error("the session's documents are closed")]
    Closed,
    #[error("the waiting access is malformed: {0}")]
    Malformed(&'static str),
    #[error("the access failed: {0}")]
    Failed(String),
}

/// Run the document access a waiting effect recorded (its `payload`:
/// function, cwd, arguments, path) once more, as the user's own act: that
/// one path is allowed, every other access gates as usual. The text the
/// call answers.
///
/// # Errors
///
/// The session closed, the payload is not one the gate recorded, or the
/// access failed.
pub async fn rerun_approved(
    deps: &HostDeps,
    payload: &serde_json::Value,
) -> Result<String, RerunError> {
    let host = deps
        .documents
        .get()
        .and_then(Weak::upgrade)
        .ok_or(RerunError::Closed)?;
    let text = |key: &'static str| {
        payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or(RerunError::Malformed(key))
    };
    let name = text("function")?;
    let function = [
        Function::Read,
        Function::Write,
        Function::Edit,
        Function::Grep,
        Function::Glob,
    ]
    .into_iter()
    .find(|function| function.name() == name)
    .ok_or(RerunError::Malformed("function"))?;
    let (cwd, path) = (PathBuf::from(text("cwd")?), PathBuf::from(text("path")?));
    let Some(args) = payload.get("args").and_then(serde_json::Value::as_object) else {
        return Err(RerunError::Malformed("args"));
    };
    let performed = perform(
        &host,
        function,
        cwd,
        &host.workspace,
        args.clone(),
        Caller::Approved(path),
        &BACKGROUND_CONTEXT,
    )
    .await
    .map_err(|error| RerunError::Failed(error.to_string()))?;
    match performed {
        Ok(performed) => Ok(performed.done.rendered().to_owned()),
        Err(rejection) => Err(RerunError::Failed(rejection.message)),
    }
}

/// The documents extension of one session. Path locks live under
/// `<agent dir>/locks`, shared by every session of the eukhe home.
#[must_use]
pub fn extension(deps: &Arc<HostDeps>) -> Arc<Extension> {
    let lock_dir = deps.agent_dir.join("locks");
    let home = eukhe_types::platform::home_dir();
    let documents = match &deps.storage_dir {
        Some(dir) => {
            Documents::journaled(lock_dir, home, Arc::new(seen_log::SeenLog::in_storage(dir)))
        }
        None => Documents::new(lock_dir, home),
    };
    let host = Arc::new(DocumentsHost {
        documents,
        workspace: deps.cwd.clone(),
        resources: Arc::clone(&deps.resources),
        memory: deps.memory.clone(),
        deps: Arc::downgrade(deps),
        gate: GateInputs {
            agent_dir: deps.agent_dir.clone(),
            settings: Arc::clone(&deps.settings),
            sink: deps.shell_approval.clone(),
            recorder: Recorder::of(deps),
        },
    });
    // The first install of the session's documents is the one `/approve`
    // re-runs through.
    let _ = deps.documents.set(Arc::downgrade(&host));
    requests::register(&deps.host_requests, &host);
    define_extension(Extension {
        tools: [
            Function::Read,
            Function::Edit,
            Function::Grep,
            Function::Glob,
        ]
        .into_iter()
        .map(|function| tool::document_tool(function, &host, deps))
        .collect(),
        ..Extension::named(DOCUMENTS_EXTENSION)
    })
}
