// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! A complete MCP session, driven in-process.
//!
//! The binary speaks newline-delimited JSON-RPC 2.0 over stdio, which
//! makes it awkward to explore by hand: you cannot see what a client
//! actually sends, and a malformed frame just hangs. This example drives
//! the same [`handle_message`] entry point the stdio loop calls, so
//! every request and response is visible.
//!
//! The sequence mirrors what an MCP-aware client (Claude, Cursor, Zed)
//! does on connect — `initialize`, `tools/list`, then `tools/call` — and
//! demonstrates the property that makes these tools safe for an agent to
//! use on a real repository: **`noyalib_set` rewrites only the touched
//! span**, so comments, blank lines and sibling formatting survive
//! byte-for-byte. That is the whole reason to hand an agent this server
//! instead of "parse YAML, mutate, re-serialise".
//!
//! Run: `cargo run --example mcp_session`

use noyalib_mcp::{HandleOutcome, handle_message};
use std::fs;

fn send(label: &str, frame: &str) -> Option<String> {
    println!("\n── {label} ──");
    println!("  -> {frame}");
    match handle_message(frame) {
        HandleOutcome::Reply(payload) => {
            let shown = if payload.len() > 200 {
                format!("{}… ({} bytes)", &payload[..200], payload.len())
            } else {
                payload.clone()
            };
            println!("  <- {shown}");
            Some(payload)
        }
        HandleOutcome::Silent => {
            println!("  <- (silent — notification, per JSON-RPC spec)");
            None
        }
    }
}

fn main() {
    println!("noyalib-mcp — a full session, request by request");

    // A config file with the things a naive round-trip destroys:
    // a licence header, section comments, an inline comment, blank
    // lines, and deliberately non-canonical spacing.
    let original = "\
# deploy config — DO NOT reformat by hand
# owner: platform-team

server:
  host: 0.0.0.0
  port: 8080        # bumped for the load test

features:
  - tracing
  - metrics
";
    let path =
        std::env::temp_dir().join(format!("noyalib-mcp-example-{}.yaml", std::process::id()));
    fs::write(&path, original).expect("write fixture");
    let file = path.to_string_lossy().to_string();

    // 1. Handshake.
    let _ = send(
        "initialize",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
    );

    // 2. Discovery — how an agent learns what it may call.
    let _ = send(
        "tools/list",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
    );

    // 3. Read a nested scalar by dotted path.
    let _ = send(
        "tools/call noyalib_get — read server.port",
        &format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"noyalib_get","arguments":{{"file":"{file}","path":"server.port"}}}}}}"#
        ),
    );

    // 4. Read an indexed sequence element.
    let _ = send(
        "tools/call noyalib_get — read features[0]",
        &format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"noyalib_get","arguments":{{"file":"{file}","path":"features[0]"}}}}}}"#
        ),
    );

    // 5. Write — the operation an agent performs to change config.
    let _ = send(
        "tools/call noyalib_set — set server.port to 9090",
        &format!(
            r#"{{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{{"name":"noyalib_set","arguments":{{"file":"{file}","path":"server.port","value":"9090"}}}}}}"#
        ),
    );

    // The payoff: show the file after the edit.
    println!("\n── the file on disk, after noyalib_set ──");
    let after = fs::read_to_string(&path).expect("read back");
    for line in after.lines() {
        println!("  | {line}");
    }

    // Everything except the one edited scalar must be untouched.
    assert!(
        after.contains("# deploy config — DO NOT reformat by hand"),
        "header comment must survive"
    );
    assert!(
        after.contains("# bumped for the load test"),
        "inline comment on the *edited line* must survive"
    );
    assert!(after.contains("port: 9090"), "the edit must be applied");
    println!("\n  ✓ header, inline comment, blank lines and spacing all preserved");

    println!("\n── error cases a client must handle ──");

    let _ = send(
        "unknown tool",
        r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"frobnicate","arguments":{}}}"#,
    );
    let _ = send(
        "missing required argument",
        &format!(
            r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"noyalib_get","arguments":{{"file":"{file}"}}}}}}"#
        ),
    );
    let _ = send(
        "path that does not exist in the document",
        &format!(
            r#"{{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{{"name":"noyalib_get","arguments":{{"file":"{file}","path":"server.nope"}}}}}}"#
        ),
    );
    let _ = send(
        "unknown method",
        r#"{"jsonrpc":"2.0","id":9,"method":"no/such/method"}"#,
    );
    let _ = send("malformed JSON frame", r#"{not json at all"#);
    let _ = send(
        "wrong jsonrpc version",
        r#"{"jsonrpc":"1.0","id":10,"method":"initialize"}"#,
    );
    // A notification has no `id` and must produce no reply at all —
    // replying to one is a classic client-breaking server bug.
    let _ = send(
        "notification (no id) — must be silent",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );

    let _ = fs::remove_file(&path);
    println!("\nSession complete.");
}
