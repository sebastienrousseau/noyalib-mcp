// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Library surface for `noyalib-mcp`.
//!
//! Hosts the JSON-RPC 2.0 dispatch logic and the tool implementations.
//! The `noyalib-mcp` binary in `main.rs` is a thin stdio loop that
//! drives [`handle_message`]; tests reach the same handlers
//! directly so coverage no longer depends on standing up a real
//! stdio process.
//!
//! # Cargo features
//!
//! This crate exposes no optional features; the MCP tool set
//! (`noyalib_get`, `noyalib_set`, `noyalib_set_multidoc`) is
//! fixed. Optional `noyalib`
//! features pulled in by a downstream packager (`schema`,
//! `parallel`, …) do not change the MCP wire surface — they
//! only affect what `noyalib::Error` messages can appear inside
//! tool-call error envelopes. The canonical `noyalib` feature
//! matrix lives in
//! [`crates/noyalib/src/lib.rs`](https://docs.rs/noyalib).
//!
//! # MSRV
//!
//! **Rust 1.86.0** stable — same as the core `noyalib` library
//! (and this crate's `rust-version`; an earlier revision of this
//! paragraph said 1.75.0 while the manifest said 1.86.0 — the
//! manifest is the contract).
//! The MCP wire surface is text-only JSON-RPC and pulls no
//! nightly-only deps. CI verifies the floor via the
//! `Per-crate MSRV` workflow job. See the workspace
//! [`POLICIES.md`](https://github.com/sebastienrousseau/noyalib/blob/main/doc/POLICIES.md#1-msrv-minimum-supported-rust-version)
//! for the bump policy.
//!
//! # Panics
//!
//! Public functions in this crate do not panic on well-formed
//! input. The MCP binary `unwrap`s once on stdin acquisition
//! during boot — that's deliberate, every caller invokes the
//! binary via a host process that controls the pipe.
//!
//! # Errors
//!
//! Tool calls return JSON-RPC error envelopes per the
//! [MCP specification](https://modelcontextprotocol.io). The
//! error code taxonomy lives in
//! [`crates/noyalib-mcp/doc/tools-reference.md`](https://github.com/sebastienrousseau/noyalib/blob/main/crates/noyalib-mcp/doc/tools-reference.md):
//! `-32000` (file I/O), `-32001` (parse), `-32002` (path not
//! found), `-32003` (set), `-32602` (missing arg), `-32601`
//! (unknown method).
//!
//! # Concurrency
//!
//! Each MCP request is processed sequentially on the binary's
//! stdio loop. The host (Claude Desktop, Cursor, Zed, …) is
//! responsible for not pipelining requests; if it does, the
//! tool execution is serialised by the loop's `BufRead` reader.
//!
//! # Platform support
//!
//! Tier-1 (CI-verified each PR): `aarch64-apple-darwin`,
//! `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`.
//!
//! `noyalib_set` writes via an *atomic file replacement*
//! helper: write to a sibling temp file → `sync_all` →
//! `rename`. This is naturally atomic on POSIX; on Windows it
//! uses `MoveFileExW(MOVEFILE_REPLACE_EXISTING |
//! MOVEFILE_WRITE_THROUGH)` semantics so concurrent readers
//! always see either the old or the new contents — never a
//! half-write or a stale-page-cache observation. This was the
//! fix for the historical Windows-only `tool_call_set_preserves_comments`
//! flake.
//!
//! # Performance
//!
//! Each `tools/call` round-trip goes through one
//! `noyalib::cst::parse_document` (`O(n)` over input bytes)
//! and, for `noyalib_set`, one `Document::to_string` emit
//! (`O(n)` over output bytes). JSON-RPC line framing is
//! amortised constant-time per message. Tool calls do **not**
//! cache the parsed CST between requests — every call is a
//! fresh parse so concurrent edits from outside the MCP server
//! are always observed. Typical tool-call latency on a 100 KB
//! YAML file: 1–3 ms parse + emit on commodity hardware.
//!
//! # Security
//!
//! `#![forbid(unsafe_code)]`. No FFI. No network I/O —
//! `noyalib-mcp` is stdio-only by design; remote hosting goes
//! through a separate broker (see `examples/hosted-mcp-run.md`).
//! The server has no auth layer; restrict the working
//! directory of the spawned process via container mounts /
//! systemd `ReadWritePaths=` for production deployments.
//! Resource-limit gates are inherited from `noyalib`'s
//! `ParserConfig` defaults. Full posture:
//! [`SECURITY.md`](https://github.com/sebastienrousseau/noyalib/blob/main/SECURITY.md).
//!
//! # API stability and SemVer
//!
//! Pre-1.0 (`0.0.x`): the MCP wire contract (tool names,
//! input-schema shapes, error code ranges, the
//! [`SUPPORTED_PROTOCOL_VERSIONS`] set) is **stable** within a
//! 0.0.x line — bug fixes only.
//! Adding a new tool is allowed within a 0.0.x bump; removing
//! or renaming a tool, or repurposing an error code, is held
//! to a 0.x bump (e.g. 0.0.x → 0.1.0). The Rust library
//! surface (`handle_message`, `dispatch`, `error_str`,
//! `Request`, `Response`, `ErrorResponse`, `HandleOutcome`) is
//! covered by the workspace SemVer policy in
//! [`POLICIES.md`](https://github.com/sebastienrousseau/noyalib/blob/main/doc/POLICIES.md#2-semver--api-stability).
//! `cargo-semver-checks` runs in CI on every PR.
//!
//! # Documentation
//!
//! - **Engineering policies** — workspace
//!   [`POLICIES.md`](https://github.com/sebastienrousseau/noyalib/blob/main/doc/POLICIES.md).
//! - **MCP specification**: <https://modelcontextprotocol.io>.
//! - **Tools reference** (input schemas, error codes):
//!   [`doc/tools-reference.md`](https://github.com/sebastienrousseau/noyalib/blob/main/crates/noyalib-mcp/doc/tools-reference.md).
//! - **Host configurations** (Claude Desktop, Cursor,
//!   Continue.dev, Zed, hosted gateways):
//!   [`examples/`](https://github.com/sebastienrousseau/noyalib/tree/main/crates/noyalib-mcp/examples).

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Opt-in coverage exclusion (`NOYALIB_COVERAGE=1`) — see
// `build.rs` for the flag, individual `coverage(off)` annotations
// are below.
#![cfg_attr(noyalib_coverage, allow(unstable_features))]
#![cfg_attr(noyalib_coverage, feature(coverage_attribute))]

