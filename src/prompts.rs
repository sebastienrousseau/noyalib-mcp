// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Prompt registry for the MCP server.
//!
//! Each entry in [`descriptors`] is what a client sees from
//! `prompts/list`; [`get`] is the dispatch entry point for
//! `prompts/get`, returning the MCP prompt-message payload. Prompts
//! are pure text templates — they run no tools and touch no files;
//! they teach the host model the recommended `noyalib_get` /
//! `noyalib_set` workflow.

use serde_json::{Value as JsonValue, json};

/// Descriptors returned to MCP clients via `prompts/list`.
pub fn descriptors() -> Vec<JsonValue> {
    vec![json!({
        "name": "format_and_lint_yaml",
        "title": "Format and lint a YAML file (lossless)",
        "description": "Guided workflow for inspecting and losslessly \
            fixing a YAML file with noyalib_get and noyalib_set, preserving \
            comments and formatting.",
        "arguments": [
            {
                "name": "file",
                "description": "Path to the YAML file to review. Optional; \
                    omit for a general workflow.",
                "required": false
            }
        ]
    })]
}

/// `prompts/get` dispatcher. Returns the JSON-RPC `result` payload on
/// success, or `(code, message)` for an error envelope.
pub fn get(params: JsonValue) -> Result<JsonValue, (i32, String)> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (-32602, "missing field: name".to_string()))?;
    match name {
        "format_and_lint_yaml" => Ok(format_and_lint_yaml(&params)),
        _ => Err((-32602, format!("unknown prompt: {name}"))),
    }
}

/// Build the `format_and_lint_yaml` prompt messages, embedding the
/// caller-supplied `file` argument when present.
fn format_and_lint_yaml(params: &JsonValue) -> JsonValue {
    let file = params
        .get("arguments")
        .and_then(|a| a.get("file"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let target = if file.is_empty() {
        "the YAML file".to_string()
    } else {
        format!("`{file}`")
    };
    let text = format!(
        "Help me format and lint {target} without losing comments or \
         formatting. First call noyalib_get on the paths you want to inspect \
         to read the current values exactly as written. Propose a minimal set \
         of fixes (indentation, key ordering, quoting, obviously wrong \
         values). For each fix, call noyalib_set with the dotted/indexed path \
         and the replacement YAML fragment: it rewrites only the touched span, \
         so every comment, blank line and sibling entry is preserved \
         byte-for-byte. Re-read with noyalib_get to confirm each change."
    );
    json!({
        "description": "Guided lossless YAML format-and-lint workflow.",
        "messages": [
            { "role": "user", "content": { "type": "text", "text": text } }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_lists_the_prompt_with_arguments() {
        let d = descriptors();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0]["name"].as_str(), Some("format_and_lint_yaml"));
        assert!(d[0]["arguments"].is_array());
    }

    #[test]
    fn get_rejects_missing_name() {
        let err = get(json!({})).unwrap_err();
        assert_eq!(err.0, -32602);
        assert!(err.1.contains("name"));
    }

    #[test]
    fn get_rejects_unknown_prompt() {
        let err = get(json!({"name": "nope"})).unwrap_err();
        assert_eq!(err.0, -32602);
        assert!(err.1.contains("nope"));
    }

    #[test]
    fn get_without_file_uses_generic_target() {
        let v = get(json!({"name": "format_and_lint_yaml"})).unwrap();
        let text = v["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("the YAML file"));
        assert!(text.contains("noyalib_get"));
        assert!(text.contains("noyalib_set"));
        assert_eq!(v["messages"][0]["role"].as_str(), Some("user"));
    }

    #[test]
    fn get_with_file_embeds_the_path() {
        let v = get(json!({
            "name": "format_and_lint_yaml",
            "arguments": { "file": "config.yml" }
        }))
        .unwrap();
        let text = v["messages"][0]["content"]["text"].as_str().unwrap();
        assert!(text.contains("`config.yml`"));
    }
}
