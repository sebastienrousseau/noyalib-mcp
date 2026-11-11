<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# MCP operational reference

Use the current [README](../README.md) for installation and client setup.
This reference covers the existing server boundaries and links to the
executable contracts. It replaces the stale monorepo-era README copy.

Source builds need **Rust 1.89.0 or newer**. The executable examples
need Bash and Python 3.

## Start a server

```sh
noyalib-mcp --root /path/to/project
noyalib-mcp --root /path/to/project --transport streamable-http --host 127.0.0.1 --port 8000
noyalib-mcp --root /path/to/project --transport sse --host 127.0.0.1 --port 8001
```

Stdio carries newline-delimited JSON-RPC. Streamable HTTP uses `/mcp`;
legacy SSE uses `/sse` and its announced message endpoint. The configured
root constrains the file tools. It does not authenticate remote clients.
Use a trusted local client and the documented transport guards; do not
interpret an open listener as an authenticated service.

There are six tools, covering file reads/replacement, selected-document
replacement, inline parsing, inline editing and validation. Tools, prompts
and resources are all advertised. See the [tools reference](tools-reference.md)
for argument/output shapes, error handling, limits and concurrency.
[SDK tests](../tests/serve.rs) execute the catalogues and each tool;
[HTTP tests](../tests/http.rs) execute negotiated, stateless and legacy
transport envelopes.

## Examples

```sh
bash examples/handshake.sh
bash examples/set-then-get.sh
```

The file example edits a private temporary scratch file and verifies
readback before removing it. The caller's files stay untouched. These paths are relative to this standalone repository, not
`crates/noyalib-mcp` in the former workspace. Shell and client prerequisites
are declared in each script. Rust clients can use the
[SDK client example](../examples/).

## Files and recovery

`noyalib_get`, `noyalib_set` and `noyalib_set_multidoc` access files under
`--root`. Inline parse/edit/validate tools do not persist their results.
Replacement preserves untouched source and uses an atomic file replacement;
it does not promise all metadata, cross-client transaction isolation or
power-loss durability. Unix directory-relative confinement and the other
platforms' canonical-path fallback have different race guarantees.

A refused fragment leaves the original source intact. A timed-out blocking
operation can finish later; timeout is not proof that a write was cancelled.
Coordinate concurrent writers and verify the resulting file before retrying
a mutation whose completion is unknown. The
[filesystem implementation](../src/fsio.rs),
[limit tests](../tests/limits.rs) and [security policy](../SECURITY.md)
define the supported boundary.

## Distribution and verification

Published artifacts and the held release have separate status. Use the
current [README installation guide](../README.md) and the family
[verification guide](https://github.com/sebastienrousseau/noyalib/blob/main/pkg/VERIFY.md)
for registry, signed native archive, container and provenance instructions.
A source-built candidate with a local core override does not verify
registry dependency resolution. Do not publish a held immutable tag on
the basis of a corrected future branch.

**MSRV: Rust 1.89.0**. Server transports and filesystem work
require a host operating system; the core library's alloc-only support
does not make this application a bare-metal/no_std server. See
[development](../DEVELOPMENT.md), [architecture](ARCHITECTURE.md),
[policies](POLICIES.md), [benchmarks](BENCHMARKS.md) and
[agent integration](agent-integration.md) for the corresponding scope.