use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

pub mod prompts;
pub mod resources;
pub mod tools;

/// Protocol revisions this server speaks, newest first. The server
/// is **dual-era** (MCP 2026-07-28 "Versioning and Compatibility"
/// terminology): a modern client declares its version per request in
/// `_meta` and may probe with `server/discover`; a legacy client
/// opens with an `initialize` handshake and negotiates
/// [`LEGACY_PROTOCOL_VERSION`].
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 2] = ["2026-07-28", "2025-06-18"];

/// The newest handshake-based revision this server implements —
/// what `initialize` answers when the client's requested version is
/// not supported (per the 2025-06-18 negotiation rules, the server
/// then responds with a version it does support).
pub const LEGACY_PROTOCOL_VERSION: &str = "2025-06-18";

/// The `_meta` key a modern (2026-07-28+) client uses to declare the
/// protocol revision of each request.
pub const META_PROTOCOL_VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";

/// `_meta` key under which each result identifies this server
/// (2026-07-28 "servers SHOULD identify themselves in each result's
/// `_meta`").
const META_SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// `UnsupportedProtocolVersionError` code (2026-07-28 error-code
/// allocation: `-32020..=-32099` reserved for the MCP spec).
pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;

/// One hour, in milliseconds — the `ttlMs` freshness hint on the
/// cacheable results (`tools/list`, `prompts/list`,
/// `resources/list`, `resources/templates/list`, `resources/read`).
/// The catalogue is fixed per binary, so any bound would do; an
/// hour keeps re-listing cheap without making a stale cache
/// survive a server upgrade for long.
const CACHE_TTL_MS: u64 = 3_600_000;

