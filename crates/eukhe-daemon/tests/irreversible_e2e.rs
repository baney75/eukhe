//! Irreversible effects end to end (I1-I12): a real supervisor and
//! worker, the scripted faux model, the shipped grant (`read, write,
//! exec`). A `git push` step does not run and nobody is asked: it waits
//! in the session's ledger as `w1`, the script's later steps do not run,
//! the agent goes on, and the run's end sends `run_waiting` listing it.
//! `/approve` (`approve_waiting_effects`) runs it once, as the user's own
//! act; a second approve of the same id runs nothing and says it already
//! ran. An MCP tool the server declares read-only (`readOnlyHint: true`)
//! runs; one that declares nothing waits like the push and runs with its
//! arguments when approved. Watching (`set_approval_mode` `watch`), each
//! irreversible step is asked: yes runs it, no refuses it (not run, not
//! waiting), silence (the client left) leaves it waiting.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

#[path = "support/daemon.rs"]
mod daemon;
#[path = "support/process.rs"]
mod process;
use daemon::supervisor_command;
use process::Daemon;

/// JSONL supervisor client that keeps every frame it reads while it waits
/// for a response.
struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    frames: VecDeque<Value>,
}

impl Client {
    fn connect(socket: &Path) -> Self {
        let stream =
            eukhe_types::platform::transport::connect_std(socket).expect("connect supervisor");
        let writer = stream.try_clone().expect("clone socket");
        let mut client = Client {
            reader: BufReader::new(stream),
            writer,
            frames: VecDeque::new(),
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

    /// Send `command` and read until its response, which must succeed.
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
            self.frames.push_back(line);
        }
    }

    /// The first frame (kept or new) that `wanted` accepts.
    fn frame(&mut self, wanted: impl Fn(&Value) -> bool) -> Value {
        if let Some(at) = self.frames.iter().position(&wanted) {
            return self.frames.remove(at).expect("a kept frame");
        }
        let deadline = Instant::now() + Duration::from_mins(2);
        loop {
            assert!(Instant::now() < deadline, "the awaited frame never came");
            let line = self.read_line();
            if wanted(&line) {
                return line;
            }
            self.frames.push_back(line);
        }
    }

    /// The next `shell_approval` frame.
    fn approval(&mut self) -> Value {
        self.frame(|line| line["type"] == "shell_approval")
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

/// Run `git` in `dir`, which must succeed; its trimmed stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=e2e", "-c", "user.email=e2e@example.com"])
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Whether the bare repository `origin` has the branch `name`.
fn has_branch(origin: &Path, name: &str) -> bool {
    Command::new("git")
        .current_dir(origin)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ])
        .output()
        .expect("run git")
        .status
        .success()
}

