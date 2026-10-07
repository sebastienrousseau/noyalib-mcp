// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The six tools, as plain functions and as MCP tools.
//!
//! [`get`], [`set`], [`set_multidoc`], [`parse`], [`edit`] and
//! [`validate`] are the YAML work; nothing in them knows about MCP.
//! The `#[tool_router]` block at the bottom is what `tools/list`
//! advertises and `tools/call` reaches: each tool deserialises its
//! arguments, calls the function, and returns the answer as text for
//! the model and as `structuredContent` for the client. Every edit
//! goes through noyalib's `cst::Document`, so the untouched bytes of a
//! file -- comments, indentation, sibling entries -- survive.

use std::fmt;
#[cfg(test)]
use std::fs;
use std::path::Path;

use noyalib::cst::{parse_document_with_config, parse_stream_with_config};
use rmcp::handler::server::tool::schema_for_output;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData};
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::YamlServer;
use crate::fsio::{FileError, Kept, Located, RootDir};

// --- Outputs -------------------------------------------------------------

/// The value `noyalib_get` read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct GetOutput {
    /// The source slice at the path, exactly as written: no
    /// re-quoting, no canonicalisation. Empty for a key with no value.
    pub value: String,
}

impl fmt::Display for GetOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

/// What `noyalib_set` wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SetOutput {
    /// The file that was rewritten.
    pub file: String,
    /// The path that was set.
    pub path: String,
    /// The YAML fragment now at that path.
    pub value: String,
}

impl fmt::Display for SetOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "set {} = {} in {} (lossless: comments and formatting preserved)",
            self.path, self.value, self.file
        )
    }
}

/// What `noyalib_set_multidoc` wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SetMultidocOutput {
    /// The file that was rewritten.
    pub file: String,
    /// The zero-based index of the document that changed.
    pub doc_index: usize,
    /// The path that was set, within that document.
    pub path: String,
    /// The YAML fragment now at that path.
    pub value: String,
}

impl fmt::Display for SetMultidocOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "set {} = {} in document {} of {} \
             (lossless: other documents, comments and formatting preserved)",
            self.path, self.value, self.doc_index, self.file
        )
    }
}

/// The JSON data model of the YAML `noyalib_parse` was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ParseOutput {
    /// One JSON value per document of the stream, custom tags
    /// stripped. A single-document text gives one element.
    pub documents: Vec<JsonValue>,
}

impl fmt::Display for ParseOutput {
    /// A single document prints as itself, a stream as an array: the
    /// projection the official YAML test suite expects.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self.documents.as_slice() {
            [one] => serde_json::to_string_pretty(one),
            many => serde_json::to_string_pretty(many),
        }
        .map_err(|_| fmt::Error)?;
        f.write_str(&text)
    }
}

/// The text `noyalib_edit` produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EditOutput {
    /// The whole YAML text with the one value replaced.
    pub yaml: String,
}

impl fmt::Display for EditOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.yaml)
    }
}

/// One JSON Schema violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Violation {
    /// The RFC 6901 path of the offending value.
    pub path: String,
    /// The schema keyword that failed.
    pub keyword: String,
    /// What is wrong with it.
    pub message: String,
}

/// The verdict of `noyalib_validate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ValidateOutput {
    /// Whether the text parses and, when a schema was given, satisfies
    /// it.
    pub valid: bool,
    /// The parse error, when the text does not parse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The line of the parse error, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The column of the parse error, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// Every schema violation; empty when the document is valid or
    /// no schema was given.
    pub violations: Vec<Violation>,
}

impl fmt::Display for ValidateOutput {
    /// The verdict as JSON, which is what the previous release printed
    /// and what a model parses back.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&text)
    }
}

// --- Parse profile ------------------------------------------------------

/// The rules every tool parses under.
///
/// The server's input is whatever a client sends, so the default is
/// noyalib's strict YAML 1.2 profile, the one built for untrusted
/// input: duplicate keys are an error rather than last-wins, only
/// `true` and `false` are booleans, indentation must be even, and the
/// tighter resource limits apply. `--profile standard` restores the
/// library defaults. The file tools and `noyalib_edit` parse through
/// the lossless CST under the same rules and limits, and the profile's
/// document limit also caps the size of a file read and of YAML text
/// in a request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParseProfile {
    /// `ParserConfig::strict()`.
    #[default]
    Strict,
    /// `ParserConfig::default()`, the library's YAML 1.2 defaults.
    Standard,
}

