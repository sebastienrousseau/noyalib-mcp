// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! The `noyalib-mcp` executable.
//!
//! Everything the server does lives in the library. This binary picks
//! the transport from the command line -- stdio by default, streamable
//! HTTP or the older HTTP+SSE on request -- and hands the library's
//! handler to it. `transport.rs` is the same file in every Rust server
//! of the suite.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Opt-in coverage exclusion: when `NOYALIB_COVERAGE=1`, `main` is
// excluded from instrumentation. It is one call into the transport
// module, which `tests/http.rs` and `tests/protocol.rs` drive through
// the real binary.
#![cfg_attr(noyalib_coverage, allow(unstable_features))]
#![cfg_attr(noyalib_coverage, feature(coverage_attribute))]

mod transport;

use std::process::ExitCode;

#[cfg_attr(noyalib_coverage, coverage(off))]
fn main() -> ExitCode {
    transport::run(
        "noyalib-mcp",
        env!("CARGO_PKG_VERSION"),
        std::env::args().skip(1),
        noyalib_mcp::YamlServer::new,
    )
}
