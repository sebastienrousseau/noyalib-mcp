<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# ADR-0001: Interim MCP builds from reviewed const-oid Git source

Status: Superseded by the fixed registry dependency, 2026-11-10.

## Context

The published const-oid 0.10.2 parser truncates over-u32 BER arcs. The fix
is merged upstream at `399ec598b509a728eb13f4e82be0a2526137e81f` but has not
been published. MCP reaches const-oid through the SDK's SHA-2 dependency.
Copying a function into MCP would leave that transitive code unchanged.
Cargo strips patch overrides from published packages.

## Decision

Pin only const-oid to the exact merged upstream revision for source builds
and separately labeled interim binaries and container images. Continue
using the exact published core dependency and Rust 1.89.0. Normal registry
release gates keep rejecting Git patches. Interim tags use the existing
crate version plus `-interim.1`; no crates.io, npm or MCP registry publish
runs, and the stable container latest tag is untouched.

The fixed Git source receives a full manual source review recorded in
`supply-chain/reviews/const-oid-interim.md`. Before build or merge,
`scripts/verify-interim-source.py` verifies the resolved revision and every
reviewed manifest/source file, rejects unexpected Git dependencies, checks
all three locks and runs the original regression inputs. Registry
libraries remain under existing cargo-vet trust and exemption policy.

Cargo-vet requires a published-base audit before a Git delta audit. This
would incorrectly certify the unfixed registry source. An exact-revision
policy therefore delegates only this Git source to the explicit source
review and hash/regression gate. It does not certify or exempt registry
0.10.2, apply to another revision, or broaden existing exemptions/trust.

## Removal

After a compatible fixed registry release exists, review its archive,
remove the overrides, exact Git policy, Git allowlist and interim source
check, update all affected locks, and rerun normal release gates. Preserve
the immutable interim tags, provenance and public review evidence.

## Outcome

const-oid 0.10.3 was published on 2026-11-10 and its authenticated source
contains the reviewed fix. No interim tag or release was published. The
Git override, exact-revision policy and interim publisher were removed
before the normal release, and a full 0.10.3 registry audit was recorded.
Existing registry trust and exemptions remain unchanged.
