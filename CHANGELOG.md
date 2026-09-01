<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Changelog

All notable changes to `noyalib-mcp` are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
and versions in lockstep with the
[`noyalib`](https://github.com/sebastienrousseau/noyalib) core crate —
see that repository's `CHANGELOG.md` for the release-wide notes.

## [Unreleased]

### Added

- **Dual-era protocol support — rebased on MCP 2026-07-28.** The
  server now implements the stateless revision alongside the
  handshake era: `server/discover` (a MUST, and the stdio
  backward-compatibility probe) advertises
  `supportedVersions: ["2026-07-28", "2025-06-18"]`; a per-request
  `_meta` `io.modelcontextprotocol/protocolVersion` is honoured,
  with an unsupported one answered by
  `UnsupportedProtocolVersionError` (`-32022`, carrying the
  `supported`/`requested` detail); every result is stamped
  `resultType: "complete"` plus the server's identity in `_meta`;
  and the cacheable results (`tools/list`, `prompts/list`,
  `resources/list`, `resources/templates/list`, `resources/read`)
  carry the required `ttlMs`/`cacheScope` fields. Legacy clients
  are unaffected: `initialize`, `notifications/initialized` and
  `ping` behave as before, and the extra result fields are ignored
  by 2025-06-18 hosts.

### Fixed

- **`initialize` now negotiates instead of dictating.** The reply
  hard-coded `protocolVersion: "2025-06-18"` and ignored the
  client's requested version; per the 2025-06-18 negotiation rules
  it now echoes a supported requested version and answers with
  `2025-06-18` otherwise.
- **The MCP conformance workflow never ran.** `mcp-inspect.yml`'s
  path filters named monorepo paths (`crates/noyalib-mcp/**`) that
  do not exist in this standalone repo, so no push or PR ever
  triggered it. Filters now match the real layout, and the
  workflow accepts `workflow_dispatch`.
- **Documentation drift.** README, `doc/tools-reference.md`,
  `doc/agent-integration.md` and the three `examples/*.sh`
  handshakes advertised `2024-11-05` — a revision the server never
  actually negotiated; the crate docs said "two tools" (there are
  three) and an MSRV of 1.75.0 (the manifest says 1.86.0);
  `glama.json` spelled the licence `Apache-2.0 OR MIT` where every
  other manifest says `MIT OR Apache-2.0`.

## [v0.0.28] - 2026-08-23

Lockstep release with the `noyalib` core. No changes in this crate; the
version moves so the `=0.0.28` pin resolves.

The core ships two correctness fixes around implicit nulls — inserting
over one appended a duplicate key, and a `:` at end of input was not
read as a value indicator. See the core's `CHANGELOG.md` for detail.

## [v0.0.27] - 2026-08-21

Lockstep release with `noyalib` 0.0.27. No behaviour change in this
crate, but the core carries one worth reading: only a **plain** `<<`
scalar is a merge key now — a quoted `"<<"`, and an alias resolving to
the string `<<`, are ordinary keys. A document relying on either
spelling to merge will stop merging, silently. See the core's
`CHANGELOG.md` for that and for @mathstuf's alias-resolution fix.

### Changed

- `noyalib` dependency pin `=0.0.26` -> `=0.0.27`, with both the
  `noyalib` and the self `cargo-vet` exemptions moved alongside it.
- Crate version -> 0.0.27.
- Lockfile refreshed against the published core; only `noyalib` moved.
- Server descriptors follow the bump: `server.json`, `glama.json` and the
  npm wrapper `package.json`, including the ghcr image tags.

## [v0.0.26] - 2026-08-20

Lockstep release with `noyalib` 0.0.26. No behaviour change in this
crate — see the core's `CHANGELOG.md` for @zoosky's wrapped-flow fix
(#294 / #296): a flow member alone on its line now takes the line with
it, so removing from a collection wrapped one member per line no longer
leaves a whitespace-only line behind.

### Changed

- `noyalib` dependency pin `=0.0.25` -> `=0.0.26`, with the matching
  `cargo-vet` exemption moved alongside it.
- Crate version -> 0.0.26.
- Lockfile refreshed against the published core; only `noyalib` moved.
- Server descriptors follow the bump: `server.json`, `glama.json` and the
  npm wrapper `package.json`, including the
  `ghcr.io/sebastienrousseau/noyalib-mcp` image tags.

## [v0.0.25] - 2026-08-20

Lockstep release with `noyalib` 0.0.25. No behaviour change in this
crate — see the core's `CHANGELOG.md` for the four CST editor fixes
contributed by @zoosky (#283, #285, #288, #290), `remove` refusing an
alias-valued entry instead of silently doing nothing, and the
differential-fuzz invariant correction.

### Changed

- `noyalib` dependency pin `=0.0.24` -> `=0.0.25`, with the matching
  `cargo-vet` exemption moved alongside it.
- Crate version -> 0.0.25.
- Lockfile refreshed against the published core; only `noyalib` moved.
- Server descriptors pinned to the release: `server.json`, `glama.json`
  and the npm wrapper `package.json`, including the
  `ghcr.io/sebastienrousseau/noyalib-mcp` image tags.

## [v0.0.24] - 2026-08-19

Lockstep release with `noyalib` 0.0.24. No behaviour change in this
crate — see the core's `CHANGELOG.md`: `remove` now takes a sole entry's
head comment with it (#280), plus a dependency consolidation.

### Changed

- `noyalib` dependency pin `=0.0.23` -> `=0.0.24`, with the matching
  `cargo-vet` exemption moved alongside it.
- Crate version -> 0.0.24.
- Lockfile refreshed against the published core; only `noyalib` moved.
- `server.json`, `glama.json` (version **and** ghcr image tag) and the npm
  wrapper follow the bump.

### Fixed

- Release assets now include the detached `.asc` signatures. The signing
  step produced them and `upload-artifact` carried them, but the
  `gh release create` call named every asset explicitly and omitted
  them, so they never reached the release. noyalib v0.0.24 shipped
  without signatures for this reason; the list is now a `nullglob`
  array, so the entries disappear when signing is skipped rather than
  failing the release.

## [v0.0.23] - 2026-08-16

Lockstep release with `noyalib` 0.0.23. No behaviour change in this
crate — see the core's `CHANGELOG.md` for what 0.0.23 carries: `remove`
extended to flow members and sole entries (closing #221), and
`swap_items` / `move_item` exchanging whole entries so comments travel
with the item they document (#269).

### Changed

- `noyalib` dependency pin `=0.0.22` -> `=0.0.23`, with the matching
  `cargo-vet` exemption moved alongside it.
- Crate version -> 0.0.23.
- Lockfile refreshed against the published core. Only `noyalib` moved —
  no new transitive dependencies, and no broad `cargo update`.

### Fixed

- Registry manifests and the npm wrapper follow the bump: `server.json`
  and `glama.json` carry both the version **and** the ghcr image tag, and
  `pkg/npm-wrapper/package.json` matches. The release `validate` job
  requires all three to agree with the tag.

## [v0.0.22] - 2026-08-13

Lockstep release with `noyalib` 0.0.22. No behaviour change in the server
itself — see the core's `CHANGELOG.md` for what 0.0.22 carries (CRLF-aware
CST splices, #261).

**On the version jump.** The published sequence for this crate goes
`0.0.18 → 0.0.22`. `0.0.19` was prepared on a release branch but never
tagged or published; `0.0.20` and `0.0.21` were core-only releases that
the satellites did not follow. Lockstep resumes here.

### Fixed

- **Registry manifests were internally inconsistent.** `server.json` and
  `glama.json` carried `"version": "0.0.18"` while the ghcr image they
  pointed at was still tagged `0.0.17` — the v0.0.18 manifest bump moved
  the version field and not the image reference. Anyone resolving the
  server through the MCP registry or Glama got the previous image. Both
  now read `0.0.22`, and the release `validate` job's requirement that all
  three agree with the tag is satisfied by construction.
- **`pkg/npm-wrapper/package.json` had drifted to `0.0.15`** — seven
  releases behind — so `npx @sebastienrousseau/noyalib-mcp` advertised a
  stale version. Now `0.0.22`.

### Changed

- `noyalib` dependency pin `=0.0.18` → `=0.0.22`, with the matching
  `cargo-vet` exemption moved alongside it.
- Crate version → 0.0.22.

### Security

- **SHA-pinned the last floating GitHub Actions.** `publish-mcp.yml` and
  `mcp-inspect.yml` were the only workflows here still referencing mutable
  tags (`actions/checkout@v7`, `docker/build-push-action@v7`, …), so a
  retagged action would have executed with ghcr push and `id-token`
  rights. All eight now pin to the same commit SHAs `release.yml` already
  used, so no new action versions are introduced. OpenSSF Scorecard
  *Pinned-Dependencies*.

  `dtolnay/rust-toolchain` resolves its toolchain from the ref name, so
  the SHA pin carries an explicit `toolchain: stable` — matching how
  `release.yml` already invokes it.

- **Narrowed workflow-level token scope.** `publish-mcp.yml` granted
  `id-token: write` and `packages: write` at the workflow level. Its single
  job re-declares both, so the top level is now `contents: read` and the
  elevated scopes no longer extend to any job added later. OpenSSF
  Scorecard *Token-Permissions*.

- Dropped the stale `RUSTSEC-2026-0173` ignore from `deny.toml`.
  `cargo-deny` reported it as `advisory-not-detected`: `proc-macro-error2`
  is not in this crate's graph on any platform, because it reaches
  `noyalib` only through the optional `validator` feature, which this
  crate does not enable.

---

## Earlier releases

This file starts at `v0.0.22`. `noyalib-mcp` split out of the `noyalib`
monorepo at **v0.0.13** ([ADR-0005](https://github.com/sebastienrousseau/noyalib/blob/main/doc/adr/0005-workspace-split.md))
and released `v0.0.13` through `v0.0.18` without a crate-local changelog.
Those releases are documented in:

- the core's [`CHANGELOG.md`](https://github.com/sebastienrousseau/noyalib/blob/main/CHANGELOG.md),
  which carries the release-wide notes for every lockstep version, and
- this repository's [releases](https://github.com/sebastienrousseau/noyalib-mcp/releases)
  and tags.

They are deliberately not backfilled here rather than reconstructed after
the fact.
