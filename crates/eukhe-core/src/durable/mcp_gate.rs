//! `mcp.gate`: the approval gate of an MCP tool call, which the kernel's
//! MCP client (`rlm.mcp`) asks before each call. The client gathers one
//! fact: whether the server declares the tool read-only (`readOnlyHint:
//! true`). A read-only tool declares the effect `read`; any other declares
//! nothing, an effect of unknown kind, which is in no grant. The compiled
//! core routes it (`eukhe_bend::rules::irreversible::route`): in the
//! grant it runs; else it waits for the user's approval after the run,
//! recorded in the ledger with its server, tool and arguments (`/approve`
//! re-runs it), or, watching, the session's approval sink asks (no answer:
//! it waits). The reply is `{"run": true}`, or `{"run": false, "text"}`
//! with the text the model reads.

use std::sync::{Arc, Weak};

use eukhe_bend::rules::irreversible::{self, Declared, Fate, Route, Said};
use eukhe_types::effects::{WaitingAt, WaitingKind};
use eukhe_types::shell::{ShellAnswer, ShellApprovalRequest, ShellEffect, ShellEffects};
use futures::FutureExt as _;
use serde_json::{json, Value};

use super::deps::HostCall;
use super::shell::{auto_set, RefusalText};
use super::waiting::{core_mode, Held, Recorder};
use super::HostDeps;

/// The longest command a waiting MCP call shows, in characters.
const SHOWN_CHARS: usize = 200;

/// Register `mcp.gate` on the session's host requests.
pub(crate) fn register(deps: &Arc<HostDeps>) {
    let weak = Arc::downgrade(deps);
    deps.host_requests.register(
        "mcp.gate",
        Arc::new(move |call: HostCall| gate(weak.clone(), call).boxed()),
    );
}

/// The call as people read it: `server.tool {arguments}`, bounded.
fn shown(server: &str, tool: &str, arguments: &Value) -> String {
    let whole = format!("{server}.{tool} {arguments}");
    let mut shown: String = whole.chars().take(SHOWN_CHARS).collect();
    if shown.len() < whole.len() {
        shown.push('…');
    }
    eukhe_shell::visible(&shown)
}

async fn gate(deps: Weak<HostDeps>, call: HostCall) -> anyhow::Result<Value> {
    let deps = deps
        .upgrade()
        .ok_or_else(|| anyhow::anyhow!("the session is closed"))?;
    let text = |key: &str| {
        call.data
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("mcp.gate requires a {key}"))
    };
    let (server, tool) = (text("server")?, text("tool")?);
    let arguments = call.data.get("arguments").cloned().unwrap_or(Value::Null);
    let declared = match call.data.get("readOnly").and_then(Value::as_bool) {
        Some(true) => Declared::Effects(ShellEffects::of(&[ShellEffect::Read])),
        Some(false) | None => Declared::Unknown,
    };
    let auto = auto_set(&deps.settings);
    let mode = deps.settings.manager().get_approval_mode();
    let command = shown(&server, &tool, &arguments);
    let fate = match irreversible::route(auto, declared, core_mode(mode)) {
        Route::Runs => Fate::Runs,
        Route::Waits => Fate::Waits,
        Route::Asks => irreversible::settle(ask(&deps, &call, &command).await),
    };
    match fate {
        Fate::Runs => Ok(json!({ "run": true })),
        Fate::Refused => Ok(json!({
            "run": false,
            "text": format!("`{command}` did not run: {}", RefusalText::DENIED),
        })),
        Fate::Waits => {
            let held = Held {
                kind: WaitingKind::McpCall,
                command: command.clone(),
                cwd: deps.cwd.display().to_string(),
                effects: ShellEffects::NONE,
                payload: json!({ "server": server, "tool": tool, "arguments": arguments }),
            };
            let text = match Recorder::of(&deps).record(held).await {
                Ok(effect) => effect.waiting_text(WaitingAt::Call),
                Err(error) => format!(
                    "`{command}` did not run: it needs the user's approval and could not be \
                     recorded waiting ({error})"
                ),
            };
            Ok(json!({ "run": false, "text": text }))
        }
    }
}

/// Ask the user through the session's approval sink (watching): what
/// they said; no sink, or no answer, is silence.
async fn ask(deps: &HostDeps, call: &HostCall, command: &str) -> Said {
    let Some(sink) = &deps.shell_approval else {
        return Said::Silent;
    };
    let asker = call.call.as_ref().map_or_else(
        || format!("mcp-{}", uuid::Uuid::new_v4().simple()),
        |api| api.task_id().to_string(),
    );
    let request = ShellApprovalRequest {
        request_id: format!("{asker}:mcp-{}", uuid::Uuid::new_v4().simple()),
        step: 0,
        command: command.to_owned(),
        effects: ShellEffects::NONE,
    };
    match sink(request).await {
        ShellAnswer::Approve => Said::Yes,
        ShellAnswer::Deny => Said::No,
        ShellAnswer::Unanswered => Said::Silent,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::shown;

    #[test]
    fn a_waiting_call_shows_its_server_tool_and_bounded_arguments() {
        let long = json!({ "body": "x".repeat(300) });
        let bounded = shown("github", "create_issue", &long);
        assert_eq!(
            (
                shown("github", "create_issue", &json!({ "title": "t" })),
                bounded.chars().count(),
                bounded.ends_with('…'),
            ),
            (
                "github.create_issue {\"title\":\"t\"}".to_owned(),
                super::SHOWN_CHARS + 1,
                true
            )
        );
    }
}
