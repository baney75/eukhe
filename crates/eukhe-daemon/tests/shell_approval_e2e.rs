//! The `bash` tool's approval path end to end (G5, I3): a real
//! supervisor and worker, the scripted faux model calling `bash`, in a
//! session whose user narrowed `shell.autoEffects` to `read` (by default
//! a write and a run of a program run unasked) and watches the run
//! (`approvals.mode` `watch`). An attached client gets one
//! `shell_approval` frame per gated step and answers with
//! `answer_shell_approval`: it approves the first write and denies the
//! second, so the script stops there. With no client attached no one
//! answers, and the first gated step waits for `/approve`, never run. The
//! phone gateway is asked like any other client.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

#[path = "support/daemon.rs"]
mod daemon;
#[path = "support/process.rs"]
mod process;
use daemon::supervisor_command;
use process::Daemon;

/// The script the faux model runs: three gated writes.
const SCRIPT: &str = "echo one > approved.txt\necho two > denied.txt\necho three > never.txt";

/// The script of a session driven from the phone: a write, a run of a
/// program, a write of the project's eukhe config (protected), a write.
const PHONE_SCRIPT: &str = "echo one > written.txt\nsh -c 'echo two > ran.txt'\necho three > .eukhe/settings.json\necho four > never.txt";

/// JSONL supervisor client that keeps the `shell_approval` frames it
/// reads while it waits for a response.
struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    approvals: VecDeque<Value>,
}

impl Client {
    fn connect(socket: &Path) -> Self {
        let stream =
            eukhe_types::platform::transport::connect_std(socket).expect("connect supervisor");
        let writer = stream.try_clone().expect("clone socket");
        let mut client = Client {
            reader: BufReader::new(stream),
            writer,
            approvals: VecDeque::new(),
        };
        assert_eq!(client.read_line()["type"], "daemon_hello");
        client
    }

    fn send(&mut self, id: &str, command: &Value) {
        let mut line = serde_json::to_string(&json!({
            "type": "command",
            "id": id,
            "protocol": { "name": "eukhe.daemon", "version": 7 },
            "command": command,
        }))
        .expect("serialize command");
        line.push('\n');
        self.writer.write_all(line.as_bytes()).expect("send");
        self.writer.flush().expect("flush");
    }

    /// Send `command` and read until its response.
    fn command(&mut self, id: &str, command: &Value) -> Value {
        self.send(id, command);
        let deadline = Instant::now() + Duration::from_mins(4);
        loop {
            assert!(Instant::now() < deadline, "no response for id {id}");
            let line = self.read_line();
            if line.get("id").and_then(Value::as_str) == Some(id) {
                assert_eq!(line["success"], true, "{id} failed: {line}");
                return line;
            }
            self.keep(line);
        }
    }

    /// The next `shell_approval` frame.
    fn approval(&mut self) -> Value {
        let deadline = Instant::now() + Duration::from_mins(2);
        loop {
            if let Some(frame) = self.approvals.pop_front() {
                return frame;
            }
            assert!(Instant::now() < deadline, "no shell_approval frame came");
            let line = self.read_line();
            self.keep(line);
        }
    }

    fn keep(&mut self, line: Value) {
        if line["type"] == "shell_approval" {
            self.approvals.push_back(line);
        }
    }

    fn read_line(&mut self) -> Value {
        let mut line = String::new();
        let deadline = Instant::now() + Duration::from_mins(2);
        self.reader
            .get_mut()
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("set timeout");
        loop {
            match self.reader.read_line(&mut line) {
                Ok(0) => panic!("supervisor closed the connection"),
                Ok(_) if line.trim().is_empty() => line.clear(),
                Ok(_) => return serde_json::from_str(line.trim()).expect("parse response line"),
                Err(error) => {
                    assert!(
                        Instant::now() < deadline,
                        "timed out waiting for supervisor line: {error}"
                    );
                }
            }
        }
    }
}

/// A daemon over `dir` whose faux model calls `bash` with `script`, one
/// connected client, and the session it created. The user's auto set is
/// `read` alone, so every write asks; the user watches.
fn open(dir: &Path, script: &str) -> (Daemon, Client, String) {
    let agent_dir = dir.join("agent");
    let sessions_dir = agent_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).expect("sessions dir");
    std::fs::write(
        agent_dir.join("settings.json"),
        json!({ "shell": { "autoEffects": ["read"] }, "approvals": { "mode": "watch" } })
            .to_string(),
    )
    .expect("write settings");
    let faux = dir.join("faux.json");
    std::fs::write(
        &faux,
        json!({
            "engine": "faux",
            "responses": [
                { "content": [{ "type": "toolCall", "name": "bash", "arguments": { "script": script } }] },
                { "text": "done" },
            ],
        })
        .to_string(),
    )
    .expect("write faux script");
    let socket = dir.join("shell.sock");
    let daemon = Daemon::spawn_listening(
        supervisor_command(&socket, &agent_dir)
            .env("EUKHE_CODING_AGENT_DIR", &agent_dir)
            .env(
                eukhe_daemon::worker::WORKER_SUPERVISOR_LOST_EXIT_MS_ENV,
                "15000",
            ),
        &socket,
        Duration::from_secs(15),
    );
    let mut client = Client::connect(&socket);
    let created = client.command(
        "c1",
        &json!({
            "type": "create",
            "config": {
                "cwd": dir.to_string_lossy(),
                "sessionDir": sessions_dir.to_string_lossy(),
                "script": faux.to_string_lossy(),
            },
        }),
    );
    let session = created["data"]["activeSessionId"]
        .as_str()
        .or_else(|| created["data"]["id"].as_str())
        .expect("active session id")
        .to_owned();
    (daemon, client, session)
}

