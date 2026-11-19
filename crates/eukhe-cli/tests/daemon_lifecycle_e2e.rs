//! The daemon's core lifecycle through the CLI, on every platform: `eukhe
//! --mode daemon` starts the supervisor, a client connects over the
//! platform endpoint (a Unix socket, a named pipe on Windows), creates a
//! session on the scripted faux provider and runs one turn, and `eukhe
//! shutdown --force` stops the daemon, which then exits and stops serving.

use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

#[path = "support/eukhe.rs"]
mod eukhe;
#[path = "../../eukhe-daemon/tests/support/process.rs"]
mod process;
#[path = "../../eukhe-daemon/tests/support/wire_client.rs"]
mod wire_client;

use eukhe::{daemon_mode_args, eukhe_command, scrub_provider_keys};
use process::Daemon;
use wire_client::WireClient;

/// The bound on each step: a daemon start, a turn, a shutdown.
const BUDGET: Duration = Duration::from_secs(60);

/// The scripted answer of the one turn.
const ANSWER: &str = "hello over the platform endpoint";

/// Wait for `child` to exit by itself within `budget`.
fn wait_exit(child: &mut Child, budget: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + budget;
    loop {
        if let Some(status) = child.try_wait().expect("poll the supervisor") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Poll `get_last_assistant_text` until the session's turn answered.
fn wait_answer(client: &mut WireClient, session_id: &str) -> Value {
    let deadline = Instant::now() + BUDGET;
    loop {
        let reply = client.request(
            &json!({ "type": "get_last_assistant_text", "activeSessionId": session_id }),
            BUDGET,
        );
        if reply["success"] == true && reply["data"]["text"] == ANSWER {
            return reply;
        }
        assert!(
            Instant::now() < deadline,
            "the scripted turn never answered: {reply}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// `eukhe shutdown --force` scoped to the daemon on `socket`.
fn shutdown(socket: &Path, agent_dir: &Path, home: &Path) -> std::process::Output {
    let mut command = eukhe_command();
    scrub_provider_keys(&mut command)
        .args(["shutdown", "--force", "--daemon-socket"])
        .arg(socket)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("EUKHE_CODING_AGENT_DIR", agent_dir)
        .env_remove("EUKHE_DAEMON_SOCKET")
        .current_dir(home)
        .output()
        .expect("run eukhe shutdown")
}

#[test]
fn the_cli_starts_serves_and_stops_the_daemon() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path();
    let agent_dir = home.join("agent");
    std::fs::create_dir_all(&agent_dir).expect("agent dir");
    let socket = home.join("daemon.sock");

    // Start: the CLI's daemon mode runs the supervisor on `socket`.
    let mut command = eukhe_command();
    scrub_provider_keys(daemon_mode_args(&mut command, &socket, &agent_dir))
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("EUKHE_DAEMON_SOCKET")
        .current_dir(home);
    let mut daemon = Daemon::spawn_listening(&mut command, &socket, BUDGET);

    // Connect: the hello arrives over the platform endpoint.
    let mut client = WireClient::connect(&socket);

    // A session on the scripted faux provider.
    let script = home.join("script.json");
    std::fs::write(
        &script,
        json!({ "responses": [{ "text": ANSWER }] }).to_string(),
    )
    .expect("write the script");
    let created = client.request(
        &json!({
            "type": "create",
            "config": {
                "cwd": home.to_string_lossy(),
                "sessionDir": agent_dir.join("sessions").to_string_lossy(),
                "script": script.to_string_lossy(),
            },
        }),
        BUDGET,
    );
    assert_eq!(created["success"], true, "create failed: {created}");
    let session_id = created["data"]["id"]
        .as_str()
        .or_else(|| created["data"]["sessionId"].as_str())
        .expect("the session id")
        .to_string();

    // One scripted turn.
    let prompted = client.request(
        &json!({ "type": "prompt", "activeSessionId": session_id, "message": "hi" }),
        BUDGET,
    );
    assert_eq!(prompted["success"], true, "prompt failed: {prompted}");
    wait_answer(&mut client, &session_id);
    let listed = client.request(&json!({ "type": "list" }), BUDGET);
    let sessions = listed["data"]["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 1, "one live session: {listed}");
    drop(client);

    // Stop: the CLI shuts the daemon down and the supervisor exits.
    let stopped = shutdown(&socket, &agent_dir, home);
    assert!(
        stopped.status.success(),
        "eukhe shutdown failed: {}{}",
        String::from_utf8_lossy(&stopped.stdout),
        String::from_utf8_lossy(&stopped.stderr)
    );
    let status = wait_exit(&mut daemon.child, BUDGET);
    assert!(status.is_some(), "the supervisor outlived eukhe shutdown");
    assert!(
        eukhe_types::platform::transport::connect_blocking(
            &socket,
            eukhe_bend::rules::server_peer::PEER_RULES,
        )
        .is_err(),
        "the endpoint still serves after the shutdown"
    );
}
