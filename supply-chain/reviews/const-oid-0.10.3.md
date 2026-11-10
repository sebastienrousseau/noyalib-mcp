<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Registry const-oid 0.10.3 source review

Reviewed 2026-11-10. The official registry archive SHA-256 is
`be95d4a884358a21a199689323c29e9256b4a1b315e22f183d155422041d3a9a`.
Its VCS revision is `3c35e972cd4bbaf72c947364fdb964f8f95c3e9a`.
The full reviewed manifest/source inventory is `const-oid-0.10.3.json`.

Every runtime Rust file matches the previously reviewed fixed Git source
`399ec598b509a728eb13f4e82be0a2526137e81f`. The only Rust-file difference is
a documentation link in src/db.rs. Cargo.toml changes the package version, description and excludes the
OID database generator from the archive. No runtime
dependency, feature, build script or ambient capability is added.

The checked arc accumulator rejects values above u32::MAX. The full
source review includes constructors, encoder bounds, borrowed reference
validation, the transparent slice cast and lifetime, parser arithmetic,
buffer invariants, database iteration and optional arbitrary support.
No new unsafe block is introduced. The scope and limitations of the
original full fixed-source review are retained in const-oid-interim.md.

The original over-u32 five-byte and six-byte inputs fail on registry
0.10.2 and pass on the reviewed fixed source. The 0.10.3 graph keeps those
inputs and the maximum representable arc boundary in its regression crate.
The source gate verifies the actual registry version, archive checksum and
all resolved source hashes in the production/fuzz/regression graphs.

This full audit certifies registry 0.10.3 only. The faulty 0.10.2 remains
uncertified. All Git overrides and their temporary policy/allowlist are
removed; existing trust, exemptions and audit criteria are unchanged.
No interim distribution was published.
