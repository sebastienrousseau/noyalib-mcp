// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! `handle_message` is the MCP server's entire trust boundary.
//!
//! Everything it receives arrives over stdio from an LLM agent — a
//! Claude Desktop, a Cursor, an mcp.run host — and none of it is
//! trustworthy. The contract is that no input, however malformed, may
//! panic the server: a crash there takes the whole session with it and
//! the caller cannot distinguish it from the tool going silent.
//!
//! So the invariant is simply *it returns*. Any `HandleOutcome` is
//! acceptable, including an error reply; an abort is not.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Non-UTF-8 never reaches `handle_message` — the transport decodes
    // first — so feeding it here would fuzz a path that cannot occur.
    if let Ok(raw) = core::str::from_utf8(data) {
        let _ = noyalib_mcp::handle_message(raw);
    }
});