/// JSON-RPC 2.0 request envelope. Method-specific parameters live
/// in [`JsonValue`] to keep parsing flexible across the few methods
/// the MCP spec asks of a server.
#[derive(Debug, Deserialize)]
pub struct Request {
    /// JSON-RPC version. MCP requires `"2.0"`.
    pub jsonrpc: String,
    /// Method name, e.g. `tools/call`. Notifications have no `id`.
    pub method: String,
    /// Method parameters. Shape depends on `method`.
    #[serde(default)]
    pub params: JsonValue,
    /// Request id; absent on notifications.
    pub id: Option<JsonValue>,
}

/// JSON-RPC 2.0 success response envelope.
#[derive(Debug, Serialize)]
pub struct Response {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// The result payload.
    pub result: JsonValue,
    /// Echo of the corresponding request's id.
    pub id: JsonValue,
}

/// JSON-RPC 2.0 error envelope.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Error payload.
    pub error: ErrorObject,
    /// Echo of the corresponding request's id.
    pub id: JsonValue,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct ErrorObject {
    /// Numeric error code per JSON-RPC convention.
    pub code: i32,
    /// Human-readable message.
    pub message: String,
    /// Optional structured detail — e.g.
    /// `UnsupportedProtocolVersionError` carries
    /// `{"supported": […], "requested": "…"}` so a modern client
    /// can pick a mutually supported revision and retry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}

/// What the stdio loop should do with a parsed message — write a
/// reply on stdout, or stay silent (notifications never receive a
/// response).
#[derive(Debug, PartialEq, Eq)]
pub enum HandleOutcome {
    /// Send the wrapped JSON payload back on stdout.
    Reply(String),
    /// Notification — no reply expected.
    Silent,
}

/// Process one newline-delimited JSON-RPC message. The stdio loop
/// in `main` calls this per line; tests call it with crafted
/// strings.
///
/// # Examples
///
/// ```
/// use noyalib_mcp::{handle_message, HandleOutcome};
/// let req = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
/// match handle_message(req) {
///     // Every result carries the 2026-07-28 envelope fields.
///     HandleOutcome::Reply(s) => assert!(s.contains("\"resultType\":\"complete\"")),
///     HandleOutcome::Silent => panic!("expected reply"),
/// }
/// ```
#[must_use]
pub fn handle_message(raw: &str) -> HandleOutcome {
    let req: Request = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            return HandleOutcome::Reply(error_str(
                JsonValue::Null,
                -32700,
                format!("parse error: {e}"),
            ));
        }
    };
    if req.jsonrpc != "2.0" {
        return HandleOutcome::Reply(error_str(
            req.id.unwrap_or(JsonValue::Null),
            -32600,
            "invalid request: jsonrpc must be \"2.0\"".to_string(),
        ));
    }
    // Notifications (no id) get processed but never replied to.
    let id = req.id.clone();

    // Modern (2026-07-28) clients declare their protocol revision on
    // every request in `_meta`. An unsupported declaration MUST be
    // answered with `UnsupportedProtocolVersionError` listing what
    // the server does speak; an absent one means a legacy client (or
    // a modern client relying on the inline-retry flow) and the
    // request proceeds.
    if let Some(requested) = req
        .params
        .get("_meta")
        .and_then(|m| m.get(META_PROTOCOL_VERSION_KEY))
        .and_then(JsonValue::as_str)
    {
        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
            return match id {
                None => HandleOutcome::Silent,
                Some(id) => HandleOutcome::Reply(
                    serde_json::to_string(&ErrorResponse {
                        jsonrpc: "2.0",
                        error: ErrorObject {
                            code: UNSUPPORTED_PROTOCOL_VERSION,
                            message: "Unsupported protocol version".to_string(),
                            data: Some(json!({
                                "supported": SUPPORTED_PROTOCOL_VERSIONS,
                                "requested": requested,
                            })),
                        },
                        id,
                    })
                    .expect("infallible serialise"),
                ),
            };
        }
    }

    let result = dispatch(&req.method, req.params);
    match (id, result) {
        (None, _) => HandleOutcome::Silent,
        (Some(id), Ok(value)) => HandleOutcome::Reply(
            serde_json::to_string(&Response {
                jsonrpc: "2.0",
                result: decorate_result(value),
                id,
            })
            .expect("infallible serialise"),
        ),
        (Some(id), Err((code, msg))) => HandleOutcome::Reply(error_str(id, code, msg)),
    }
}

