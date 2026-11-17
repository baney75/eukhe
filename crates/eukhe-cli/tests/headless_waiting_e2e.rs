//! The headless run's end message (I4): the real binary, an isolated
//! HOME and the scripted faux provider (`EUKHE_FAUX_SCRIPT`). A `git push`
//! step is irreversible under the shipped grant: it waits, unasked, and
//! the run goes on to its answer. At the run's end the one waiting text
//! goes to stderr in both output modes, and json mode adds a
//! `waiting_effects` event carrying the waiting effects.

use std::path::Path;

use serde_json::{json, Value};

#[path = "support/eukhe.rs"]
mod eukhe;
use eukhe::eukhe_command;

/// A push that waits, then the answer.
fn push_script() -> Value {
    json!({ "responses": [
        { "content": [{ "type": "toolCall", "name": "bash", "id": "call-1",
            "arguments": { "script": "git push origin main" } }],
          "stopReason": "toolUse" },
        { "content": [{ "type": "text", "text": "pushed later" }], "stopReason": "stop" },
    ] })
}

fn print_command(home: &Path, args: &[&str]) -> std::process::Command {
    let agent = home.join("agent");
    std::fs::create_dir_all(&agent).expect("agent dir");
    let mut command = eukhe_command();
    command
        .args(args)
        .env("HOME", home)
        .env("EUKHE_CODING_AGENT_DIR", &agent)
        .env("EUKHE_DAEMON_SOCKET", home.join("daemon.sock"))
        .env("EUKHE_FAUX_SCRIPT", push_script().to_string())
        .env("EUKHE_OFFLINE", "1")
        .env_remove("EUKHE_SESSION_DIR")
        .env_remove("EUKHE_CODING_AGENT_SESSION_DIR")
        .env_remove("RLM_DEPTH")
        .current_dir(home);
    command
}

/// The end message, from its first line to its last.
fn end_message(stderr: &str) -> Option<String> {
    let start = stderr.find("Waiting for approval:")?;
    let rest = &stderr[start..];
    let end = rest.find("changes nothing.")? + "changes nothing.".len();
    Some(rest[..end].to_owned())
}

#[test]
fn print_mode_ends_with_the_waiting_message_e2e() {
    let home = tempfile::TempDir::new().expect("home");
    let output = print_command(home.path(), &["-p", "Push the branch."])
        .output()
        .expect("binary present");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        (stdout.lines().next(), end_message(&stderr)),
        (
            Some("pushed later"),
            Some(
                "Waiting for approval:\nw1  git push origin main  (net)\n`/approve` runs them all, `/approve <id>` one; ignoring them changes nothing."
                    .to_owned()
            )
        ),
        "stderr: {stderr}"
    );
}

#[test]
fn json_mode_adds_a_waiting_effects_event_e2e() {
    let home = tempfile::TempDir::new().expect("home");
    let output = print_command(home.path(), &["--mode", "json", "-p", "Push the branch."])
        .output()
        .expect("binary present");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let waiting: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["type"] == "waiting_effects")
        .collect();
    assert_eq!(waiting.len(), 1, "stdout: {stdout}\nstderr: {stderr}");
    let effect = &waiting[0]["effects"][0];
    assert_eq!(
        (
            waiting[0]["effects"].as_array().map(Vec::len),
            effect["id"].clone(),
            effect["kind"].clone(),
            effect["command"].clone(),
            effect["effects"].clone(),
            effect["status"].clone(),
            end_message(&stderr).is_some(),
        ),
        (
            Some(1),
            json!("w1"),
            json!("shellStep"),
            json!("git push origin main"),
            json!(["net"]),
            json!("waiting"),
            true,
        ),
        "stdout: {stdout}"
    );
}
