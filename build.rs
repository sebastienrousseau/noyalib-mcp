// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

//! Build script: opt-in coverage exclusion. Mirrors the parent
//! crate's flag so `#[cfg_attr(noyalib_coverage, coverage(off))]`
//! annotations on the binary surface honour the same toggle.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(noyalib_coverage)");
    if std::env::var_os("NOYALIB_COVERAGE").is_some() {
        println!("cargo:rustc-cfg=noyalib_coverage");
    }
    println!("cargo:rerun-if-env-changed=NOYALIB_COVERAGE");
}
