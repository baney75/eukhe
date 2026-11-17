//! Effects on the wire: an effect whose outcome a restart could not tell
//! ([`UnknownEffect`]), how a client settles one ([`EffectResolution`]),
//! an irreversible effect that waits for the user's approval
//! ([`WaitingEffect`]), and the replies of the daemon commands
//! `get_unknown_effects` ([`UnknownEffectsReply`]),
//! `resolve_unknown_effect` ([`ResolvedEffectReply`]),
//! `get_waiting_effects` ([`WaitingEffectsReply`]) and
//! `approve_waiting_effects` ([`ApprovedEffectsReply`]). The session keeps
//! them in its durable `eukhe.effects` ledger
//! (`eukhe_durable::harness::effects`); the `unknown_effects_update` and
//! `waiting_effects_update` session events carry the whole lists after
//! each change.

use serde::{Deserialize, Serialize};

use crate::shell::ShellEffect;

/// What kind of effect it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectKind {
    /// A tool call.
    Tool,
    /// A provider request.
    Request,
    /// A background job.
    Job,
    /// A file write.
    Write,
    /// A subagent spawn.
    Spawn,
    /// A cron fire or a heartbeat.
    Fire,
    /// The delivery of a queued or steering message.
    Message,
}

impl EffectKind {
    /// The wire name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Request => "request",
            Self::Job => "job",
            Self::Write => "write",
            Self::Spawn => "spawn",
            Self::Fire => "fire",
            Self::Message => "message",
        }
    }
}

/// How a client settled an unknown effect after looking at the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectResolution {
    /// It ran.
    Ran,
    /// It did not run.
    NotRun,
}

impl EffectResolution {
    /// The word a person types for it (`eukhe effects resolve <id>
    /// ran|not-run`, `/effects ran|not-run <id>`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ran => "ran",
            Self::NotRun => "not-run",
        }
    }

    /// The resolution `word` names ([`Self::word`]), in any case.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        [Self::Ran, Self::NotRun]
            .into_iter()
            .find(|resolution| resolution.word().eq_ignore_ascii_case(word))
    }
}

/// An effect whose outcome a restart could not tell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownEffect {
    /// The effect's id in its conversation: a tool call's id, a request's
    /// `request:<task>:<attempt>`, a job's `job:<id>`.
    pub id: String,
    pub kind: EffectKind,
    /// What it is, for people: the tool and its call, the job's label.
    pub what: String,
    /// When the restart settled it, in ms since the epoch.
    pub at: f64,
    /// How a client resolved it; none while nobody knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<EffectResolution>,
}

impl UnknownEffect {
    /// Nobody settled it yet.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.resolution.is_none()
    }

    /// One line for people: `<id> <kind> <unknown|ran|not-run> <what>`.
    #[must_use]
    pub fn line(&self) -> String {
        let state = self.resolution.map_or("unknown", EffectResolution::word);
        format!("{} {} {state} {}", self.id, self.kind.name(), self.what)
    }
}

/// The `get_unknown_effects` response data: the session's ledger, oldest
/// first, open and resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnknownEffectsReply {
    pub effects: Vec<UnknownEffect>,
}

/// The `resolve_unknown_effect` response data: the effect as resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedEffectReply {
    pub effect: UnknownEffect,
}

/// When an irreversible effect is asked about (`approvals.mode`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalMode {
    /// It waits, unasked, for `/approve` after the run.
    #[default]
    End,
    /// It is asked mid-run; no answer leaves it waiting.
    Watch,
}

impl ApprovalMode {
    /// The setting's value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::Watch => "watch",
        }
    }

    /// The mode a setting value names; `None` for any other value.
    #[must_use]
    pub fn from_setting(value: &str) -> Option<Self> {
        [Self::End, Self::Watch]
            .into_iter()
            .find(|mode| mode.as_str() == value)
    }
}