impl ParseProfile {
    /// Parse a `--profile` value.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "strict" => Some(Self::Strict),
            "standard" => Some(Self::Standard),
            _ => None,
        }
    }

    /// The parser configuration this profile stands for.
    #[must_use]
    pub fn config(self) -> noyalib::ParserConfig {
        match self {
            Self::Strict => noyalib::ParserConfig::strict(),
            Self::Standard => noyalib::ParserConfig::default(),
        }
    }
}

// --- The functions -------------------------------------------------------

/// Read the value at `path` in the YAML file `file`.
///
/// The value is the source slice, exactly as written. A key with no
/// value (`key:`) reads as the empty string rather than as a missing
/// path.
///
/// # Errors
///
/// The file cannot be read, does not parse, or has no such path. The
/// message says which.
pub fn get(file: &str, path: &str) -> Result<GetOutput, String> {
    get_at(
        &locate_unconfined(file)?,
        file,
        path,
        &noyalib::ParserConfig::default(),
    )
}

/// Locate a file the library functions were given, with no root.
fn locate_unconfined(file: &str) -> Result<Located, String> {
    Located::unconfined(Path::new(file)).map_err(|e| format!("read {file}: {e}"))
}

/// Read a located file within `config`'s document size limit.
fn read_at(
    at: &Located,
    file: &str,
    config: &noyalib::ParserConfig,
) -> Result<(String, Kept), String> {
    at.read(config.max_document_length)
        .map_err(|e| format!("read {file}: {e}"))
}

/// [`get`] on a file already located; `file` is how to name it.
pub(crate) fn get_at(
    at: &Located,
    file: &str,
    path: &str,
    config: &noyalib::ParserConfig,
) -> Result<GetOutput, String> {
    let (src, _) = read_at(at, file, config)?;
    let doc = parse_document_with_config(&src, config).map_err(|e| format!("parse {file}: {e}"))?;
    match doc.get(path) {
        Some(value) => Ok(GetOutput {
            value: value.to_string(),
        }),
        // `get` yields `None` for an implicit null (`key:` with no
        // value) as well as for a missing path; the key span tells
        // the two apart so an empty value reads as its (empty) source
        // slice instead of a spurious "not found".
        None if doc.key_span(path).is_some() => Ok(GetOutput {
            value: String::new(),
        }),
        None => Err(format!("path not found in {}: {}", clip(file), clip(path))),
    }
}

/// Set the value at `path` in the YAML file `file` to the fragment
/// `value`, rewriting only the touched span and writing atomically.
///
/// # Errors
///
/// The file cannot be read or written, does not parse, or the fragment
/// cannot be applied at the path. The file is unchanged on any of
/// them.
pub fn set(file: &str, path: &str, value: &str) -> Result<SetOutput, String> {
    set_at(
        &locate_unconfined(file)?,
        file,
        path,
        value,
        &noyalib::ParserConfig::default(),
    )
}

/// [`set`] on a file already located; `file` is how to name it.
pub(crate) fn set_at(
    at: &Located,
    file: &str,
    path: &str,
    value: &str,
    config: &noyalib::ParserConfig,
) -> Result<SetOutput, String> {
    let (src, kept) = read_at(at, file, config)?;
    check_fragment(value, config)?;
    let mut doc =
        parse_document_with_config(&src, config).map_err(|e| format!("parse {file}: {e}"))?;
    doc.set(path, value)
        .map_err(|e| set_failed(path, value, &e))?;
    at.replace(doc.to_string().as_bytes(), kept)
        .map_err(|e| format!("write {file}: {e}"))?;
    Ok(SetOutput {
        file: file.to_owned(),
        path: path.to_owned(),
        value: value.to_owned(),
    })
}

