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
