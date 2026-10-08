// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Tests of the six tools, as functions and through the router.

use super::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Allocate a unique scratch path under the system temp dir so
/// parallel test runs don't collide.
fn temp_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("noyalib-mcp-{label}-{pid}-{id}.yml"))
}

fn write_temp(label: &str, contents: &str) -> PathBuf {
    let p = temp_path(label);
    fs::write(&p, contents).unwrap();
    p
}

/// Call a tool the way a request reaches it: JSON arguments,
/// deserialised into the tool's parameter type.
fn args<T: serde::de::DeserializeOwned>(v: JsonValue) -> Parameters<T> {
    Parameters(serde_json::from_value(v).expect("arguments"))
}

fn call(tool: &str, v: JsonValue) -> CallToolResult {
    // The fixtures are written under the system temp directory, so
    // that is the root the file tools are confined to here.
    call_on(&YamlServer::with_root(std::env::temp_dir()), tool, v)
}

/// One runtime for every test call: the tools run their work on
/// its blocking pool.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("runtime"))
}

fn call_on(server: &YamlServer, tool: &str, v: JsonValue) -> CallToolResult {
    runtime()
        .block_on(async {
            match tool {
                "noyalib_get" => server.noyalib_get(args(v)).await,
                "noyalib_set" => server.noyalib_set(args(v)).await,
                "noyalib_set_multidoc" => server.noyalib_set_multidoc(args(v)).await,
                "noyalib_parse" => server.noyalib_parse(args(v)).await,
                "noyalib_edit" => server.noyalib_edit(args(v)).await,
                "noyalib_validate" => server.noyalib_validate(args(v)).await,
                other => panic!("no such tool {other}"),
            }
        })
        .expect("a tool failure is a result, not a protocol error")
}

fn text_of(r: &CallToolResult) -> &str {
    r.content
        .first()
        .and_then(ContentBlock::as_text)
        .map(|t| t.text.as_str())
        .expect("text content")
}

fn is_error(r: &CallToolResult) -> bool {
    r.is_error == Some(true)
}

// ── the catalogue ──────────────────────────────────────────────

#[test]
fn every_tool_is_registered_with_schemas_and_annotations() {
    let tools = YamlServer::tool_router().list_all();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort_unstable();
    let mut want = TOOL_NAMES;
    want.sort_unstable();
    assert_eq!(names, want);
    for t in &tools {
        assert!(t.description.is_some(), "{} has no description", t.name);
        assert!(t.title.is_some(), "{} has no title", t.name);
        assert_eq!(
            t.input_schema.get("type").and_then(JsonValue::as_str),
            Some("object")
        );
        assert!(
            t.input_schema
                .get("required")
                .is_some_and(JsonValue::is_array)
        );
        assert!(t.output_schema.is_some(), "{} has no outputSchema", t.name);
        let a = t.annotations.as_ref().expect("annotations");
        assert!(a.read_only_hint.is_some(), "{} lacks readOnlyHint", t.name);
        // Every argument carries an example, so an auditor with no
        // file of its own can still make a well-formed call.
        let props = t.input_schema["properties"]
            .as_object()
            .expect("properties");
        for (name, schema) in props {
            assert!(
                schema.get("examples").is_some_and(JsonValue::is_array),
                "{}.{name} has no example: {schema}",
                t.name
            );
            assert!(schema.get("description").is_some(), "{}.{name}", t.name);
        }
    }
}

#[test]
fn input_schemas_keep_the_field_names_and_descriptions() {
    let tools = YamlServer::tool_router().list_all();
    let get = tools.iter().find(|t| t.name == "noyalib_get").expect("get");
    let props = &get.input_schema["properties"];
    assert_eq!(
        props["file"]["description"].as_str(),
        Some("Path to the YAML file on disk.")
    );
    assert_eq!(get.input_schema["required"], json!(["file", "path"]));
    let multidoc = tools
        .iter()
        .find(|t| t.name == "noyalib_set_multidoc")
        .expect("multidoc");
    assert_eq!(
        multidoc.input_schema["properties"]["doc_index"]["type"],
        "integer"
    );
    assert_eq!(
        multidoc.input_schema["required"],
        json!(["file", "doc_index", "path", "value"])
    );
    let validate = tools
        .iter()
        .find(|t| t.name == "noyalib_validate")
        .expect("validate");
    assert_eq!(validate.input_schema["required"], json!(["yaml"]));
    let write: Vec<&str> = tools
        .iter()
        .filter(|t| t.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(false))
        .map(|t| t.name.as_ref())
        .collect();
    assert_eq!(write, ["noyalib_set", "noyalib_set_multidoc"]);
}

// ── get ────────────────────────────────────────────────────────

