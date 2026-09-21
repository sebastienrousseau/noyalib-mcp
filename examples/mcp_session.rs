// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! A complete MCP session, driven in-process.
//!
//! The binary speaks MCP over stdio, which makes it awkward to explore
//! by hand: you cannot see what a client actually sends, and a
//! malformed frame just hangs. The six operations are plain functions,
//! so a program can call them directly; and the server that wraps them
//! speaks MCP over anything that reads and writes bytes, so a session
//! can be held through an in-memory pipe with the SDK's own client.
//! This example does both.
//!
//! The sequence mirrors what an MCP-aware client (Claude, Cursor, Zed)
//! does on connect — the handshake, `tools/list`, then `tools/call` —
//! and demonstrates the property that makes these tools safe for an
//! agent to use on a real repository: **`noyalib_set` rewrites only
//! the touched span**, so comments, blank lines and sibling formatting
//! survive byte-for-byte. That is the whole reason to hand an agent
//! this server instead of "parse YAML, mutate, re-serialise".
//!
//! Run: `cargo run --example mcp_session`

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock};
use serde_json::{Map, Value, json};
use std::fs;

fn arguments(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => unreachable!("arguments are an object"),
    }
}

fn show(label: &str, result: &CallToolResult) {
    let text = result
        .content
        .first()
        .and_then(ContentBlock::as_text)
        .map_or("", |t| t.text.as_str());
    let flag = if result.is_error == Some(true) {
        "isError"
    } else {
        "ok"
    };
    println!("\n── {label} ──");
    println!("  <- [{flag}] {text}");
    if let Some(structured) = &result.structured_content {
        println!("  <- structuredContent: {structured}");
    }
}

#[tokio::main]
async fn main() {
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

    // The functions, without any protocol around them.
    let port = noyalib_mcp::get(&file, "server.port").expect("a value");
    println!("\nserver.port, read directly: {port}");
    let parsed = noyalib_mcp::parse("a: 1\nb: [x, y]\n").expect("parses");
    println!("parsed: {parsed}");
    let fault = noyalib_mcp::edit("a: 1\n", "missing", "2").expect_err("no such path");
    println!("a failure is text a model can act on: {fault}");

    // The same six, as MCP tools over an in-memory pipe. A host
    // process would do this over the server's stdin and stdout.
    let (client_io, server_io) = tokio::io::duplex(1 << 16);
    // `serve` returns once the handshake is done, so the server side
    // must already be waiting when the client starts talking.
    drop(tokio::spawn(async move {
        if let Ok(server) = noyalib_mcp::YamlServer::new().serve(server_io).await {
            let _ = server.waiting().await;
        }
    }));
    let client = ().serve(client_io).await.expect("handshake");

    // 1. The handshake is done; this is what it negotiated.
    let info = client.peer_info().expect("initialize result");
    println!(
        "\n── initialize ──\n  <- {} {} speaking {}",
        info.server_info.as_ref().map_or("?", |s| s.name.as_str()),
        info.server_info
            .as_ref()
            .map_or("?", |s| s.version.as_str()),
        info.protocol_version
    );

    // 2. Discovery — how an agent learns what it may call.
    let tools = client.list_all_tools().await.expect("tools/list");
    println!("\n── tools/list ──");
    for t in &tools {
        println!("  <- {}", t.name);
    }

    let call = |name: &'static str, args: Value| {
        let client = &client;
        async move {
            client
                .call_tool(CallToolRequestParams::new(name).with_arguments(arguments(args)))
                .await
                .expect("tools/call")
        }
    };

    // 3. Read a nested scalar by dotted path.
    show(
        "tools/call noyalib_get — read server.port",
        &call("noyalib_get", json!({"file": file, "path": "server.port"})).await,
    );

    // 4. Read an indexed sequence element.
    show(
        "tools/call noyalib_get — read features[0]",
        &call("noyalib_get", json!({"file": file, "path": "features[0]"})).await,
    );

    // 5. Write — the operation an agent performs to change config.
    show(
        "tools/call noyalib_set — set server.port to 9090",
        &call(
            "noyalib_set",
            json!({"file": file, "path": "server.port", "value": "9090"}),
        )
        .await,
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

    println!("\n── failures a client must handle: each is a result, not a crash ──");
    show("unknown tool", &call("frobnicate", json!({})).await);
    show(
        "missing required argument",
        &call("noyalib_get", json!({"file": file})).await,
    );
    show(
        "path that does not exist in the document",
        &call("noyalib_get", json!({"file": file, "path": "server.nope"})).await,
    );
    show(
        "YAML that does not parse",
        &call("noyalib_validate", json!({"yaml": "a: [\n"})).await,
    );

    let _ = client.cancel().await;
    let _ = fs::remove_file(&path);
    println!("\nSession complete.");
}
