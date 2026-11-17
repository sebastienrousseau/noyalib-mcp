// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Session lifetime and capacity through the HTTP wire boundary.

use super::{ACCEPT_BOTH, Http, JSON, Server, initialize};
use std::time::{Duration, Instant};

fn delete_releases_capacity(version: &str) {
    let server = Server::start_with("streamable-http", &["--max-sessions", "1"]);
    let open = || {
        Http::send(
            &server.addr,
            "POST",
            "/mcp",
            &[ACCEPT_BOTH, JSON],
            &initialize(version),
        )
    };
    let mut first = open();
    assert_eq!(first.status, 200);
    let id = first
        .header("mcp-session-id")
        .expect("session id")
        .to_owned();
    assert_eq!(first.next_message()["result"]["protocolVersion"], version);
    drop(first);
    let headers = [
        ACCEPT_BOTH,
        JSON,
        ("Mcp-Session-Id", id.as_str()),
        ("MCP-Protocol-Version", version),
    ];
    let initialized = Http::send(
        &server.addr,
        "POST",
        "/mcp",
        &headers,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    assert_eq!(initialized.status, 202);
    drop(initialized);

    let full = open();
    assert_ne!(full.status, 200, "the one-session limit was exceeded");
    assert!(full.body().contains("session limit"));

    let deleted = Http::send(&server.addr, "DELETE", "/mcp", &headers, "");
    assert!(deleted.status < 300, "DELETE failed: {}", deleted.status);
    drop(deleted);
    let stale = Http::send(
        &server.addr,
        "POST",
        "/mcp",
        &headers,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
    );
    assert_eq!(stale.status, 404, "a deleted session remained usable");
    drop(stale);

    let mut replacement = open();
    assert_eq!(replacement.status, 200, "the deleted slot was not reusable");
    assert_ne!(replacement.header("mcp-session-id"), Some(id.as_str()));
    assert_eq!(
        replacement.next_message()["result"]["protocolVersion"],
        version
    );
}

#[test]
fn march_revision_delete_releases_capacity() {
    delete_releases_capacity("2025-03-26");
}

#[test]
fn june_revision_delete_releases_capacity() {
    delete_releases_capacity("2025-06-18");
}

#[test]
fn november_revision_delete_releases_capacity() {
    delete_releases_capacity("2025-11-25");
}

#[test]
fn sse_disconnect_revokes_the_post_endpoint_and_releases_capacity() {
    let server = Server::start_with("sse", &["--max-sessions", "1"]);
    let mut stream = Http::send(&server.addr, "GET", "/sse", &[], "");
    assert_eq!(stream.status, 200);
    let (_, endpoint) = stream.next_event();
    let accepted = Http::send(
        &server.addr,
        "POST",
        &endpoint,
        &[JSON],
        &initialize("2024-11-05"),
    );
    assert_eq!(accepted.status, 202);
    assert_eq!(
        stream.next_message()["result"]["protocolVersion"],
        "2024-11-05"
    );
    drop(accepted);
    let full = Http::send(&server.addr, "GET", "/sse", &[], "");
    assert_eq!(full.status, 503);
    drop(full);
    drop(stream);

    // TCP disconnect is observed asynchronously. Poll the public endpoint
    // within a deadline instead of assuming a particular scheduler turn.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let stale = Http::send(
            &server.addr,
            "POST",
            &endpoint,
            &[JSON],
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        );
        if stale.status == 404 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "disconnected endpoint remained live: {}",
            stale.status
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut replacement = Http::send(&server.addr, "GET", "/sse", &[], "");
    assert_eq!(
        replacement.status, 200,
        "disconnected slot remained occupied"
    );
    let (event, next_endpoint) = replacement.next_event();
    assert_eq!(event, "endpoint");
    assert_ne!(next_endpoint, endpoint);
    let accepted = Http::send(
        &server.addr,
        "POST",
        &next_endpoint,
        &[JSON],
        &initialize("2024-11-05"),
    );
    assert_eq!(accepted.status, 202);
    assert_eq!(
        replacement.next_message()["result"]["protocolVersion"],
        "2024-11-05"
    );
}