#[test]
fn get_reads_the_source_slice() {
    let p = write_temp("call-get", "name: noyalib\n");
    let r = call(
        "noyalib_get",
        json!({ "file": p.to_str().unwrap(), "path": "name" }),
    );
    assert!(!is_error(&r));
    assert_eq!(text_of(&r), "noyalib");
    assert_eq!(r.structured_content, Some(json!({"value": "noyalib"})));
    let _ = fs::remove_file(&p);
}

#[test]
fn get_of_an_empty_value_is_the_empty_slice_not_an_error() {
    // yaml-test-suite 7W2P: `? a` / `c:` are present keys with an
    // implicit null value. They must not read as "path not found".
    let p = write_temp("call-get-empty", "a:\nb: 1\nc:\n");
    for key in ["a", "c"] {
        let r = call(
            "noyalib_get",
            json!({ "file": p.to_str().unwrap(), "path": key }),
        );
        assert!(!is_error(&r));
        assert_eq!(text_of(&r), "");
    }
    let r = call(
        "noyalib_get",
        json!({ "file": p.to_str().unwrap(), "path": "missing" }),
    );
    assert!(is_error(&r));
    assert!(text_of(&r).contains("path not found"), "{}", text_of(&r));
    assert!(r.structured_content.is_none());
    let _ = fs::remove_file(&p);
}

#[test]
fn get_reports_every_failure_as_text() {
    let err = get("/this/path/definitely/does/not/exist.yml", "k").unwrap_err();
    assert!(err.starts_with("read "), "{err}");
    let p = write_temp("get-parse", "key: [\n");
    let err = get(p.to_str().unwrap(), "key").unwrap_err();
    assert!(err.starts_with("parse "), "{err}");
    let _ = fs::remove_file(&p);
}

// ── set ────────────────────────────────────────────────────────

#[test]
fn set_rewrites_only_the_touched_span() {
    let p = write_temp("call-set", "# keep\nversion: 1 # inline\n");
    let r = call(
        "noyalib_set",
        json!({ "file": p.to_str().unwrap(), "path": "version", "value": "2" }),
    );
    assert!(!is_error(&r), "{r:?}");
    assert!(text_of(&r).contains("set version = 2"), "{}", text_of(&r));
    assert_eq!(
        r.structured_content,
        Some(json!({"file": p.to_str().unwrap(), "path": "version", "value": "2"}))
    );
    assert_eq!(
        fs::read_to_string(&p).unwrap(),
        "# keep\nversion: 2 # inline\n"
    );
    let _ = fs::remove_file(&p);
}

#[test]
fn set_reports_every_failure_and_leaves_the_file_alone() {
    let err = set("/this/path/does/not/exist.yml", "k", "v").unwrap_err();
    assert!(err.starts_with("read "), "{err}");

    let p = write_temp("set-parse", "k: [\n");
    let err = set(p.to_str().unwrap(), "k", "v").unwrap_err();
    assert!(err.starts_with("parse "), "{err}");
    assert_eq!(fs::read_to_string(&p).unwrap(), "k: [\n");
    let _ = fs::remove_file(&p);

    let p = write_temp("set-bad-path", "a: 1\n");
    let err = set(p.to_str().unwrap(), "missing.path", "v").unwrap_err();
    assert!(err.starts_with("set missing.path = v"), "{err}");
    assert_eq!(fs::read_to_string(&p).unwrap(), "a: 1\n");
    let _ = fs::remove_file(&p);
}

// ── set_multidoc ───────────────────────────────────────────────

#[test]
fn set_multidoc_changes_one_document_only() {
    let p = write_temp("call-set-multidoc", "name: first\n---\nname: second\n");
    let r = call(
        "noyalib_set_multidoc",
        json!({
            "file": p.to_str().unwrap(),
            "doc_index": 1,
            "path": "name",
            "value": "changed"
        }),
    );
    assert!(!is_error(&r), "{r:?}");
    assert!(text_of(&r).contains("document 1"), "{}", text_of(&r));
    assert_eq!(
        r.structured_content
            .as_ref()
            .and_then(|s| s.get("doc_index")),
        Some(&json!(1))
    );
    assert_eq!(
        fs::read_to_string(&p).unwrap(),
        "name: first\n---\nname: changed\n"
    );
    let _ = fs::remove_file(&p);
}