/// What a waiting effect re-runs when approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaitingKind {
    /// A `bash` step: its command in its cwd.
    ShellStep,
    /// A document tool's write: the tool call, with its path allowed.
    DocumentWrite,
    /// A `python` cell outside the auto set: its code, in the
    /// conversation's kernel.
    PythonCell,
    /// An MCP tool call not declared read-only: the call with its
    /// arguments.
    McpCall,
}

/// Where a waiting effect stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaitingStatus {
    /// Not run; `/approve` runs it.
    Waiting,
    /// Approved and running.
    Running,
    /// Approved and ran; `summary` is its outcome for people and the
    /// model (exit code, output tail).
    Ran { ok: bool, summary: String },
    /// The user said no while watching: it never runs.
    Refused,
}

/// An irreversible effect that did not run and waits for the user's
/// approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitingEffect {
    /// `w<N>`: per root session, never reused.
    pub id: String,
    pub kind: WaitingKind,
    /// What runs, for people.
    pub command: String,
    /// The working directory it runs in.
    pub cwd: String,
    /// Every effect of it; none for an effect of unknown kind (an MCP
    /// tool call not declared read-only).
    pub effects: Vec<ShellEffect>,
    /// The subagent that asked; `None` for the root agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The session that asked.
    pub session_id: String,
    /// When it was recorded, in ms since the epoch.
    pub asked_at_ms: u64,
    pub status: WaitingStatus,
    /// What re-runs it (by kind: the tool call's arguments, the server
    /// and tool).
    pub payload: serde_json::Value,
}

/// Where in its tool call an effect waits, for the model's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitingAt {
    /// A step of a script: it and every later step did not run.
    Step(u64),
    /// The whole call did not run.
    Call,
}

impl WaitingEffect {
    /// The effects' names joined by `+` (`exec+net`); `unknown` for an
    /// effect of unknown kind.
    #[must_use]
    pub fn effect_names(&self) -> String {
        if self.effects.is_empty() {
            return "unknown".to_owned();
        }
        self.effects
            .iter()
            .map(|effect| effect.as_str())
            .collect::<Vec<_>>()
            .join("+")
    }

    /// One line for people: `w3  git push origin main  (net)`, the asking
    /// subagent named.
    #[must_use]
    pub fn line(&self) -> String {
        let effects = self.effect_names();
        match &self.agent {
            Some(agent) => format!("{}  {}  ({effects}, {agent})", self.id, self.command),
            None => format!("{}  {}  ({effects})", self.id, self.command),
        }
    }

    /// The tool result the model reads for this effect when it waits.
    #[must_use]
    pub fn waiting_text(&self, at: WaitingAt) -> String {
        let (id, effects, command) = (&self.id, self.effect_names(), &self.command);
        let what = match at {
            WaitingAt::Step(step) => format!(
                "step {step} ({effects}) `{command}` is {id}; it and the steps after it did not run"
            ),
            WaitingAt::Call => format!("`{command}` ({effects}) is {id}; it did not run"),
        };
        format!(
            "[waiting for the user's approval: {what}. Continue with the rest of the work; do not \
             try another way to do it.]"
        )
    }

    /// The transcript row of an approved effect that ran; `None` while it
    /// did not.
    #[must_use]
    pub fn ran_text(&self) -> Option<String> {
        match &self.status {
            WaitingStatus::Ran { ok, summary } => {
                let outcome = if *ok { "ran" } else { "failed" };
                Some(format!(
                    "[the user approved {}: `{}` {outcome}]\n{summary}",
                    self.id, self.command
                ))
            }
            WaitingStatus::Waiting | WaitingStatus::Running | WaitingStatus::Refused => None,
        }
    }

    #[must_use]
    pub fn is_waiting(&self) -> bool {
        match self.status {
            WaitingStatus::Waiting => true,
            WaitingStatus::Running | WaitingStatus::Ran { .. } | WaitingStatus::Refused => false,
        }
    }
}

