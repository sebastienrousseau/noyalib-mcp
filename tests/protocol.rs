// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The server over a real pipe.
//!
//! The tools are unit-tested in the library and the session in
//! `tests/serve.rs`. What these cover is the process around them: the
//! stdio transport's line framing, the handshake a client performs
//! first, which inputs draw a reply at all, that a write lands on disk
//! through the real binary, and that the process exits cleanly at end
//! of input. All of it is what an MCP client depends on, and none of
//! it is reachable from a unit test.

#![allow(missing_docs)]

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_noyalib-mcp")
}

/// Send `lines` to the server and collect the replies, parsed.
fn converse(lines: &[&str]) -> Vec<Value> {
    let mut child = Command::new(bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server starts");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        for l in lines {
            writeln!(stdin, "{l}").expect("write");
        }
    }
    // Dropping stdin signals end of input; the server must then exit.
    let out = child.wait_with_output().expect("server exits");
    assert!(
        out.status.success(),
        "server exited with {}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

/// The reply carrying `id`.
///
/// Requests are dispatched concurrently, so replies may come back in
/// any order; a client correlates them by id, and so must a test.
fn by_id(replies: &[Value], id: u64) -> &Value {
    replies
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no reply with id {id} in {replies:?}"))
}

fn tempfile(contents: &str) -> std::path::PathBuf {
    // `process::id() + SystemTime::now().as_nanos()` is normally
    // unique per call — but on Windows-nightly under cargo-test's
    // parallel scheduler two calls have been observed landing in
    // the same nanosecond, leading to a path collision that
    // silently shared a fixture between concurrent tests. Append
    // a monotonically-increasing process-local counter so the
    // path is guaranteed unique even under nano collisions.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);

    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "noyalib-mcp-test-{}-{}-{}.yaml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        seq,
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

fn call(id: u64, name: &str, arguments: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
           "params": {"name": name, "arguments": arguments}})
    .to_string()
}

#[test]
fn initialize_returns_protocol_version_and_server_info() {
    let replies = converse(&[INITIALIZE]);
    assert_eq!(replies.len(), 1, "{replies:?}");
    let r = &replies[0];
    assert_eq!(r["jsonrpc"], "2.0");
    assert_eq!(r["id"], 1);
    assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(r["result"]["serverInfo"]["name"], "noyalib-mcp");
    assert_eq!(
        r["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(r["result"]["capabilities"]["tools"].is_object());
    assert!(r["result"]["capabilities"]["prompts"].is_object());
    assert!(r["result"]["capabilities"]["resources"].is_object());
    assert!(r["result"]["instructions"].is_string());
}

#[test]
fn an_older_client_is_answered_in_its_own_revision() {
    // A client pinned to 2024-11-05 gets 2024-11-05 back, not a newer
    // revision it would have to refuse.
    let replies = converse(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"old","version":"0"}}}"#,
    ]);
    assert_eq!(replies[0]["result"]["protocolVersion"], "2024-11-05");
}

#[test]
fn tools_list_announces_every_tool_with_its_schemas() {
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        r#"{"jsonrpc":"2.0","id":7,"method":"tools/list"}"#,
    ]);
    let list = by_id(&replies, 7);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for want in noyalib_mcp::TOOL_NAMES {
        assert!(names.contains(&want), "{names:?} lacks {want}");
    }
    // Every tool must have an inputSchema (clients use it for arg
    // validation and prompt-generation), an outputSchema and
    // annotations.
    for t in tools {
        assert!(t["inputSchema"].is_object(), "{t}");
        assert!(t["outputSchema"].is_object(), "{t}");
        assert!(t["annotations"]["readOnlyHint"].is_boolean(), "{t}");
    }
}

#[test]
fn tool_call_get_reads_value_at_path() {
    let path = tempfile("name: noyalib\nport: 8080\n");
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        &call(
            11,
            "noyalib_get",
            json!({ "file": path.to_str().unwrap(), "path": "port" }),
        ),
    ]);
    let r = by_id(&replies, 11);
    assert_eq!(r["result"]["content"][0]["text"], "8080", "{r}");
    assert_eq!(r["result"]["structuredContent"]["value"], "8080");
    assert_eq!(r["result"]["isError"], false);
}

