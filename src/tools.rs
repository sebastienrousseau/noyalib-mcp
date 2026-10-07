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

use noyalib::cst::{parse_document, parse_stream};
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

/// The rules `noyalib_parse` and `noyalib_validate` parse under.
///
/// The server's input is whatever a client sends, so the default is
/// noyalib's strict YAML 1.2 profile, the one built for untrusted
/// input: duplicate keys are an error rather than last-wins, only
/// `true` and `false` are booleans, indentation must be even, and the
/// tighter resource limits apply. `--profile standard` restores the
/// library defaults. The file tools and `noyalib_edit` go through the
/// lossless CST and are not affected.
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
    let doc = parse_document(&src).map_err(|e| format!("parse {file}: {e}"))?;
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
    let mut doc = parse_document(&src).map_err(|e| format!("parse {file}: {e}"))?;
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
    let mut docs = parse_stream(&src).map_err(|e| format!("parse {file}: {e}"))?;
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
    let mut doc = parse_document(yaml).map_err(|e| format!("parse: {e}"))?;
    doc.set(path, value)
        .map_err(|e| set_failed(path, value, &e))?;
    Ok(EditOutput {
        yaml: doc.to_string(),
    })
}

// --- Request limits ------------------------------------------------------

/// The largest replacement fragment `noyalib_set`,
/// `noyalib_set_multidoc` and `noyalib_edit` take, in bytes. A value
/// is one node of a document; this is far beyond any real one.
pub const MAX_FRAGMENT_BYTES: usize = 256 * 1024;

/// How much of a client's value an error message repeats.
const ECHO_BYTES: usize = 120;

/// `text` as an error message may repeat it: whole when short, else
/// its start and its length.
fn clip(text: &str) -> std::borrow::Cow<'_, str> {
    if text.len() <= ECHO_BYTES {
        return text.into();
    }
    let mut end = ECHO_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &text[..end], text.len()).into()
}

/// The message for a fragment the document refused.
fn set_failed(path: &str, value: &str, e: &impl fmt::Display) -> String {
    format!("set {} = {}: {e}", clip(path), clip(value))
}

/// Refuse YAML text longer than the profile's document limit before
/// anything parses it.
fn check_text(yaml: &str, config: &noyalib::ParserConfig) -> Result<(), String> {
    if yaml.len() > config.max_document_length {
        return Err(format!(
            "the YAML text is {} bytes, over the {}-byte document limit",
            yaml.len(),
            config.max_document_length
        ));
    }
    Ok(())
}

/// Refuse a replacement fragment that is too long or nests deeper than
/// the profile allows, before the document parses it.
///
/// The fragment is parsed on its own before the edited document is
/// checked against the limits, so the nesting is bounded here, by a
/// linear count that never under-estimates.
fn check_fragment(value: &str, config: &noyalib::ParserConfig) -> Result<(), String> {
    if value.len() > MAX_FRAGMENT_BYTES {
        return Err(format!(
            "the value is {} bytes, over the {MAX_FRAGMENT_BYTES}-byte fragment limit",
            value.len()
        ));
    }
    let depth = nesting_depth(value);
    if depth > config.max_depth {
        return Err(format!(
            "the value nests {depth} levels deep, over the limit of {}",
            config.max_depth
        ));
    }
    Ok(())
}

/// An upper bound on how deep `text` nests, in one pass.
///
/// Every level of YAML nesting is opened by a flow bracket, a deeper
/// indentation, or a block indicator (`- `, `? `, `: `) on the line.
/// The count takes all three everywhere, quoted text and comments
/// included, so it can over-count but never under-count.
pub(crate) fn nesting_depth(text: &str) -> usize {
    let mut flow = 0usize;
    let mut indents: Vec<usize> = Vec::new();
    let mut deepest = 0;
    for line in text.lines() {
        let body = line.trim_start_matches(' ');
        let indent = line.len() - body.len();
        if !body.trim().is_empty() {
            while indents.last().is_some_and(|&top| top > indent) {
                let _ = indents.pop();
            }
            if indents.last().is_none_or(|&top| top < indent) {
                indents.push(indent);
            }
        }
        let (indicators, peak) = scan_line(body, &mut flow);
        deepest = deepest.max(indents.len() + indicators + peak);
    }
    deepest
}

/// The block indicators on one line, and the deepest flow nesting it
/// reaches; `flow` carries the open brackets from line to line.
fn scan_line(body: &str, flow: &mut usize) -> (usize, usize) {
    let bytes = body.as_bytes();
    let mut indicators = 0;
    let mut peak = *flow;
    for (i, &b) in bytes.iter().enumerate() {
        let spaced = bytes.get(i + 1).is_none_or(|n| *n == b' ' || *n == b'\t');
        match b {
            b'[' | b'{' => {
                *flow += 1;
                peak = peak.max(*flow);
            }
            b']' | b'}' => *flow = flow.saturating_sub(1),
            b'-' | b'?' | b':' if spaced => indicators += 1,
            _ => {}
        }
    }
    (indicators, peak)
}

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