/// The one end-of-run text: what waits, one line per effect, then how to
/// run them; `None` when nothing waits.
#[must_use]
pub fn waiting_summary(effects: &[WaitingEffect]) -> Option<String> {
    let lines: Vec<String> = effects
        .iter()
        .filter(|effect| effect.is_waiting())
        .map(WaitingEffect::line)
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Waiting for approval:\n{}\n`/approve` runs them all, `/approve <id>` one; ignoring them \
         changes nothing.",
        lines.join("\n")
    ))
}

/// Why an id named in `/approve` ran nothing: `w3 already ran`, `no
/// waiting effect w3`, read in the session's `ledger`.
#[must_use]
pub fn missing_line(ledger: &[WaitingEffect], id: &str) -> String {
    match ledger.iter().find(|effect| effect.id == id) {
        Some(effect) => match effect.status {
            WaitingStatus::Running => format!("{id} is running"),
            WaitingStatus::Ran { .. } => format!("{id} already ran"),
            WaitingStatus::Refused => format!("{id} was refused"),
            WaitingStatus::Waiting => format!("no waiting effect {id}"),
        },
        None => format!("no waiting effect {id}"),
    }
}

/// Which waiting effects `/approve` runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApproveWhich {
    All,
    Ids(Vec<String>),
}

impl ApproveWhich {
    /// The selection the words after `/approve` name: none or `all` is
    /// every waiting effect, else the ids.
    #[must_use]
    pub fn from_words<S: AsRef<str>>(words: &[S]) -> Self {
        if words.is_empty() || words.iter().any(|word| word.as_ref() == "all") {
            Self::All
        } else {
            Self::Ids(words.iter().map(|word| word.as_ref().to_owned()).collect())
        }
    }
}

/// The `get_waiting_effects` response data: the session's ledger, oldest
/// first, every status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaitingEffectsReply {
    pub effects: Vec<WaitingEffect>,
}

