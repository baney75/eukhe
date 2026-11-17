//! Exactly-once effects (spec O1-O10): the `eukhe.effects` document, the
//! ledger of one conversation's effects whose outcome a restart could not
//! tell.
//!
//! Every effect is journaled where its owner keeps it (a tool call in its
//! task's checkpoints, a provider request in its generation's, a job in its
//! node), and at a restart the owner asks the compiled core what to do
//! with it (`eukhe_bend::rules::once`). An effect the core says to
//! interrupt ended `unknown`: its owner reports it interrupted where the
//! model reads it (a tool result, a notice) and, in the same commit, notes
//! it here ([`note_unknown`]), so any client can list the effects nobody
//! knows the outcome of ([`unknown_effects`]) and settle one after looking
//! ([`resolve_unknown`]). The ledger is durable with the session and never
//! drops an open effect. Clients read it ([`read_unknown_effects`]) and
//! follow it ([`watch_unknown_effects`]); its vocabulary is the shared
//! wire one (`eukhe_types::effects`).

#[cfg(test)]
mod tests;

use std::sync::Arc;

use eukhe_bend::rules::irreversible::{self, Held, LedgerEntry, Which};
use eukhe_chord::context::{await_with_context, Context};
use eukhe_chord::delta::Draft;
use eukhe_chord::json::{from_json, to_json, JsonValue};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub use eukhe_types::effects::{
    ApproveWhich, EffectKind, EffectResolution, UnknownEffect, WaitingEffect, WaitingStatus,
};

use crate::documents::{ConversationDoc, DocDefinition};
use crate::harness::Harness;
use crate::session::{CommitListener, SessionError, SessionResult, Tx, Unsubscribe};
use crate::types::{
    CommitChange, CommitPublication, ConversationId, DocumentCommitChange, DocumentIdentity,
    LatestFork,
};

/// The resolved effects the ledger keeps for people to read; an open one
/// is never dropped, whatever their number.
pub const RESOLVED_KEPT: usize = 64;

/// The persisted kind of [`EFFECTS_DOC`].
const EFFECTS_DOC_KIND: &str = "eukhe.effects";

/// The ledger: the unknown effects, oldest first, and the irreversible
/// effects recorded waiting for the user's approval, by id (`w1` first;
/// a ledger written before it had them reads as none).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EffectsState {
    pub unknown: Vec<UnknownEffect>,
    #[serde(default)]
    pub waiting: Vec<WaitingEffect>,
}

/// The `eukhe.effects` document token.
pub static EFFECTS_DOC: ConversationDoc<EffectsState> = match ConversationDoc::define(
    DocDefinition {
        kind: EFFECTS_DOC_KIND,
        version: 1,
        initial: EffectsState::default,
        migrate: None,
        checkpoint_when: Some(|_, _, _| true),
    },
    LatestFork::Initial,
) {
    Ok(token) => token,
    Err(_) => panic!("eukhe.effects has a valid version"),
};

fn index(at: usize) -> SessionResult<i64> {
    i64::try_from(at).map_err(|_| SessionError::error("An effect ledger index past i64"))
}

/// Note that a restart settled `effect` unknown, in the commit that
/// reports it interrupted. An effect is noted once: a second note of its
/// id changes nothing.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn note_unknown(
    tx: &Tx,
    conversation_id: ConversationId,
    effect: &UnknownEffect,
) -> SessionResult<()> {
    let unknown = tx
        .doc(&EFFECTS_DOC, conversation_id)
        .await?
        .child("unknown")?;
    let held: Vec<UnknownEffect> = from_json(&unknown.value()?)?;
    if held.iter().any(|noted| noted.id == effect.id) {
        return Ok(());
    }
    // The oldest resolved effect makes room; an open one stays.
    let resolved = held
        .iter()
        .filter(|noted| noted.resolution.is_some())
        .count();
    if resolved >= RESOLVED_KEPT {
        if let Some(oldest) = held.iter().position(|noted| noted.resolution.is_some()) {
            unknown.splice(index(oldest)?, 1, Vec::<JsonValue>::new())?;
        }
    }
    unknown.push([to_json(effect)?])?;
    Ok(())
}

