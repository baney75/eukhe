//! `/approve`: the waiting irreversible effects of a session run, each
//! once, in order, as the user's own act. The compiled core picks what
//! runs (`eukhe_bend::rules::irreversible::approve`, through
//! [`take_waiting`]): each chosen effect is marked running in the ledger
//! before it runs, so a crash mid-run never runs it again
//! ([`interrupt_after_crash`] ends it failed at the next open). A shell
//! step runs its stored command in its stored cwd (the session's when that
//! directory is gone) with every effect allowed; a document access re-runs
//! its call with that one path allowed; a `python` cell runs its code in
//! the root conversation's kernel; an MCP call re-runs with its arguments,
//! past the kernel's gate. Each result is a model-visible row of the
//! conversation the user talks to ([`APPROVED_EFFECT_CUSTOM_TYPE`]): the
//! model reads it next turn. Approvals of one session run one at a time.

use std::fmt::Write as _;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use eukhe_chord::context::Context;
use eukhe_durable::harness::effects::{
    interrupt_running, read_waiting_effects, set_waiting_status, take_waiting,
};
use eukhe_durable::harness::types::WriteSubmissionDraft;
use eukhe_durable::harness::{Conversation, Harness};
use eukhe_durable::session::{SessionError, SessionResult};
use eukhe_durable::types::ConversationId;
use eukhe_shell::{Answer, Approver, Ask, End, Shell, Sink, Stream};
use eukhe_types::custom_type::APPROVED_EFFECT_CUSTOM_TYPE;
use eukhe_types::effects::{
    ApprovalMode, ApproveWhich, ApprovedEffectsReply, WaitingEffect, WaitingKind, WaitingStatus,
};
use eukhe_types::pi_ai::UserContent;
use eukhe_types::shell::{AutoSet, ShellEffect};
use serde_json::Value;

use super::entries::custom_entry_draft;
use super::rlm::{now_millis, session_env};
use super::HostDeps;
use crate::kernel::manager::local_env;
use crate::kernel::shared::{ExecuteOptions, ExecuteStatus};

/// The bound of an approved MCP call, as the `dyn` tool's.
const MCP_TIMEOUT_MS: u64 = 30_000;

/// The characters of an effect's output its summary keeps: the tail.
const SUMMARY_CHARS: usize = 2_000;

/// Why an approval ran nothing.
#[derive(Debug, thiserror::Error)]
pub enum ApproveError {
    #[error("the session is closed")]
    Closed,
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// Run the waiting effects `which` names, each once, in ledger order, as
/// the user's own act; each result becomes a row of `conversation` (the
/// one the user talks to). The effects as they ended, and the named ids
/// that ran nothing (unknown, or not waiting).
///
/// # Errors
///
/// The session closed, or the ledger could not be read or written.
pub async fn approve_waiting(
    deps: &HostDeps,
    conversation: &Conversation,
    which: &ApproveWhich,
    cx: &Context,
) -> Result<ApprovedEffectsReply, ApproveError> {
    let _one_at_a_time = deps.approvals.lock().await;
    let root = deps.harness.root().ok_or(ApproveError::Closed)?;
    let root_id = root.id();
    let chosen = which.clone();
    let (taken, missing) = root
        .commit(
            move |tx| async move { take_waiting(&tx, root_id, &chosen).await },
            cx,
        )
        .await?;
    let mut ran = Vec::with_capacity(taken.len());
    for effect in taken {
        let status = run_one(deps, &effect).await;
        let id = effect.id.clone();
        let ended = root
            .commit(
                move |tx| async move { set_waiting_status(&tx, root_id, &id, status).await },
                cx,
            )
            .await?;
        let Some(ended) = ended else {
            continue;
        };
        if let Some(text) = ended.ran_text() {
            let entry = custom_entry_draft(
                APPROVED_EFFECT_CUSTOM_TYPE,
                UserContent::Text(text),
                true,
                Some(serde_json::json!({ "id": ended.id })),
                now_millis(),
            )
            .map_err(SessionError::from)?;
            conversation
                .submit(
                    WriteSubmissionDraft {
                        request_id: Some(format!("approved-effect:{}", ended.id)),
                        entry,
                    },
                    cx,
                )
                .await?;
        }
        ran.push(ended);
    }
    Ok(ApprovedEffectsReply { ran, missing })
}

/// End every approved effect a crash left running in the ledger of
/// `conversation_id`: failed (interrupted, never run again) and noted
/// unknown for a person to settle. Run at open, before anything resumes.
///
/// # Errors
///
/// The ledger could not be read or written.
pub(crate) async fn interrupt_after_crash(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
) -> SessionResult<Vec<WaitingEffect>> {
    let running = read_waiting_effects(harness, conversation_id, cx)
        .await?
        .iter()
        .any(|effect| match effect.status {
            WaitingStatus::Running => true,
            WaitingStatus::Waiting | WaitingStatus::Ran { .. } | WaitingStatus::Refused => false,
        });
    if !running {
        return Ok(Vec::new());
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "epoch milliseconds stay below 2^53"
    )]
    let at = now_millis() as f64;
    harness
        .commit(
            move |tx| async move { interrupt_running(&tx, conversation_id, at).await },
            cx,
        )
        .await
}

/// Run one approved effect: how it ended.
async fn run_one(deps: &HostDeps, effect: &WaitingEffect) -> WaitingStatus {
    let ran = match effect.kind {
        WaitingKind::ShellStep => run_step(deps, effect).await,
        WaitingKind::DocumentWrite => super::documents::rerun_approved(deps, &effect.payload)
            .await
            .map(|text| (true, text))
            .map_err(|error| error.to_string()),
        WaitingKind::PythonCell => run_cell(deps, effect).await,
        WaitingKind::McpCall => run_mcp(deps, effect).await,
    };
    match ran {
        Ok((ok, output)) => WaitingStatus::Ran {
            ok,
            summary: tail(&output),
        },
        Err(error) => WaitingStatus::Ran {
            ok: false,
            summary: tail(&error),
        },
    }
}