/// [`set`] for document `doc_index` of a `---`-separated stream.
///
/// # Errors
///
/// As [`set`], or the index is out of range. The file is unchanged on
/// any of them.
pub fn set_multidoc(
    file: &str,
    doc_index: usize,
    path: &str,
    value: &str,
) -> Result<SetMultidocOutput, String> {
    let at = locate_unconfined(file)?;
    let config = noyalib::ParserConfig::default();
    set_multidoc_at(&at, file, doc_index, path, value, &config)
}

/// [`set_multidoc`] on a file already located; `file` is how to name
/// it.
pub(crate) fn set_multidoc_at(
    at: &Located,
    file: &str,
    doc_index: usize,
    path: &str,
    value: &str,
    config: &noyalib::ParserConfig,
) -> Result<SetMultidocOutput, String> {
    let (src, kept) = read_at(at, file, config)?;
    // parse_stream keeps each `---`-delimited document as its own
    // lossless Document, retaining its separator; concatenating their
    // rendered forms reproduces the stream byte-for-byte, so editing one
    // document leaves every other document untouched.
    let mut docs =
        parse_stream_with_config(&src, config).map_err(|e| format!("parse {file}: {e}"))?;
    if doc_index >= docs.len() {
        return Err(format!(
            "doc_index {doc_index} out of range: stream has {} document(s)",
            docs.len()
        ));
    }
    check_fragment(value, config)?;
    docs[doc_index]
        .set(path, value)
        .map_err(|e| set_failed(path, value, &e))?;
    let out: String = docs.iter().map(ToString::to_string).collect();
    at.replace(out.as_bytes(), kept)
        .map_err(|e| format!("write {file}: {e}"))?;
    Ok(SetMultidocOutput {
        file: file.to_owned(),
        doc_index,
        path: path.to_owned(),
        value: value.to_owned(),
    })
}

/// Parse YAML text into its JSON data model. Nothing on disk is read.
///
/// # Errors
///
/// The text does not parse under the library's limits.
pub fn parse(yaml: &str) -> Result<ParseOutput, String> {
    parse_with_profile(yaml, ParseProfile::default())
}

/// [`parse`] under an explicit [`ParseProfile`].
///
/// # Errors
///
/// The text does not parse under the profile's rules and limits.
pub fn parse_with_profile(yaml: &str, profile: ParseProfile) -> Result<ParseOutput, String> {
    check_text(yaml, &profile.config())?;
    let docs: Vec<noyalib::Value> = noyalib::load_all_with_config(yaml, &profile.config())
        .and_then(Iterator::collect)
        .map_err(|e| format!("parse: {e}"))?;
    let documents = docs
        .into_iter()
        .map(|d| serde_json::to_value(d.untag()).map_err(internal))
        .collect::<Result<_, _>>()?;
    Ok(ParseOutput { documents })
}

/// Set one value in YAML text and return the whole edited text.
/// Nothing on disk is touched. The text is parsed under the library's
/// default limits ([`ParseProfile::Standard`]).
///
/// # Errors
///
/// The text does not parse, or the fragment cannot be applied at the
/// path.
pub fn edit(yaml: &str, path: &str, value: &str) -> Result<EditOutput, String> {
    edit_with_profile(yaml, path, value, ParseProfile::Standard)
}

/// [`edit`] under an explicit [`ParseProfile`]: its document size and
/// nesting limits apply to the text and to the fragment.
///
/// # Errors
///
/// The text or the fragment is over the profile's limits, the text
/// does not parse, or the fragment cannot be applied at the path.
pub fn edit_with_profile(
    yaml: &str,
    path: &str,
    value: &str,
    profile: ParseProfile,
) -> Result<EditOutput, String> {
    let config = profile.config();
    check_text(yaml, &config)?;
    check_fragment(value, &config)?;
    let mut doc = parse_document_with_config(yaml, &config).map_err(|e| format!("parse: {e}"))?;
    doc.set(path, value)
        .map_err(|e| set_failed(path, value, &e))?;
    Ok(EditOutput {
        yaml: doc.to_string(),
    })
}

mod limits;

pub use limits::MAX_FRAGMENT_BYTES;
#[cfg(test)]
use limits::nesting_depth;
use limits::{check_fragment, check_text, clip, set_failed};

