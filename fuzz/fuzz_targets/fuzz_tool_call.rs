// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The `tools/call` path, reached with well-formed JSON-RPC framing.
//!
//! `fuzz_handle_message` spends most of its budget being rejected by
//! `serde_json` before it reaches any MCP logic. This target wraps the
//! fuzzer's bytes in a valid envelope so the arguments — which are where
//! YAML actually gets parsed and edited — are what varies.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = core::str::from_utf8(data) else {
        return;
    };
    // JSON string escaping, so arbitrary bytes cannot break the envelope
    // and turn this back into a framing fuzzer.
    let escaped: String = s
        .chars()
        .flat_map(|c| match c {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            c if (c as u32) < 0x20 => format!("\\u{:04x}", c as u32).chars().collect(),
            c => vec![c],
        })
        .collect();

    for tool in ["yaml_validate", "yaml_format", "yaml_query"] {
        let msg = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool}","arguments":{{"yaml":"{escaped}"}}}}}}"#
        );
        let _ = noyalib_mcp::handle_message(&msg);
    }
});
