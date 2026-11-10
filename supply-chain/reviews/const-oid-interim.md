<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Exact source review for interim MCP builds

Reviewed 2026-11-10. Git repository:
<https://github.com/RustCrypto/formats.git>. Exact revision:
`399ec598b509a728eb13f4e82be0a2526137e81f`, the merged upstream fix in
[RustCrypto/formats#2412](https://github.com/RustCrypto/formats/pull/2412).
Credit: Jaideep Pyne, upstream fix and regression test author.

## Scope and source identity

The reviewed source inventory is `const-oid-interim.json`. It includes
Cargo.toml, README.md and every file beneath src. Cargo resolves exactly
this revision in the production, fuzz and regression graphs. Before build,
the source gate checks those locks, the resolved Cargo metadata and every
source hash, rejects extra Git dependencies, and runs the original inputs.

Compared against the independently authenticated registry 0.10.2 archive,
only arcs.rs and the generated OID database change in runtime Rust. The
byte decoder now uses checked multiplication/addition and returns
ArcTooBig on overflow. This preserves the maximum representable u32 arc
and removes truncation. Generated database changes add two RFC6211 names
for the same static identifier; no executable I/O or build script is
added. No dependency or feature is added to the const-oid manifest.

The full fixed source was read, including constructors, checked arithmetic,
encoder bounds, borrowed reference validation, the transparent slice cast,
parser overflow paths, buffer invariants, database iteration and optional
arbitrary support. The existing unsafe cast preserves slice layout and
lifetime; no new unsafe block is introduced. This is a source review and
regression evidence, not a formal proof of every OID representation.

Two regression tests fail on the published source: over-u32 five-byte arcs
and a six-byte arc silently decode to 1.2.0. Both pass on this exact Git
revision; a third round-trip test covers representable arc boundaries,
including u32::MAX. These tests use the same resolved source as MCP.

## Registry audit boundaries

Published const-oid 0.10.2 remains uncertified. Cargo-vet cannot represent a
full Git-source audit without certifying the published base. Only the exact
Git revision is delegated to this explicit source-review/hash/regression
gate. Existing registry audit criteria, trust records and exemptions are
retained. Normal release gates still reject patch sections and Git
allowlists, preventing accidental crates.io publication of another graph.

Current main's SDK baseline is 3.7.0; its macros baseline is 3.8.0. Their
registry archives were independently authenticated before reviewing their
deltas to 3.9.0. Runtime SDK additions are already covered by the prior
full 3.5.1-to-3.9.0 review. Four extra added lines in the narrower delta
only set up and clean up test sessions. Macros Rust source is identical.
Separate delta records use these actual retained main baselines, without
adding back obsolete exemptions.

## Observed validation

- Exact Git source: 32 upstream tests passed on Rust 1.89.0.
- Bare-metal no_std build: aarch64-unknown-none passed on Rust 1.89.0.
- Overflow harness: two failures and one pass on registry 0.10.2; all
  three pass on the exact fixed revision.
- Source gate acceptance tests: six passed, including wrong revision,
  unexpected Git dependency, changed bytes and extra source rejection.
- MCP all-features test suite: 124 passed, no failures or ignored tests.