/// Check that YAML text parses and, when `schema` is given, that it
/// satisfies that JSON Schema.
///
/// A text that does not parse, or a document that violates the schema,
/// is a verdict with `valid: false`, not an error: the location or the
/// violations are the answer.
///
/// # Errors
///
/// The schema is not JSON or not a valid schema. Nothing was
/// validated, so there is no verdict.
pub fn validate(yaml: &str, schema: Option<&str>) -> Result<ValidateOutput, String> {
    validate_with_profile(yaml, schema, ParseProfile::default())
}

/// [`validate`] under an explicit [`ParseProfile`].
///
/// # Errors
///
/// The schema is not valid JSON or not a valid JSON Schema. A document
/// that fails to parse or to validate is a result, not an error.
pub fn validate_with_profile(
    yaml: &str,
    schema: Option<&str>,
    profile: ParseProfile,
) -> Result<ValidateOutput, String> {
    let value = match parse_for_validation(yaml, &profile.config()) {
        Ok(v) => v,
        Err(verdict) => return Ok(verdict),
    };
    let violations = match schema {
        Some(schema_text) => schema_violations(&value, schema_text)?,
        None => Vec::new(),
    };
    Ok(ValidateOutput {
        valid: violations.is_empty(),
        error: None,
        line: None,
        column: None,
        violations,
    })
}

/// The document `noyalib_validate` checks, or the verdict that it does
/// not parse.
fn parse_for_validation(
    yaml: &str,
    config: &noyalib::ParserConfig,
) -> Result<noyalib::Value, ValidateOutput> {
    let failed = |error: String, line, column| ValidateOutput {
        valid: false,
        error: Some(error),
        line: Some(line),
        column: Some(column),
        violations: Vec::new(),
    };
    check_text(yaml, config).map_err(|e| failed(e, 0, 0))?;
    noyalib::from_str_with_config::<noyalib::Value>(yaml, config).map_err(|e| {
        let (line, column) = e.location().map_or((0, 0), |l| (l.line(), l.column()));
        failed(e.to_string(), line, column)
    })
}

/// The largest JSON Schema `noyalib_validate` compiles, in bytes.
pub const MAX_SCHEMA_BYTES: usize = 64 * 1024;

/// How many violations a verdict lists. Past it, one more entry says
/// the list was cut short. The core stops collecting one past this cap,
/// so a verdict never costs more than `MAX_VIOLATIONS + 1` violations.
pub const MAX_VIOLATIONS: usize = 100;

/// Compile a client's JSON Schema, refusing one over
/// [`MAX_SCHEMA_BYTES`] before it is parsed.
fn compile_schema(schema_text: &str) -> Result<noyalib::CompiledSchema, String> {
    if schema_text.len() > MAX_SCHEMA_BYTES {
        return Err(format!(
            "the schema is {} bytes, over the {MAX_SCHEMA_BYTES}-byte schema limit",
            schema_text.len()
        ));
    }
    let schema: JsonValue =
        serde_json::from_str(schema_text).map_err(|e| format!("schema is not JSON: {e}"))?;
    let schema_value: noyalib::Value =
        serde_json::from_value(schema).map_err(|e| format!("schema: {e}"))?;
    noyalib::CompiledSchema::builder(&schema_value)
        .max_errors(MAX_VIOLATIONS + 1)
        .build()
        .map_err(|e| format!("schema: {e}"))
}

/// The violations of the JSON Schema `schema_text` by `value`: the
/// first [`MAX_VIOLATIONS`] of them with their messages clipped, and a
/// `truncated` entry when there were more.
fn schema_violations(value: &noyalib::Value, schema_text: &str) -> Result<Vec<Violation>, String> {
    let compiled = compile_schema(schema_text)?;
    let all = compiled.iter_errors(value).map_err(internal)?;
    let mut violations: Vec<Violation> = all
        .iter()
        .take(MAX_VIOLATIONS)
        .map(|v| Violation {
            path: clip(&v.instance_path).into_owned(),
            keyword: v.keyword.clone(),
            message: clip(&v.message).into_owned(),
        })
        .collect();
    if all.len() > MAX_VIOLATIONS {
        violations.push(Violation {
            path: String::new(),
            keyword: "truncated".to_owned(),
            message: format!(
                "more than {MAX_VIOLATIONS} violations; the first {MAX_VIOLATIONS} are listed"
            ),
        });
    }
    Ok(violations)
}