#[test]
fn tool_call_set_preserves_comments() {
    let path = tempfile(
        "# version is bumped by Renovate\n\
         version: 0.0.1  # do not edit by hand\n\
         name: noyalib\n",
    );
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        &call(
            13,
            "noyalib_set",
            json!({ "file": path.to_str().unwrap(), "path": "version", "value": "0.0.2" }),
        ),
    ]);
    assert_eq!(by_id(&replies, 13)["result"]["isError"], false);
    let after = std::fs::read_to_string(&path).unwrap();
    // The CST guarantee: only the touched span changes.
    assert!(after.contains("version: 0.0.2"));
    assert!(after.contains("# version is bumped by Renovate"));
    assert!(after.contains("# do not edit by hand"));
    assert!(after.contains("name: noyalib"));
}

#[test]
fn unknown_method_returns_error() {
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        r#"{"jsonrpc":"2.0","method":"definitely/not/a/method","id":99}"#,
    ]);
    assert_eq!(by_id(&replies, 99)["error"]["code"], -32601);
}

#[test]
fn an_unknown_tool_is_a_result_the_model_can_read() {
    // A method the protocol does not have is a JSON-RPC error; a tool
    // the server does not have is a result naming the tools that exist.
    let replies = converse(&[INITIALIZE, INITIALIZED, &call(3, "frobnicate", json!({}))]);
    let r = by_id(&replies, 3);
    assert_eq!(r["result"]["isError"], true, "{r}");
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Unknown tool: frobnicate"), "{text}");
    assert!(text.contains("noyalib_get"), "{text}");
}

#[test]
fn a_missing_argument_is_a_result_naming_the_field() {
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        &call(4, "noyalib_get", json!({ "file": "x.yaml" })),
    ]);
    let r = by_id(&replies, 4);
    assert_eq!(r["result"]["isError"], true, "{r}");
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("path"), "{text}");
}

#[test]
fn notification_gets_no_reply() {
    // A `notifications/initialized` message has no id, so the
    // server should process it silently — no response on stdout.
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        // Send a regular request after to keep the round-trip
        // collector unblocked and prove the server stays alive.
        r#"{"jsonrpc":"2.0","method":"ping","id":2}"#,
    ]);
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert!(by_id(&replies, 2)["result"].is_object());
}

#[test]
fn a_stateless_client_needs_no_handshake() {
    // The 2026-07-28 revision has no `initialize`: every request names
    // its protocol version in `_meta`, and the first one may be the
    // real work.
    let replies = converse(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
    ]);
    assert_eq!(replies.len(), 2, "{replies:?}");
    let list = by_id(&replies, 1);
    assert_eq!(list["result"]["resultType"], "complete");
    assert!(list["result"]["tools"].is_array());
    let discover = by_id(&replies, 2);
    let versions = discover["result"]["supportedVersions"]
        .as_array()
        .expect("supportedVersions");
    assert!(versions.contains(&json!("2026-07-28")), "{discover}");
    assert!(versions.contains(&json!("2025-11-25")), "{discover}");
    assert_eq!(
        discover["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "noyalib-mcp"
    );
}

#[test]
fn blank_lines_and_malformed_input_do_not_stop_the_server() {
    // The connection must survive a bad line: an MCP client would
    // otherwise see the whole server die on one typo. The SDK skips
    // bytes that are not JSON without a reply, and answers JSON that
    // is not a JSON-RPC message with an error.
    let replies = converse(&[
        "",
        "this is not json",
        INITIALIZE,
        "   ",
        INITIALIZED,
        r#"{"jsonrpc":"2.0","id":7}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
    ]);
    assert_eq!(replies.len(), 3, "{replies:?}");
    assert!(by_id(&replies, 1)["result"]["protocolVersion"].is_string());
    // The SDK cannot bind that error to the request it could not
    // read, so it answers with a null id and the invalid-request code.
    assert!(
        replies.iter().any(|r| r["error"]["code"] == -32600),
        "{replies:?}"
    );
    assert!(by_id(&replies, 2)["result"]["tools"].is_array());
}