/// Stamp the 2026-07-28 result envelope fields onto a dispatch
/// result: `resultType: "complete"` (required on every result since
/// SEP-2322; legacy clients ignore unknown fields, and clients MUST
/// read an absent field as `"complete"`, so stamping is always
/// safe) and the server's identity in `_meta` (a SHOULD). Non-object
/// results — the `null` a legacy `initialized` sent *with* an id
/// gets back — pass through untouched.
fn decorate_result(mut value: JsonValue) -> JsonValue {
    if let JsonValue::Object(map) = &mut value {
        let _ = map.entry("resultType").or_insert_with(|| json!("complete"));
        let meta = map.entry("_meta").or_insert_with(|| json!({}));
        if let Some(meta) = meta.as_object_mut() {
            let _ = meta.entry(META_SERVER_INFO_KEY).or_insert_with(|| {
                json!({
                    "name": "noyalib-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                })
            });
        }
    }
    value
}

/// MCP method dispatcher. Returns the `result` payload on success
/// or a `(code, message)` pair for the error envelope.
///
/// # Examples
///
/// ```
/// use noyalib_mcp::dispatch;
/// use serde_json::Value;
/// let v = dispatch("ping", Value::Null).unwrap();
/// assert!(v.is_object());
/// ```
pub fn dispatch(method: &str, params: JsonValue) -> Result<JsonValue, (i32, String)> {
    match method {
        // Legacy (handshake-era) lifecycle. A dual-era server keeps
        // answering `initialize`: the 2026-07-28 stateless flow never
        // sends it, so its presence identifies a legacy client.
        "initialize" => {
            // 2025-06-18 negotiation: echo the requested version when
            // this server supports it; otherwise answer with the
            // newest handshake-based revision it does support and
            // let the client decide. The previous implementation
            // hard-coded the answer and ignored the request.
            let requested = params.get("protocolVersion").and_then(JsonValue::as_str);
            let negotiated = match requested {
                Some(v) if SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
                _ => LEGACY_PROTOCOL_VERSION,
            };
            Ok(json!({
                "protocolVersion": negotiated,
                "serverInfo": {
                    "name": "noyalib-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "tools": {},
                    "prompts": {},
                    "resources": {}
                }
            }))
        }
        "initialized" | "notifications/initialized" => Ok(JsonValue::Null),
        // Modern (2026-07-28) discovery — MUST be implemented; also
        // the stdio backward-compatibility probe a dual-era client
        // sends before deciding which era this server belongs to.
        "server/discover" => Ok(json!({
            "supportedVersions": SUPPORTED_PROTOCOL_VERSIONS,
            "capabilities": {
                "tools": {},
                "prompts": {},
                "resources": {}
            },
            "instructions": "Read and edit YAML files losslessly: \
                             noyalib_get reads the value at a path, \
                             noyalib_set / noyalib_set_multidoc \
                             rewrite one value while preserving all \
                             comments and formatting.",
            "ttlMs": CACHE_TTL_MS,
            "cacheScope": "public",
        })),
        "tools/list" => Ok(json!({
            "tools": tools::descriptors(),
            "ttlMs": CACHE_TTL_MS,
            "cacheScope": "public",
        })),
        "tools/call" => tools::call(params),
        "prompts/list" => Ok(json!({
            "prompts": prompts::descriptors(),
            "ttlMs": CACHE_TTL_MS,
            "cacheScope": "public",
        })),
        "prompts/get" => prompts::get(params),
        "resources/list" => Ok(json!({
            "resources": resources::descriptors(),
            "ttlMs": CACHE_TTL_MS,
            "cacheScope": "public",
        })),
        "resources/templates/list" => Ok(json!({
            "resourceTemplates": resources::templates(),
            "ttlMs": CACHE_TTL_MS,
            "cacheScope": "public",
        })),
        "resources/read" => resources::read(params).map(|mut v| {
            // `resources/read` is cacheable too (SEP-2549); the
            // served documents are fixed per binary.
            if let JsonValue::Object(map) = &mut v {
                let _ = map.entry("ttlMs").or_insert_with(|| json!(CACHE_TTL_MS));
                let _ = map.entry("cacheScope").or_insert_with(|| json!("public"));
            }
            v
        }),
        // Removed in 2026-07-28 but kept for legacy clients
        // (implementation-defined methods are grandfathered; a
        // modern client simply never sends it).
        "ping" => Ok(JsonValue::Object(serde_json::Map::new())),
        other => Err((-32601, format!("method not found: {other}"))),
    }
}

/// Render a JSON-RPC error envelope to a single line string.
///
/// # Examples
///
/// ```
/// use noyalib_mcp::error_str;
/// use serde_json::json;
/// let s = error_str(json!(1), -32601, "method not found".into());
/// assert!(s.contains("\"code\":-32601"));
/// ```
pub fn error_str(id: JsonValue, code: i32, message: String) -> String {
    serde_json::to_string(&ErrorResponse {
        jsonrpc: "2.0",
        error: ErrorObject {
            code,
            message,
            data: None,
        },
        id,
    })
    .expect("infallible serialise")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_reply(out: HandleOutcome) -> JsonValue {
        match out {
            HandleOutcome::Reply(s) => serde_json::from_str(&s).unwrap(),
            HandleOutcome::Silent => panic!("expected Reply, got Silent"),
        }
    }

    // ── handle_message ─────────────────────────────────────────────────

    #[test]
    fn handle_message_returns_parse_error_on_bad_json() {
        let out = handle_message("not json {");
        let v = parse_reply(out);
        assert_eq!(v["error"]["code"].as_i64().unwrap(), -32700);
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("parse error")
        );
        // Per JSON-RPC: parse errors carry `id: null`.
        assert!(v["id"].is_null());
    }

    #[test]
    fn handle_message_rejects_non_2_0_jsonrpc() {
        let req = json!({"jsonrpc": "1.0", "method": "ping", "id": 1});
        let out = handle_message(&req.to_string());
        let v = parse_reply(out);
        assert_eq!(v["error"]["code"].as_i64().unwrap(), -32600);
        assert_eq!(v["id"].as_i64().unwrap(), 1);
    }

    #[test]
    fn handle_message_returns_silent_for_notifications() {
        let req = json!({"jsonrpc": "2.0", "method": "ping"});
        let out = handle_message(&req.to_string());
        assert_eq!(out, HandleOutcome::Silent);
    }

    #[test]
    fn handle_message_returns_silent_for_notifications_initialized() {
        let req = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        let out = handle_message(&req.to_string());
        assert_eq!(out, HandleOutcome::Silent);
    }

    #[test]
    fn handle_message_returns_unknown_method_error() {
        let req = json!({"jsonrpc": "2.0", "method": "frobnicate", "id": 7});
        let out = handle_message(&req.to_string());
        let v = parse_reply(out);
        assert_eq!(v["error"]["code"].as_i64().unwrap(), -32601);
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("frobnicate")
        );
        assert_eq!(v["id"].as_i64().unwrap(), 7);
    }

    #[test]
    fn handle_message_returns_jsonrpc_error_when_jsonrpc_field_missing() {
        let req = json!({"method": "ping", "id": 1});
        let out = handle_message(&req.to_string());
        let v = parse_reply(out);
        // Either parse error (missing field) or invalid request — both
        // are valid envelopes; the contract is "you get an error".
        assert!(v["error"].is_object());
    }

    // ── dispatch ──────────────────────────────────────────────────────

    #[test]
    fn dispatch_initialize_returns_protocol_metadata() {
        let v = dispatch("initialize", JsonValue::Null).unwrap();
        assert_eq!(v["protocolVersion"].as_str().unwrap(), "2025-06-18");
        assert_eq!(v["serverInfo"]["name"].as_str().unwrap(), "noyalib-mcp");
        assert!(v["capabilities"]["tools"].is_object());
        assert!(v["capabilities"]["prompts"].is_object());
        assert!(v["capabilities"]["resources"].is_object());
    }

    // ── dual-era protocol (2026-07-28) ────────────────────────────────

    #[test]
    fn initialize_echoes_a_supported_requested_version() {
        for v in SUPPORTED_PROTOCOL_VERSIONS {
            let r = dispatch("initialize", json!({"protocolVersion": v})).unwrap();
            assert_eq!(r["protocolVersion"].as_str().unwrap(), v, "requested {v}");
        }
    }

    #[test]
    fn initialize_answers_legacy_for_an_unknown_version() {
        // Per the 2025-06-18 negotiation rules the server responds
        // with a version it does support; the client then decides.
        // The previous implementation ignored the request entirely.
        let r = dispatch("initialize", json!({"protocolVersion": "2024-11-05"})).unwrap();
        assert_eq!(
            r["protocolVersion"].as_str().unwrap(),
            LEGACY_PROTOCOL_VERSION
        );
    }

    #[test]
    fn server_discover_lists_versions_and_capabilities() {
        let v = dispatch("server/discover", JsonValue::Null).unwrap();
        let versions: Vec<&str> = v["supportedVersions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect();
        assert_eq!(versions, SUPPORTED_PROTOCOL_VERSIONS);
        assert!(v["capabilities"]["tools"].is_object());
        assert!(v["ttlMs"].is_u64());
        assert_eq!(v["cacheScope"].as_str().unwrap(), "public");
    }

    #[test]
    fn results_carry_the_modern_envelope_fields() {
        let req = json!({"jsonrpc": "2.0", "method": "tools/list", "id": 7});
        let v = parse_reply(handle_message(&req.to_string()));
        assert_eq!(v["result"]["resultType"].as_str().unwrap(), "complete");
        assert_eq!(
            v["result"]["_meta"][META_SERVER_INFO_KEY]["name"]
                .as_str()
                .unwrap(),
            "noyalib-mcp"
        );
        assert!(v["result"]["ttlMs"].is_u64());
        assert_eq!(v["result"]["cacheScope"].as_str().unwrap(), "public");
    }

    #[test]
    fn a_supported_meta_version_is_served() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 8,
            "params": {"_meta": {META_PROTOCOL_VERSION_KEY: "2026-07-28"}},
        });
        let v = parse_reply(handle_message(&req.to_string()));
        assert!(v["result"]["tools"].is_array());
    }

    #[test]
    fn an_unsupported_meta_version_is_refused_with_the_supported_list() {
        let req = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 9,
            "params": {"_meta": {META_PROTOCOL_VERSION_KEY: "1900-01-01"}},
        });
        let v = parse_reply(handle_message(&req.to_string()));
        assert_eq!(
            v["error"]["code"].as_i64().unwrap(),
            i64::from(UNSUPPORTED_PROTOCOL_VERSION)
        );
        assert_eq!(v["error"]["data"]["requested"], "1900-01-01");
        let supported = v["error"]["data"]["supported"].as_array().unwrap();
        assert_eq!(supported.len(), SUPPORTED_PROTOCOL_VERSIONS.len());
    }

    #[test]
    fn resources_read_is_cacheable() {
        let v = dispatch("resources/read", json!({"uri": "noyalib://tools"})).unwrap();
        assert!(v["ttlMs"].is_u64());
        assert_eq!(v["cacheScope"].as_str().unwrap(), "public");
    }

    #[test]
    fn dispatch_prompts_list_returns_prompt_array() {
        let v = dispatch("prompts/list", JsonValue::Null).unwrap();
        let prompts = v["prompts"].as_array().unwrap();
        assert!(prompts.iter().any(|p| p["name"] == "format_and_lint_yaml"));
    }

    #[test]
    fn dispatch_prompts_get_returns_messages() {
        let v = dispatch("prompts/get", json!({"name": "format_and_lint_yaml"})).unwrap();
        assert!(v["messages"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn dispatch_resources_list_returns_resource_array() {
        let v = dispatch("resources/list", JsonValue::Null).unwrap();
        let resources = v["resources"].as_array().unwrap();
        assert!(resources.iter().any(|r| r["uri"] == "noyalib://tools"));
    }

    #[test]
    fn dispatch_resources_templates_list_returns_templates() {
        let v = dispatch("resources/templates/list", JsonValue::Null).unwrap();
        let templates = v["resourceTemplates"].as_array().unwrap();
        assert!(
            templates
                .iter()
                .any(|t| t["uriTemplate"] == "noyalib://tool/{name}")
        );
    }

    #[test]
    fn dispatch_resources_read_returns_contents() {
        let v = dispatch("resources/read", json!({"uri": "noyalib://error-codes"})).unwrap();
        assert!(v["contents"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn dispatch_initialized_returns_null() {
        let v = dispatch("initialized", JsonValue::Null).unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn dispatch_notifications_initialized_returns_null() {
        let v = dispatch("notifications/initialized", JsonValue::Null).unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn dispatch_tools_list_returns_descriptor_array() {
        let v = dispatch("tools/list", JsonValue::Null).unwrap();
        let tools = v["tools"].as_array().unwrap();
        assert!(tools.iter().any(|t| t["name"] == "noyalib_get"));
        assert!(tools.iter().any(|t| t["name"] == "noyalib_set"));
    }

    #[test]
    fn dispatch_ping_returns_empty_object() {
        let v = dispatch("ping", JsonValue::Null).unwrap();
        assert!(v.is_object());
        assert!(v.as_object().unwrap().is_empty());
    }

    #[test]
    fn dispatch_unknown_method_returns_method_not_found() {
        let err = dispatch("frobnicate", JsonValue::Null).unwrap_err();
        assert_eq!(err.0, -32601);
        assert!(err.1.contains("frobnicate"));
    }

    #[test]
    fn dispatch_tools_call_propagates_tools_errors() {
        // Missing `name` argument — tools::call returns -32602.
        let err = dispatch("tools/call", json!({})).unwrap_err();
        assert_eq!(err.0, -32602);
    }

    // ── error_str ─────────────────────────────────────────────────────

    #[test]
    fn error_str_renders_canonical_envelope() {
        let s = error_str(json!(42), -32000, "boom".into());
        let v: JsonValue = serde_json::from_str(&s).unwrap();
        assert_eq!(v["jsonrpc"].as_str().unwrap(), "2.0");
        assert_eq!(v["id"].as_i64().unwrap(), 42);
        assert_eq!(v["error"]["code"].as_i64().unwrap(), -32000);
        assert_eq!(v["error"]["message"].as_str().unwrap(), "boom");
    }

    #[test]
    fn error_str_handles_null_id() {
        let s = error_str(JsonValue::Null, -32700, "parse".into());
        let v: JsonValue = serde_json::from_str(&s).unwrap();
        assert!(v["id"].is_null());
    }
}