/// An error no request can provoke (a JSON conversion of a value the
/// parser already accepted): one function, so the unreachable paths do
/// not each count as an uncovered closure.
fn internal(e: impl fmt::Display) -> String {
    format!("internal: {e}")
}

/// Find a `file` argument under the server root.
pub(crate) fn locate(root: &RootDir, file: &str) -> Result<Located, String> {
    root.locate(Path::new(file)).map_err(|e| match e {
        FileError::Outside => outside(file),
        FileError::Io(e) => format!("read {file}: {e}"),
    })
}

/// The one answer for a path outside the root, whatever is there.
pub(crate) fn outside(file: &str) -> String {
    format!("{file} is outside the server root; start noyalib-mcp with --root to allow it")
}

/// How long one tool call may run before the client is answered with
/// an error. See [`YamlServer::with_call_timeout`].
pub const DEFAULT_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Run a tool's work on the blocking pool, off the async workers, and
/// answer with an error if it outlasts `limit`.
///
/// File I/O and parsing block. On a worker thread a slow call would
/// stall every other request on that worker, `ping` included. A call
/// past its time limit is answered at once; its thread cannot be
/// interrupted and finishes in the background, its result discarded.
async fn off_thread<T, F>(limit: std::time::Duration, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    match tokio::time::timeout(limit, tokio::task::spawn_blocking(work)).await {
        Ok(joined) => joined.unwrap_or_else(|e| Err(internal(e))),
        Err(_) => Err(format!(
            "the call ran past the {} ms time limit and was abandoned",
            limit.as_millis()
        )),
    }
}

// --- Arguments -----------------------------------------------------------
//
// The doc comments are the descriptions a client shows the model, kept
// word for word from the previous release. The examples are what an
// auditor sends when it has no file of its own.

/// Arguments of `noyalib_get`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    /// Path to the YAML file on disk.
    #[schemars(example = &"config.yaml")]
    pub file: String,
    /// Dotted/indexed path into the YAML, e.g. `server.host` or
    /// `items[0].name`.
    #[schemars(example = &"server.host")]
    pub path: String,
}

/// Arguments of `noyalib_set`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetArgs {
    /// Path to the YAML file on disk.
    #[schemars(example = &"config.yaml")]
    pub file: String,
    /// Dotted/indexed path into the YAML.
    #[schemars(example = &"server.port")]
    pub path: String,
    /// Replacement value as a YAML fragment (e.g. `0.0.2`, `"hello"`,
    /// `[1, 2, 3]`). Must parse in the target position; the document
    /// is left unchanged on parse error.
    #[schemars(example = &"9090")]
    pub value: String,
}

/// Arguments of `noyalib_set_multidoc`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetMultidocArgs {
    /// Path to the multi-document YAML file on disk.
    #[schemars(example = &"manifests.yaml")]
    pub file: String,
    /// Zero-based index of the document within the `---`-separated
    /// stream to modify.
    #[schemars(example = &0, range(min = 0))]
    pub doc_index: usize,
    /// Dotted/indexed path into the selected document.
    #[schemars(example = &"metadata.name")]
    pub path: String,
    /// Replacement value as a YAML fragment. Must parse in the target
    /// position; the file is left unchanged on parse error.
    #[schemars(example = &"api")]
    pub value: String,
}

/// Arguments of `noyalib_parse`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParseArgs {
    /// The YAML text to parse.
    #[schemars(example = &"server:\n  host: api.example.com  # public\n  port: 8080\n")]
    pub yaml: String,
}

/// Arguments of `noyalib_edit`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditArgs {
    /// The YAML text to edit.
    #[schemars(example = &"server:\n  host: api.example.com  # public\n  port: 8080\n")]
    pub yaml: String,
    /// Dotted/indexed path, e.g. `server.port` or `items[0].name`.
    #[schemars(example = &"server.port")]
    pub path: String,
    /// Replacement value as a YAML fragment.
    #[schemars(example = &"9090")]
    pub value: String,
}