#[test]
fn no_input_at_all_exits_successfully() {
    assert!(converse(&[]).is_empty());
}

#[test]
fn prompts_and_resources_are_served_over_stdio() {
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"format_and_lint_yaml","arguments":{"file":"deploy.yaml"}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#,
        r#"{"jsonrpc":"2.0","id":5,"method":"resources/templates/list"}"#,
        r#"{"jsonrpc":"2.0","id":6,"method":"resources/read","params":{"uri":"noyalib://tool/noyalib_edit"}}"#,
        r#"{"jsonrpc":"2.0","id":7,"method":"resources/read","params":{"uri":"noyalib://nope"}}"#,
    ]);
    assert_eq!(replies.len(), 7, "{replies:?}");
    assert_eq!(
        by_id(&replies, 2)["result"]["prompts"][0]["name"],
        "format_and_lint_yaml"
    );
    let text = by_id(&replies, 3)["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("prompt text");
    assert!(text.contains("`deploy.yaml`"), "{text}");
    assert_eq!(
        by_id(&replies, 4)["result"]["resources"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        by_id(&replies, 5)["result"]["resourceTemplates"][0]["uriTemplate"],
        "noyalib://tool/{name}"
    );
    let read = by_id(&replies, 6);
    let text = read["result"]["contents"][0]["text"]
        .as_str()
        .expect("resource text");
    let tool: Value = serde_json::from_str(text).expect("a tool descriptor");
    assert_eq!(tool["name"], "noyalib_edit");
    assert_eq!(by_id(&replies, 7)["error"]["code"], -32002);
}

/// A parse error in a later document of a stream is reported at its
/// position in the file, not in the document that failed (noyalib
/// #407). The bad alias is on line 5 of the file and line 2 of its own
/// document; the message must say line 5.
#[test]
fn tool_call_set_multidoc_locates_parse_errors_in_the_file() {
    let path = tempfile("a: 1\n---\nb: 2\n---\nc: *nope\n");
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        &call(
            14,
            "noyalib_set_multidoc",
            json!({ "file": path.to_str().unwrap(), "doc_index": 0, "path": "a", "value": "2" }),
        ),
    ]);
    let r = by_id(&replies, 14);
    assert_eq!(r["result"]["isError"], true, "{r}");
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("line 5, column 4"),
        "expected the file position: {text}"
    );
    assert!(!text.contains("line 2, column 4"), "{text}");
    // The file is left untouched.
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "a: 1\n---\nb: 2\n---\nc: *nope\n"
    );
}

/// Document 2 of the core's ultra-complex fixture through
/// `noyalib_parse`: the tool's text is exactly the expected JSON model,
/// and it stays on one line of the transport however many newlines
/// the document holds.
#[test]
fn tool_call_parse_projects_the_ultra_complex_fixture_onto_json() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ultra-complex");
    let yaml = std::fs::read_to_string(dir.join("valid.yaml")).unwrap();
    let doc2 = &yaml[yaml.find("---\n# Document 2").expect("document 2")..];
    let expected: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("valid.json")).unwrap()).unwrap();
    let replies = converse(&[
        INITIALIZE,
        INITIALIZED,
        &call(21, "noyalib_parse", json!({ "yaml": doc2 })),
    ]);
    let r = by_id(&replies, 21);
    let text = r["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{r}"));
    let got: Value = serde_json::from_str(text).expect("tool text is JSON");
    assert_eq!(got, expected[1]);
    assert_eq!(
        r["result"]["structuredContent"]["documents"][0],
        expected[1]
    );
}

#[test]
fn version_and_help_print_and_exit() {
    let out = Command::new(bin()).arg("--version").output().expect("runs");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        text.trim(),
        format!("noyalib-mcp {}", env!("CARGO_PKG_VERSION"))
    );

    let out = Command::new(bin()).arg("--help").output().expect("runs");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--transport"), "{text}");
    assert!(text.contains("/mcp"), "{text}");
}

#[test]
fn a_bad_argument_is_a_usage_error() {
    let out = Command::new(bin())
        .args(["--transport", "telepathy"])
        .output()
        .expect("runs");
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("telepathy"), "{text}");
    assert!(text.contains("Usage:"), "{text}");
}
