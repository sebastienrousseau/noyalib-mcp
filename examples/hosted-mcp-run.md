<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Hosting `noyalib-mcp` on a remote MCP runtime

The MCP spec is host-agnostic — any runtime that can spawn a
binary and broker its stdin/stdout to an LLM client works.
This document is a recipe-style guide for the most common
hosted options.

> The MCP spec is at <https://modelcontextprotocol.io>. The
> reference TypeScript SDK lives at
> <https://github.com/modelcontextprotocol/typescript-sdk>.

## Option A — `mcp.run` (third-party hosted MCP gateway)

[`mcp.run`](https://www.mcp.run) packages MCP servers as
deployable, signed WebAssembly bundles called *servlets*. As
of 2026-05-08, `noyalib-mcp` is *not* yet on the official mcp.run
catalog (it ships as a native binary, not WASM). Two paths to
adopt anyway:

### A.1. Self-host the native binary alongside an mcp.run gateway

If your team uses mcp.run as the gateway / discovery layer but
allows native MCP servers behind it, run `noyalib-mcp` as a
sidecar:

```yaml
# Your gateway config (mcp.run-style — adjust to your gateway's
# actual schema)
servers:
  noyalib:
    command: noyalib-mcp
    args: []
    transport: stdio
    capabilities: [tools]
```

### A.2. Wait for / contribute a WASM port

`noyalib-mcp` *could* run on `wasm32-wasip1` — the
JSON-RPC over stdio surface is small enough to port. The
limiting factor is `std::fs` access on `wasm32-wasip1`, which
WASI `preview-1` does support but mcp.run's sandbox model
needs to allow. PR welcome at
<https://github.com/sebastienrousseau/noyalib/issues>.

## Option B — Generic stdio-over-process MCP brokers

Several open-source brokers wrap stdio MCP servers in
HTTP / WebSocket / Server-Sent-Event (SSE) envelopes for remote
clients:

- [`mcp-proxy`](https://github.com/sparfenyuk/mcp-proxy) —
  Python-based, exposes any stdio MCP server over SSE.

  ```bash
  pip install mcp-proxy
  mcp-proxy --port 8080 -- noyalib-mcp
  ```

  Then point your MCP-over-HTTP client at
  `http://localhost:8080/sse`.

- [`mcp-gateway`](https://github.com/lasso-security/mcp-gateway)
  — Go-based, adds OAuth / API-key auth in front of stdio MCP
  servers.

## Option C — Docker / Kubernetes deployment

For team-internal hosted use:

```dockerfile
# Dockerfile
FROM rust:1.86-slim AS build
WORKDIR /src
COPY . .
RUN cargo install --path crates/noyalib-mcp --locked

FROM debian:bookworm-slim
COPY --from=build /usr/local/cargo/bin/noyalib-mcp /usr/local/bin/
ENTRYPOINT ["noyalib-mcp"]
```

Wrap in a stdio-over-HTTP broker (Option B) for client access.
Note: `noyalib-mcp` on its own is **stdio-only** — it does
*not* speak the WebSocket / SSE / HTTP variants of the MCP
transport directly. Use a broker to expose it over the wire.

## Authentication & hardening

`noyalib-mcp` itself has no authentication layer — it trusts
any caller on its stdin. For production hosted use:

1. **Network**: bind brokers to localhost only, or run them
   behind an authenticated reverse proxy.
2. **Filesystem scope**: `noyalib_set` writes to whatever path
   the caller asks for. Restrict the working directory of the
   server process via container filesystem mounts or systemd
   `ReadWritePaths=`.
3. **Resource limits**: large YAML inputs are bounded by
   `noyalib`'s `max_document_length` / `max_alias_expansions`
   defaults, but the underlying file system reads are
   unbounded — wrap the server with `ulimit -f` or container
   memory limits to defend against unbounded input.

## Verifying the install

The host-level smoke test is the same as for local install:

```bash
crates/noyalib-mcp/examples/handshake.sh
```

…executed via the broker / gateway / orchestrator instead of
directly. If `tools/list` returns the expected `noyalib_get` /
`noyalib_set` pair, the server is reachable end-to-end.
