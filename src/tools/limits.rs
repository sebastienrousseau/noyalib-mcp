// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! What one request may cost: the size and nesting of a replacement
//! value, the size of YAML text, and how much of a client's input an
//! error repeats.

use std::fmt;

/// The largest replacement fragment `noyalib_set`,
/// `noyalib_set_multidoc` and `noyalib_edit` take, in bytes. A value
/// is one node of a document; this is far beyond any real one.
pub const MAX_FRAGMENT_BYTES: usize = 256 * 1024;

/// How much of a client's value an error message repeats.
const ECHO_BYTES: usize = 120;

/// `text` as an error message may repeat it: whole when short, else
/// its start and its length.
pub(super) fn clip(text: &str) -> std::borrow::Cow<'_, str> {
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
pub(super) fn set_failed(path: &str, value: &str, e: &impl fmt::Display) -> String {
    format!("set {} = {}: {e}", clip(path), clip(value))
}

/// Refuse YAML text longer than the profile's document limit before
/// anything parses it.
pub(super) fn check_text(yaml: &str, config: &noyalib::ParserConfig) -> Result<(), String> {
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
pub(super) fn check_fragment(value: &str, config: &noyalib::ParserConfig) -> Result<(), String> {
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
pub(super) fn nesting_depth(text: &str) -> usize {
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
