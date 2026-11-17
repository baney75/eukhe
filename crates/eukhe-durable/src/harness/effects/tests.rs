use eukhe_ai::providers::faux::RegisterFauxProviderOptions;
use eukhe_chord::json::{from_json, JsonValue};
use eukhe_types::shell::ShellEffect;

use super::{
    interrupt_running, read_unknown_effects, read_waiting_effects, record_waiting,
    set_waiting_status, still_waiting, take_waiting, waiting_effects, ApproveWhich, EffectKind,
    EffectsState, UnknownEffect, WaitingEffect, WaitingStatus, EFFECTS_DOC,
};
use crate::harness::tests::chat_support::{chat_setup, open_chat, OpenChat};
use crate::harness::tests::support::context;
use crate::storage::test_support::{jsonl, store_path};

fn push(command: &str) -> WaitingEffect {
    WaitingEffect {
        id: String::new(),
        kind: eukhe_types::effects::WaitingKind::ShellStep,
        command: command.to_owned(),
        cwd: "/w".to_owned(),
        effects: vec![ShellEffect::Net],
        agent: None,
        session_id: "s1".to_owned(),
        asked_at_ms: 1,
        status: WaitingStatus::Refused,
        payload: serde_json::json!({ "script": command }),
    }
}

async fn open(path: &std::path::Path) -> OpenChat {
    let setup = chat_setup(RegisterFauxProviderOptions::default());
    open_chat(jsonl(path).await, &setup, None).await.unwrap()
}

