// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! A whole session through an in-memory pipe.
//!
//! `tests/protocol.rs` drives the real binary over stdio, which is the
//! honest end-to-end check but can only assert on lines of text. These
//! hold a session with the SDK's own client, so what is asserted is
//! what a client sees: the negotiated protocol, the catalogues, and
//! results with their structured half.

#![allow(missing_docs)]

use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, ContentBlock, GetPromptRequestParams, ProtocolVersion,
    ReadResourceRequestParams, ResourceContents,
};
use rmcp::service::{RoleClient, RunningService};
use serde_json::{Map, Value, json};

/// A client connected to a fresh server over a duplex pipe.
async fn session() -> RunningService<RoleClient, ()> {
    let (client_io, server_io) = tokio::io::duplex(1 << 16);
    // `serve` returns once the handshake is done, so the server must
    // already be waiting when the client starts talking.
    drop(tokio::spawn(async move {
        if let Ok(server) = noyalib_mcp::YamlServer::new().serve(server_io).await {
            let _ = server.waiting().await;
        }
    }));
    ().serve(client_io)
        .await
        .expect("client completes the handshake")
}

fn arguments(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => panic!("arguments must be an object"),
    }
}

fn text_of(content: &[ContentBlock]) -> &str {
    content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|t| t.text.as_str())
        .expect("text content")
}

