//! Irreversible effects that wait for the user's approval. A gate that
//! routes an effect to wait (`eukhe_bend::rules::irreversible::route`)
//! records it through the session's [`Recorder`]: in the root session's
//! ledger (`eukhe_durable::harness::effects`), relayed up the chain by the
//! daemon for a subagent ([`WaitingSink`]), or in this session's own
//! ledger without one. The model reads where it waits
//! ([`WaitingEffect::waiting_text`]); `/approve` runs it later as the
//! user's own act.

use eukhe_bend::rules::irreversible::Mode;
use eukhe_chord::context::BACKGROUND_CONTEXT;
use eukhe_durable::harness::effects::record_waiting;
use eukhe_durable::session::{SessionError, SessionResult};
use eukhe_types::effects::{ApprovalMode, WaitingEffect, WaitingKind, WaitingStatus};
use eukhe_types::shell::ShellEffects;

use super::deps::{HarnessCell, HostDeps, WaitingSink};

/// One effect a gate holds, before the ledger gives it an id.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Held {
    pub(crate) kind: WaitingKind,
    /// What runs, for people.
    pub(crate) command: String,
    pub(crate) cwd: String,
    pub(crate) effects: ShellEffects,
    /// What re-runs it.
    pub(crate) payload: serde_json::Value,
}

/// Where the gates of one session record what waits.
#[derive(Clone)]
pub(crate) struct Recorder {
    pub(crate) sink: Option<WaitingSink>,
    pub(crate) harness: HarnessCell,
    pub(crate) session_id: String,
}

impl Recorder {
    pub(crate) fn of(deps: &HostDeps) -> Self {
        Self {
            sink: deps.waiting.clone(),
            harness: deps.harness.clone(),
            session_id: deps.session_id.clone(),
        }
    }

    /// Record `held` waiting: the effect as recorded, with its id.
    ///
    /// # Errors
    ///
    /// The ledger (or the relay to the root's) could not record it.
    pub(crate) async fn record(&self, held: Held) -> Result<WaitingEffect, String> {
        let effect = WaitingEffect {
            id: String::new(),
            kind: held.kind,
            command: held.command,
            cwd: held.cwd,
            effects: held.effects.iter().collect(),
            agent: None,
            session_id: self.session_id.clone(),
            asked_at_ms: super::rlm::now_millis(),
            status: WaitingStatus::Waiting,
            payload: held.payload,
        };
        match &self.sink {
            Some(sink) => sink(effect).await,
            None => record_waiting_effect(&self.harness, effect)
                .await
                .map_err(|error| error.to_string()),
        }
    }
}

#[cfg(test)]
impl Recorder {
    /// A recorder for tests: every effect gets the id `w1`; nothing is
    /// recorded.
    pub(crate) fn numbering_w1() -> Self {
        use futures::FutureExt as _;
        let sink: WaitingSink = std::sync::Arc::new(|effect: WaitingEffect| {
            async move {
                Ok(WaitingEffect {
                    id: "w1".to_owned(),
                    ..effect
                })
            }
            .boxed()
        });
        Self {
            sink: Some(sink),
            harness: HarnessCell::default(),
            session_id: "session".to_owned(),
        }
    }
}

/// Record `effect` waiting in the ledger of the root conversation of
/// `harness`, under its next id: the effect as recorded. The daemon
/// records a subagent's effect relayed up to the root session with it.
///
/// # Errors
///
/// The Harness is closed, or the commit failed.
pub async fn record_waiting_effect(
    harness: &HarnessCell,
    effect: WaitingEffect,
) -> SessionResult<WaitingEffect> {
    let root = harness
        .root()
        .ok_or_else(|| SessionError::error("Harness is closed"))?
        .id();
    harness
        .require()?
        .commit(
            move |tx| async move { record_waiting(&tx, root, effect).await },
            &BACKGROUND_CONTEXT,
        )
        .await
}

/// The compiled core's name of the approval mode.
pub(crate) fn core_mode(mode: ApprovalMode) -> Mode {
    match mode {
        ApprovalMode::End => Mode::End,
        ApprovalMode::Watch => Mode::Watch,
    }
}