/// Record `effects` in one commit: the effects as recorded.
async fn record(chat: &OpenChat, effects: Vec<WaitingEffect>) -> Vec<WaitingEffect> {
    let id = chat.root.id();
    chat.harness
        .commit(
            move |tx| async move {
                let mut recorded = Vec::new();
                for effect in effects {
                    recorded.push(record_waiting(&tx, id, effect).await?);
                }
                Ok(recorded)
            },
            context(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn recorded_effects_take_the_next_id_wait_and_survive_a_reopen() {
    let (_dir, path) = store_path();
    let chat = open(&path).await;
    let recorded = record(&chat, vec![push("git push"), push("npm publish")]).await;
    let id = chat.root.id();
    let ran = WaitingStatus::Ran {
        ok: true,
        summary: "exit 0".to_owned(),
    };
    let changed = chat
        .harness
        .commit(
            {
                let ran = ran.clone();
                move |tx| async move { set_waiting_status(&tx, id, "w1", ran).await }
            },
            context(),
        )
        .await
        .unwrap();
    chat.harness.close(context()).await.unwrap();
    let reopened = open(&path).await;
    let after = read_waiting_effects(&reopened.harness, reopened.root.id(), context())
        .await
        .unwrap();
    let first = WaitingEffect {
        id: "w1".to_owned(),
        status: WaitingStatus::Waiting,
        ..push("git push")
    };
    let second = WaitingEffect {
        id: "w2".to_owned(),
        status: WaitingStatus::Waiting,
        ..push("npm publish")
    };
    let first_ran = WaitingEffect {
        status: ran,
        ..first.clone()
    };
    assert_eq!(
        (recorded, changed, after),
        (
            vec![first, second.clone()],
            Some(first_ran.clone()),
            vec![first_ran, second]
        )
    );
    reopened.harness.close(context()).await.unwrap();
}

#[tokio::test]
async fn a_ledger_written_without_a_waiting_list_reads_none_and_records() {
    let old: EffectsState = from_json(&serde_json::json!({ "unknown": [] }).into()).unwrap();
    assert_eq!(old, EffectsState::default());
    let (_dir, path) = store_path();
    let chat = open(&path).await;
    let id = chat.root.id();
    // The ledger as an earlier release wrote it: no `waiting` key.
    chat.harness
        .commit(
            move |tx| async move {
                tx.doc(&EFFECTS_DOC, id).await?.delete("waiting")?;
                Ok(())
            },
            context(),
        )
        .await
        .unwrap();
    let stored = chat
        .harness
        .snapshot(&EFFECTS_DOC, id, context())
        .await
        .unwrap()
        .map(JsonValue::Object);
    let listed = read_waiting_effects(&chat.harness, id, context())
        .await
        .unwrap();
    let recorded = record(&chat, vec![push("git push")]).await;
    assert_eq!(
        (
            stored,
            listed,
            recorded
                .into_iter()
                .map(|effect| effect.id)
                .collect::<Vec<_>>()
        ),
        (
            Some(serde_json::json!({ "unknown": [] }).into()),
            Vec::new(),
            vec!["w1".to_owned()]
        )
    );
    chat.harness.close(context()).await.unwrap();
}

#[tokio::test]
async fn a_restart_ends_a_running_effect_failed_and_notes_it_unknown() {
    let (_dir, path) = store_path();
    let chat = open(&path).await;
    record(&chat, vec![push("git push"), push("npm publish")]).await;
    let id = chat.root.id();
    let (ended, ledger) = chat
        .harness
        .commit(
            move |tx| async move {
                set_waiting_status(&tx, id, "w1", WaitingStatus::Running).await?;
                let ended = interrupt_running(&tx, id, 9.0).await?;
                Ok((ended, waiting_effects(&tx, id).await?))
            },
            context(),
        )
        .await
        .unwrap();
    let unknown = read_unknown_effects(&chat.harness, id, context())
        .await
        .unwrap();
    let interrupted = WaitingEffect {
        id: "w1".to_owned(),
        status: WaitingStatus::Ran {
            ok: false,
            summary: "interrupted by a restart: whether it ran is unknown (/effects)".to_owned(),
        },
        ..push("git push")
    };
    assert_eq!(
        (ended, ledger, unknown),
        (
            vec![interrupted.clone()],
            vec![
                interrupted,
                WaitingEffect {
                    id: "w2".to_owned(),
                    status: WaitingStatus::Waiting,
                    ..push("npm publish")
                }
            ],
            vec![UnknownEffect {
                id: "approved:w1".to_owned(),
                kind: EffectKind::Tool,
                what: "git push".to_owned(),
                at: 9.0,
                resolution: None,
            }]
        )
    );
    chat.harness.close(context()).await.unwrap();
}

#[tokio::test]
async fn approve_takes_each_waiting_effect_once_and_names_the_rest() {
    let (_dir, path) = store_path();
    let chat = open(&path).await;
    record(&chat, vec![push("git push"), push("npm publish")]).await;
    let id = chat.root.id();
    let take = |which: ApproveWhich| {
        chat.harness.commit(
            move |tx| async move { take_waiting(&tx, id, &which).await },
            context(),
        )
    };
    let ids = |taken: &[WaitingEffect]| -> Vec<String> {
        taken.iter().map(|effect| effect.id.clone()).collect()
    };
    let (first, first_missing) = take(ApproveWhich::Ids(vec!["w2".into(), "w9".into()]))
        .await
        .unwrap();
    let (again, again_missing) = take(ApproveWhich::Ids(vec!["w2".into()])).await.unwrap();
    let (rest, rest_missing) = take(ApproveWhich::All).await.unwrap();
    let ledger = read_waiting_effects(&chat.harness, id, context())
        .await
        .unwrap();
    assert_eq!(
        (
            (ids(&first), first_missing),
            (ids(&again), again_missing),
            (ids(&rest), rest_missing),
            ledger
                .iter()
                .map(|effect| effect.status.clone())
                .collect::<Vec<_>>(),
            still_waiting(&ledger).unwrap(),
        ),
        (
            (vec!["w2".to_owned()], vec!["w9".to_owned()]),
            (Vec::new(), vec!["w2".to_owned()]),
            (vec!["w1".to_owned()], Vec::new()),
            vec![WaitingStatus::Running, WaitingStatus::Running],
            Vec::new(),
        )
    );
    chat.harness.close(context()).await.unwrap();
}