/// Settle the open unknown effect `id` as `resolution`: the effect as
/// resolved, or `None` when the ledger holds no open effect of that id.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn resolve_unknown(
    tx: &Tx,
    conversation_id: ConversationId,
    id: &str,
    resolution: EffectResolution,
) -> SessionResult<Option<UnknownEffect>> {
    let unknown = tx
        .doc(&EFFECTS_DOC, conversation_id)
        .await?
        .child("unknown")?;
    let held: Vec<UnknownEffect> = from_json(&unknown.value()?)?;
    let Some(at) = held
        .iter()
        .position(|noted| noted.id == id && noted.resolution.is_none())
    else {
        return Ok(None);
    };
    let resolved = UnknownEffect {
        resolution: Some(resolution),
        ..held[at].clone()
    };
    unknown.splice(index(at)?, 1, [to_json(&resolved)?])?;
    Ok(Some(resolved))
}

/// The unknown effects of the conversation, oldest first, as the commit in
/// flight sees them.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn unknown_effects(
    tx: &Tx,
    conversation_id: ConversationId,
) -> SessionResult<Vec<UnknownEffect>> {
    let unknown = tx
        .doc(&EFFECTS_DOC, conversation_id)
        .await?
        .child("unknown")?;
    Ok(from_json(&unknown.value()?)?)
}

/// The waiting list of the ledger in flight; a ledger written before it
/// had one gains an empty one.
async fn waiting_list(tx: &Tx, conversation_id: ConversationId) -> SessionResult<Draft> {
    let ledger = tx.doc(&EFFECTS_DOC, conversation_id).await?;
    if !ledger.has("waiting")? {
        ledger.set("waiting", JsonValue::array())?;
    }
    Ok(ledger.child("waiting")?)
}

/// The number `N` of the id `w<N>`; `None` for any other text.
fn ledger_number(id: &str) -> Option<u64> {
    id.strip_prefix('w')?.parse().ok()
}

/// The compiled core's view of the waiting list: each effect's number and
/// whether it still waits.
fn ledger_entries(list: &[WaitingEffect]) -> Vec<LedgerEntry> {
    list.iter()
        .filter_map(|effect| {
            let held = match effect.status {
                WaitingStatus::Waiting => Held::Waiting,
                WaitingStatus::Running | WaitingStatus::Ran { .. } | WaitingStatus::Refused => {
                    Held::Taken
                }
            };
            Some(LedgerEntry {
                id: ledger_number(&effect.id)?,
                held,
            })
        })
        .collect()
}

fn core_error(error: &eukhe_bend::Error) -> SessionError {
    SessionError::error(format!("the waiting ledger's core call failed: {error}"))
}

/// Record `effect` waiting, under the next id (`w<N>`, never reused):
/// the effect as recorded.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn record_waiting(
    tx: &Tx,
    conversation_id: ConversationId,
    effect: WaitingEffect,
) -> SessionResult<WaitingEffect> {
    let waiting = waiting_list(tx, conversation_id).await?;
    let held: Vec<WaitingEffect> = from_json(&waiting.value()?)?;
    let number = u64::try_from(held.len() + 1)
        .map_err(|_| SessionError::error("A waiting ledger past u64"))?;
    let ledger =
        irreversible::hold(&ledger_entries(&held), number).map_err(|error| core_error(&error))?;
    let status = match ledger.iter().find(|entry| entry.id == number) {
        Some(LedgerEntry {
            held: Held::Waiting,
            ..
        }) => WaitingStatus::Waiting,
        Some(LedgerEntry {
            held: Held::Taken, ..
        })
        | None => return Err(SessionError::error("the core held no new waiting effect")),
    };
    let recorded = WaitingEffect {
        id: format!("w{number}"),
        status,
        ..effect
    };
    waiting.push([to_json(&recorded)?])?;
    Ok(recorded)
}

/// Take the waiting effects `which` names to run (the compiled core
/// selects them, in order): each is set running in the commit in flight.
/// The effects taken, and the named ids that took nothing (unknown, or no
/// longer waiting).
///
/// # Errors
///
/// Document access or draft failures.
pub async fn take_waiting(
    tx: &Tx,
    conversation_id: ConversationId,
    which: &ApproveWhich,
) -> SessionResult<(Vec<WaitingEffect>, Vec<String>)> {
    let held = waiting_effects(tx, conversation_id).await?;
    let named = match which {
        ApproveWhich::All => Which::All,
        ApproveWhich::Ids(ids) => {
            Which::Ids(ids.iter().filter_map(|id| ledger_number(id)).collect())
        }
    };
    let approved = irreversible::approve(&named, &ledger_entries(&held))
        .map_err(|error| core_error(&error))?;
    let mut taken = Vec::with_capacity(approved.run.len());
    for number in &approved.run {
        let id = format!("w{number}");
        if let Some(effect) =
            set_waiting_status(tx, conversation_id, &id, WaitingStatus::Running).await?
        {
            taken.push(effect);
        }
    }
    let missing = match which {
        ApproveWhich::All => Vec::new(),
        ApproveWhich::Ids(ids) => ids
            .iter()
            .filter(|id| !taken.iter().any(|effect| &effect.id == *id))
            .cloned()
            .collect(),
    };
    Ok((taken, missing))
}

