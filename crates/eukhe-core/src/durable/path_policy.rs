//! The path policy of a session (`laws/path_policy/` (G1)) and
//! the gate the document tools pass every path access through.
//!
//! [`session_policy`] builds the one [`PathPolicy`] the `bash` tool and
//! the document tools judge paths by. Its protected and secret sets come
//! from the state list ([`crate::state_list`]) and the settings loaders:
//!
//! - the agent dir, the work root and the executables' dir, each with
//!   every entry directly in it: the global scope's files live in the agent
//!   dir (`settings.json`, `config.cfg`, `subagent.cfg`, `models.json`,
//!   `auth.json`, `binds.cfg`);
//! - the run dir and the auto-start dirs, with everything under them;
//! - the profile dir ([`PROFILE_DIR`], `agents/<agent>.cfg`);
//! - the project config dir of the session (`<cwd>/.eukhe`) and every
//!   `.eukhe` dir below the workspace root ([`CONFIG_DIR_NAME`]);
//! - every settings file the session loads and re-reads
//!   ([`EukheSettings::watched`]);
//! - every `.git` dir below the workspace root (its config and hooks name
//!   programs git runs), and every directory on `PATH` (a file put there
//!   replaces a command).
//!
//! The secret set holds the state list's credentials and the credential
//! stores other programs keep in the home dir.
//!
//! [`DocumentsGate`] is the gate of one document tool call: an access the
//! user's auto set (`shell.autoEffects`) does not allow is irreversible,
//! as a `bash` step is: it waits for the user's approval after the run,
//! recorded with the call that re-runs it, or, watching, is asked through
//! the session's approval sink.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eukhe_bend::rules::irreversible::{self, Declared, Fate, Route, Said};
use eukhe_docs::{Access, AccessGate, AccessRefusal};
use eukhe_types::effects::{ApprovalMode, WaitingAt, WaitingKind};
use eukhe_types::path_policy::{PathClass, PathPolicy, PathRule, RealPath};
use eukhe_types::shell::{AutoSet, ShellAnswer, ShellApprovalRequest, ShellEffects};

use super::deps::ShellApprovalSink;
use super::settings::EukheSettings;
use super::shell::RefusalText;
use super::waiting::{core_mode, Held, Recorder};
use crate::settings::{CONFIG_DIR_NAME, PROFILE_DIR};
use crate::state_list::{state_list, Install, StateKind, StatePart};

/// The credential stores other programs keep in the home dir: ssh keys,
/// cloud and registry credentials, password stores (the AWS files are
/// Eukhe's credentials: the state list names them).
const HOME_SECRETS: &[&str] = &[
    ".ssh",
    ".azure",
    ".config/gcloud",
    ".config/gh",
    ".docker/config.json",
    ".kube",
    ".gnupg",
    ".password-store",
    ".local/share/keyrings",
    ".netrc",
    ".git-credentials",
    ".npmrc",
    ".pypirc",
    ".cargo/credentials",
    ".cargo/credentials.toml",
];

/// The environment variables that move a credential store away from its
/// home-dir place (the gcloud config dir).
const SECRET_ENV: &[&str] = &["CLOUDSDK_CONFIG"];

/// Where a session's files are: what [`policy`] reads besides the
/// workspace.
pub(crate) struct Layout<'a> {
    pub(crate) agent_dir: &'a Path,
    pub(crate) settings: &'a EukheSettings,
    pub(crate) home: Option<&'a Path>,
    /// The running executable's path; `None` when it does not resolve.
    pub(crate) executable: Option<&'a Path>,
    /// The `PATH` value; `None` when unset.
    pub(crate) path: Option<&'a OsStr>,
    /// The value of an environment variable; `None` when unset.
    pub(crate) env: &'a dyn Fn(&str) -> Option<OsString>,
}