/// Arguments of `noyalib_validate`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateArgs {
    /// The YAML text to validate.
    #[schemars(example = &"server:\n  host: api.example.com  # public\n  port: 8080\n")]
    pub yaml: String,
    /// A JSON Schema, as JSON text (optional).
    #[schemars(example = &"{\"type\":\"object\",\"required\":[\"server\"]}")]
    #[serde(default)]
    pub schema: Option<String>,
}

// --- The MCP tools -------------------------------------------------------

/// A tool result carrying the same answer twice: as text for the
/// model and as a structured value for the client.
///
/// A failure keeps the text only. The structured schema describes a
/// result, and an error is not one.
fn reply<T: Serialize + fmt::Display>(
    outcome: Result<T, String>,
    is_error: impl FnOnce(&T) -> bool,
) -> Result<CallToolResult, ErrorData> {
    match outcome {
        Ok(value) => {
            let structured = serde_json::to_value(&value)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            let content = vec![ContentBlock::text(value.to_string())];
            let mut result = if is_error(&value) {
                CallToolResult::error(content)
            } else {
                CallToolResult::success(content)
            };
            result.structured_content = Some(structured);
            Ok(result)
        }
        // A tool that ran and could not do the job: a *successful*
        // JSON-RPC response carrying `isError`, so the model sees the
        // text and can react to it. A JSON-RPC error would be handled
        // by the client and never shown.
        Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)])),
    }
}

/// The names of the tools, for the message an unknown name draws.
pub const TOOL_NAMES: [&str; 6] = [
    "noyalib_get",
    "noyalib_set",
    "noyalib_set_multidoc",
    "noyalib_parse",
    "noyalib_edit",
    "noyalib_validate",
];

