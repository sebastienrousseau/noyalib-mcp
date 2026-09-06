// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Tool registry for the MCP server.
//!
//! Each entry in [`descriptors`] is the JSON Schema that a client
//! sees from `tools/list`; [`call`] is the dispatch entry point for
//! `tools/call`. Tools delegate the actual YAML work to noyalib's
//! `cst::Document` so edits round-trip with comments, indentation,
//! and sibling entries preserved byte-for-byte.

use noyalib::cst::{parse_document, parse_stream};
use serde_json::{Value as JsonValue, json};
use std::fs;

/// Descriptors returned to MCP clients via `tools/list`.
pub fn descriptors() -> Vec<JsonValue> {
    vec![
        json!({
            "name": "noyalib_get",
            "title": "Read a YAML value (lossless)",
            // Reads a caller-supplied YAML file without modifying it:
            // read-only, idempotent, never destructive, and open-world
            // (it touches the local filesystem). These MCP annotations let
            // clients and the Glama quality grader reason about safety and
            // auto-approval without executing the tool.
            "annotations": {
                "title": "Read a YAML value (lossless)",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": true
            },
            "description": "Read the YAML value at a dotted/indexed path \
                in the given file and return the source slice exactly — no \
                re-quoting, no canonicalisation, comments and formatting \
                preserved. Use this to inspect a value before changing it; \
                use `noyalib_set` to write a value back losslessly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "Path to the YAML file on disk."
                    },
                    "path": {
                        "type": "string",
                        "description": "Dotted/indexed path into the YAML, \
                            e.g. `server.host` or `items[0].name`."
                    }
                },
                "required": ["file", "path"]
            }
        }),
        json!({
            "name": "noyalib_set",
            "title": "Write a YAML value (lossless)",
            // Overwrites the value at a path in a caller-supplied file on
            // disk: NOT read-only, and destructive (it replaces existing
            // content in place). Re-running with the same arguments yields
            // the same file state, so it is idempotent; it touches the
            // filesystem, so it is open-world.
            "annotations": {
                "title": "Write a YAML value (lossless)",
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": true
            },
            "description": "Set the YAML value at a dotted/indexed path in \
                the given file, rewriting only the touched span so every \
                comment, blank line, and sibling entry is preserved \
                byte-for-byte (written atomically). Use this for \
                Renovate-style version bumps and config patches; use \
                `noyalib_get` first when you need to read the current \
                value. On a parse error the document is left unchanged.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "Path to the YAML file on disk."
                    },
                    "path": {
                        "type": "string",
                        "description": "Dotted/indexed path into the YAML."
                    },
                    "value": {
                        "type": "string",
                        "description": "Replacement value as a YAML \
                            fragment (e.g. `0.0.2`, `\\\"hello\\\"`, \
                            `[1, 2, 3]`). Must parse in the target \
                            position; the document is left unchanged on \
                            parse error."
                    }
                },
                "required": ["file", "path", "value"]
            }
        }),
        json!({
            "name": "noyalib_set_multidoc",
            "title": "Write a YAML value in one document of a multi-doc stream (lossless)",
            // Same write semantics as noyalib_set, but targets one
            // document of a `---`-separated multi-document YAML stream by
            // index: not read-only, destructive (replaces content in
            // place), idempotent, and open-world (touches the filesystem).
            "annotations": {
                "title": "Write a YAML value in one document of a multi-doc stream (lossless)",
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": true
            },
            "description": "Set the YAML value at a dotted/indexed path within \
                a single document of a multi-document (`---`-separated) YAML \
                stream, selected by zero-based document index. Only the touched \
                span of that one document is rewritten; every other document, \
                comment, blank line and separator is preserved byte-for-byte \
                (written atomically). Use `noyalib_set` for a single-document \
                file. On a parse error or out-of-range index the file is left \
                unchanged.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {
                        "type": "string",
                        "description": "Path to the multi-document YAML file on disk."
                    },
                    "doc_index": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Zero-based index of the document within \
                            the `---`-separated stream to modify."
                    },
                    "path": {
                        "type": "string",
                        "description": "Dotted/indexed path into the selected document."
                    },
                    "value": {
                        "type": "string",
                        "description": "Replacement value as a YAML fragment. Must \
                            parse in the target position; the file is left \
                            unchanged on parse error."
                    }
                },
                "required": ["file", "doc_index", "path", "value"]
            }
        }),
        json!({
            "name": "noyalib_parse",
            "title": "Parse YAML text into JSON (stateless)",
            // Content-in-request: nothing on disk is read or written. Pure,
            // read-only, idempotent, closed-world.
            "annotations": {
                "title": "Parse YAML text into JSON (stateless)",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            },
            "description": "Parse the YAML text given in the request and return \
                its JSON data model (custom tags stripped, the projection the \
                official YAML test suite expects). Multi-document streams return \
                a JSON array with one element per document. Refuses hostile \
                input (nesting, alias expansion, size) with the same limits as \
                the library. Nothing is read from or written to disk.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "yaml": { "type": "string", "description": "The YAML text to parse." }
                },
                "required": ["yaml"]
            }
        }),
        json!({
            "name": "noyalib_edit",
            "title": "Edit a value in YAML text, losslessly (stateless)",
            "annotations": {
                "title": "Edit a value in YAML text, losslessly (stateless)",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            },
            "description": "Set the value at a dotted/indexed path in the YAML text \
                given in the request and return the whole edited text. Only the \
                touched span changes; every comment, blank line and quote style \
                elsewhere is preserved byte-for-byte. Nothing on disk is touched: \
                the caller decides where the result goes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "yaml": { "type": "string", "description": "The YAML text to edit." },
                    "path": { "type": "string", "description": "Dotted/indexed path, e.g. `server.port` or `items[0].name`." },
                    "value": { "type": "string", "description": "Replacement value as a YAML fragment." }
                },
                "required": ["yaml", "path", "value"]
            }
        }),
        json!({
            "name": "noyalib_validate",
            "title": "Validate YAML text, optionally against a JSON Schema (stateless)",
            "annotations": {
                "title": "Validate YAML text, optionally against a JSON Schema (stateless)",
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            },
            "description": "Check that the YAML text parses under the library's \
                limits, and when a JSON Schema (as JSON text) is given, that the \
                document satisfies it. Returns `valid` with an empty list, or the \
                parse error with its line and column, or every schema violation \
                with its RFC 6901 path. Nothing on disk is touched.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "yaml": { "type": "string", "description": "The YAML text to validate." },
                    "schema": { "type": "string", "description": "A JSON Schema, as JSON text (optional)." }
                },
                "required": ["yaml"]
            }
        }),
    ]
}

