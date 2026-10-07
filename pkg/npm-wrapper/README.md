<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# noyalib-mcp (npm wrapper)

Run the `noyalib-mcp` Model Context Protocol server from any
machine with Node.js — no Rust toolchain required.

```bash
npx noyalib-mcp                       # one-shot, downloads on first run
# or
npm install -g noyalib-mcp            # global install for repeat use
noyalib-mcp                           # spawns the MCP server over stdio
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
