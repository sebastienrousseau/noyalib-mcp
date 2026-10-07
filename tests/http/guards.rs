// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! What the HTTP transports refuse: a foreign `Host` or `Origin`, a
//! post that is not JSON, and a session past the limit.

use std::time::Duration;

use serde_json::json;

use super::{ACCEPT_BOTH, Http, JSON, Server, initialize, stateless};

const EVIL_HOST: (&str, &str) = ("Host", "evil.example");
const EVIL_ORIGIN: (&str, &str) = ("Origin", "http://evil.example");

#[test]
fn sse_refuses_a_foreign_host_or_origin() {
    // A page that rebound its own name to 127.0.0.1 carries that name
    // in `Host` and `Origin`. Neither route may serve it.
    let server = Server::start("sse");
    for bad in [EVIL_HOST, EVIL_ORIGIN] {
        let r = Http::send(&server.addr, "GET", "/sse", &[bad], "");
        assert_eq!(r.status, 403, "GET /sse with {bad:?}");
    }
    let mut stream = Http::send(&server.addr, "GET", "/sse", &[], "");
    let (_, endpoint) = stream.next_event();
    let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    for bad in [EVIL_HOST, EVIL_ORIGIN] {
        let r = Http::send(&server.addr, "POST", &endpoint, &[bad, JSON], ping);
        assert_eq!(r.status, 403, "POST with {bad:?}");
    }
    // A loopback origin, the server's own, is a local client.
    let own = ("Origin", "http://localhost:1234");
    let r = Http::send(&server.addr, "POST", &endpoint, &[own, JSON], ping);
    assert_eq!(r.status, 202);
    assert_eq!(stream.next_message()["id"], 1);
}

#[test]
fn sse_takes_json_posts_only() {
    // `text/plain` is what a browser may send cross-origin without a
    // preflight, so it is refused rather than parsed.
    let server = Server::start("sse");
    let mut stream = Http::send(&server.addr, "GET", "/sse", &[], "");
    let (_, endpoint) = stream.next_event();
    let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    for headers in [&[("Content-Type", "text/plain")][..], &[][..]] {
        let r = Http::send(&server.addr, "POST", &endpoint, headers, ping);
        assert_eq!(r.status, 415, "{headers:?}");
    }
    let charset = ("Content-Type", "application/json; charset=utf-8");
    let r = Http::send(&server.addr, "POST", &endpoint, &[charset], ping);
    assert_eq!(r.status, 202);
    assert_eq!(stream.next_message()["id"], 1);
}

#[test]
fn streamable_http_refuses_a_foreign_origin() {
    let server = Server::start("streamable-http");
    let r = Http::send(
        &server.addr,
        "POST",
        "/mcp",
        &[ACCEPT_BOTH, JSON, EVIL_ORIGIN],
        &initialize("2025-11-25"),
    );
    assert_eq!(r.status, 403);
    let r = Http::send(
        &server.addr,
        "POST",
        "/mcp",
        &[ACCEPT_BOTH, JSON, ("Origin", "http://127.0.0.1:9")],
        &initialize("2025-11-25"),
    );
    assert_eq!(r.status, 200);
}

#[test]
fn sse_caps_the_number_of_live_sessions() {
    let server = Server::start_with("sse", &["--max-sessions", "2"]);
    let mut first = Http::send(&server.addr, "GET", "/sse", &[], "");
    let _ = first.next_event();
    let mut second = Http::send(&server.addr, "GET", "/sse", &[], "");
    let _ = second.next_event();
    let third = Http::send(&server.addr, "GET", "/sse", &[], "");
    assert_eq!(third.status, 503);
    // Hanging up frees the slot.
    drop(first);
    let mut status = 0;
    for _ in 0..50 {
        let mut again = Http::send(&server.addr, "GET", "/sse", &[], "");
        status = again.status;
        if status == 200 {
            let (event, _) = again.next_event();
            assert_eq!(event, "endpoint");
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(status, 200, "the slot was not freed");
}

#[test]
fn streamable_http_caps_the_number_of_live_sessions() {
    let server = Server::start_with("streamable-http", &["--max-sessions", "2"]);
    let open = || {
        Http::send(
            &server.addr,
            "POST",
            "/mcp",
            &[ACCEPT_BOTH, JSON],
            &initialize("2025-11-25"),
        )
    };
    let a = open();
    assert_eq!(a.status, 200);
    let b = open();
    assert_eq!(b.status, 200);
    let c = open();
    assert_ne!(c.status, 200, "a third session was created");
    assert!(c.body().contains("session limit"));
    // A request with no session (the stateless revision) still works.
    let r = Http::send(
        &server.addr,
        "POST",
        "/mcp",
        &[
            ACCEPT_BOTH,
            JSON,
            ("MCP-Protocol-Version", "2026-07-28"),
            ("Mcp-Method", "server/discover"),
        ],
        &stateless(7, "server/discover", json!({})),
    );
    assert_eq!(r.status, 200);
}

#[test]
fn sse_refuses_what_it_cannot_route() {
    let server = Server::start("sse");
    let r = Http::send(
        &server.addr,
        "POST",
        "/messages/?sessionId=unknown",
        &[JSON],
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    );
    assert_eq!(r.status, 404);

    let mut stream = Http::send(&server.addr, "GET", "/sse", &[], "");
    let (_, endpoint) = stream.next_event();
    let r = Http::send(&server.addr, "POST", &endpoint, &[JSON], "{not json");
    assert_eq!(r.status, 400);
    assert!(r.body().contains("invalid JSON-RPC"));

    // Hanging up ends the session: the endpoint stops existing.
    drop(stream);
    let mut gone = 0;
    for _ in 0..50 {
        let r = Http::send(
            &server.addr,
            "POST",
            &endpoint,
            &[JSON],
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        );
        gone = r.status;
        if gone == 404 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(gone, 404, "the session outlived its stream");
}