/// The ids of the effects that still wait, in order (the end-of-run
/// list, from the compiled core).
///
/// # Errors
///
/// The core call failed.
pub fn still_waiting(list: &[WaitingEffect]) -> SessionResult<Vec<WaitingEffect>> {
    let numbers =
        irreversible::waiting(&ledger_entries(list)).map_err(|error| core_error(&error))?;
    Ok(numbers
        .iter()
        .filter_map(|number| {
            list.iter()
                .find(|effect| ledger_number(&effect.id) == Some(*number))
                .cloned()
        })
        .collect())
}

/// Set the status of the waiting effect `id`: the effect as changed, or
/// `None` when the ledger holds no effect of that id.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn set_waiting_status(
    tx: &Tx,
    conversation_id: ConversationId,
    id: &str,
    status: WaitingStatus,
) -> SessionResult<Option<WaitingEffect>> {
    let waiting = waiting_list(tx, conversation_id).await?;
    let held: Vec<WaitingEffect> = from_json(&waiting.value()?)?;
    let Some(at) = held.iter().position(|effect| effect.id == id) else {
        return Ok(None);
    };
    let changed = WaitingEffect {
        status,
        ..held[at].clone()
    };
    waiting.splice(index(at)?, 1, [to_json(&changed)?])?;
    Ok(Some(changed))
}

/// The waiting effects of the conversation, by id, as the commit in
/// flight sees them.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn waiting_effects(
    tx: &Tx,
    conversation_id: ConversationId,
) -> SessionResult<Vec<WaitingEffect>> {
    let waiting = waiting_list(tx, conversation_id).await?;
    Ok(from_json(&waiting.value()?)?)
}

/// The approved effects a crash left running: each ends failed
/// (interrupted, never run again) and is noted unknown (`approved:<id>`)
/// for a person to settle, in the commit in flight. The effects as ended.
///
/// # Errors
///
/// Document access or draft failures.
pub async fn interrupt_running(
    tx: &Tx,
    conversation_id: ConversationId,
    at_ms: f64,
) -> SessionResult<Vec<WaitingEffect>> {
    let running: Vec<WaitingEffect> = waiting_effects(tx, conversation_id)
        .await?
        .into_iter()
        .filter(|effect| match effect.status {
            WaitingStatus::Running => true,
            WaitingStatus::Waiting | WaitingStatus::Ran { .. } | WaitingStatus::Refused => false,
        })
        .collect();
    let mut ended = Vec::with_capacity(running.len());
    for effect in running {
        let status = WaitingStatus::Ran {
            ok: false,
            summary: "interrupted by a restart: whether it ran is unknown (/effects)".to_owned(),
        };
        if let Some(changed) = set_waiting_status(tx, conversation_id, &effect.id, status).await? {
            let unknown = UnknownEffect {
                id: format!("approved:{}", changed.id),
                kind: EffectKind::Tool,
                what: changed.command.clone(),
                at: at_ms,
                resolution: None,
            };
            note_unknown(tx, conversation_id, &unknown).await?;
            ended.push(changed);
        }
    }
    Ok(ended)
}

/// The committed ledger of the conversation (empty while it was never
/// written).
async fn read_ledger(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
) -> SessionResult<EffectsState> {
    match harness.snapshot(&EFFECTS_DOC, conversation_id, cx).await? {
        Some(value) => Ok(from_json(&JsonValue::Object(value))?),
        None => Ok(EffectsState::default()),
    }
}

/// The committed unknown effects of the conversation, oldest first (none
/// while the ledger was never written).
///
/// # Errors
///
/// The document cannot be read or decoded.
pub async fn read_unknown_effects(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
) -> SessionResult<Vec<UnknownEffect>> {
    Ok(read_ledger(harness, conversation_id, cx).await?.unknown)
}

