<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# 0001. Serve stdio, streamable HTTP and SSE from one command line

- **Status:** Accepted
- **Date:** 2026-09-19
- **Deciders:** maintainer

## Context

Until v0.0.45 the server was a hand-written JSON-RPC loop over stdio,
speaking two revisions it implemented itself: the 2025-06-18
`initialize` handshake and a dual-era reading of 2026-07-28
(per-request `_meta`, `server/discover`). A client spawned the
process; nothing listened. That fits a developer's laptop. It does not
fit a shared deployment, a gateway that fans one server out to many
agents, or an auditor that speaks HTTP.

Keeping up with the specification by hand is a protocol project, not a
YAML one — and the hand-written loop had already drifted: it answered
`initialize` with 2025-06-18 while the current handshake revision is
2025-11-25, it had no session layer, and it could not have served the
mirrored routing headers (SEP-2243) that 2026-07-28 requires over
HTTP. The official Rust SDK, `rmcp`, implements all of it, is
maintained by the people who write the specification, and is what the
sibling Rust servers of the suite are moving to.

## Options considered

1. Stay on stdio and the hand-written loop, and leave remote use to a
   wrapper process.
2. Extend the hand-written loop with HTTP, sessions and both current
   revisions.
3. Adopt `rmcp` for the protocol, keep the six tools as plain
   functions, and put the transport dispatch in one module copied
   verbatim into every Rust server of the suite.

## Decision

Option 3. `noyalib-mcp` runs stdio; `noyalib-mcp --transport
streamable-http` listens on `--host`/`--port` at `/mcp` and speaks
both current protocol revisions on that one endpoint, streaming
responses as server-sent events and offering the server-to-client
stream on `GET`; `noyalib-mcp --transport sse` serves the older
HTTP+SSE transport at `/sse` and `/messages/`. The SDK has no server
side for that last transport, so `src/transport.rs` carries a small
bridge: one event stream per session, a `POST` endpoint that feeds it,
and the SDK's service running over an in-memory pair of channels.

The module binds loopback unless told otherwise and adds no
authentication of its own: a routable deployment sits behind a gateway
the operator trusts. It depends only on the SDK and on a
`ServerHandler` passed in, so the same file serves `oxml-mcp` and
`rlg-mcp` unchanged. It is the same code here, re-wrapped by this
repository's rustfmt settings — `oxml-mcp` pins `max_width = 80` and
this crate pins rustfmt's defaults, so a byte-identical copy would
fail `cargo fmt --check`.

The tools keep their names, argument names and descriptions. Each now
also declares an `outputSchema` and returns its answer as structured
content beside the text, carries behaviour annotations, and gives
every argument an example so a client that has no document of its own
can still generate a well-formed call. A tool that ran and failed is
an `isError` result the model can read; so is an unknown tool, because
the stateless revision carries the SDK's `-32602` as an HTTP 400 that
no model ever sees. A request the protocol rejects is still a JSON-RPC
error.

## Consequences

`main()` delegates to `transport::run`, which is the same file in
every server, so the suite is started, documented and tested the same
way. The server is verified over streamable HTTP with an independent
MCP auditor in both protocol eras and over SSE with the Python SDK's
client before release.

The crate is no longer close to dependency-free: the SDK brings
`tokio`, `axum`, `schemars` and `uuid`, and the MSRV moves to 1.88.
The security posture changes shape rather than degrading: the tools
still touch only the files a caller names, and the listeners exist
only when asked for on the command line.

The failure taxonomy moves from JSON-RPC codes to result text. A
client that switched on `-32000`/`-32001`/`-32002`/`-32003` now reads
`isError` and the message, which is what a model was always shown.
That is a wire-visible change inside a 0.0.x line, taken deliberately:
the codes were this server's invention, the results are the
specification's.

A stdio client must now perform the handshake — or, in the stateless
revision, name its protocol version in `_meta` — before its first
request. A bare `tools/call` as the first line, which the old loop
tolerated, is refused.
