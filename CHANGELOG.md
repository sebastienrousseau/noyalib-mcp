<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Changelog

All notable changes to `noyalib-mcp` are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
and versions in lockstep with the
[`noyalib`](https://github.com/sebastienrousseau/noyalib) core crate —
see that repository's `CHANGELOG.md` for the release-wide notes.

## [v0.0.47] - 2026-09-21

### Changed

- Tracks `noyalib` 0.0.47 under the exact lockstep pin and aligns the server
  registry and container metadata with the release.

### Fixed

- Aligned MCP integration and YAML-suite fixtures with the core library's
  explicit single-document contract for `noyalib_get` and `noyalib_set`.

## [v0.0.46] - 2026-09-20

### Added

- **Three transports from one command line.** `noyalib-mcp` speaks
  stdio as before; `noyalib-mcp --transport streamable-http --host
  127.0.0.1 --port 8000` serves `/mcp`; `noyalib-mcp --transport sse
  --port 8001` serves the older HTTP+SSE transport at `/sse` and
  `/messages/`. `--version` and `--help` still do what they say. The
  transport layer is `src/transport.rs`, one file shared with the
  other Rust servers of the suite. See
  [ADR 0001](docs/adr/0001-three-transports-one-command-line.md).

- **Both current MCP revisions.** `2025-11-25` (`initialize`,
  `Mcp-Session-Id`) and `2026-07-28` (stateless, `server/discover`,
  per-request `_meta`, mirrored routing headers) on the one streamable
  HTTP endpoint, and over stdio. Older revisions back to `2024-11-05`
  are accepted from a client that asks for them.

- **Structured results and annotations.** Every tool declares an
  `outputSchema` and returns its answer as `structuredContent` beside
  the text; every tool carries behaviour annotations, and every
  argument an example. The library exposes the outputs as `GetOutput`,
  `SetOutput`, `SetMultidocOutput`, `ParseOutput`, `EditOutput` and
  `ValidateOutput`.

### Changed

- **Release and documentation baseline.** The crate and exact core dependency
  move to 0.0.46. The README now follows the ecosystem template, with the
  complete previous guide retained in `docs/README-REFERENCE.md`; comparison,
  benchmark-method, policy, and compliance pages join the manual.

- **The protocol is the official SDK's.** `rmcp` 3.4 replaces the
  hand-written JSON-RPC loop. The six tools are unchanged in name,
  arguments and descriptions; what changed is around them. A stdio
  client must now open with the handshake (or, in the stateless
  revision, name its protocol version in `_meta`) before its first
  request. A missing or mistyped argument is an `isError` result
  naming the field, rather than `-32602`. An unknown tool is an
  `isError` result naming the six that exist, for the same reason: the
  stateless HTTP revision carries `-32602` as an HTTP 400, which a
  model never reads. A file that cannot be read, YAML that does not
  parse and a path that does not exist are `isError` results too,
  carrying the same messages the error envelopes used to; the
  `noyalib://error-codes` resource now describes both kinds of
  failure.

- The crate depends on `rmcp`, `tokio`, `axum`, `schemars` and `uuid`.
  The minimum supported Rust version is **1.88**, the SDK's floor.

- `prompts/get` and `resources/read` are served through the SDK's
  routers; `resources/list` gains titles, and the tool descriptor
  resources mirror the live catalogue including the output schemas.

- The example drives a real session with the SDK's client over an
  in-memory pipe; the example scripts open with the handshake and call
  the tools by their real names.

### Removed

- `noyalib_mcp::handle_message`, `dispatch`, `error_str`,
  `HandleOutcome`, `Request`, `Response`, `ErrorResponse`,
  `ErrorObject`, `SUPPORTED_PROTOCOL_VERSIONS`,
  `LEGACY_PROTOCOL_VERSION`, `META_PROTOCOL_VERSION_KEY`,
  `UNSUPPORTED_PROTOCOL_VERSION`, and `tools::descriptors` /
  `tools::call`. The library is the six functions, their argument and
  output types, and `YamlServer`. The `fuzz_handle_message` and
  `fuzz_tool_call` targets are replaced by `tools`, which feeds the
  stateless functions directly.

## [v0.0.45] - 2026-09-18

### Changed

- Tracks `noyalib` 0.0.45. The core release fixes CST edit defects
  behind the `format` and `parse` operations this server exposes,
  including a leading comment anchoring on the wrong line. The fixes
  arrive via the version pin; no source change.

  See the core crate's `CHANGELOG.md` for the full release notes.

## [v0.0.44] - 2026-09-17

### Changed

- Tracks `noyalib` 0.0.44. The core release fixes a serializer defect
  that moved a struct's fields up a level when it was wrapped in
  `SpaceAfter` or `Commented`, producing valid YAML that meant something
  else — worth having in a server that writes YAML back to agents. The
  fix arrives via the version pin; no source change here.

  See the core crate's `CHANGELOG.md` for the full release notes.


## [v0.0.43] - 2026-09-08

### Changed

- Lockstep release with noyalib 0.0.43: a hardening pass on the
  VS Code extension publish step. No local code change unless listed
  below.

## [v0.0.42] - 2026-09-08

### Changed

- Lockstep release with noyalib 0.0.42: the VS Code extension
  gains a publish path. No local code change unless listed below.

## [v0.0.41] - 2026-09-07

### Changed

- Lockstep release with noyalib 0.0.41: the September GitHub
  Actions bumps, cherry-picked from Dependabot with authorship intact.
  No local code change unless listed below.

## [v0.0.40] - 2026-09-07

### Changed

- Lockstep release with noyalib 0.0.40: this crate is now
  registered on bestpractices.dev and passing at 100%, and the README
  carries its badge. No local code change unless listed below.

## [v0.0.39] - 2026-09-07

### Changed

- Lockstep release with noyalib 0.0.39: fifteen spec-torture
  documents and the three parser defects they found. No local code
  change unless listed below.

## [v0.0.38] - 2026-09-06

### Added

- **The ultra-complex fixture through `noyalib_parse`**
  (`tests/fixtures/ultra-complex/`) projects onto exactly its expected
  JSON. The README names and links the official yaml-test-suite.

### Changed

- Lockstep release with noyalib 0.0.38: the Scorecard pinned-dependency
  fixes across the family. No local code change unless listed below.

### Fixed

- **The release no longer runs `npm install -g npm@latest`.** It asserts
  that the runner's npm already supports OIDC trusted publishing
  (npm 11.5.1 or newer) and fails otherwise. Scorecard reported the
  upgrade as an unpinned dependency (alert #14).

## [v0.0.37] - 2026-09-06

### Changed

- Lockstep release with noyalib 0.0.37: the line opened for the
  noyalib-wasm npm package gate repair (its 0.0.36 npm publish failed
  inside the gate under npm 12). No local code change unless listed
  below.

## [v0.0.36] - 2026-09-06

### Changed

- Lockstep release with noyalib 0.0.36: stream parse errors located in
  the stream (core #408) and jsonschema 0.53 (core #405). No local code
  change unless listed below.

## [v0.0.35] - 2026-09-06

### Changed

- Lockstep release with noyalib 0.0.35: the 10/10 programme (formal
  budget proofs, a wasip2 build, scorecard hardening, the cookbook) and
  the family gaps closed in this cycle. No local code change unless
  listed below.

### Added

- Three stateless tools that take content in the request and touch
  nothing on disk, the shape the 2026-07-28 MCP specification's
  stateless deployments want: `noyalib_parse` (YAML text to its JSON
  data model, streams as arrays), `noyalib_edit` (set a value in the
  given text losslessly and return the whole text) and
  `noyalib_validate` (parse check with line and column, or JSON Schema
  violations with their paths). The file-bound tools are unchanged.

## [v0.0.34] - 2026-09-05

### Changed

- Lockstep release with noyalib 0.0.34: property tests for the emitter
  and path grammar, structure-aware and alloc-only fuzzing, the
  `arbitrary` feature, and an unterminated-verbatim-tag parser fix
  (core #396). No local code change unless listed below.
- yaml-test-suite conformance gate: `tests/yaml_test_suite.rs` drives all
  406 official cases through this crate's own entry point and CI runs it
  via the family's shared `yaml-test-suite` workflow, so the surface
  cannot drift from the core (which passes 406/406).

### Fixed

- `noyalib_get` on a key whose value is empty (`key:` with nothing
  after it, an implicit null) returns the empty source slice instead
  of a "path not found" error; a missing key still errors. Found by
  driving the yaml-test-suite through the server (case 7W2P).

## [v0.0.33] - 2026-09-05

### Changed

- Repository hygiene for the family standard: the community files
  (code of conduct, governance, support, agent invariants, citation),
  `docs/ARCHITECTURE.md`, a rendered manual deployed to Pages, and a
  seed corpus replayed by CI on every push for the fuzz targets.
- Lockstep release with noyalib 0.0.33: bracket-quoted path segments
  (core #389), located duplicate-key errors (core #393), and serializer
  fixes for tag-like keys, non-printable characters, and block scalars
  (core #381, #391, #392). No local code change.

## [v0.0.32] - 2026-09-03

### Changed

- Lockstep release with noyalib 0.0.32: block sequence spans report
  their full extent (core #375). No local behaviour change.

## [v0.0.31] - 2026-09-03

### Changed

- **Repository layout, Phase 1 of the family structure plan**:
  `doc/` renamed to `docs/`, `DEVELOPMENT.md` added as the developer
  entry point, `.editorconfig` / `.markdownlint.yaml` /
  `.codespellrc` land with a per-push `docs-lint` CI gate consuming
  the core repo's shared-docs-lint.yml.

## [v0.0.30] - 2026-09-02

### Changed

- Lockstep release with noyalib 0.0.30 (exact serde_yaml location
  parity: tagged/anchored node spans anchor at their properties;
  the `custom-explicit-tag` contract case now pins `1:8:7`). No
  satellite-local changes.

## [v0.0.29] - 2026-09-01

### Added

- **CycloneDX SBOM in the release pipeline** (mirrors the core
  repo). Releases now emit a machine-readable CycloneDX 1.5
  `SBOM.cdx.json` — attested, sigstore-signed, optionally
  GPG-signed, and attached to the GitHub Release — alongside the
  human-readable `SBOM.txt`, which was never a machine-readable
  SBOM format.

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

- **A GPG-less release could not publish.** The release asset list
  relied on `nullglob` to drop the `.asc` entries when GPG signing
  is skipped, but `artifacts/SBOM.txt.asc` was a literal path, so
  `gh release create` failed on the missing file for any fork
  without the signing key. Real globs now (mirrors the core fix).

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
- **Documentation drift.** README, `docs/tools-reference.md`,
  `docs/agent-integration.md` and the three `examples/*.sh`
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
monorepo at **v0.0.13** ([ADR-0005](https://github.com/sebastienrousseau/noyalib/blob/main/docs/adr/0005-workspace-split.md))
and released `v0.0.13` through `v0.0.18` without a crate-local changelog.
Those releases are documented in:

- the core's [`CHANGELOG.md`](https://github.com/sebastienrousseau/noyalib/blob/main/CHANGELOG.md),
  which carries the release-wide notes for every lockstep version, and
- this repository's [releases](https://github.com/sebastienrousseau/noyalib-mcp/releases)
  and tags.

They are deliberately not backfilled here rather than reconstructed after
the fact.