#[test]
fn set_multidoc_reports_every_failure_and_leaves_the_file_alone() {
    let err = set_multidoc("/this/path/does/not/exist.yml", 0, "a", "1").unwrap_err();
    assert!(err.starts_with("read "), "{err}");

    let p = write_temp("md-oob", "a: 1\n---\nb: 2\n");
    let err = set_multidoc(p.to_str().unwrap(), 9, "b", "3").unwrap_err();
    assert!(err.contains("out of range"), "{err}");
    assert!(err.contains("2 document(s)"), "{err}");
    let err = set_multidoc(p.to_str().unwrap(), 0, "missing.deep", "1").unwrap_err();
    assert!(err.starts_with("set missing.deep = 1"), "{err}");
    assert_eq!(fs::read_to_string(&p).unwrap(), "a: 1\n---\nb: 2\n");
    let _ = fs::remove_file(&p);

    let p = write_temp("md-parse", "a: [\n");
    let err = set_multidoc(p.to_str().unwrap(), 0, "a", "1").unwrap_err();
    assert!(err.starts_with("parse "), "{err}");
    let _ = fs::remove_file(&p);
}

// ── parse ──────────────────────────────────────────────────────

#[test]
fn parse_is_stateless_and_returns_the_json_model() {
    let r = call(
        "noyalib_parse",
        json!({ "yaml": "a: 0x2A\nb: !custom x\nc:\n" }),
    );
    assert!(!is_error(&r));
    let parsed: JsonValue = serde_json::from_str(text_of(&r)).unwrap();
    assert_eq!(parsed, json!({"a": 42, "b": "x", "c": null}));
    assert_eq!(
        r.structured_content,
        Some(json!({"documents": [{"a": 42, "b": "x", "c": null}]}))
    );
    let bad = call("noyalib_parse", json!({ "yaml": "a: [\n" }));
    assert!(is_error(&bad));
    assert!(text_of(&bad).starts_with("parse: "), "{}", text_of(&bad));
}

#[test]
fn parse_returns_an_array_for_a_stream() {
    let r = call("noyalib_parse", json!({ "yaml": "--- 1\n--- 2\n" }));
    let parsed: JsonValue = serde_json::from_str(text_of(&r)).unwrap();
    assert_eq!(parsed, json!([1, 2]));
    assert_eq!(r.structured_content, Some(json!({"documents": [1, 2]})));
}

// ── edit ───────────────────────────────────────────────────────

#[test]
fn edit_returns_the_whole_text_with_one_span_changed() {
    let r = call(
        "noyalib_edit",
        json!({ "yaml": "# keep\nversion: 0.0.34 # inline\nname: x\n", "path": "version", "value": "0.0.35" }),
    );
    assert!(!is_error(&r));
    assert_eq!(text_of(&r), "# keep\nversion: 0.0.35 # inline\nname: x\n");
    assert_eq!(
        r.structured_content,
        Some(json!({"yaml": "# keep\nversion: 0.0.35 # inline\nname: x\n"}))
    );
}

#[test]
fn edit_reports_every_failure_as_text() {
    let r = call(
        "noyalib_edit",
        json!({ "yaml": "a: [\n", "path": "a", "value": "1" }),
    );
    assert!(is_error(&r));
    assert!(text_of(&r).starts_with("parse: "), "{}", text_of(&r));
    let r = call(
        "noyalib_edit",
        json!({ "yaml": "a: 1\n", "path": "missing.key", "value": "1" }),
    );
    assert!(is_error(&r));
    assert!(
        text_of(&r).starts_with("set missing.key = 1"),
        "{}",
        text_of(&r)
    );
}

// ── validate ───────────────────────────────────────────────────

