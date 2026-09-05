<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Architecture

`noyalib-mcp` is a Model Context Protocol server that gives an AI
agent lossless YAML editing. It speaks newline-delimited JSON-RPC 2.0
over stdio and delegates every YAML operation to
`noyalib::cst::Document`, so an edit preserves comments, indentation,
and sibling entries byte for byte.

## Layers

- **`src/main.rs`** is the transport: a stdio loop that reads one
  JSON-RPC message per line and drives `handle_message`. It holds no
  logic, so tests never need a running process.
- **`src/lib.rs`** is the dispatch core: `handle_message` parses the
  envelope into `Request` and returns a `HandleOutcome` (reply, or
  nothing for a notification); `dispatch` routes by method name;
  `Response`, `ErrorResponse`, and `ErrorObject` are the wire shapes.
  Two protocol eras are supported at once: the 2026-07-28 stateless
  form (`_meta` protocol-version key, `server/discover`) and the
  2025-06-18 `initialize` handshake. `SUPPORTED_PROTOCOL_VERSIONS`
  and `UNSUPPORTED_PROTOCOL_VERSION` (-32022) define the negotiation.
- **`src/tools.rs`** is the tool registry: `descriptors` is what
  `tools/list` returns (one JSON Schema per tool), `call` is the
  `tools/call` entry point. The tool set is fixed: `noyalib_get`,
  `noyalib_set`, and `noyalib_set_multidoc` (the same edit for one
  document of a `---`-separated stream).
- **`src/resources.rs`** backs `resources/list`,
  `resources/templates/list`, and `resources/read` with read-only
  reference material an agent can pin without a tool call: the error
  code taxonomy, the tool reference, and per-tool descriptors. None of
  it touches the filesystem or accepts caller paths.
- **`src/prompts.rs`** backs `prompts/list` and `prompts/get` with
  pure text templates that teach the host model the recommended
  `noyalib_get` / `noyalib_set` workflow.

## Trust boundary

`handle_message` is the entire trust boundary; everything the agent
sends passes through it. It is fuzzed directly (`fuzz_handle_message`)
and through the tool layer (`fuzz_tool_call`), and CI replays the seed
corpus on every push.

## Distribution

`server.json` and `glama.json` are the registry manifests; both carry
the version and the `ghcr.io` image tag, and the release workflow's
Validate job refuses a tag they disagree with. `pkg/npm-wrapper` ships
the same binary through npm.

## Lockstep

The crate pins `noyalib` at the identical `=0.0.X` and releases with
it (core ADR-0005).