/// The text of the session's one tool result.
fn tool_result(client: &mut Client, session: &str) -> String {
    let messages = client.command(
        "m1",
        &json!({ "type": "get_messages", "activeSessionId": session }),
    );
    let results: Vec<String> = messages["data"]["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter(|message| message["role"] == "toolResult")
        .map(|message| match &message["content"] {
            Value::String(text) => text.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            other => other.to_string(),
        })
        .collect();
    assert_eq!(results.len(), 1, "{results:#?}");
    results[0].clone()
}

#[test]
fn an_attached_client_approves_one_step_and_denies_the_next() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (_daemon, mut client, session) = open(dir.path(), SCRIPT);
    let attached = client.command(
        "a1",
        &json!({ "type": "attach", "activeSessionId": session, "clientId": "shell-e2e" }),
    );
    assert_eq!(attached["success"], true);
    client.command(
        "p1",
        &json!({ "type": "prompt", "activeSessionId": session, "message": "write the files" }),
    );
    let mut asked = Vec::new();
    for (at, verdict) in ["approve", "deny"].into_iter().enumerate() {
        let frame = client.approval();
        let request = frame["request"].clone();
        client.command(
            &format!("v{at}"),
            &json!({
                "type": "answer_shell_approval",
                "activeSessionId": session,
                "requestId": request["requestId"],
                "verdict": verdict,
            }),
        );
        asked.push((
            frame["activeSessionId"].clone(),
            request["step"].clone(),
            request["command"].clone(),
            request["effects"].clone(),
        ));
    }
    client.command(
        "w1",
        &json!({ "type": "wait_for_idle", "activeSessionId": session }),
    );
    let result = tool_result(&mut client, &session);
    println!("asked: {asked:?}\nresult: {result}");
    assert_eq!(
        (
            asked,
            result,
            std::fs::read_to_string(dir.path().join("approved.txt")).ok(),
            dir.path().join("denied.txt").exists(),
            dir.path().join("never.txt").exists(),
        ),
        (
            vec![
                (
                    json!(session),
                    json!(0),
                    json!("echo one >approved.txt"),
                    json!(["read", "write"])
                ),
                (
                    json!(session),
                    json!(1),
                    json!("echo two >denied.txt"),
                    json!(["read", "write"])
                ),
            ],
            "$ echo one >approved.txt\n[stopped at step 1 (read+write): `echo two >denied.txt` and the steps after it did not run: the user denied it. Do not try it again unless the user asks for it.]\n"
                .to_owned(),
            Some("one\n".to_owned()),
            false,
            false,
        )
    );
}

/// I3: a watched question no one answers is never a yes: the step waits
/// in the ledger for `/approve`, and it and the steps after it never ran.
#[test]
fn with_no_client_attached_a_watched_step_waits_unanswered() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (_daemon, mut client, session) = open(dir.path(), SCRIPT);
    client.command(
        "p1",
        &json!({ "type": "prompt", "activeSessionId": session, "message": "write the files" }),
    );
    client.command(
        "w1",
        &json!({ "type": "wait_for_idle", "activeSessionId": session }),
    );
    let result = tool_result(&mut client, &session);
    let listed = client.command(
        "l1",
        &json!({ "type": "get_waiting_effects", "activeSessionId": session }),
    );
    println!("result: {result}");
    assert_eq!(
        (
            result,
            client.approvals.len(),
            listed["data"]["effects"][0]["command"].clone(),
            listed["data"]["effects"][0]["status"].clone(),
            dir.path().join("approved.txt").exists(),
        ),
        (
            "[waiting for the user's approval: step 0 (read+write) `echo one >approved.txt` is w1; it and the steps after it did not run. Continue with the rest of the work; do not try another way to do it.]\n"
                .to_owned(),
            0,
            json!("echo one >approved.txt"),
            json!("waiting"),
            false,
        )
    );
}

/// Regression (I5, I9): the phone gateway answered steps itself (a step
/// that wrote no protected path ran unasked). It is an ordinary client
/// now, a gateway of an earlier release attaching as `gateway` too: it
/// gets the question, and its no refuses the step.
#[test]
fn the_phone_gateway_is_asked_like_any_client() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (_daemon, mut client, session) = open(dir.path(), PHONE_SCRIPT);
    let attached = client.command(
        "a1",
        &json!({
            "type": "attach",
            "activeSessionId": session,
            "clientId": "phone",
            "clientKind": "gateway",
        }),
    );
    assert_eq!(attached["success"], true);
    client.command(
        "p1",
        &json!({ "type": "prompt", "activeSessionId": session, "message": "write the files" }),
    );
    let frame = client.approval();
    client.command(
        "v0",
        &json!({
            "type": "answer_shell_approval",
            "activeSessionId": session,
            "requestId": frame["request"]["requestId"],
            "verdict": "deny",
        }),
    );
    client.command(
        "w1",
        &json!({ "type": "wait_for_idle", "activeSessionId": session }),
    );
    assert_eq!(
        (
            frame["request"]["command"].clone(),
            dir.path().join("written.txt").exists(),
        ),
        (json!("echo one >written.txt"), false)
    );
}