/// `tools/call` dispatcher. Returns the JSON-RPC `result` payload on
/// success, or `(code, message)` for an error envelope.
pub fn call(params: JsonValue) -> Result<JsonValue, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "missing field: name".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or(JsonValue::Null);

    match name {
        "noyalib_get" => tool_get(&args),
        "noyalib_set" => tool_set(&args),
        "noyalib_set_multidoc" => tool_set_multidoc(&args),
        "noyalib_parse" => tool_parse(&args),
        "noyalib_edit" => tool_edit(&args),
        "noyalib_validate" => tool_validate(&args),
        _ => Err((-32601, format!("unknown tool: {name}"))),
    }
}

/// Wrap a tool result string into the MCP `tools/call` reply shape.
fn ok_text(text: String) -> JsonValue {
    json!({
        "content": [
            { "type": "text", "text": text }
        ]
    })
}

fn tool_get(args: &JsonValue) -> Result<JsonValue, (i32, String)> {
    let file = arg_str(args, "file")?;
    let path = arg_str(args, "path")?;
    let src = fs::read_to_string(file).map_err(|e| (-32000, format!("read {file}: {e}")))?;
    let doc = parse_document(&src).map_err(|e| (-32001, format!("parse {file}: {e}")))?;
    match doc.get(path) {
        Some(value) => Ok(ok_text(value.to_string())),
        // `get` yields `None` for an implicit null (`key:` with no
        // value) as well as for a missing path; the key span tells
        // the two apart so an empty value reads as its (empty) source
        // slice instead of a spurious "not found".
        None if doc.key_span(path).is_some() => Ok(ok_text(String::new())),
        None => Err((-32002, format!("path not found in {file}: {path}"))),
    }
}