/// Every violation of the JSON Schema `schema_text` by `value`.
fn schema_violations(value: &noyalib::Value, schema_text: &str) -> Result<Vec<Violation>, String> {
    let schema: JsonValue =
        serde_json::from_str(schema_text).map_err(|e| format!("schema is not JSON: {e}"))?;
    let schema_value: noyalib::Value =
        serde_json::from_value(schema).map_err(|e| format!("schema: {e}"))?;
    let compiled =
        noyalib::CompiledSchema::compile(&schema_value).map_err(|e| format!("schema: {e}"))?;
    Ok(compiled
        .iter_errors(value)
        .map_err(internal)?
        .iter()
        .map(|v| Violation {
            path: v.instance_path.clone(),
            keyword: v.keyword.clone(),
            message: v.message.clone(),
        })
        .collect())
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

/// Run a tool's work on the blocking pool, off the async workers.
///
/// File I/O and parsing block. On a worker thread a slow call would
/// stall every other request on that worker, `ping` included.
async fn off_thread<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|e| Err(internal(e)))
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
        let outcome = off_thread(move || {
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
        let outcome = off_thread(move || {
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
        let outcome = off_thread(move || {
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
        let outcome = off_thread(move || parse_with_profile(&args.yaml, profile));
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
        let outcome =
            off_thread(move || edit_with_profile(&args.yaml, &args.path, &args.value, profile));
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
        let outcome =
            off_thread(move || validate_with_profile(&args.yaml, args.schema.as_deref(), profile));
        // An invalid document is a failure the model must see, so it
        // is flagged `isError` -- but it is also a complete verdict, so
        // the structured half is kept.
        reply(outcome.await, |v| !v.valid)
    }
}

#[cfg(test)]
mod tests {
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

    // ── file safety ──────────────────────────────────────────────────

    /// A fresh directory under the system temp dir.
    fn scratch_dir(label: &str) -> PathBuf {
        let p = temp_path(label).with_extension("d");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[cfg(unix)]
    #[test]
    fn set_keeps_the_file_mode() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_dir("mode");
        let server = YamlServer::with_root(root.clone());
        for mode in [0o600, 0o755, 0o640] {
            let name = format!("m{mode:o}.yml");
            let p = root.join(&name);
            fs::write(&p, "a: 1\n").unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
            let r = call_on(
                &server,
                "noyalib_set",
                json!({"file": name, "path": "a", "value": "2"}),
            );
            assert!(!is_error(&r), "{}", text_of(&r));
            assert_eq!(fs::read_to_string(&p).unwrap(), "a: 2\n");
            let got = fs::metadata(&p).unwrap().permissions().mode() & 0o7777;
            assert_eq!(got, mode, "mode of {name}: {got:o}");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn outside_the_root_is_one_answer_whatever_is_there() {
        // Whether a path outside the root exists, is missing or is
        // unreadable must not show, and neither may the root itself.
        let root = scratch_dir("oracle");
        let outside = write_temp("oracle-outside", "a: 1\n");
        let server = YamlServer::with_root(root.clone());
        let missing = outside.with_extension("missing");
        let mut answers = Vec::new();
        for file in [
            outside.to_str().unwrap().to_owned(),
            missing.to_str().unwrap().to_owned(),
            format!("../{}", outside.file_name().unwrap().to_str().unwrap()),
            format!("../{}", missing.file_name().unwrap().to_str().unwrap()),
            "/etc/does-not-exist".to_owned(),
        ] {
            let r = call_on(&server, "noyalib_get", json!({"file": file, "path": "a"}));
            assert!(is_error(&r));
            let text = text_of(&r).replace(&file, "<file>");
            assert!(!text.contains(root.to_str().unwrap()), "{text}");
            assert!(!text.contains(server.root().to_str().unwrap()), "{text}");
            answers.push(text);
        }
        assert!(answers.windows(2).all(|w| w[0] == w[1]), "{answers:#?}");
        assert!(
            answers[0].contains("outside the server root"),
            "{}",
            answers[0]
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_file(outside);
    }

    #[cfg(unix)]
    #[test]
    fn a_racing_symlink_swap_never_escapes_the_root() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;
        let root = scratch_dir("race-root");
        let outside = scratch_dir("race-outside");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/secret.yml"), "secret: inside\n").unwrap();
        fs::write(outside.join("secret.yml"), "secret: OUTSIDE\n").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let swapper = {
            let (root, outside, stop) = (root.clone(), outside.clone(), Arc::clone(&stop));
            std::thread::spawn(move || {
                let (sub, real) = (root.join("sub"), root.join("sub.real"));
                while !stop.load(Ordering::Relaxed) {
                    let _ = fs::rename(&sub, &real);
                    let _ = std::os::unix::fs::symlink(&outside, &sub);
                    let _ = fs::remove_file(&sub);
                    let _ = fs::rename(&real, &sub);
                }
            })
        };
        let server = YamlServer::with_root(root.clone());
        let file = json!("sub/secret.yml");
        for i in 0..6000 {
            let r = call_on(
                &server,
                "noyalib_get",
                json!({"file": file, "path": "secret"}),
            );
            assert_ne!(text_of(&r), "OUTSIDE", "read escaped the root on call {i}");
            if i % 4 == 0 {
                let v = format!("w{i}");
                let _ = call_on(
                    &server,
                    "noyalib_set",
                    json!({"file": file, "path": "secret", "value": v}),
                );
            }
        }
        stop.store(true, Ordering::Relaxed);
        swapper.join().unwrap();
        assert_eq!(
            fs::read_to_string(outside.join("secret.yml")).unwrap(),
            "secret: OUTSIDE\n",
            "a write escaped the root"
        );
        let stray: Vec<_> = fs::read_dir(&outside)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(stray.len(), 1, "temp files landed outside: {stray:?}");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    // ── request limits ───────────────────────────────────────────────

    #[test]
    fn nesting_depth_never_under_counts() {
        let cases = [
            ("1", 1),
            ("[1, [2, [3]]]", 4),
            ("{a: {b: [c]}}", 4),
            ("- - - x", 4),
            ("a:\n  b:\n    c: 1\n", 4),
            ("a:\n- b\n- c\n", 2),
            ("[\n [\n  [\n  ]\n ]\n]\n", 6),
            ("? a\n: b\n", 2),
        ];
        for (text, at_least) in cases {
            let got = nesting_depth(text);
            assert!(got >= at_least, "{text:?}: {got} < {at_least}");
        }
        assert_eq!(nesting_depth(&"[".repeat(100_000)), 100_001);
        assert_eq!(nesting_depth(&"- ".repeat(100_000)), 100_001);
        assert!(nesting_depth("plain scalar text") < 3);
    }

    #[test]
    fn fragments_are_checked_against_the_profile() {
        let strict = ParseProfile::Strict.config();
        let deep = "[".repeat(65) + &"]".repeat(65);
        let err = check_fragment(&deep, &strict).unwrap_err();
        assert!(err.contains("over the limit of 64"), "{err}");
        assert!(check_fragment(&deep, &ParseProfile::Standard.config()).is_ok());
        let long = "x".repeat(MAX_FRAGMENT_BYTES + 1);
        let err = check_fragment(&long, &strict).unwrap_err();
        assert!(err.contains("fragment limit"), "{err}");
        assert!(check_fragment("[1, 2, {a: b}]", &strict).is_ok());
    }

    #[test]
    fn edit_refuses_a_deep_fragment_without_echoing_it() {
        let deep = "[".repeat(100_000) + &"]".repeat(100_000);
        let err = edit_with_profile("a: 1\n", "a", &deep, ParseProfile::Strict).unwrap_err();
        assert!(err.contains("nests"), "{err}");
        // A 40 KB fragment the document refuses is named, not repeated.
        let bad = format!("\"{}", "x".repeat(40_000));
        let err = edit("a: 1\n", "a", &bad).unwrap_err();
        assert!(err.len() < 600, "{} bytes: {err}", err.len());
        assert!(err.contains("(40001 bytes)"), "{err}");
    }

    #[test]
    fn clip_keeps_short_text_and_cuts_on_a_char_boundary() {
        assert_eq!(clip("short"), "short");
        let long = "é".repeat(100);
        let clipped = clip(&long);
        assert!(clipped.ends_with("... (200 bytes)"), "{clipped}");
        assert!(clipped.len() < 140);
    }

    #[test]
    fn text_over_the_document_limit_is_refused_before_parsing() {
        let long = format!("a: {}\n", "x".repeat(1 << 20));
        let err = parse(&long).unwrap_err();
        assert!(err.contains("document limit"), "{err}");
        let err = edit_with_profile(&long, "a", "1", ParseProfile::Strict).unwrap_err();
        assert!(err.contains("document limit"), "{err}");
        let verdict = validate(&long, None).unwrap();
        assert!(!verdict.valid);
        assert!(verdict.error.unwrap().contains("document limit"));
        assert!(parse_with_profile(&long, ParseProfile::Standard).is_ok());
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
}