#[test]
fn validate_reports_parse_errors_and_schema_violations() {
    let r = call("noyalib_validate", json!({ "yaml": "a: [\n" }));
    assert!(
        is_error(&r),
        "an invalid document is a failure the model must see"
    );
    let v: JsonValue = serde_json::from_str(text_of(&r)).unwrap();
    assert_eq!(v["valid"], false);
    assert!(v["error"].as_str().unwrap().len() > 3);
    assert!(v["line"].is_u64());
    // A verdict is a complete result even when it is a failure.
    assert_eq!(r.structured_content, Some(v));

    let schema = r#"{"type":"object","properties":{"port":{"type":"integer","maximum":65535}},"required":["port"]}"#;
    let r = call(
        "noyalib_validate",
        json!({ "yaml": "port: 70000\n", "schema": schema }),
    );
    assert!(is_error(&r));
    let v: JsonValue = serde_json::from_str(text_of(&r)).unwrap();
    assert_eq!(v["valid"], false);
    assert!(v["error"].is_null());
    let violations = v["violations"].as_array().unwrap();
    assert!(!violations.is_empty());
    assert!(violations[0]["path"].as_str().unwrap().contains("port"));
    assert!(violations[0]["keyword"].is_string());

    let r = call(
        "noyalib_validate",
        json!({ "yaml": "port: 8080\n", "schema": schema }),
    );
    assert!(!is_error(&r), "{r:?}");
    assert_eq!(
        r.structured_content,
        Some(json!({"valid": true, "violations": []}))
    );

    let r = call("noyalib_validate", json!({ "yaml": "a: 1\n" }));
    assert!(!is_error(&r));
    assert_eq!(text_of(&r), r#"{"valid":true,"violations":[]}"#);
}

#[test]
fn validate_refuses_a_schema_it_cannot_use() {
    let r = call(
        "noyalib_validate",
        json!({ "yaml": "a: 1\n", "schema": "{not json" }),
    );
    assert!(is_error(&r));
    assert!(
        text_of(&r).starts_with("schema is not JSON"),
        "{}",
        text_of(&r)
    );
    assert!(r.structured_content.is_none());
    let r = call(
        "noyalib_validate",
        json!({ "yaml": "a: 1\n", "schema": "{\"type\": 12}" }),
    );
    assert!(is_error(&r));
    assert!(text_of(&r).starts_with("schema: "), "{}", text_of(&r));
}

#[test]
fn outputs_print_what_the_model_reads() {
    let out = SetOutput {
        file: "f.yml".into(),
        path: "a".into(),
        value: "1".into(),
    };
    assert_eq!(
        out.to_string(),
        "set a = 1 in f.yml (lossless: comments and formatting preserved)"
    );
    let out = ParseOutput {
        documents: vec![json!({"a": 1})],
    };
    assert_eq!(out.to_string(), "{\n  \"a\": 1\n}");
    assert_eq!(internal("boom"), "internal: boom");
}

// ── root confinement ─────────────────────────────────────────────

#[test]
fn a_file_outside_the_root_is_refused_before_it_is_read() {
    let inside = std::env::temp_dir().join(format!("noyalib-mcp-root-{}", std::process::id()));
    fs::create_dir_all(&inside).unwrap();
    let outside = write_temp("outside", "a: 1\n");
    let server = YamlServer::with_root(inside.clone());
    let r = call_on(
        &server,
        "noyalib_get",
        json!({"file": outside.to_str().unwrap(), "path": "a"}),
    );
    assert_eq!(r.is_error, Some(true));
    let msg = text_of(&r);
    assert!(msg.contains("outside the server root"), "{msg}");
    assert!(msg.contains("--root"), "{msg}");
    let _ = fs::remove_dir_all(inside);
    let _ = fs::remove_file(outside);
}

#[test]
fn a_relative_path_resolves_against_the_root() {
    let root = std::env::temp_dir().join(format!("noyalib-mcp-rel-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("c.yml"), "k: v\n").unwrap();
    let server = YamlServer::with_root(root.clone());
    let r = call_on(
        &server,
        "noyalib_get",
        json!({"file": "c.yml", "path": "k"}),
    );
    assert_ne!(r.is_error, Some(true), "{}", text_of(&r));
    assert!(text_of(&r).contains('v'));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn a_symlink_that_escapes_the_root_is_refused() {
    let root = std::env::temp_dir().join(format!("noyalib-mcp-sym-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let target = write_temp("symtarget", "a: 1\n");
    std::os::unix::fs::symlink(&target, root.join("link.yml")).unwrap();
    let server = YamlServer::with_root(root.clone());
    let r = call_on(
        &server,
        "noyalib_set",
        json!({"file": "link.yml", "path": "a", "value": "2"}),
    );
    assert_eq!(r.is_error, Some(true));
    assert!(
        text_of(&r).contains("outside the server root"),
        "{}",
        text_of(&r)
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "a: 1\n",
        "the target must be untouched"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(target);
}

// ── parse profile ────────────────────────────────────────────────

#[test]
fn parse_rejects_a_duplicate_key_by_default() {
    let err = parse("a: 1\na: 2\n").unwrap_err();
    assert!(err.to_lowercase().contains("duplicate"), "{err}");
}

#[test]
fn the_standard_profile_keeps_last_wins() {
    let out = parse_with_profile("a: 1\na: 2\n", ParseProfile::Standard).unwrap();
    assert_eq!(out.documents, vec![json!({"a": 2})]);
}

#[test]
fn validate_reports_a_duplicate_key_as_invalid_by_default() {
    let out = validate("a: 1\na: 2\n", None).unwrap();
    assert!(!out.valid);
    assert!(out.error.unwrap().to_lowercase().contains("duplicate"));
    assert!(
        validate_with_profile("a: 1\na: 2\n", None, ParseProfile::Standard)
            .unwrap()
            .valid
    );
}

#[test]
fn profile_names_round_trip() {
    assert_eq!(
        ParseProfile::from_name("strict"),
        Some(ParseProfile::Strict)
    );
    assert_eq!(
        ParseProfile::from_name("standard"),
        Some(ParseProfile::Standard)
    );
    assert_eq!(ParseProfile::from_name("lax"), None);
    assert_eq!(ParseProfile::default(), ParseProfile::Strict);
}

// File safety, request limits and the profile on the CST tools, in
// their own file to hold this one to size.
mod safety;
