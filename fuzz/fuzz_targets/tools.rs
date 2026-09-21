// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Arbitrary arguments must never panic a tool.
//!
//! The JSON-RPC layer belongs to the SDK. What is still this crate's
//! own surface is the tools, and what they are fed is whatever a
//! client sends: YAML text, a path, a fragment, a schema, each usually
//! written by a language model. The contract is total: any strings at
//! all produce a result or an error message, never a panic. A panic
//! takes down the session for every tool call, not just the malformed
//! one.
//!
//! The input is split on the first three NUL bytes into YAML text,
//! path, value and schema, so one corpus exercises every stateless
//! tool. The file tools are the same parse and edit behind a read and
//! an atomic write, and are not given the fuzzer's bytes as paths.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = core::str::from_utf8(data) else {
        return;
    };
    let mut parts = text.splitn(4, '\0');
    let yaml = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("a");
    let value = parts.next().unwrap_or("1");
    let schema = parts.next();

    let _ = noyalib_mcp::parse(yaml);
    let _ = noyalib_mcp::edit(yaml, path, value);
    let _ = noyalib_mcp::validate(yaml, schema);
});
