// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Resource registry for the MCP server.
//!
//! [`descriptors`] backs `resources/list` (static resources) and
//! [`templates`] backs `resources/templates/list`; [`read`] is the
//! dispatch entry point for `resources/read`. Resources are read-only
//! reference context an agent can pin without a tool call: the error
//! code taxonomy, the tool reference, and per-tool descriptors. None
//! of them touch the filesystem or accept caller-supplied paths.

use crate::tools;
use serde_json::{Value as JsonValue, json};

/// Static resource descriptors returned via `resources/list`.
pub fn descriptors() -> Vec<JsonValue> {
    vec![
        json!({
            "uri": "noyalib://error-codes",
            "name": "error-codes",
            "title": "noyalib-mcp error code taxonomy",
            "description": "The JSON-RPC error codes this server returns, \
                each with its meaning.",
            "mimeType": "application/json"
        }),
        json!({
            "uri": "noyalib://tools",
            "name": "tools",
            "title": "noyalib-mcp tool reference",
            "description": "The tool descriptors this server exposes \
                (mirrors tools/list).",
            "mimeType": "application/json"
        }),
    ]
}

/// Templated resource descriptors returned via
/// `resources/templates/list`.
pub fn templates() -> Vec<JsonValue> {
    vec![json!({
        "uriTemplate": "noyalib://tool/{name}",
        "name": "tool",
        "title": "noyalib-mcp tool descriptor",
        "description": "The descriptor for a single tool by name \
            (noyalib_get or noyalib_set).",
        "mimeType": "application/json"
    })]
}

/// `resources/read` dispatcher. Returns the JSON-RPC `result` payload
/// on success, or `(code, message)` for an error envelope.
pub fn read(params: JsonValue) -> Result<JsonValue, (i32, String)> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "missing field: uri".to_string()))?;
    let text = match uri {
        "noyalib://error-codes" => error_codes_json(),
        "noyalib://tools" => json!({ "tools": tools::descriptors() }).to_string(),
        _ => tool_descriptor_text(uri)?,
    };
    Ok(json!({
        "contents": [
            { "uri": uri, "mimeType": "application/json", "text": text }
        ]
    }))
}

/// Resolve a templated `noyalib://tool/{name}` URI to that tool's
/// descriptor text, or an error for an unknown URI/tool name.
fn tool_descriptor_text(uri: &str) -> Result<String, (i32, String)> {
    let name = uri
        .strip_prefix("noyalib://tool/")
        .ok_or_else(|| (-32002, format!("resource not found: {uri}")))?;
    tools::descriptors()
        .into_iter()
        .find(|d| d["name"].as_str() == Some(name))
        .map(|d| d.to_string())
        .ok_or_else(|| (-32002, format!("resource not found: {uri}")))
}

/// The error code taxonomy serialised as a JSON object string.
fn error_codes_json() -> String {
    json!({
        "-32000": "file I/O error (read or write failed)",
        "-32001": "YAML parse error",
        "-32002": "path or resource not found",
        "-32003": "set failed (value could not be written at the path)",
        "-32600": "invalid request (jsonrpc field must be \"2.0\")",
        "-32601": "method not found",
        "-32602": "missing or invalid parameter",
        "-32700": "JSON parse error"
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_lists_static_resources() {
        let d = descriptors();
        let uris: Vec<&str> = d.iter().map(|r| r["uri"].as_str().unwrap()).collect();
        assert!(uris.contains(&"noyalib://error-codes"));
        assert!(uris.contains(&"noyalib://tools"));
        for r in &d {
            assert_eq!(r["mimeType"].as_str(), Some("application/json"));
        }
    }

    #[test]
    fn templates_lists_the_tool_template() {
        let t = templates();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0]["uriTemplate"].as_str(), Some("noyalib://tool/{name}"));
    }

    #[test]
    fn read_rejects_missing_uri() {
        let err = read(json!({})).unwrap_err();
        assert_eq!(err.0, -32602);
        assert!(err.1.contains("uri"));
    }

    #[test]
    fn read_returns_error_code_taxonomy() {
        let v = read(json!({"uri": "noyalib://error-codes"})).unwrap();
        let text = v["contents"][0]["text"].as_str().unwrap();
        let parsed: JsonValue = serde_json::from_str(text).unwrap();
        assert!(parsed["-32001"].is_string());
        assert_eq!(
            v["contents"][0]["uri"].as_str(),
            Some("noyalib://error-codes")
        );
    }

    #[test]
    fn read_returns_tool_reference() {
        let v = read(json!({"uri": "noyalib://tools"})).unwrap();
        let text = v["contents"][0]["text"].as_str().unwrap();
        let parsed: JsonValue = serde_json::from_str(text).unwrap();
        // Mirrors the live tool registry (count-agnostic so adding a tool
        // does not break this resource test).
        assert_eq!(
            parsed["tools"].as_array().unwrap().len(),
            tools::descriptors().len()
        );
    }

    #[test]
    fn read_returns_single_tool_descriptor() {
        let v = read(json!({"uri": "noyalib://tool/noyalib_get"})).unwrap();
        let text = v["contents"][0]["text"].as_str().unwrap();
        let parsed: JsonValue = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["name"].as_str(), Some("noyalib_get"));
    }

    #[test]
    fn read_rejects_unknown_tool_name() {
        let err = read(json!({"uri": "noyalib://tool/frobnicate"})).unwrap_err();
        assert_eq!(err.0, -32002);
    }

    #[test]
    fn read_rejects_unknown_uri() {
        let err = read(json!({"uri": "noyalib://mystery"})).unwrap_err();
        assert_eq!(err.0, -32002);
    }
}