/// A workspace `dir/work` with one commit on `main` whose `origin` is the
/// bare repository `dir/origin.git` (nothing pushed yet).
fn repository(dir: &Path) -> (PathBuf, PathBuf) {
    let (work, origin) = (dir.join("work"), dir.join("origin.git"));
    std::fs::create_dir_all(&work).expect("work dir");
    git(dir, &["init", "--quiet", "--bare", "origin.git"]);
    git(&work, &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(work.join("README"), "e2e\n").expect("write README");
    git(&work, &["add", "README"]);
    git(&work, &["commit", "--quiet", "-m", "first"]);
    git(
        &work,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    (work, origin)
}

/// A daemon whose agent dir holds `settings`, one connected client, and a
/// session working in `cwd` whose faux model answers `responses`.
fn open(dir: &Path, cwd: &Path, settings: &Value, responses: &Value) -> (Daemon, Client, String) {
    let agent_dir = dir.join("agent");
    std::fs::create_dir_all(agent_dir.join("sessions")).expect("sessions dir");
    std::fs::write(agent_dir.join("settings.json"), settings.to_string()).expect("settings");
    std::fs::write(
        dir.join("faux.json"),
        json!({ "engine": "faux", "responses": responses }).to_string(),
    )
    .expect("write faux script");
    let (daemon, mut client) = start(dir);
    let created = create(&mut client, dir, cwd, None);
    (daemon, client, active_id(&created))
}

/// The supervisor over `dir/agent` and one connected client.
fn start(dir: &Path) -> (Daemon, Client) {
    let agent_dir = dir.join("agent");
    let socket = dir.join("irreversible.sock");
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
    let client = Client::connect(&socket);
    (daemon, client)
}

/// Create a session in `cwd` driven by `dir/faux.json`: a new one, or the
/// saved `session_path` reopened. The create response.
fn create(client: &mut Client, dir: &Path, cwd: &Path, session_path: Option<&Path>) -> Value {
    let mut config = json!({
        "cwd": cwd.to_string_lossy(),
        "sessionDir": dir.join("agent").join("sessions").to_string_lossy(),
        "script": dir.join("faux.json").to_string_lossy(),
    });
    if let Some(path) = session_path {
        config["sessionPath"] = json!(path.to_string_lossy());
    }
    client.command("c1", &json!({ "type": "create", "config": config }))
}

fn active_id(created: &Value) -> String {
    created["data"]["activeSessionId"]
        .as_str()
        .or_else(|| created["data"]["id"].as_str())
        .expect("active session id")
        .to_owned()
}

/// One `bash` tool call of the faux model.
fn bash(script: &str) -> Value {
    json!({ "content": [{ "type": "toolCall", "name": "bash", "arguments": { "script": script } }] })
}

/// The texts of the session's tool results, in order.
fn tool_results(client: &mut Client, session: &str) -> Vec<String> {
    let messages = client.command(
        "m1",
        &json!({ "type": "get_messages", "activeSessionId": session }),
    );
    messages["data"]["messages"]
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
        .collect()
}

/// The session's ledger: each effect's id, command, effects and status.
fn ledger(client: &mut Client, id: &str, session: &str) -> Vec<Value> {
    let listed = client.command(
        id,
        &json!({ "type": "get_waiting_effects", "activeSessionId": session }),
    );
    listed["data"]["effects"]
        .as_array()
        .expect("effects")
        .iter()
        .map(brief)
        .collect()
}

/// An effect as the tests compare it: id, command, effects, status.
fn brief(effect: &Value) -> Value {
    json!([
        effect["id"],
        effect["command"],
        effect["effects"],
        effect["status"]
    ])
}

/// Approve `which` in `session`: the effects that ran (brief) and the ids
/// that ran nothing.
fn approve(client: &mut Client, id: &str, session: &str, which: &Value) -> (Vec<Value>, Value) {
    let reply = client.command(
        id,
        &json!({ "type": "approve_waiting_effects", "activeSessionId": session, "which": which }),
    );
    (
        reply["data"]["ran"]
            .as_array()
            .expect("ran")
            .iter()
            .map(brief)
            .collect(),
        reply["data"]["missing"].clone(),
    )
}

fn prompt_until_idle(client: &mut Client, session: &str) {
    client.command(
        "p1",
        &json!({ "type": "prompt", "activeSessionId": session, "message": "ship it" }),
    );
    client.command(
        "w1",
        &json!({ "type": "wait_for_idle", "activeSessionId": session }),
    );
}

/// I2, I4, I5: the push waits unasked, the agent goes on, the run's end
/// lists it, approve runs it once, a second approve says it already ran.
#[test]
fn a_push_waits_and_one_approve_runs_it_once_e2e() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (work, origin) = repository(dir.path());
    let (_daemon, mut client, session) = open(
        dir.path(),
        &work,
        &json!({}),
        &json!([
            bash("echo built > built.txt\ngit push origin main\necho after > after.txt"),
            bash("echo next > next.txt"),
            { "text": "done" },
        ]),
    );
    client.command(
        "a1",
        &json!({ "type": "attach", "activeSessionId": session, "clientId": "irreversible-e2e" }),
    );
    prompt_until_idle(&mut client, &session);
    let ended = client
        .frame(|line| line["type"] == "session_event" && line["event"]["type"] == "run_waiting");
    let results = tool_results(&mut client, &session);
    println!("results: {results:#?}");
    let waiting = json!(["w1", "git push origin main", ["net"], "waiting"]);
    assert_eq!(
        (
            results,
            ended["event"]["effects"]
                .as_array()
                .expect("run_waiting effects")
                .iter()
                .map(brief)
                .collect::<Vec<_>>(),
            ledger(&mut client, "l1", &session),
            work.join("built.txt").is_file(),
            work.join("after.txt").exists(),
            work.join("next.txt").is_file(),
            has_branch(&origin, "main"),
        ),
        (
            vec![
                "$ echo built >built.txt\n[waiting for the user's approval: step 1 (net) `git push origin main` is w1; it and the steps after it did not run. Continue with the rest of the work; do not try another way to do it.]\n".to_owned(),
                "$ echo next >next.txt\n".to_owned(),
            ],
            vec![waiting.clone()],
            vec![waiting],
            true,
            false,
            true,
            false,
        )
    );

    let (ran, missing) = approve(&mut client, "y1", &session, &json!("all"));
    assert_eq!(
        (ran.len(), missing, has_branch(&origin, "main")),
        (1, json!([]), true),
        "{ran:?}"
    );
    assert_eq!(ran[0][0], "w1");
    assert_eq!(ran[0][3]["ran"]["ok"], true, "{ran:?}");
    let (again, missing) = approve(&mut client, "y2", &session, &json!({ "ids": ["w1", "w9"] }));
    assert_eq!((again, missing), (Vec::new(), json!(["w1", "w9"])));
    assert_eq!(
        eukhe_types::effects::missing_line(
            &serde_json::from_value::<Vec<eukhe_types::effects::WaitingEffect>>(
                client.command(
                    "l2",
                    &json!({ "type": "get_waiting_effects", "activeSessionId": session }),
                )["data"]["effects"]
                    .clone()
            )
            .expect("the ledger decodes"),
            "w1"
        ),
        "w1 already ran"
    );
}

/// I1, I5: an MCP tool declared read-only runs; one that declares nothing
/// waits, and the approve re-runs it once with its arguments.
#[test]
fn an_mcp_call_waits_unless_the_tool_is_read_only_e2e() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("work dir");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mcp_echo_server.py");
    let stamps = dir.path().join("stamps.txt");
    let receipt = dir.path().join("receipt.json");
    let cell = [
        "from rlm import mcp",
        "import json",
        "echoed = await mcp.call_tool(\"fixture-echo\", \"echo\", {\"message\": \"hi\"})",
        "try:",
        "    await mcp.call_tool(\"fixture-echo\", \"stamp\", {\"message\": \"one\"})",
        "    stamped = \"ran\"",
        "except mcp.McpCallNotRun as not_run:",
        "    stamped = str(not_run)",
        "open(RECEIPT, \"w\").write(json.dumps({\"echoed\": echoed, \"stamped\": stamped}))",
    ]
    .join("\n")
    .replace("RECEIPT", &format!("{:?}", receipt.display().to_string()));
    let (_daemon, mut client, session) = open(
        dir.path(),
        &work,
        &json!({
            "mcpServers": {
                "fixture-echo": {
                    "type": "stdio",
                    "command": "python3",
                    "args": [fixture.to_string_lossy(), "--stamp", stamps.to_string_lossy()],
                },
            },
        }),
        &json!([
            { "content": [{ "type": "toolCall", "name": "python", "arguments": { "code": cell } }] },
            { "text": "done" },
        ]),
    );
    prompt_until_idle(&mut client, &session);
    let receipt: Value = serde_json::from_str(
        &std::fs::read_to_string(&receipt).expect("the cell wrote its receipt"),
    )
    .expect("receipt json");
    let waiting = json!([
        "w1",
        "fixture-echo.stamp {\"message\":\"one\"}",
        [],
        "waiting"
    ]);
    assert_eq!(
        (
            receipt,
            ledger(&mut client, "l1", &session),
            stamps.exists()
        ),
        (
            json!({
                "echoed": "hi",
                "stamped": "[waiting for the user's approval: `fixture-echo.stamp {\"message\":\"one\"}` (unknown) is w1; it did not run. Continue with the rest of the work; do not try another way to do it.]",
            }),
            vec![waiting],
            false,
        )
    );
    let (ran, missing) = approve(&mut client, "y1", &session, &json!({ "ids": ["w1"] }));
    assert_eq!(
        (ran.len(), missing, std::fs::read_to_string(&stamps).ok()),
        (1, json!([]), Some("one\n".to_owned())),
        "{ran:?}"
    );
    assert_eq!(ran[0][3]["ran"]["ok"], true, "{ran:?}");
    let (again, missing) = approve(&mut client, "y2", &session, &json!("all"));
    assert_eq!(
        (again, missing, std::fs::read_to_string(&stamps).ok()),
        (Vec::new(), json!([]), Some("one\n".to_owned()))
    );
}