/// The `approve_waiting_effects` response data: the effects that ran, in
/// order, and the named ids that ran nothing (unknown or not waiting).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovedEffectsReply {
    pub ran: Vec<WaitingEffect>,
    pub missing: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire form: camelCase fields and kinds, no `resolution` while
    /// open, `notRun` once settled so.
    #[test]
    fn an_unknown_effect_round_trips_its_wire_form() {
        let open = serde_json::json!({
            "id": "tool:call-1",
            "kind": "tool",
            "what": "bash: make deploy",
            "at": 1_700_000_000_000.0,
        });
        let effect: UnknownEffect = serde_json::from_value(open.clone()).expect("open effect");
        assert_eq!(
            effect,
            UnknownEffect {
                id: "tool:call-1".to_owned(),
                kind: EffectKind::Tool,
                what: "bash: make deploy".to_owned(),
                at: 1_700_000_000_000.0,
                resolution: None,
            }
        );
        assert_eq!(serde_json::to_value(&effect).expect("encodes"), open);
        let settled = UnknownEffect {
            resolution: Some(EffectResolution::NotRun),
            ..effect
        };
        assert_eq!(
            serde_json::to_value(&settled).expect("encodes")["resolution"],
            "notRun"
        );
    }

    /// The typed words read back as their resolution in any case; any
    /// other word is none.
    #[test]
    fn resolution_words_read_back() {
        for resolution in [EffectResolution::Ran, EffectResolution::NotRun] {
            assert_eq!(
                EffectResolution::from_word(resolution.word()),
                Some(resolution)
            );
        }
        assert_eq!(
            EffectResolution::from_word("NOT-RUN"),
            Some(EffectResolution::NotRun)
        );
        assert_eq!(EffectResolution::from_word("notRun"), None);
    }

    #[test]
    fn the_line_names_the_effect_and_its_state() {
        let effect = UnknownEffect {
            id: "job:7".to_owned(),
            kind: EffectKind::Job,
            what: "cargo build".to_owned(),
            at: 0.0,
            resolution: None,
        };
        assert_eq!(effect.line(), "job:7 job unknown cargo build");
        let ran = UnknownEffect {
            resolution: Some(EffectResolution::Ran),
            ..effect
        };
        assert_eq!(ran.line(), "job:7 job ran cargo build");
    }

    fn waiting(id: &str, command: &str, agent: Option<&str>) -> WaitingEffect {
        WaitingEffect {
            id: id.to_owned(),
            kind: WaitingKind::ShellStep,
            command: command.to_owned(),
            cwd: "/w".to_owned(),
            effects: vec![ShellEffect::Exec, ShellEffect::Net],
            agent: agent.map(str::to_owned),
            session_id: "s1".to_owned(),
            asked_at_ms: 7,
            status: WaitingStatus::Waiting,
            payload: serde_json::json!({ "script": command }),
        }
    }

    #[test]
    fn a_waiting_effect_round_trips_its_wire_form() {
        let ran = WaitingEffect {
            status: WaitingStatus::Ran {
                ok: true,
                summary: "exit 0".to_owned(),
            },
            ..waiting("w3", "git push", Some("scout"))
        };
        let json = serde_json::json!({
            "id": "w3",
            "kind": "shellStep",
            "command": "git push",
            "cwd": "/w",
            "effects": ["exec", "net"],
            "agent": "scout",
            "sessionId": "s1",
            "askedAtMs": 7,
            "status": { "ran": { "ok": true, "summary": "exit 0" } },
            "payload": { "script": "git push" },
        });
        assert_eq!(serde_json::to_value(&ran).expect("encodes"), json);
        assert_eq!(
            serde_json::from_value::<WaitingEffect>(json).expect("decodes"),
            ran
        );
    }

    #[test]
    fn the_summary_lists_only_what_waits() {
        let ran = WaitingEffect {
            status: WaitingStatus::Ran {
                ok: true,
                summary: String::new(),
            },
            ..waiting("w1", "npm publish", None)
        };
        let effects = [
            ran.clone(),
            waiting("w2", "git push origin main", None),
            waiting("w3", "cargo publish", Some("release")),
        ];
        assert_eq!(
            (waiting_summary(&effects), waiting_summary(&[ran])),
            (
                Some(
                    "Waiting for approval:\nw2  git push origin main  (exec+net)\nw3  cargo \
                     publish  (exec+net, release)\n`/approve` runs them all, `/approve <id>` \
                     one; ignoring them changes nothing."
                        .to_owned()
                ),
                None
            )
        );
    }

    #[test]
    fn the_model_reads_where_the_effect_waits() {
        let effect = waiting("w3", "git push origin main", None);
        assert_eq!(
            effect.waiting_text(WaitingAt::Step(2)),
            "[waiting for the user's approval: step 2 (exec+net) `git push origin main` is w3; \
             it and the steps after it did not run. Continue with the rest of the work; do not \
             try another way to do it.]"
        );
    }

    #[test]
    fn approve_words_name_all_or_ids() {
        assert_eq!(
            [
                ApproveWhich::from_words::<&str>(&[]),
                ApproveWhich::from_words(&["all"]),
                ApproveWhich::from_words(&["w1", "w3"]),
            ],
            [
                ApproveWhich::All,
                ApproveWhich::All,
                ApproveWhich::Ids(vec!["w1".to_owned(), "w3".to_owned()]),
            ]
        );
    }

    #[test]
    fn a_missing_id_says_why() {
        let ran = WaitingEffect {
            status: WaitingStatus::Ran {
                ok: true,
                summary: String::new(),
            },
            ..waiting("w3", "git push", None)
        };
        assert_eq!(
            [
                missing_line(std::slice::from_ref(&ran), "w3"),
                missing_line(&[ran], "w9")
            ],
            [
                "w3 already ran".to_owned(),
                "no waiting effect w9".to_owned()
            ]
        );
    }
}