#[tool_router(vis = "pub(crate)")]
#[allow(
    clippy::unused_self,
    reason = "the SDK's tool router calls tools as methods"
)]
impl YamlServer {
    #[tool(
        name = "noyalib_get",
        title = "Read a YAML value (lossless)",
        description = "Read the YAML value at a dotted/indexed path \
                       in the given file and return the source slice exactly — no \
                       re-quoting, no canonicalisation, comments and formatting \
                       preserved. Use this to inspect a value before changing it; \
                       use `noyalib_set` to write a value back losslessly.",
        // Reads a caller-supplied YAML file without modifying it:
        // read-only, idempotent, never destructive, and open-world (it
        // touches the local filesystem).
        annotations(
            title = "Read a YAML value (lossless)",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        ),
        output_schema = schema_for_output::<GetOutput>()
    )]
    async fn noyalib_get(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let (root, config) = (self.root.clone(), self.profile.config());
        let outcome = off_thread(self.call_timeout, move || {
            let at = locate(&root, &args.file)?;
            get_at(&at, &args.file, &args.path, &config)
        });
        reply(outcome.await, |_| false)
    }

    #[tool(
        name = "noyalib_set",
        title = "Write a YAML value (lossless)",
        description = "Set the YAML value at a dotted/indexed path in \
                       the given file, rewriting only the touched span so every \
                       comment, blank line, and sibling entry is preserved \
                       byte-for-byte (written atomically). Use this for \
                       Renovate-style version bumps and config patches; use \
                       `noyalib_get` first when you need to read the current \
                       value. On a parse error the document is left unchanged.",
        // Overwrites the value at a path in a caller-supplied file on
        // disk: not read-only, and destructive (it replaces existing
        // content in place). Re-running with the same arguments yields
        // the same file state, so it is idempotent; it touches the
        // filesystem, so it is open-world.
        annotations(
            title = "Write a YAML value (lossless)",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        ),
        output_schema = schema_for_output::<SetOutput>()
    )]
    async fn noyalib_set(
        &self,
        Parameters(args): Parameters<SetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let (root, config) = (self.root.clone(), self.profile.config());
        let outcome = off_thread(self.call_timeout, move || {
            let at = locate(&root, &args.file)?;
            set_at(&at, &args.file, &args.path, &args.value, &config)
        });
        reply(outcome.await, |_| false)
    }

    #[tool(
        name = "noyalib_set_multidoc",
        title = "Write a YAML value in one document of a multi-doc stream (lossless)",
        description = "Set the YAML value at a dotted/indexed path within \
                       a single document of a multi-document (`---`-separated) YAML \
                       stream, selected by zero-based document index. Only the touched \
                       span of that one document is rewritten; every other document, \
                       comment, blank line and separator is preserved byte-for-byte \
                       (written atomically). Use `noyalib_set` for a single-document \
                       file. On a parse error or out-of-range index the file is left \
                       unchanged.",
        annotations(
            title = "Write a YAML value in one document of a multi-doc stream (lossless)",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = true
        ),
        output_schema = schema_for_output::<SetMultidocOutput>()
    )]
    async fn noyalib_set_multidoc(
        &self,
        Parameters(args): Parameters<SetMultidocArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let (root, config) = (self.root.clone(), self.profile.config());
        let outcome = off_thread(self.call_timeout, move || {
            let at = locate(&root, &args.file)?;
            let SetMultidocArgs {
                file,
                doc_index,
                path,
                value,
            } = &args;
            set_multidoc_at(&at, file, *doc_index, path, value, &config)
        });
        reply(outcome.await, |_| false)
    }

    #[tool(
        name = "noyalib_parse",
        title = "Parse YAML text into JSON (stateless)",
        description = "Parse the YAML text given in the request and return \
                       its JSON data model (custom tags stripped, the projection the \
                       official YAML test suite expects). Multi-document streams return \
                       a JSON array with one element per document. Refuses hostile \
                       input (nesting, alias expansion, size) and duplicate keys under \
                       the strict YAML 1.2 profile, the same rules as \
                       the library. Nothing is read from or written to disk.",
        // Content-in-request: nothing on disk is read or written. Pure,
        // read-only, idempotent, closed-world.
        annotations(
            title = "Parse YAML text into JSON (stateless)",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<ParseOutput>()
    )]
    async fn noyalib_parse(
        &self,
        Parameters(args): Parameters<ParseArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let profile = self.profile();
        let outcome = off_thread(self.call_timeout, move || {
            parse_with_profile(&args.yaml, profile)
        });
        reply(outcome.await, |_| false)
    }

    #[tool(
        name = "noyalib_edit",
        title = "Edit a value in YAML text, losslessly (stateless)",
        description = "Set the value at a dotted/indexed path in the YAML text \
                       given in the request and return the whole edited text. Only the \
                       touched span changes; every comment, blank line and quote style \
                       elsewhere is preserved byte-for-byte. Nothing on disk is touched: \
                       the caller decides where the result goes.",
        annotations(
            title = "Edit a value in YAML text, losslessly (stateless)",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<EditOutput>()
    )]
    async fn noyalib_edit(
        &self,
        Parameters(args): Parameters<EditArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let profile = self.profile();
        let outcome = off_thread(self.call_timeout, move || {
            edit_with_profile(&args.yaml, &args.path, &args.value, profile)
        });
        reply(outcome.await, |_| false)
    }

    #[tool(
        name = "noyalib_validate",
        title = "Validate YAML text, optionally against a JSON Schema (stateless)",
        description = "Check that the YAML text parses under the library's \
                       limits, and when a JSON Schema (as JSON text) is given, that the \
                       document satisfies it. Returns `valid` with an empty list, or the \
                       parse error with its line and column, or every schema violation \
                       with its RFC 6901 path. Nothing on disk is touched.",
        annotations(
            title = "Validate YAML text, optionally against a JSON Schema (stateless)",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        output_schema = schema_for_output::<ValidateOutput>()
    )]
    async fn noyalib_validate(
        &self,
        Parameters(args): Parameters<ValidateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let profile = self.profile();
        let outcome = off_thread(self.call_timeout, move || {
            validate_with_profile(&args.yaml, args.schema.as_deref(), profile)
        });
        // An invalid document is a failure the model must see, so it
        // is flagged `isError` -- but it is also a complete verdict, so
        // the structured half is kept.
        reply(outcome.await, |v| !v.valid)
    }
}

#[cfg(test)]
mod tests;
