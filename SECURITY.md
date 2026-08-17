# Security Policy

## Supported Versions

| Version | Supported |
|:--------|:---------:|
| 0.0.x   | Yes       |

`noyalib-mcp` follows the [ADR-0005 strict-lockstep versioning
contract](https://github.com/sebastienrousseau/noyalib/blob/main/doc/adr/0005-workspace-split.md).
Every release of this satellite is coordinated with a release of
the parent `noyalib` crate at the same version, published from
[`sebastienrousseau/noyalib`](https://github.com/sebastienrousseau/noyalib).

## Reporting a Vulnerability

Report security vulnerabilities by emailing **sebastian.rousseau@gmail.com**.

Do not open a public issue for security reports.

Include:

- A description of the vulnerability.
- Steps to reproduce (a minimal MCP client dialogue is ideal).
- Affected versions.
- Any suggested fix (optional).

Expect an initial response within 48 hours. A fix or mitigation
plan will follow within 7 days of confirmation.

Vulnerabilities affecting the underlying `noyalib` YAML engine
should be reported through the same channel; the coordinated
patch will land in both repositories simultaneously.

## Threat Model — MCP Server Specific

`noyalib-mcp` speaks JSON-RPC 2.0 over stdio (per the Model
Context Protocol specification). The threat model:

- **Untrusted YAML input via `tools/call`**: every `parse` /
  `format` / `validate` tool invocation feeds the argument's
  YAML through the same parser hardening path as the library
  crate. `max_depth`, `max_document_length`, `max_alias_expansions`,
  `max_mapping_keys`, `max_sequence_length` all apply. AI agents
  that pipe user-attacker-controlled YAML into this server are
  bounded by those limits.
- **Untrusted JSON-RPC frames**: message-length caps enforced
  before deserialisation to prevent memory exhaustion via
  outsized `params` blobs.
- **Subprocess model**: `noyalib-mcp` runs as a child of the
  MCP client (Claude Desktop, Cursor, Continue.dev, Zed). It
  never opens listening sockets, never accepts network
  connections, never writes outside stdout / stderr. `#[forbid(unsafe_code)]`
  workspace-wide.
- **Tool inventory stability**: the exposed `tools/list` output
  is treated as a public API surface. Removing or renaming a
  tool is a breaking change and requires a major-version bump.
  A schema-diff regression in CI blocks silent removals.

## Security Design

`noyalib-mcp` inherits every security invariant from the parent
`noyalib` crate:

- `#![forbid(unsafe_code)]` workspace-wide.
- No C dependencies, no FFI calls.
- Every parser DoS guard from `noyalib`'s
  [Parser Hardening section](https://github.com/sebastienrousseau/noyalib/blob/main/SECURITY.md#parser-hardening)
  applies here transparently.

## Supply Chain

- Rust dependencies audited (`cargo-deny` in CI): license
  validation, RustSec advisory checks, source verification.
- All GitHub Actions SHA-pinned. CI itself is composed from
  `sebastienrousseau/noyalib`'s shared reusable workflows,
  pinned by SHA; a hardening pass in the parent repo reaches
  this satellite within 48 hours via Dependabot per the
  [ADR-0005 propagation SLA](https://github.com/sebastienrousseau/noyalib/blob/main/scripts/shared-workflow-propagation-monitor.sh).
- `Cargo.lock` committed for deterministic builds.

## Build Provenance & Artefact Signing

`noyalib-mcp` releases publish across four channels; every one
carries verifiable provenance:

1. **crates.io** — SLSA Level 3 build provenance via
   `actions/attest-build-provenance` + keyless sigstore
   signatures on the `.crate` artefact.
2. **npm** (`@sebastienrousseau/noyalib-mcp` wrapper) — Trusted
   Publishing + `--provenance` attestation.
3. **GHCR** (`ghcr.io/sebastienrousseau/noyalib-mcp`) — cosign
   keyless-signed multi-arch container images.
4. **MCP Registry** — OCI-based registration via
   `registry.modelcontextprotocol.io`, tying the registry entry
   to the signed GHCR image.

Software bill of materials (SBOM) attached to each GitHub Release.

### Detached GPG signatures

Additive to the sigstore signing above, not a replacement. Keyless
signing is the stronger primitive — nothing long-lived to steal, and
every signature publicly logged in Rekor — but verifying it needs
`cosign` and network access. A detached `.asc` can be checked with the
`gpg` any distribution already ships, offline, which is what package
maintainers and air-gapped consumers ask for.

Every `.crate` and the SBOM in a release carry a matching `.asc`:

```sh
# Import the release-signing key (also in KEYS.asc at the repo root)
gpg --recv-keys 4B7F16C909C7A8EE9BED338A4F047EDF5F90F638

gpg --verify <artefact>.asc <artefact>
```

**Release-signing key fingerprint:**

```text
4B7F16C909C7A8EE9BED338A4F047EDF5F90F638
```

Signing key `Sebastien Rousseau <sebastian.rousseau@gmail.com>`,
ed25519, signing-only, expires 2028-08-16. Verify the fingerprint out of
band before trusting it — a key fetched over the same channel as the
artefact proves nothing on its own. The sigstore bundle needs no such
step, which is why it remains the recommended check.

## Commit Integrity

Every commit on `main` must be signed. CI rejects unsigned pull
request commits via the shared `shared-verify-signatures.yml`
workflow from `noyalib`.