/// The text of `key` in an effect's payload.
fn payload_text<'a>(effect: &'a WaitingEffect, key: &str) -> Result<&'a str, String> {
    effect
        .payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("the waiting effect {} has no {key}", effect.id))
}

/// Where an effect runs: its cwd, the session's when that directory is
/// gone (a subagent's copy removed since).
fn run_dir(deps: &HostDeps, effect: &WaitingEffect) -> PathBuf {
    let stored = Path::new(&effect.cwd);
    if stored.is_dir() {
        stored.to_path_buf()
    } else {
        deps.cwd.clone()
    }
}

/// No one is asked: every effect of an approved step is allowed.
struct Approved;

impl Approver for Approved {
    fn approve(&self, _ask: Ask) -> impl Future<Output = Answer> + Send {
        std::future::ready(Answer::Approve)
    }
}

/// The output of an approved step, as one text.
#[derive(Default)]
struct Output(Mutex<Vec<u8>>);

impl Sink for Output {
    fn begin(&self, _step: u64, _command: &str) {}

    fn output(&self, _step: u64, _stream: Stream, bytes: &[u8]) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
    }

    fn end(&self, _step: u64, _status: i32) {}
}

/// Run a shell step's stored command with every effect allowed.
async fn run_step(deps: &HostDeps, effect: &WaitingEffect) -> Result<(bool, String), String> {
    let command = payload_text(effect, "command")?;
    let cwd = run_dir(deps, effect);
    let paths = super::path_policy::session_policy(&deps.agent_dir, &deps.settings, &cwd);
    let output = Output::default();
    let ran = Shell::new(cwd, local_env(&session_env(deps)), "bash")
        .run(
            command,
            AutoSet::new(ShellEffect::ALL),
            ApprovalMode::End,
            &paths,
            &Approved,
            &output,
        )
        .await;
    let text = String::from_utf8_lossy(
        &output
            .0
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner),
    )
    .into_owned();
    match ran.end {
        End::Finished { status } | End::Exited { status } => {
            Ok((status == 0, format!("exit {status}\n{text}")))
        }
        End::Refused { .. } | End::Waits { .. } => Err(format!("it did not run\n{text}")),
    }
}

/// The booted kernel of the root conversation, in `cwd`.
async fn root_kernel(
    deps: &HostDeps,
    cwd: PathBuf,
) -> Result<crate::kernel::ReplKernelManager, String> {
    let pool = deps
        .rlm_kernels
        .get()
        .and_then(std::sync::Weak::upgrade)
        .ok_or_else(|| "the session has no python kernel".to_owned())?;
    let root = deps
        .harness
        .root()
        .ok_or_else(|| "the session is closed".to_owned())?;
    let kernel = pool
        .kernel(root.id(), cwd)
        .map_err(|error| format!("{error:#}"))?;
    kernel
        .provisioner
        .ensure(None, None)
        .await
        .map_err(|error| format!("{error:#}"))
}

/// Run a `python` cell's stored code in the root conversation's kernel.
async fn run_cell(deps: &HostDeps, effect: &WaitingEffect) -> Result<(bool, String), String> {
    let code = payload_text(effect, "code")?;
    let manager = root_kernel(deps, run_dir(deps, effect)).await?;
    let result = manager
        .execute(code, ExecuteOptions::default())
        .await
        .map_err(|error| format!("{error:#}"))?;
    let mut text = result.stdout;
    text.push_str(&result.stderr);
    if let Some(value) = result.result {
        text.push_str(&value);
    }
    if let Some(error) = result.error {
        let _ = write!(text, "\n{}: {}", error.ename, error.evalue);
    }
    let ok = match result.status {
        ExecuteStatus::Ok => true,
        ExecuteStatus::Error | ExecuteStatus::Aborted | ExecuteStatus::Killed => false,
    };
    Ok((ok, text))
}

/// Re-run an MCP call with its stored arguments, past the kernel's gate.
async fn run_mcp(deps: &HostDeps, effect: &WaitingEffect) -> Result<(bool, String), String> {
    let server = payload_text(effect, "server")?;
    let tool = payload_text(effect, "tool")?;
    let arguments = effect
        .payload
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let manager = root_kernel(deps, run_dir(deps, effect)).await?;
    match manager
        .mcp_tool_call_approved(server, tool, &arguments, MCP_TIMEOUT_MS)
        .await
    {
        Ok(Value::String(text)) => Ok((true, text)),
        Ok(value) => Ok((true, value.to_string())),
        Err(error) => Err(error.to_string()),
    }
}

/// The last [`SUMMARY_CHARS`] characters of `text`, trimmed.
fn tail(text: &str) -> String {
    let text = text.trim_end();
    let count = text.chars().count();
    if count <= SUMMARY_CHARS {
        return text.to_owned();
    }
    let start = text
        .char_indices()
        .nth(count - SUMMARY_CHARS)
        .map_or(0, |(index, _)| index);
    format!("…{}", &text[start..])
}

#[cfg(test)]
mod tests {
    use super::tail;

    #[test]
    fn a_summary_keeps_the_tail_of_a_long_output() {
        let long = "x".repeat(super::SUMMARY_CHARS) + "end";
        let kept = tail(&long);
        assert_eq!(
            (
                kept.starts_with('…'),
                kept.ends_with("end"),
                tail("short\n")
            ),
            (true, true, "short".to_owned())
        );
    }
}