/// I3: watching, each irreversible step is asked: yes runs it, no refuses
/// it (not run, not waiting), and a question nobody answers (the client
/// left) leaves the step waiting, never run.
#[test]
fn watching_yes_runs_no_refuses_and_silence_waits_e2e() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (work, origin) = repository(dir.path());
    let (_daemon, mut client, session) = open(
        dir.path(),
        &work,
        &json!({}),
        &json!([
            bash("git push origin HEAD:refs/heads/yes"),
            bash("git push origin HEAD:refs/heads/no"),
            bash("git push origin HEAD:refs/heads/silence"),
            { "text": "done" },
        ]),
    );
    client.command(
        "m0",
        &json!({ "type": "set_approval_mode", "activeSessionId": session, "mode": "watch" }),
    );
    client.command(
        "a1",
        &json!({ "type": "attach", "activeSessionId": session, "clientId": "watcher" }),
    );
    client.command(
        "p1",
        &json!({ "type": "prompt", "activeSessionId": session, "message": "ship it" }),
    );
    let mut asked = Vec::new();
    for (at, verdict) in ["approve", "deny"].into_iter().enumerate() {
        let frame = client.approval();
        asked.push(frame["request"]["command"].clone());
        client.command(
            &format!("v{at}"),
            &json!({
                "type": "answer_shell_approval",
                "activeSessionId": session,
                "requestId": frame["request"]["requestId"],
                "verdict": verdict,
            }),
        );
    }
    let silent = client.approval();
    asked.push(silent["request"]["command"].clone());
    client.command(
        "d1",
        &json!({ "type": "detach", "activeSessionId": session }),
    );
    client.command(
        "w1",
        &json!({ "type": "wait_for_idle", "activeSessionId": session }),
    );
    let results = tool_results(&mut client, &session);
    println!("results: {results:#?}");
    assert_eq!(
        (
            asked,
            ledger(&mut client, "l1", &session),
            has_branch(&origin, "yes"),
            has_branch(&origin, "no"),
            has_branch(&origin, "silence"),
        ),
        (
            vec![
                json!("git push origin HEAD:refs/heads/yes"),
                json!("git push origin HEAD:refs/heads/no"),
                json!("git push origin HEAD:refs/heads/silence"),
            ],
            vec![json!([
                "w1",
                "git push origin HEAD:refs/heads/silence",
                ["net"],
                "waiting"
            ])],
            true,
            false,
            false,
        )
    );
}

