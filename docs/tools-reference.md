<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# MCP tools reference

The server advertises six tools, one prompt and read-only reference
resources. The authoritative tool names, input/output schemas and
annotations come from [`src/tools.rs`](../src/tools.rs) and the live
`tools/list` response. The SDK and [transport adapter](../src/transport.rs)
serve stdio, streamable HTTP and legacy HTTP+SSE.

## Tool catalogue

All arguments in the table are required strings unless marked otherwise.
`doc_index` is a required non-negative integer, starting at zero.

| Tool | Arguments | Successful structured output |
|---|---|---|
| `noyalib_get` | `file`, `path` | `value`: the source fragment exactly as written |
| `noyalib_set` | `file`, `path`, `value` | `file`, `path`, `value` after replacing a YAML fragment |
| `noyalib_set_multidoc` | `file`, `doc_index`, `path`, `value` | `file`, `doc_index`, `path`, `value` for the selected document |
| `noyalib_parse` | `yaml` | `documents`: one JSON value per YAML document, tags stripped |
| `noyalib_edit` | `yaml`, `path`, `value` | `yaml`: the edited source |
| `noyalib_validate` | `yaml`, optional `schema` as JSON text | `valid`, optional parse-error location and schema violations |

File tools read or replace files under the configured root. Inline tools
operate on their arguments. Edits preserve untouched source and refuse
invalid replacement fragments. Read the full generated schema before
constructing a request; field descriptions and examples are part of that
catalogue. Successful tool results carry text and `structuredContent`.

For example, after a negotiated stdio handshake:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"noyalib_edit","arguments":{"yaml":"port: 8080 # keep\n","path":"port","value":"9090"}}}
```

## Paths and failures

Dotted paths select mappings; brackets select sequence elements:
`server.port`, `items[0]`, `items[0].name`. A replacement value is a YAML
fragment, not an encoded JSON value. A missing path or invalid fragment
is a tool failure whose readable text explains the refusal.

A tool that runs but cannot complete returns a JSON-RPC result carrying
`isError: true`; it is not a fixed `-32002` or `-32003` protocol error.
Invalid JSON-RPC envelopes and argument decoding remain protocol failures.
The live `noyalib://error-codes` resource describes the distinction.
[SDK boundary tests](../tests/serve.rs) check structured results,
unknown tools and failure shapes.

## Root, resources and work limits

Use `--root DIRECTORY` to restrict file tools; the default is the working
directory. On Unix, directory-relative opens and replacement constrain
path traversal and symlinks to that root. Other platforms use the
canonical-path fallback described in [`src/fsio.rs`](../src/fsio.rs),
which does not provide the same protection against a concurrent path swap.
The root is a file-access boundary, not an authentication mechanism.

Document, nesting, file-type, schema and session limits apply before the
corresponding work where supported. Schema text is limited to 65,536 bytes;
validation reports up to 100 violations plus a truncation entry. See
[limit tests](../tests/limits.rs) and [HTTP guards](../tests/http/guards.rs).

The process admits at most 64 outstanding blocking tool jobs across all
handlers and transports. Excess calls receive a readable busy tool result
without executing their work. This is separate from the HTTP session cap.
A timed-out or cancelled request keeps its slot until its blocking job
actually finishes; completion or panic returns capacity. Retry a busy call
after outstanding work completes. The
[ownership decision](adr/0002-bounded-blocking-work.md) records the boundary.

Tool work runs on a blocking pool. A call timeout answers with a failure,
but cannot interrupt an already running blocking operation: it may finish
in the background. Do not treat timeout or client cancellation as a
transaction rollback. Multiple sessions do not serialize edits to the same
file; coordinate writers externally. Atomic replacement avoids partial
file contents, but does not provide a cross-client read/modify/write lock
or a power-loss guarantee.

## Protocol and discovery

The SDK supports negotiated revisions through the current initialize-era
revision and the 2026-07-28 stateless revision. In stateless HTTP, send the
required protocol, routing headers and per-request `_meta`; use
`server/discover` instead of an initialize session. The executable
[HTTP tests](../tests/http.rs) show both envelopes. Legacy SSE uses its
own endpoint and session lifecycle. Unsupported extensions are not implied
by the SDK version.

Initialize advertises tools, prompts and resources; the server version
comes from the crate build. The `format_and_lint_yaml` prompt is a text
workflow, not a tool that automatically edits a file. Static resources
include `noyalib://tools` and `noyalib://error-codes`, plus descriptors
at `noyalib://tool/{name}`. They do not read arbitrary caller paths.
