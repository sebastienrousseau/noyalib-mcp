// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Requests built to tie the server up, through the real binary.
//!
//! Each test holds a stdio session with a deadline on every reply: a
//! server that wedges fails the test instead of hanging it, and is
//! killed when the session is dropped.

#![allow(missing_docs)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// A stdio session with the server, killed when dropped.
struct Session {
    child: Child,
    stdin: ChildStdin,
    replies: Receiver<Value>,
    /// Replies that arrived while another was awaited.
    early: Vec<Value>,
    next_id: u64,
}

impl Session {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_noyalib-mcp"))
            .arg("--root")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("server starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, replies) = channel();
        drop(std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&line)
                    && tx.send(v).is_err()
                {
                    break;
                }
            }
        }));
        let mut s = Self {
            child,
            stdin,
            replies,
            early: Vec::new(),
            next_id: 1,
        };
        let init = s.send(
            "initialize",
            json!({"protocolVersion": "2025-11-25", "capabilities": {},
                   "clientInfo": {"name": "limits", "version": "0"}}),
        );
        let _ = s.reply(init, Duration::from_secs(10));
        s.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        s
    }

    fn write(&mut self, message: &Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).expect("write");
        self.stdin.flush().expect("flush");
    }

    /// Send a request without waiting for it. Returns its id.
    fn send(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    fn call(&mut self, tool: &str, arguments: Value) -> u64 {
        self.send("tools/call", json!({"name": tool, "arguments": arguments}))
    }

    /// The reply to `id`, or `None` when it does not come in time.
    fn reply(&mut self, id: u64, within: Duration) -> Option<Value> {
        if let Some(i) = self.early.iter().position(|v| v["id"] == id) {
            return Some(self.early.swap_remove(i));
        }
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            let v = self.replies.recv_timeout(left).ok()?;
            if v["id"] == id {
                return Some(v);
            }
            self.early.push(v);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn scratch(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("noyalib-mcp-limits-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("scratch dir");
    p
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap_or("")
}

#[cfg(unix)]
#[test]
fn a_fifo_is_refused_and_the_server_keeps_answering() {
    // A read of a FIFO with no writer blocks forever. One such call per
    // worker thread used to wedge the whole server, `ping` included.
    let root = scratch("fifo");
    let fifo = root.join("pipe.yml");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(made.success());
    let mut s = Session::start(&root);
    let calls: Vec<u64> = (0..16)
        .map(|_| s.call("noyalib_get", json!({"file": "pipe.yml", "path": "a"})))
        .collect();
    let ping = s.send("ping", json!({}));
    let pong = s.reply(ping, Duration::from_secs(10));
    for id in calls {
        let r = s.reply(id, Duration::from_secs(10));
        let r = r.unwrap_or_else(|| panic!("no reply to the FIFO read {id}"));
        assert_eq!(r["result"]["isError"], true, "{r}");
        assert!(text(&r).contains("not a regular file"), "{r}");
    }
    assert!(pong.is_some(), "ping went unanswered");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_file_over_the_size_limit_is_refused_unread() {
    // The strict profile caps a document at 1 MiB. A sparse 4 GiB file
    // costs nothing on disk; reading it would cost 4 GiB of memory.
    let root = scratch("big");
    let big = std::fs::File::create(root.join("big.yml")).expect("create");
    big.set_len(4 << 30).expect("sparse file");
    drop(big);
    let mut s = Session::start(&root);
    let started = Instant::now();
    let id = s.call("noyalib_get", json!({"file": "big.yml", "path": "a"}));
    let r = s.reply(id, Duration::from_secs(10)).expect("a reply");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(r["result"]["isError"], true, "{r}");
    assert!(text(&r).contains("limit"), "{r}");
    let _ = std::fs::remove_dir_all(root);
}

/// `[` n deep: the shape that recursed once per bracket.
fn nested(n: usize) -> String {
    "[".repeat(n) + &"]".repeat(n)
}

#[test]
fn a_deeply_nested_fragment_is_refused_and_the_server_lives() {
    let root = scratch("deep");
    std::fs::write(root.join("f.yml"), "a: 1\n").expect("fixture");
    std::fs::write(root.join("m.yml"), "a: 1\n---\nb: 2\n").expect("fixture");
    let mut s = Session::start(&root);
    let deep = nested(100_000);
    let calls = [
        (
            "noyalib_edit",
            json!({"yaml": "a: 1\n", "path": "a", "value": deep}),
        ),
        (
            "noyalib_set",
            json!({"file": "f.yml", "path": "a", "value": deep}),
        ),
        (
            "noyalib_set_multidoc",
            json!({"file": "m.yml", "doc_index": 1, "path": "b", "value": deep}),
        ),
    ];
    for (tool, args) in calls {
        let id = s.call(tool, args);
        let r = s
            .reply(id, Duration::from_secs(30))
            .unwrap_or_else(|| panic!("{tool}: no reply, the server died"));
        assert_eq!(r["result"]["isError"], true, "{tool}");
        // The error names the problem and does not echo 200 KB back.
        assert!(text(&r).len() < 1024, "{tool}: {} bytes", text(&r).len());
        let id = s.call("noyalib_parse", json!({"yaml": "ok: 1\n"}));
        let next = s.reply(id, Duration::from_secs(10));
        assert!(next.is_some(), "{tool}: the next call went unanswered");
    }
    let file = std::fs::read_to_string(root.join("f.yml")).expect("read");
    assert_eq!(file, "a: 1\n");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn yaml_text_over_the_document_limit_is_refused_before_parsing() {
    // Strict profile: 1 MiB. Each stateless tool refuses a 2 MiB text.
    let root = scratch("long");
    let mut s = Session::start(&root);
    let long = format!("a: {}\n", "x".repeat(2 << 20));
    for (tool, args) in [
        ("noyalib_parse", json!({"yaml": long})),
        (
            "noyalib_edit",
            json!({"yaml": long, "path": "a", "value": "1"}),
        ),
        ("noyalib_validate", json!({"yaml": long})),
    ] {
        let id = s.call(tool, args);
        let r = s.reply(id, Duration::from_secs(30)).expect("a reply");
        assert_eq!(r["result"]["isError"], true, "{tool}: {r}");
        assert!(text(&r).contains("limit"), "{tool}: {}", text(&r));
    }
    let _ = std::fs::remove_dir_all(root);
}