/// I6: the ledger is durable. The daemon dies (its whole tree killed)
/// with a push waiting; the session reopened under a new daemon still
/// lists it waiting, never ran it, and its approve runs it once.
#[test]
fn a_waiting_push_survives_a_daemon_crash_e2e() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let (work, origin) = repository(dir.path());
    let (daemon, mut client, session) = open(
        dir.path(),
        &work,
        &json!({}),
        &json!([bash("git push origin main"), { "text": "done" }]),
    );
    prompt_until_idle(&mut client, &session);
    let session_file = client.command(
        "s1",
        &json!({ "type": "get_state", "activeSessionId": session }),
    )["data"]["sessionFile"]
        .as_str()
        .map(PathBuf::from)
        .expect("get_state names the session's file");
    let waiting = vec![json!(["w1", "git push origin main", ["net"], "waiting"])];
    assert_eq!(ledger(&mut client, "l1", &session), waiting);
    drop(client);
    drop(daemon);

    let (_daemon, mut client) = start(dir.path());
    let reopened = active_id(&create(&mut client, dir.path(), &work, Some(&session_file)));
    assert_eq!(
        (
            ledger(&mut client, "l2", &reopened),
            has_branch(&origin, "main")
        ),
        (waiting, false)
    );
    let (ran, missing) = approve(&mut client, "y1", &reopened, &json!("all"));
    assert_eq!(
        (ran.len(), missing, has_branch(&origin, "main")),
        (1, json!([]), true),
        "{ran:?}"
    );
}
