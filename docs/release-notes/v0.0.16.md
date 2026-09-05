<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# noyalib-mcp v0.0.16 Release Notes

Lockstep release with `noyalib` v0.0.16 (ADR-0005 strict-lockstep: the
MCP server always publishes `=X.Y.Z` pinned to the core). No wire-format
or tool-inventory change to the MCP surface.

## What changed

### Security

- **`crossbeam-epoch` bumped to 0.9.20 (RUSTSEC-2026-0204).** Closes an
  invalid-pointer-dereference advisory in the `fmt::Pointer` impl for
  `Atomic`/`Shared`, previously locked at a vulnerable version through a
  transitive dependency.

### Dependencies

- **`noyalib` pin `=0.0.15` → `=0.0.16`** and the crate version bumped in
  lockstep.
- **MSRV raised 1.85.0 → 1.86.0**, matching the single lockstep floor the
  core adopted in v0.0.16. The crate builds cleanly on 1.86.

### Docs / registry manifests

- README **Tools** section reformatted as a bullet list.
- `server.json` and `glama.json` (MCP-registry manifests) aligned to
  `0.0.16` — version field and the `ghcr.io/...:0.0.16` image tags. The
  release `Validate` job enforces that these match the tag.

## Engineering / CI (no user-facing change)

- **Signed-history enforcement.** Commits above the last release were
  re-signed and the branch-protection ruleset split so signatures are
  required with no bypass, while signed release pushes still work.
- **Supply-chain hardening.** Imported the upstream audit sets the core
  uses (mozilla, google, et al.), so most dependency bumps no longer
  churn `cargo-vet`; a `dependabot-vet` workflow auto-refreshes the
  remaining exemptions.
- **New CI gates** brought to parity with the core: a coverage gate
  (measured 95.7 % regions / 98.3 % functions / 94.4 % lines, enforced
  against the 93/96/94 floor), an MSRV gate verifying the 1.86 build,
  CodeQL, and OpenSSF Scorecard.

## What did not change

- The MCP tool inventory, JSON-RPC wire format, and stdio transport.
- `#![forbid(unsafe_code)]` — intact.
- Public API of the `noyalib_mcp` library.

## Upgrading

Drop-in for most users:

```toml
noyalib-mcp = "0.0.16"
```

You must be on **Rust 1.86.0+** (or stay on v0.0.15). The container image
is `ghcr.io/sebastienrousseau/noyalib-mcp:0.0.16`.
