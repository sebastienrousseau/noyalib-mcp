// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The resources: `resources/list`, `resources/templates/list` and
//! `resources/read`.
//!
//! Resources are read-only reference context an agent can pin without
//! a tool call: the failure taxonomy, the tool reference, and one
//! descriptor per tool. None of them touch the filesystem or accept a
//! caller-supplied path.

use rmcp::model::{ErrorData, Resource, ResourceContents, ResourceTemplate, Tool};
use serde_json::json;

/// The URI of the failure taxonomy.
pub const ERROR_CODES_URI: &str = "noyalib://error-codes";
/// The URI of the tool reference.
pub const TOOLS_URI: &str = "noyalib://tools";
/// The prefix of a single tool's descriptor.
pub const TOOL_URI_PREFIX: &str = "noyalib://tool/";
/// The template every single-tool descriptor matches.
pub const TOOL_URI_TEMPLATE: &str = "noyalib://tool/{name}";

const JSON: &str = "application/json";

/// The static resources, for `resources/list`.
#[must_use]
pub fn list() -> Vec<Resource> {
    vec![
        Resource::new(ERROR_CODES_URI, "error-codes")
            .with_title("noyalib-mcp failure taxonomy")
            .with_description(
                "How this server reports failure: the tool failures that \
                 arrive as isError results, and the JSON-RPC error codes.",
            )
            .with_mime_type(JSON),
        Resource::new(TOOLS_URI, "tools")
            .with_title("noyalib-mcp tool reference")
            .with_description("The tool descriptors this server exposes (mirrors tools/list).")
            .with_mime_type(JSON),
    ]
}

/// The templated resources, for `resources/templates/list`.
#[must_use]
pub fn templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate::new(TOOL_URI_TEMPLATE, "tool")
            .with_title("noyalib-mcp tool descriptor")
            .with_description(
                "The descriptor for a single tool by name (noyalib_get, \
                 noyalib_set, noyalib_set_multidoc, noyalib_parse, \
                 noyalib_edit or noyalib_validate).",
            )
            .with_mime_type(JSON),
    ]
}

/// Read the resource at `uri`, given the tools the server advertises.
///
/// # Errors
///
/// No resource has that URI: `-32002`, resource not found.
pub fn read(uri: &str, tools: &[Tool]) -> Result<ResourceContents, ErrorData> {
    let text = if uri == ERROR_CODES_URI {
        error_codes_json()
    } else if uri == TOOLS_URI {
        json!({ "tools": tools }).to_string()
    } else {
        let name = uri.strip_prefix(TOOL_URI_PREFIX).unwrap_or_default();
        let tool = tools
            .iter()
            .find(|t| !name.is_empty() && t.name == name)
            .ok_or_else(|| {
                ErrorData::resource_not_found(
                    format!("resource not found: {uri}"),
                    Some(json!({ "uri": uri })),
                )
            })?;
        serde_json::to_string(tool).map_err(|e| ErrorData::internal_error(e.to_string(), None))?
    };
    Ok(ResourceContents::text(text, uri).with_mime_type(JSON))
}

/// The failure taxonomy as a JSON object string.
///
/// A tool that ran and could not do the job answers with a
/// *successful* JSON-RPC response carrying `isError: true` and text the
/// model can act on; the text starts with what failed. A request the
/// protocol rejects is a JSON-RPC error, which the client handles.
fn error_codes_json() -> String {
    json!({
        "toolFailures": {
            "read <file>": "the file could not be read",
            "write <file>": "the file could not be written",
            "parse <file>": "the file is not YAML the library accepts",
            "parse": "the request's YAML text does not parse",
            "path not found in <file>": "no value at the dotted/indexed path",
            "set <path> = <value>": "the fragment cannot be applied at the path (unknown path, or a fragment that does not parse there); the file is unchanged",
            "doc_index <n> out of range": "the stream has fewer documents; the file is unchanged",
            "schema is not JSON": "the schema argument is not JSON text",
            "schema": "the schema is not a JSON Schema the library accepts",
            "Unknown tool": "no tool of that name; the message lists the tools that exist",
            "invalid arguments": "a required argument is missing or mistyped; the message names the field"
        },
        "jsonRpc": {
            "-32002": "resource not found",
            "-32020": "a mirrored routing header disagrees with the body (2026-07-28)",
            "-32022": "unsupported protocol version",
            "-32600": "invalid request",
            "-32601": "method not found",
            "-32602": "invalid parameters (a prompt that does not exist)"
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::YamlServer;
    use serde_json::Value;

    fn text_of(c: &ResourceContents) -> (&str, Option<&str>, &str) {
        match c {
            ResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } => (uri, mime_type.as_deref(), text),
            _ => panic!("not text"),
        }
    }

    #[test]
    fn list_names_the_static_resources() {
        let r = list();
        let uris: Vec<&str> = r.iter().map(|r| r.uri.as_str()).collect();
        assert_eq!(uris, [ERROR_CODES_URI, TOOLS_URI]);
        for r in &r {
            assert_eq!(r.mime_type.as_deref(), Some(JSON));
            assert!(r.title.is_some() && r.description.is_some(), "{}", r.uri);
        }
    }

    #[test]
    fn templates_lists_the_tool_template() {
        let t = templates();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].uri_template, TOOL_URI_TEMPLATE);
        assert_eq!(t[0].mime_type.as_deref(), Some(JSON));
    }

    #[test]
    fn read_returns_the_failure_taxonomy() {
        let c = read(ERROR_CODES_URI, &[]).unwrap();
        let (uri, mime, text) = text_of(&c);
        assert_eq!(uri, ERROR_CODES_URI);
        assert_eq!(mime, Some(JSON));
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert!(parsed["jsonRpc"]["-32601"].is_string());
        assert!(parsed["toolFailures"]["Unknown tool"].is_string());
    }

    #[test]
    fn read_returns_the_tool_reference_and_single_descriptors() {
        let tools = YamlServer::tool_router().list_all();
        let c = read(TOOLS_URI, &tools).unwrap();
        let parsed: Value = serde_json::from_str(text_of(&c).2).unwrap();
        // Mirrors the live tool registry (count-agnostic so adding a tool
        // does not break this resource test).
        assert_eq!(parsed["tools"].as_array().unwrap().len(), tools.len());
        assert!(parsed["tools"][0]["inputSchema"].is_object());

        let c = read("noyalib://tool/noyalib_get", &tools).unwrap();
        let parsed: Value = serde_json::from_str(text_of(&c).2).unwrap();
        assert_eq!(parsed["name"], "noyalib_get");
        assert!(parsed["outputSchema"].is_object());
    }

    #[test]
    fn read_refuses_what_does_not_exist() {
        let tools = YamlServer::tool_router().list_all();
        for uri in [
            "noyalib://tool/frobnicate",
            "noyalib://tool/",
            "noyalib://mystery",
            "",
        ] {
            let err = read(uri, &tools).unwrap_err();
            assert_eq!(
                err.code,
                rmcp::model::ErrorCode::RESOURCE_NOT_FOUND,
                "{uri}"
            );
            assert!(err.message.contains(uri), "{uri}: {}", err.message);
        }
    }
}