fn tempfile(contents: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "noyalib-mcp-serve-{}-{}.yaml",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

#[tokio::test]
async fn the_handshake_negotiates_a_current_revision() {
    let client = session().await;
    let info = client.peer_info().expect("initialize result");
    let server_info = info.server_info.as_ref().expect("serverInfo");
    assert_eq!(server_info.name, "noyalib-mcp");
    assert_eq!(server_info.version, env!("CARGO_PKG_VERSION"));
    assert!(info.capabilities.tools.is_some(), "tools capability");
    assert!(info.capabilities.prompts.is_some(), "prompts capability");
    assert!(
        info.capabilities.resources.is_some(),
        "resources capability"
    );
    // The SDK client asks for its latest handshake revision; the server
    // must agree to it rather than fall back.
    assert_eq!(info.protocol_version, ProtocolVersion::LATEST);
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn the_catalogue_is_complete_and_annotated() {
    let client = session().await;
    let tools = client.list_all_tools().await.expect("tools/list");
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort_unstable();
    let mut want = noyalib_mcp::TOOL_NAMES;
    want.sort_unstable();
    assert_eq!(names, want);
    for tool in &tools {
        assert!(tool.description.is_some(), "{} undescribed", tool.name);
        assert!(
            tool.output_schema.is_some(),
            "{} no outputSchema",
            tool.name
        );
        let a = tool.annotations.as_ref().expect("annotations");
        assert!(a.read_only_hint.is_some(), "{} read-only", tool.name);
    }
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn a_call_returns_text_and_structured_content() {
    let client = session().await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("noyalib_edit").with_arguments(arguments(json!({
                "yaml": "# keep\nport: 8080 # inline\n",
                "path": "port",
                "value": "9090"
            }))),
        )
        .await
        .expect("tools/call");
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(text_of(&result.content), "# keep\nport: 9090 # inline\n");
    assert_eq!(
        result.structured_content,
        Some(json!({"yaml": "# keep\nport: 9090 # inline\n"}))
    );
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn a_tool_failure_is_a_result_the_model_can_read() {
    let client = session().await;
    let result = client
        .call_tool(
            CallToolRequestParams::new("noyalib_parse")
                .with_arguments(arguments(json!({"yaml": "a: [\n"}))),
        )
        .await
        .expect("a bad document is a result, not a protocol error");
    assert_eq!(result.is_error, Some(true));
    assert!(text_of(&result.content).starts_with("parse: "));
    assert!(result.structured_content.is_none());
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn protocol_mistakes_are_readable_results() {
    let client = session().await;
    // A tool the server does not have is a result the model can read,
    // naming the tools it does have.
    let unknown = client
        .call_tool(CallToolRequestParams::new("no_such_tool"))
        .await
        .expect("a result, not a protocol error");
    assert_eq!(unknown.is_error, Some(true));
    let text = text_of(&unknown.content);
    assert!(text.contains("no_such_tool"), "{text}");
    assert!(text.contains("noyalib_validate"), "{text}");

    // A required argument missing is a tool failure naming the field,
    // so the model can supply it.
    let missing = client
        .call_tool(
            CallToolRequestParams::new("noyalib_get")
                .with_arguments(arguments(json!({"file": "x.yaml"}))),
        )
        .await
        .expect("a result, not a protocol error");
    assert_eq!(missing.is_error, Some(true));
    assert!(text_of(&missing.content).contains("path"), "{missing:?}");
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn every_advertised_tool_is_callable() {
    let client = session().await;
    let file = tempfile("name: first\n");
    let multidoc_file = tempfile("name: first\n---\nname: second\n");
    let path = file.to_str().unwrap();
    let multidoc_path = multidoc_file.to_str().unwrap();
    let yaml = "server:\n  port: 8080\n";
    for (name, args) in [
        ("noyalib_get", json!({"file": path, "path": "name"})),
        (
            "noyalib_set",
            json!({"file": path, "path": "name", "value": "changed"}),
        ),
        (
            "noyalib_set_multidoc",
            json!({"file": multidoc_path, "doc_index": 1, "path": "name", "value": "third"}),
        ),
        ("noyalib_parse", json!({"yaml": yaml})),
        (
            "noyalib_edit",
            json!({"yaml": yaml, "path": "server.port", "value": "1"}),
        ),
        (
            "noyalib_validate",
            json!({"yaml": yaml, "schema": "{\"type\":\"object\"}"}),
        ),
    ] {
        let result = client
            .call_tool(CallToolRequestParams::new(name).with_arguments(arguments(args)))
            .await
            .unwrap_or_else(|e| panic!("{name} rejected: {e}"));
        assert_ne!(result.is_error, Some(true), "{name}: {result:?}");
        assert!(result.structured_content.is_some(), "{name} unstructured");
    }
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "name: changed\n");
    assert_eq!(
        std::fs::read_to_string(&multidoc_file).unwrap(),
        "name: first\n---\nname: third\n"
    );
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&multidoc_file);
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn the_prompt_is_listed_and_rendered() {
    let client = session().await;
    let prompts = client.list_all_prompts().await.expect("prompts/list");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "format_and_lint_yaml");
    let args = prompts[0].arguments.as_ref().expect("arguments");
    assert_eq!(args[0].name, "file");

    let mut params = GetPromptRequestParams::new("format_and_lint_yaml");
    params.arguments = Some(arguments(json!({"file": "deploy.yaml"})));
    let rendered = client.get_prompt(params).await.expect("prompts/get");
    assert_eq!(rendered.messages.len(), 1);
    assert!(text_of(std::slice::from_ref(&rendered.messages[0].content)).contains("`deploy.yaml`"));

    let missing = client
        .get_prompt(GetPromptRequestParams::new("no_such_prompt"))
        .await;
    assert!(missing.is_err(), "an unknown prompt is a protocol error");
    let _ = client.cancel().await.expect("clean close");
}

#[tokio::test]
async fn the_resources_are_listed_and_readable() {
    let client = session().await;
    let resources = client.list_all_resources().await.expect("resources/list");
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
    assert_eq!(uris, ["noyalib://error-codes", "noyalib://tools"]);
    let templates = client
        .list_all_resource_templates()
        .await
        .expect("resources/templates/list");
    assert_eq!(templates[0].uri_template, "noyalib://tool/{name}");

    for uri in [
        "noyalib://error-codes",
        "noyalib://tools",
        "noyalib://tool/noyalib_get",
    ] {
        let read = client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
            .unwrap_or_else(|e| panic!("{uri}: {e}"));
        let ResourceContents::TextResourceContents {
            text, mime_type, ..
        } = &read.contents[0]
        else {
            panic!("{uri}: not text");
        };
        assert_eq!(mime_type.as_deref(), Some("application/json"), "{uri}");
        let _: Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("{uri}: {e}"));
    }
    let missing = client
        .read_resource(ReadResourceRequestParams::new("noyalib://tool/nope"))
        .await;
    assert!(missing.is_err(), "an unknown resource is a protocol error");
    let _ = client.cancel().await.expect("clean close");
}