fn tool_set(args: &JsonValue) -> Result<JsonValue, (i32, String)> {
    let file = arg_str(args, "file")?;
    let path = arg_str(args, "path")?;
    let value = arg_str(args, "value")?;
    let src = fs::read_to_string(file).map_err(|e| (-32000, format!("read {file}: {e}")))?;
    let mut doc = parse_document(&src).map_err(|e| (-32001, format!("parse {file}: {e}")))?;
    doc.set(path, value)
        .map_err(|e| (-32003, format!("set {path} = {value}: {e}")))?;
    write_atomic(file, doc.to_string().as_bytes())
        .map_err(|e| (-32000, format!("write {file}: {e}")))?;
    Ok(ok_text(format!(
        "set {path} = {value} in {file} (lossless: comments and formatting preserved)"
    )))
}

fn tool_set_multidoc(args: &JsonValue) -> Result<JsonValue, (i32, String)> {
    let file = arg_str(args, "file")?;
    let doc_index = args
        .get("doc_index")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| (-32602, "missing integer argument: doc_index".to_string()))?
        as usize;
    let path = arg_str(args, "path")?;
    let value = arg_str(args, "value")?;
    let src = fs::read_to_string(file).map_err(|e| (-32000, format!("read {file}: {e}")))?;
    // parse_stream keeps each `---`-delimited document as its own
    // lossless Document, retaining its separator; concatenating their
    // rendered forms reproduces the stream byte-for-byte, so editing one
    // document leaves every other document untouched.
    let mut docs = parse_stream(&src).map_err(|e| (-32001, format!("parse {file}: {e}")))?;
    if doc_index >= docs.len() {
        return Err((
            -32602,
            format!(
                "doc_index {doc_index} out of range: stream has {} document(s)",
                docs.len()
            ),
        ));
    }
    docs[doc_index]
        .set(path, value)
        .map_err(|e| (-32003, format!("set {path} = {value}: {e}")))?;
    let out: String = docs.iter().map(ToString::to_string).collect();
    write_atomic(file, out.as_bytes()).map_err(|e| (-32000, format!("write {file}: {e}")))?;
    Ok(ok_text(format!(
        "set {path} = {value} in document {doc_index} of {file} \
         (lossless: other documents, comments and formatting preserved)"
    )))
}

/// Write `bytes` to `file` atomically: write to a sibling temp
/// file, fsync it, then `rename` over the target. The rename is
/// atomic on POSIX and `MoveFileExW(MOVEFILE_REPLACE_EXISTING |
/// MOVEFILE_WRITE_THROUGH)` semantics on Windows, so concurrent
/// readers always see either the old or the new contents — never
/// a half-written truncation. The fsync also closes a Windows
/// race where `fs::write` returned before the kernel page cache
/// flushed, leaving a freshly-spawned reader to observe the old
/// bytes.
/// Stateless: parse the request's YAML and return its JSON data model.
fn tool_parse(args: &JsonValue) -> Result<JsonValue, (i32, String)> {
    let yaml = arg_str(args, "yaml")?;
    let docs = noyalib::load_all_as::<noyalib::Value>(yaml)
        .map_err(|e| (-32001, format!("parse: {e}")))?;
    let json: Vec<serde_json::Value> = docs
        .into_iter()
        .map(|d| serde_json::to_value(d.untag()).map_err(|e| (-32001, format!("json: {e}"))))
        .collect::<Result<_, _>>()?;
    let out = match json.len() {
        1 => serde_json::to_string_pretty(&json[0]),
        _ => serde_json::to_string_pretty(&json),
    }
    .map_err(|e| (-32001, format!("json: {e}")))?;
    Ok(ok_text(out))
}

