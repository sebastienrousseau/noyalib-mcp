# Security Policy

## Supported Versions

| Version | Supported |
|:--------|:---------:|
| 0.0.x   | Yes       |

`noyalib-mcp` follows the [ADR-0005 strict-lockstep versioning
contract](https://github.com/sebastienrousseau/noyalib/blob/main/docs/adr/0005-workspace-split.md).
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

## Threat Model and Trust Boundaries

`noyalib-mcp` is a Model Context Protocol server. Whoever can send it
JSON-RPC can call every tool; the server has no notion of users and
does not authenticate. What it guarantees is where the tools can reach
and how much one request can cost.

### Transports

- **stdio (the default).** The server is a child process of the MCP
  client (Claude Desktop, Cursor, Zed and the like). The client that
  holds its stdin is the only caller. No socket is opened.
- **HTTP (`--transport streamable-http` or `--transport sse`), opt-in.**
  The server listens on a TCP port, `127.0.0.1` unless `--host` says
  otherwise. Any process on the machine that can reach the port,
  including other users' processes, can call every tool. To keep a
  page in a browser out (DNS rebinding), every request must carry a
  `Host` the server answers to (the loopback names, or the `--host`
  name), any `Origin` must be a loopback or `--host` origin, and the
  SSE message endpoint takes `application/json` only. Binding
  `0.0.0.0` or `::` turns the `Host` check off; put a gateway that
  authenticates in front before binding a routable address. At most
  `--max-sessions` sessions (64 by default) are held at once.

### File tools and the root

- `noyalib_get`, `noyalib_set` and `noyalib_set_multidoc` are confined
  to one directory, `--root`, or the working directory when it is not
  given. The server refuses to start without `--root` when the working
  directory is `/` or the home directory.
- On unix the root is opened once and every path is walked from that
  handle one component at a time with `O_NOFOLLOW`. A symlink met on
  the way is read and followed only while it stays inside the root
  (a relative target, or an absolute one under the root); one that
  leaves is refused before anything outside is looked at. The file is
  read and replaced relative to the directory the walk ended in, so a
  directory swapped for a symlink mid-request cannot redirect it. On
  Windows the path is canonicalised and compared with the root, which
  leaves a window between the check and the open.
- An absolute `file` argument is accepted only when it starts with the
  root as given or as canonicalised. A path outside the root draws one
  message whether it exists or not, and no message names the root.
- Only regular files are read (a FIFO or device is refused without
  blocking), and only up to the profile's document limit, checked
  before reading.
- A rewrite goes to a new file created exclusively under a random name
  beside the target, given the target's permission bits (and owner and
  group, where the process may set them) before any byte is written,
  synced, then renamed over the target. Extended attributes and ACLs
  are not carried over, and the file gets a new inode.

What a client can still do inside the root, by design:

- read any regular file that parses as YAML, which includes JSON
  (`package.json`, `tsconfig.json`), and rewrite any value in it, for
  example a `scripts` entry or a CI step. Choose the root accordingly;
- reach a file outside the root through a hard link inside it: a hard
  link is the file itself, and no path check can tell. Do not let
  untrusted parties create links in the root.

### Untrusted input

- Every tool parses under noyalib's strict YAML 1.2 profile unless
  `--profile standard` is given: duplicate keys are errors and the
  parser's limits for untrusted input apply (`max_depth` 64,
  `max_document_length` 1 MiB, alias, key, sequence and node budgets).
  See the parent crate's
  [Parser Hardening section](https://github.com/sebastienrousseau/noyalib/blob/main/SECURITY.md#parser-hardening).
- YAML text in a request is refused over the document limit before it
  is parsed. A replacement value is refused over 256 KiB, or when a
  linear count of its nesting exceeds `max_depth`, before it is parsed.
- `noyalib_validate` refuses a schema over 64 KiB and lists at most 100
  violations. Remote `$ref`s are not fetched.
- Every tool call runs on a blocking thread under a 30 second limit.
  Past it the client is answered with an error at once; the thread
  cannot be interrupted and finishes in the background.
- There is no JSON-RPC message-size cap of the server's own. A stdio
  line is read whole, and an HTTP body is bounded by the HTTP stack's
  defaults (2 MB on the SSE message endpoint).
- Error messages repeat at most 120 bytes of a client's path or value.

### Tool inventory stability

The `tools/list` output is a public API surface. Removing or renaming
a tool is a breaking change, held to a 0.x bump while the crate is
pre-1.0 (see the crate documentation).

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

### Verifying a release

Pin the workflow and the tag, not just the repository: an attestation
or signature from any other workflow, or from a branch, must not pass.

```sh
# SLSA provenance (any release asset)
gh attestation verify <artefact> \
  --repo sebastienrousseau/noyalib-mcp \
  --signer-workflow sebastienrousseau/noyalib-mcp/.github/workflows/release.yml \
  --source-ref refs/tags/vX.Y.Z \
  --deny-self-hosted-runners

# Keyless sigstore signature (.crate and SBOM, with its .bundle)
cosign verify-blob \
  --certificate-identity-regexp '^https://github\.com/sebastienrousseau/noyalib-mcp/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --bundle <artefact>.bundle <artefact>
```

The GHCR image and its MCP Registry entry come only from
`release.yml` (the separate `publish-mcp.yml`, which pushed unsigned
images first, was removed in v0.0.56). Verify the image by digest or
tag:

```sh
gh attestation verify oci://ghcr.io/sebastienrousseau/noyalib-mcp:X.Y.Z \
  --repo sebastienrousseau/noyalib-mcp \
  --signer-workflow sebastienrousseau/noyalib-mcp/.github/workflows/release.yml \
  --source-ref refs/tags/vX.Y.Z
```

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