/// The committed waiting effects of the conversation, by id (none while
/// the ledger was never written).
///
/// # Errors
///
/// The document cannot be read or decoded.
pub async fn read_waiting_effects(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
) -> SessionResult<Vec<WaitingEffect>> {
    Ok(read_ledger(harness, conversation_id, cx).await?.waiting)
}

/// A live stream of one list of a conversation's ledger (see
/// [`watch_unknown_effects`], [`watch_waiting_effects`]). Dropping it
/// stops the subscription.
pub struct LedgerUpdates<T> {
    receiver: mpsc::UnboundedReceiver<Vec<T>>,
    unsubscribe: Unsubscribe,
    task: JoinHandle<()>,
}

/// The unknown effects as they change.
pub type UnknownEffectUpdates = LedgerUpdates<UnknownEffect>;

/// The waiting effects as they change.
pub type WaitingEffectUpdates = LedgerUpdates<WaitingEffect>;

impl<T> LedgerUpdates<T> {
    /// The next list; `None` once the Harness closed or a read failed.
    pub async fn next(&mut self) -> Option<Vec<T>> {
        self.receiver.recv().await
    }
}

impl<T> Drop for LedgerUpdates<T> {
    fn drop(&mut self) {
        self.unsubscribe.unsubscribe();
        self.task.abort();
    }
}

/// Whether `publication` changed the ledger of `conversation_id`.
fn changes_effects(publication: &CommitPublication, conversation_id: ConversationId) -> bool {
    publication.changes.iter().any(|change| match change {
        CommitChange::Document(DocumentCommitChange::Document {
            record,
            conversation_id: owner,
            ..
        }) => record.kind() == EFFECTS_DOC_KIND && *owner == Some(conversation_id),
        CommitChange::Document(DocumentCommitChange::Copy {
            record,
            conversation_id: owner,
            ..
        }) => record.kind() == EFFECTS_DOC_KIND && *owner == conversation_id,
        CommitChange::Conversation(_)
        | CommitChange::Entry(_)
        | CommitChange::Task(_)
        | CommitChange::Submission(_) => false,
    })
}

/// Watch the unknown effects of `conversation_id`: the committed list
/// first, then the list after every commit that changed it (an effect
/// noted or resolved). Cancelling `cx` ends the stream.
///
/// # Errors
///
/// The Session is closed.
pub fn watch_unknown_effects(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
) -> SessionResult<UnknownEffectUpdates> {
    watch_ledger(harness, conversation_id, cx, |ledger| ledger.unknown)
}

/// Watch the waiting effects of `conversation_id`: the committed list
/// first, then the list after every commit that changed it (an effect
/// recorded, run, refused). Cancelling `cx` ends the stream.
///
/// # Errors
///
/// The Session is closed.
pub fn watch_waiting_effects(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
) -> SessionResult<WaitingEffectUpdates> {
    watch_ledger(harness, conversation_id, cx, |ledger| ledger.waiting)
}

/// Watch the list `of` the ledger of `conversation_id`, sent only when it
/// changed.
fn watch_ledger<T: Clone + PartialEq + Send + 'static>(
    harness: &Harness,
    conversation_id: ConversationId,
    cx: &Context,
    of: fn(EffectsState) -> Vec<T>,
) -> SessionResult<LedgerUpdates<T>> {
    let (ticks, mut ticked) = mpsc::unbounded_channel::<()>();
    let listener: CommitListener = Arc::new(move |publication, _| {
        if changes_effects(publication, conversation_id) {
            // A closed receiver means the stream ended; nothing to notify.
            let _ = ticks.send(());
        }
    });
    let unsubscribe = harness.subscribe_commits(listener)?;
    let (sender, receiver) = mpsc::unbounded_channel();
    let reader = harness.clone();
    let cx = cx.clone();
    let task = tokio::spawn(async move {
        let mut sent: Option<Vec<T>> = None;
        loop {
            let Ok(ledger) = read_ledger(&reader, conversation_id, &cx).await else {
                return;
            };
            let now = of(ledger);
            if sent.as_ref() != Some(&now) {
                sent = Some(now.clone());
                if sender.send(now).is_err() {
                    return;
                }
            }
            match await_with_context(ticked.recv(), &cx).await {
                Ok(Some(())) => {}
                Ok(None) | Err(_) => return,
            }
            // Coalesce a burst of commits into one read.
            while ticked.try_recv().is_ok() {}
        }
    });
    Ok(LedgerUpdates {
        receiver,
        unsubscribe,
        task,
    })
}
