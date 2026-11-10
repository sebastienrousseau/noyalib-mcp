<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# noyalib-mcp (npm wrapper)

Run the `noyalib-mcp` Model Context Protocol server from any
supported platform with Node.js 18 or later. The wrapper does not need a Rust toolchain.

```bash
npx @sebastienrousseau/noyalib-mcp          # downloads on first run
# or
npm install -g @sebastienrousseau/noyalib-mcp
noyalib-mcp                                 # runs over stdio
```

The wrapper downloads the platform-appropriate `noyalib-mcp`
archive from the matching GitHub Release on first run, checks it
against the SHA-256 recorded in the package's `digests.json`, unpacks
it, checks the binary, and caches it under
`~/.cache/noyalib-mcp/<version>/`. The cached binary is checked again
before every run. Downloads are https only and follow redirects only
to GitHub's release hosts.

It fails closed: when the package carries no digest for your platform
it downloads and runs nothing, and says so. Releases up to and
including 0.0.54 ship no per-platform archives and no `digests.json`,
so for those use `cargo install noyalib-mcp --locked` or the container
`ghcr.io/sebastienrousseau/noyalib-mcp`.

## Offline use and recovery

Prebuilt binaries cover Linux x64 and ARM64, macOS x64 and ARM64, and
Windows x64. Warm the cache on the same platform and with the same package
version before going offline:

```bash
npx @sebastienrousseau/noyalib-mcp --version
```

If the wrapper reports a corrupt cached binary, remove only the reported
file, then rerun the command with network access. The wrapper verifies the
replacement against the digest in the installed package. A missing cache
cannot be populated offline without a separately verified release archive.
Use the version-specific GitHub release or another documented install channel
if the download service is unavailable.

## Why a wrapper?

The MCP server is the bridge between AI agents (Claude Code,
GitHub Copilot, etc.) and noyalib's YAML tools. Most AI-agent
deployments don't ship a Rust toolchain; npm is universally
available wherever Node is, and `npx` lets agents invoke the
server with no install step.

## Verifying the downloaded binary

The wrapper does this itself on every run. `digests.json` lists, per
target, the archive name, the archive's SHA-256 and the binary's
SHA-256; it is part of the npm package, so its provenance attestation
covers it (`npm audit signatures`).

## License

Dual-licensed under MIT or Apache-2.0, at your option.