/// The policy of `workspace` (its one root) over `layout`.
pub(crate) fn policy(layout: &Layout<'_>, workspace: &Path) -> PathPolicy {
    let mut protected = vec![
        PathRule::Tree(layout.agent_dir.join(PROFILE_DIR)),
        PathRule::Tree(layout.settings.cwd().join(CONFIG_DIR_NAME)),
        PathRule::Named(OsString::from(CONFIG_DIR_NAME)),
        PathRule::Named(OsString::from(".git")),
    ];
    let mut secret = Vec::new();
    let state = state_list(&Install {
        agent_dir: layout.agent_dir,
        home: layout.home,
        executable: layout.executable,
        cwd: workspace,
        env: layout.env,
    });
    for StatePart { kind, path } in state {
        match kind {
            // The dir and its entries: a write deeper (a session's
            // artifacts, another run's copy) asks as any write outside.
            StateKind::AgentDir | StateKind::WorkRoot | StateKind::Executables => {
                protected.push(PathRule::Dir(path));
            }
            StateKind::RunDir | StateKind::AutoStart => protected.push(PathRule::Tree(path)),
            StateKind::Credentials => secret.push(path),
        }
    }
    protected.extend(
        layout
            .settings
            .watched()
            .iter()
            .cloned()
            .map(PathRule::Tree),
    );
    protected.extend(
        layout
            .path
            .into_iter()
            .flat_map(std::env::split_paths)
            .map(|dir| PathRule::Dir(workspace.join(dir))),
    );

    secret.extend(
        layout
            .home
            .into_iter()
            .flat_map(|home| HOME_SECRETS.iter().map(|entry| home.join(entry))),
    );
    secret.extend(
        SECRET_ENV
            .iter()
            .filter_map(|name| (layout.env)(name))
            .filter(|value| !value.is_empty())
            .map(|value| workspace.join(value)),
    );
    // The OS temp dir is scratch: every session writes its probe
    // scripts and files there, and a write outside the workspace would
    // ask for each. Its writes run without asking.
    let mut roots = vec![workspace.to_path_buf()];
    let temp = std::env::temp_dir();
    if temp != workspace {
        roots.push(temp);
    }
    PathPolicy::new(roots, protected, secret)
}

/// The path policy of a call of a session (its agent dir and settings)
/// working in `workspace` (the call's agent cwd, never a cwd the model
/// can change).
pub(crate) fn session_policy(
    agent_dir: &Path,
    settings: &EukheSettings,
    workspace: &Path,
) -> PathPolicy {
    let home = eukhe_types::platform::home_dir();
    let executable = std::env::current_exe().ok();
    let path = std::env::var_os("PATH");
    policy(
        &Layout {
            agent_dir,
            settings,
            home: home.as_deref(),
            executable: executable.as_deref(),
            path: path.as_deref(),
            env: &|name| std::env::var_os(name),
        },
        workspace,
    )
}

/// The effects of `access` to `real` under `policy`, judged on `real` as
/// it is (no second resolution: a loop stays a loop): a search of a
/// directory that holds a secret reads the secret.
pub(crate) fn effects(policy: &PathPolicy, access: Access, real: &RealPath) -> ShellEffects {
    let judged = eukhe_shell::paths::judge(policy, real);
    match access {
        Access::Search if judged.holds_secret => ShellEffects::read_of(PathClass::Secret),
        Access::Read | Access::List | Access::Search => ShellEffects::read_of(judged.class),
        Access::Write => ShellEffects::write_to(judged.class),
    }
}

/// The call a document access belongs to, as `/approve` re-runs it: the
/// function, the cwd its relative paths resolve against, its arguments.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Rerun {
    pub(crate) function: &'static str,
    pub(crate) cwd: PathBuf,
    pub(crate) args: serde_json::Value,
}

/// Which access a gate lets through whatever its effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Allowed {
    /// None: every access is gated.
    Gated,
    /// The one path the user approved (`/approve` of a document access).
    Approved(PathBuf),
}