/// Stateless: edit one value in the request's YAML and return the text.
fn tool_edit(args: &JsonValue) -> Result<JsonValue, (i32, String)> {
    let yaml = arg_str(args, "yaml")?;
    let path = arg_str(args, "path")?;
    let value = arg_str(args, "value")?;
    let mut doc = parse_document(yaml).map_err(|e| (-32001, format!("parse: {e}")))?;
    doc.set(path, value)
        .map_err(|e| (-32003, format!("set {path} = {value}: {e}")))?;
    Ok(ok_text(doc.to_string()))
}

/// Stateless: parse-check the request's YAML, and validate it against a
/// JSON Schema when one is given.
fn tool_validate(args: &JsonValue) -> Result<JsonValue, (i32, String)> {
    let yaml = arg_str(args, "yaml")?;
    let value = match noyalib::from_str::<noyalib::Value>(yaml) {
        Ok(v) => v,
        Err(e) => {
            let (line, column) = e.location().map_or((0, 0), |l| (l.line(), l.column()));
            return Ok(ok_text(
                json!({ "valid": false, "error": e.to_string(), "line": line, "column": column })
                    .to_string(),
            ));
        }
    };
    let Some(schema_text) = args.get("schema").and_then(|v| v.as_str()) else {
        return Ok(ok_text(
            json!({ "valid": true, "violations": [] }).to_string(),
        ));
    };
    let schema: serde_json::Value = serde_json::from_str(schema_text)
        .map_err(|e| (-32602, format!("schema is not JSON: {e}")))?;
    let schema_value: noyalib::Value =
        serde_json::from_value(schema).map_err(|e| (-32602, format!("schema: {e}")))?;
    let compiled = noyalib::CompiledSchema::compile(&schema_value)
        .map_err(|e| (-32602, format!("schema: {e}")))?;
    let violations = compiled
        .iter_errors(&value)
        .map_err(|e| (-32001, format!("validate: {e}")))?;
    let list: Vec<JsonValue> = violations
        .iter()
        .map(|v| json!({ "path": v.instance_path, "keyword": v.keyword, "message": v.message }))
        .collect();
    Ok(ok_text(
        json!({ "valid": list.is_empty(), "violations": list }).to_string(),
    ))
}

fn write_atomic(file: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::path::Path;
    let target = Path::new(file);
    let parent = target.parent().unwrap_or(Path::new("."));
    let stem = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("noyalib-set");
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = parent.join(format!(".{stem}.{pid}.{nanos}.tmp"));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, target)
}

