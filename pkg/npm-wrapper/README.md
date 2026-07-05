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
binary from the matching GitHub Release on first run, caches it
under `~/.cache/noyalib-mcp/<version>/`, then `exec`'s into it.
Subsequent invocations reuse the cached binary.

## Why a wrapper?

The MCP server is the bridge between AI agents (Claude Code,
GitHub Copilot, etc.) and noyalib's YAML tools. Most AI-agent
deployments don't ship a Rust toolchain; npm is universally
available wherever Node is, and `npx` lets agents invoke the
server with no install step.

## Verifying the downloaded binary

The download URL is over HTTPS to `github.com`, and the binary
inside the archive is signed with cosign keyless. To verify by
hand before trusting the cached copy:

```bash
cosign verify-blob \
    --certificate-identity-regexp 'https://github.com/sebastienrousseau/noyalib/' \
    --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
    --certificate ~/.cache/noyalib-mcp/<version>/noyalib-mcp.pem \
    --signature   ~/.cache/noyalib-mcp/<version>/noyalib-mcp.sig \
    ~/.cache/noyalib-mcp/<version>/noyalib-mcp
```

See [`pkg/VERIFY.md`](../VERIFY.md) for the full cookbook.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