/// The gate of one document tool call (or one kernel `fs.*` request).
pub(crate) struct DocumentsGate {
    pub(crate) policy: PathPolicy,
    pub(crate) auto: AutoSet,
    pub(crate) mode: ApprovalMode,
    /// The session's approval sink (watching); `None`: no client can
    /// answer, and every access asked waits.
    pub(crate) sink: Option<ShellApprovalSink>,
    /// Where an access that waits is recorded.
    pub(crate) recorder: Recorder,
    /// The call's id: each question's id is the call id and its number.
    pub(crate) call: String,
    /// The call as it re-runs.
    pub(crate) rerun: Rerun,
    pub(crate) allowed: Allowed,
    pub(crate) asked: AtomicU64,
    /// The runtime the sink's answer arrives on; the gate runs on a
    /// blocking thread of it.
    pub(crate) runtime: tokio::runtime::Handle,
}

/// Why a [`DocumentsGate`] stopped an access. The effects open a
/// refusal, as a `bash` step's stop line names them: the model learns
/// which effect needs the user's approval.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DocumentsRefusal {
    /// The user said no while watching.
    #[error("({}) {}", effects.names(), RefusalText::DENIED)]
    Denied { effects: ShellEffects },
    /// It waits for `/approve`: the model's text.
    #[error("{0}")]
    Waits(String),
    /// It waits, but the ledger could not record it.
    #[error("({}) it needs the user's approval and could not be recorded waiting: {error}", effects.names())]
    Unrecorded {
        effects: ShellEffects,
        error: String,
    },
}

impl DocumentsGate {
    /// The access as people read it.
    fn shown(access: Access, real: &RealPath) -> String {
        let verb = match access {
            Access::Read => "read",
            Access::List => "list",
            Access::Search => "search",
            Access::Write => "write",
        };
        eukhe_shell::visible(&format!("{verb} {}", real.path().display()))
    }

    /// Record the access waiting with the call that re-runs it.
    fn wait(&self, access: Access, real: &RealPath, effects: ShellEffects) -> DocumentsRefusal {
        let held = Held {
            kind: WaitingKind::DocumentWrite,
            command: Self::shown(access, real),
            cwd: self.rerun.cwd.display().to_string(),
            effects,
            payload: serde_json::json!({
                "function": self.rerun.function,
                "cwd": self.rerun.cwd,
                "args": self.rerun.args,
                "path": real.path(),
            }),
        };
        match self.runtime.block_on(self.recorder.record(held)) {
            Ok(effect) => DocumentsRefusal::Waits(effect.waiting_text(WaitingAt::Call)),
            Err(error) => DocumentsRefusal::Unrecorded { effects, error },
        }
    }

    /// Ask the user (watching): what they said.
    fn ask(&self, access: Access, real: &RealPath, effects: ShellEffects) -> Said {
        let Some(sink) = &self.sink else {
            return Said::Silent;
        };
        let step = self.asked.fetch_add(1, Ordering::Relaxed);
        let request = ShellApprovalRequest {
            request_id: format!("{}:{step}", self.call),
            step,
            command: Self::shown(access, real),
            effects,
        };
        match self.runtime.block_on(sink(request)) {
            ShellAnswer::Approve => Said::Yes,
            ShellAnswer::Deny => Said::No,
            ShellAnswer::Unanswered => Said::Silent,
        }
    }
}

impl AccessGate for DocumentsGate {
    fn check(&self, access: Access, real: &RealPath, _display: &str) -> Result<(), AccessRefusal> {
        match &self.allowed {
            Allowed::Approved(path) if path.as_path() == real.path() => return Ok(()),
            Allowed::Approved(_) | Allowed::Gated => {}
        }
        let effects = effects(&self.policy, access, real);
        let stopped = match irreversible::route(
            self.auto,
            Declared::Effects(effects),
            core_mode(self.mode),
        ) {
            Route::Runs => return Ok(()),
            Route::Waits => self.wait(access, real, effects),
            Route::Asks => match irreversible::settle(self.ask(access, real, effects)) {
                Fate::Runs => return Ok(()),
                Fate::Refused => DocumentsRefusal::Denied { effects },
                Fate::Waits => self.wait(access, real, effects),
            },
        };
        Err(AccessRefusal::new(stopped))
    }
}

#[cfg(test)]
mod tests;
