//! Where a session records an irreversible effect that waits for the
//! user's approval: the ROOT session's ledger. A root session records it
//! in its own (`eukhe_core::durable::record_waiting_effect`); a subagent
//! session hands it to its parent (`relay_waiting_effect` over the
//! supervisor), naming itself as the asking agent, and each session up the
//! chain does the same until the root records it. The answer is the effect
//! as recorded, with its id (`w<N>` of the root's ledger), which the
//! subagent's model reads.

use std::sync::{Arc, Mutex, PoisonError};

use eukhe_core::durable::record_waiting_effect;
use eukhe_types::daemon::DaemonCommand;
use eukhe_types::effects::WaitingEffect;
use serde::Deserialize as _;
use serde_json::{Map, Value};

use crate::protocol::{response_failure, response_success, DaemonResponse};
use crate::shell_approval::Relay;
use crate::worker::{SessionCore, SessionSlot};

const RELAY_COMMAND: &str = "relay_waiting_effect";

/// The session's way to the root session's ledger.
#[derive(Clone)]
pub(crate) struct WaitingRelay {
    core: Arc<Mutex<SessionCore>>,
    session: SessionSlot,
    active_session_id: String,
    relay: Relay,
}

impl WaitingRelay {
    pub(crate) fn new(
        core: Arc<Mutex<SessionCore>>,
        session: SessionSlot,
        active_session_id: String,
        relay: Relay,
    ) -> Self {
        Self {
            core,
            session,
            active_session_id,
            relay,
        }
    }

    /// The parent session, if this one is a subagent, and this session's
    /// name as the asking agent.
    fn chain(&self) -> (Option<String>, String) {
        let core = self.core.lock().unwrap_or_else(PoisonError::into_inner);
        let parent = core
            .parent_session_id
            .clone()
            .or_else(|| core.parent_active_session_id.clone());
        let agent = core
            .session_name
            .clone()
            .or_else(|| core.rlm_child_id.clone())
            .unwrap_or_else(|| self.active_session_id.clone());
        (parent, agent)
    }

    /// Record an effect of this session's own gates (the session's
    /// `waiting` sink).
    ///
    /// # Errors
    ///
    /// The ledger, or the relay to it, failed: why.
    pub(crate) async fn record_own(&self, effect: WaitingEffect) -> Result<WaitingEffect, String> {
        let (parent, agent) = self.chain();
        match parent {
            Some(parent) => self.relay_up(parent, effect, agent).await,
            None => self.record_here(effect).await,
        }
    }

    /// `relay_waiting_effect`: a subagent's effect, handed up by a child
    /// session; the first agent of the chain stays named.
    pub(crate) async fn relay(&self, payload: &Value) -> DaemonResponse {
        let effect = payload
            .get("effect")
            .and_then(|effect| WaitingEffect::deserialize(effect).ok());
        let agent = payload.get("agent").and_then(Value::as_str);
        let (Some(effect), Some(agent)) = (effect, agent) else {
            return response_failure(None, RELAY_COMMAND, "Invalid waiting effect relay", None);
        };
        let (parent, _) = self.chain();
        let recorded = match parent {
            Some(parent) => self.relay_up(parent, effect, agent.to_owned()).await,
            None => {
                self.record_here(WaitingEffect {
                    agent: Some(agent.to_owned()),
                    ..effect
                })
                .await
            }
        };
        match recorded
            .and_then(|effect| serde_json::to_value(effect).map_err(|error| error.to_string()))
        {
            Ok(data) => response_success(None, RELAY_COMMAND, Some(data)),
            Err(error) => response_failure(None, RELAY_COMMAND, &error, None),
        }
    }

    /// Record `effect` in this (root) session's ledger.
    async fn record_here(&self, effect: WaitingEffect) -> Result<WaitingEffect, String> {
        let hosted = self
            .session
            .get()
            .ok_or_else(|| "the session is not open".to_owned())?;
        record_waiting_effect(&hosted.deps().harness, effect)
            .await
            .map_err(|error| error.to_string())
    }

    /// Hand `effect` to the parent session: the effect as the root
    /// recorded it.
    async fn relay_up(
        &self,
        parent: String,
        effect: WaitingEffect,
        agent: String,
    ) -> Result<WaitingEffect, String> {
        let command = DaemonCommand::RelayWaitingEffect {
            id: None,
            active_session_id: parent,
            effect,
            agent,
            rest: Map::default(),
        };
        let command = serde_json::to_value(&command).map_err(|error| error.to_string())?;
        match (self.relay)(command).await {
            Ok(DaemonResponse {
                success: true,
                data: Some(data),
                ..
            }) => WaitingEffect::deserialize(&data).map_err(|error| {
                format!("the parent session's recorded effect is invalid: {error}")
            }),
            Ok(DaemonResponse { error, .. }) => Err(format!(
                "the parent session did not record the effect: {}",
                error.as_deref().unwrap_or("no reason")
            )),
            Err(error) => Err(format!("the relay to the parent session failed: {error:#}")),
        }
    }
}