fn arg_str<'a>(args: &'a JsonValue, key: &str) -> Result<&'a str, (i32, String)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, format!("missing string argument: {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;
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

    // ── descriptors ────────────────────────────────────────────────

    #[test]
    fn descriptors_lists_all_tools_with_input_schemas() {
        let d = descriptors();
        assert_eq!(d.len(), 6);
        let names: Vec<&str> = d.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"noyalib_get"));
        assert!(names.contains(&"noyalib_set"));
        assert!(names.contains(&"noyalib_set_multidoc"));
        assert!(names.contains(&"noyalib_parse"));
        assert!(names.contains(&"noyalib_edit"));
        assert!(names.contains(&"noyalib_validate"));
        for tool in &d {
            assert!(tool["description"].is_string());
            assert_eq!(tool["inputSchema"]["type"].as_str(), Some("object"));
            assert!(tool["inputSchema"]["required"].is_array());
        }
    }

    // ── call dispatcher ────────────────────────────────────────────

    #[test]
    fn call_rejects_missing_name() {
        let err = call(json!({})).unwrap_err();
        assert_eq!(err.0, -32602);
        assert!(err.1.contains("name"));
    }

    #[test]
    fn call_rejects_unknown_tool() {
        let err = call(json!({"name": "frobnicate", "arguments": {}})).unwrap_err();
        assert_eq!(err.0, -32601);
        assert!(err.1.contains("frobnicate"));
    }

    #[test]
    fn call_routes_to_get() {
        let p = write_temp("call-get", "name: noyalib\n");
        let v = call(json!({
            "name": "noyalib_get",
            "arguments": { "file": p.to_str().unwrap(), "path": "name" }
        }))
        .unwrap();
        let text = v["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "noyalib");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn get_of_an_empty_value_is_the_empty_slice_not_an_error() {
        // yaml-test-suite 7W2P: `? a` / `c:` are present keys with an
        // implicit null value. They must not read as "path not found".
        let p = write_temp("call-get-empty", "a:\nb: 1\nc:\n");
        for key in ["a", "c"] {
            let v = call(json!({
                "name": "noyalib_get",
                "arguments": { "file": p.to_str().unwrap(), "path": key }
            }))
            .unwrap();
            assert_eq!(v["content"][0]["text"].as_str().unwrap(), "");
        }
        let err = call(json!({
            "name": "noyalib_get",
            "arguments": { "file": p.to_str().unwrap(), "path": "missing" }
        }))
        .unwrap_err();
        assert_eq!(err.0, -32002);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn parse_is_stateless_and_returns_the_json_model() {
        let v = call(json!({
            "name": "noyalib_parse",
            "arguments": { "yaml": "a: 0x2A\nb: !custom x\nc:\n" }
        }))
        .unwrap();
        let text = v["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["a"], 42);
        assert_eq!(parsed["b"], "x");
        assert!(parsed["c"].is_null());
        let err = call(json!({ "name": "noyalib_parse", "arguments": { "yaml": "a: [\n" } }))
            .unwrap_err();
        assert_eq!(err.0, -32001);
    }

    #[test]
    fn parse_returns_an_array_for_a_stream() {
        let v = call(json!({ "name": "noyalib_parse", "arguments": { "yaml": "--- 1\n--- 2\n" } }))
            .unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(parsed, json!([1, 2]));
    }

    #[test]
    fn edit_returns_the_whole_text_with_one_span_changed() {
        let v = call(json!({
            "name": "noyalib_edit",
            "arguments": { "yaml": "# keep\nversion: 0.0.34 # inline\nname: x\n", "path": "version", "value": "0.0.35" }
        }))
        .unwrap();
        assert_eq!(
            v["content"][0]["text"].as_str().unwrap(),
            "# keep\nversion: 0.0.35 # inline\nname: x\n"
        );
    }

    #[test]
    fn validate_reports_parse_errors_and_schema_violations() {
        let v =
            call(json!({ "name": "noyalib_validate", "arguments": { "yaml": "a: [\n" } })).unwrap();
        let r: serde_json::Value =
            serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(r["valid"], false);
        assert!(r["error"].as_str().unwrap().len() > 3);
        let schema = r#"{"type":"object","properties":{"port":{"type":"integer","maximum":65535}},"required":["port"]}"#;
        let v = call(json!({ "name": "noyalib_validate", "arguments": { "yaml": "port: 70000\n", "schema": schema } })).unwrap();
        let r: serde_json::Value =
            serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(r["valid"], false);
        assert!(!r["violations"].as_array().unwrap().is_empty());
        assert!(
            r["violations"][0]["path"]
                .as_str()
                .unwrap()
                .contains("port")
        );
        let v = call(json!({ "name": "noyalib_validate", "arguments": { "yaml": "port: 8080\n", "schema": schema } })).unwrap();
        let r: serde_json::Value =
            serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(r["valid"], true);
    }

    #[test]
    fn call_routes_to_set() {
        let p = write_temp("call-set", "version: 1\n");
        let v = call(json!({
            "name": "noyalib_set",
            "arguments": {
                "file": p.to_str().unwrap(),
                "path": "version",
                "value": "2"
            }
        }))
        .unwrap();
        assert!(
            v["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("set version")
        );
        let updated = fs::read_to_string(&p).unwrap();
        assert_eq!(updated, "version: 2\n");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn call_routes_to_set_multidoc() {
        let p = write_temp("call-set-multidoc", "name: first\n---\nname: second\n");
        let v = call(json!({
            "name": "noyalib_set_multidoc",
            "arguments": {
                "file": p.to_str().unwrap(),
                "doc_index": 1,
                "path": "name",
                "value": "changed"
            }
        }))
        .unwrap();
        assert!(
            v["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("document 1")
        );
        let updated = fs::read_to_string(&p).unwrap();
        // First document is preserved byte-for-byte; only the second changed.
        assert!(updated.contains("name: first"));
        assert!(updated.contains("name: changed"));
        assert!(!updated.contains("name: second"));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn set_multidoc_missing_doc_index_errors() {
        let p = write_temp("md-no-index", "a: 1\n");
        let err = tool_set_multidoc(&json!({
            "file": p.to_str().unwrap(),
            "path": "a",
            "value": "2"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32602);
        assert!(err.1.contains("doc_index"));
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn set_multidoc_index_out_of_range_errors() {
        let p = write_temp("md-oob", "a: 1\n---\nb: 2\n");
        let err = tool_set_multidoc(&json!({
            "file": p.to_str().unwrap(),
            "doc_index": 9,
            "path": "b",
            "value": "3"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32602);
        assert!(err.1.contains("out of range"));
        // File left unchanged.
        assert_eq!(fs::read_to_string(&p).unwrap(), "a: 1\n---\nb: 2\n");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn set_multidoc_unreadable_file_errors() {
        let err = tool_set_multidoc(&json!({
            "file": "/this/path/does/not/exist.yml",
            "doc_index": 0,
            "path": "a",
            "value": "1"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32000);
    }

    #[test]
    fn set_multidoc_unparseable_source_errors() {
        let p = write_temp("md-parse", "a: [\n");
        let err = tool_set_multidoc(&json!({
            "file": p.to_str().unwrap(),
            "doc_index": 0,
            "path": "a",
            "value": "1"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32001);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn set_multidoc_unknown_path_errors() {
        let p = write_temp("md-badpath", "a: 1\n---\nb: 2\n");
        let err = tool_set_multidoc(&json!({
            "file": p.to_str().unwrap(),
            "doc_index": 0,
            "path": "missing.deep",
            "value": "1"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32003);
        let _ = fs::remove_file(&p);
    }

    // ── tool_get error paths ───────────────────────────────────────

    #[test]
    fn tool_get_missing_file_arg_errors() {
        let err = tool_get(&json!({"path": "k"})).unwrap_err();
        assert_eq!(err.0, -32602);
    }

    #[test]
    fn tool_get_missing_path_arg_errors() {
        let err = tool_get(&json!({"file": "/tmp/x.yml"})).unwrap_err();
        assert_eq!(err.0, -32602);
    }

    #[test]
    fn tool_get_unreadable_file_errors() {
        let err = tool_get(&json!({
            "file": "/this/path/definitely/does/not/exist.yml",
            "path": "k"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32000);
    }

    #[test]
    fn tool_get_unparseable_yaml_errors() {
        let p = write_temp("get-parse", "key: [\n");
        let err = tool_get(&json!({
            "file": p.to_str().unwrap(),
            "path": "key"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32001);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn tool_get_path_not_found_errors() {
        let p = write_temp("get-missing", "a: 1\n");
        let err = tool_get(&json!({
            "file": p.to_str().unwrap(),
            "path": "missing"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32002);
        let _ = fs::remove_file(&p);
    }

    // ── tool_set error paths ───────────────────────────────────────

    #[test]
    fn tool_set_missing_args_errors() {
        let err = tool_set(&json!({})).unwrap_err();
        assert_eq!(err.0, -32602);
    }

    #[test]
    fn tool_set_unreadable_file_errors() {
        let err = tool_set(&json!({
            "file": "/this/path/does/not/exist.yml",
            "path": "k",
            "value": "v"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32000);
    }

    #[test]
    fn tool_set_unparseable_source_errors() {
        let p = write_temp("set-parse", "k: [\n");
        let err = tool_set(&json!({
            "file": p.to_str().unwrap(),
            "path": "k",
            "value": "v"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32001);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn tool_set_unknown_path_errors() {
        let p = write_temp("set-bad-path", "a: 1\n");
        let err = tool_set(&json!({
            "file": p.to_str().unwrap(),
            "path": "missing.path",
            "value": "v"
        }))
        .unwrap_err();
        assert_eq!(err.0, -32003);
        let _ = fs::remove_file(&p);
    }
}
