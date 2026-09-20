<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Architecture

`noyalib-mcp` is a Model Context Protocol server that gives an AI
agent lossless YAML editing. It speaks MCP over stdio, streamable
HTTP or the older HTTP+SSE transport, and delegates every YAML
operation to `noyalib::cst::Document`, so an edit preserves comments,
indentation, and sibling entries byte for byte.

The protocol itself is [`rmcp`](https://crates.io/crates/rmcp), the
official Rust SDK: the framing, the handshake, the sessions and the
protocol revisions are its, and this crate supplies the tools and the
text a model reads. See
[ADR 0001](adr/0001-three-transports-one-command-line.md) for why.

## Layers

- **`src/main.rs`** is three lines: it hands the library's handler to
  `transport::run`.
- **`src/transport.rs`** is the transport dispatch: the command line
  (`--transport`, `--host`, `--port`, `--version`, `--help`), the
  stdio session, the streamable HTTP listener at `/mcp`, and a bridge
  for the 2024-11-05 HTTP+SSE transport at `/sse` and `/messages/`
  that the SDK no longer ships a server for. It depends only on the
  SDK and on a `ServerHandler` passed in — nothing in it knows what
  the tools are — so it is the same file in every Rust server of the
  suite.
- **`src/lib.rs`** is the handler: `YamlServer` holds the tool and
  prompt routers, describes the server for `initialize` and
  `server/discover`, serves the resources, and turns a call for a tool
  that does not exist into a result the model can read instead of the
  SDK's `-32602`.
- **`src/tools.rs`** is the YAML work. `get`, `set`, `set_multidoc`,
  `parse`, `edit` and `validate` are plain functions that know nothing
  about MCP; the `#[tool_router]` block below them is what `tools/list`
  advertises and `tools/call` reaches. Each tool returns its answer
  twice — as text for the model, and as `structuredContent` against
  the `outputSchema` it declares.
- **`src/resources.rs`** backs `resources/list`,
  `resources/templates/list`, and `resources/read` with read-only
  reference material an agent can pin without a tool call: the failure
  taxonomy, the tool reference, and per-tool descriptors. None of it
  touches the filesystem or accepts caller paths.
- **`src/prompts.rs`** backs `prompts/list` and `prompts/get` with a
  pure text template that teaches the host model the recommended
  `noyalib_get` / `noyalib_set` workflow.

## Two kinds of failure

A tool that ran and could not do the job — a file that cannot be read,
YAML that does not parse, a path that does not exist, a fragment that
cannot be applied — is a *successful* JSON-RPC response carrying
`isError: true` and text the model can act on. A tool the server does
not have is reported the same way, naming the ones that exist: the
stateless HTTP revision carries the SDK's `-32602` as an HTTP 400,
which the client reports as a transport fault and the model never
reads.

A request the protocol rejects — an unknown method, a resource that
does not exist, a routing header that disagrees with the body — is a
JSON-RPC error, which the client handles.

## Trust boundary

Everything the agent sends arrives through the SDK and reaches the six
functions as deserialised arguments. Those functions are the surface
this crate owns, and the stateless three are fuzzed directly
(`fuzz_targets/tools.rs`); CI replays the seed corpus on every push.

The HTTP listeners exist only when `--transport` asks for them, bind
the loopback interface unless `--host` says otherwise, and do not
authenticate: a routable deployment belongs behind a gateway the
operator trusts.

## Distribution

`server.json` and `glama.json` are the registry manifests; both carry
the version and the `ghcr.io` image tag, and the release workflow's
Validate job refuses a tag they disagree with. `pkg/npm-wrapper` ships
the same binary through npm. The container entrypoint is stdio, which
is what `docker run -i` gives an agent; the HTTP transports are
reached from it by passing `--transport`.

## Lockstep

The crate pins `noyalib` at the identical `=0.0.X` and releases with
it (core ADR-0005).
