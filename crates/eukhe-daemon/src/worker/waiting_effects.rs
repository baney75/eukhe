//! The irreversible effects waiting for the user's approval:
//! `get_waiting_effects` reads the root conversation's ledger (every
//! status, oldest first), `approve_waiting_effects` runs the effects it
//! names, each once, in order, as the user's own act
//! (`eukhe_core::durable::approve`), and `relay_waiting_effect` records a
//! subagent's effect in the root session's ledger
//! ([`crate::waiting_relay`]). Attached clients follow the ledger through
//! `waiting_effects_update` and the attach snapshot's `waitingEffects`.

use eukhe_chord::context::BACKGROUND_CONTEXT;
use eukhe_core::durable::approve::approve_waiting;
use eukhe_durable::harness::effects::read_waiting_effects;
use eukhe_types::effects::{ApproveWhich, WaitingEffectsReply};
use serde::Deserialize as _;
use serde_json::Value;

use super::{response_failure, response_success, Worker};
use crate::protocol::DaemonResponse;

impl Worker {
    /// `get_waiting_effects`: the ledger, every status, oldest first.
    #[tracing::instrument(skip(self))]
    pub(crate) async fn handle_get_waiting_effects(&self) -> DaemonResponse {
        const COMMAND: &str = "get_waiting_effects";
        let hosted = match self.hosted(COMMAND) {
            Ok(hosted) => hosted,
            Err(response) => return response,
        };
        let Some(root) = hosted.deps().harness.root() else {
            return response_failure(None, COMMAND, "the session is closed", None);
        };
        match read_waiting_effects(hosted.harness(), root.id(), &BACKGROUND_CONTEXT).await {
            Ok(effects) => response_success(
                None,
                COMMAND,
                Some(serde_json::to_value(WaitingEffectsReply { effects }).unwrap_or(Value::Null)),
            ),
            Err(error) => response_failure(None, COMMAND, &error.to_string(), None),
        }
    }

    /// `approve_waiting_effects { which }`: run the waiting effects `which`
    /// names (`"all"`, or `{"ids": [...]}`), each once, in order; each
    /// result is a row of the main conversation the model reads next turn.
    /// Allowed while a run goes on. The reply names what ran and the ids
    /// that ran nothing.
    #[tracing::instrument(skip(self, payload))]
    pub(crate) async fn handle_approve_waiting_effects(&self, payload: &Value) -> DaemonResponse {
        const COMMAND: &str = "approve_waiting_effects";
        let hosted = match self.hosted(COMMAND) {
            Ok(hosted) => hosted,
            Err(response) => return response,
        };
        let Some(which) = payload
            .get("which")
            .and_then(|which| ApproveWhich::deserialize(which).ok())
        else {
            return response_failure(
                None,
                COMMAND,
                "approve_waiting_effects requires which: \"all\" or {\"ids\": [...]}",
                None,
            );
        };
        let main = match hosted.main() {
            Ok(main) => main,
            Err(error) => return response_failure(None, COMMAND, &error.to_string(), None),
        };
        match approve_waiting(hosted.deps(), &main, &which, &BACKGROUND_CONTEXT).await {
            Ok(reply) => {
                hosted.events_delivered().await;
                response_success(
                    None,
                    COMMAND,
                    Some(serde_json::to_value(reply).unwrap_or(Value::Null)),
                )
            }
            Err(error) => response_failure(None, COMMAND, &error.to_string(), None),
        }
    }

    /// `relay_waiting_effect { effect, agent }`: a subagent's waiting
    /// effect, recorded in the root session's ledger.
    #[tracing::instrument(skip(self, payload))]
    pub(crate) async fn handle_relay_waiting_effect(&self, payload: &Value) -> DaemonResponse {
        if let Err(response) = self.require_created("relay_waiting_effect") {
            return response;
        }
        self.waiting_relay.relay(payload).await
    }
}
