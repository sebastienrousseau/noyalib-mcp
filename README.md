<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

<p align="center">
  <img src="https://cloudcdn.pro/noyalib/v1/logos/noyalib.svg" alt="noyalib-mcp logo" width="128" />
</p>

<h1 align="center">noyalib-mcp</h1>

<p align="center">
  Model Context Protocol server for lossless YAML inspection, editing, and validation.
</p>

<p align="center">
  <a href="https://github.com/sebastienrousseau/noyalib-mcp/actions"><img src="https://img.shields.io/github/actions/workflow/status/sebastienrousseau/noyalib-mcp/ci.yml?style=for-the-badge&logo=github" alt="Build" /></a>
  <a href="https://crates.io/crates/noyalib-mcp"><img src="https://img.shields.io/crates/v/noyalib-mcp.svg?style=for-the-badge&color=fc8d62&logo=rust" alt="Registry" /></a>
  <a href="https://docs.rs/noyalib-mcp"><img src="https://img.shields.io/badge/docs.rs-noyalib--mcp-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" alt="Docs" /></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/sebastienrousseau/noyalib-mcp"><img src="https://img.shields.io/ossf-scorecard/github.com/sebastienrousseau/noyalib-mcp?style=for-the-badge&label=OpenSSF%20Scorecard&logo=openssf" alt="OpenSSF Scorecard" /></a>
  <a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg?style=for-the-badge" alt="License: Apache-2.0 OR MIT" /></a>
  <a href="https://github.com/sebastienrousseau/noyalib-mcp/blob/main/docs/POLICIES.md"><img src="https://img.shields.io/badge/MSRV-1.88.0-93450a.svg?style=for-the-badge&logo=rust" alt="MSRV 1.88.0" /></a>
</p>

---

## Contents

**Getting started**

- [Install](#install) — Cargo, npm wrapper, and container
- [Requirements](#requirements) — toolchain floor, platforms
- [Quick Start](#quick-start) — start a server over stdio or HTTP

**The noyalib-mcp ecosystem**

- [The noyalib-mcp ecosystem](#the-noyalib-mcp-ecosystem) — protocol and library relationships

**Library reference**

- [Capabilities at a glance](#capabilities-at-a-glance) — the current surface by theme
- [Ecosystem comparison](#ecosystem-comparison) — short matrix; full table at [`docs/COMPARISON.md`](docs/COMPARISON.md)
- [Benchmarks](#benchmarks) — harness coverage; full method at [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md)
- [Features](#features) — MCP tools and resources
- [Configuration](#configuration) — transports and endpoints
- [Examples](#examples) — runnable clients

**Operational**

- [When not to use noyalib-mcp](#when-not-to-use-noyalib-mcp) — limitations
- [Development](#development) — make targets, fuzzing, CI
- [Security](#security) — guarantees and compliance
- [Documentation](#documentation) — all reference docs
- [Stability guarantees](#stability-guarantees) — protocol, SemVer, and toolchain discipline
- [License](#license)

---

## Install

### As a Rust library

```toml
[dependencies]
noyalib-mcp = "0.0.48"
```

Install or run the server through the channel that fits the host:

```bash
cargo install noyalib-mcp --locked
npx @sebastienrousseau/noyalib-mcp
docker run --rm -i ghcr.io/sebastienrousseau/noyalib-mcp:latest
```

## Requirements

- Rust **1.88.0 or newer** when building from source.
- Linux, macOS, and Windows are tested by CI.
- The crate pins `noyalib` at exactly `=0.0.48` under the lockstep contract.
- An MCP client is required to drive the server.

| Surface | Minimum toolchain | Enforcement |
| :--- | :---: | :--- |
| Server and library | Rust 1.88.0 | manifest and MSRV CI |
| Complete test surface | Rust 1.88.0 | all-target CI |

## Quick Start

Use stdio for a child-process integration:

```bash
noyalib-mcp
```

A stdio client begins with an MCP initialization request before sending the
initialized notification or listing tools:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-11-25",
    "capabilities": {},
    "clientInfo": {
      "name": "example",
      "version": "0.0.1"
    }
  }
}
```

Serve streamable HTTP at `/mcp`, or the legacy HTTP+SSE bridge at `/sse`
and `/messages/`:

```bash
noyalib-mcp --transport streamable-http --host 127.0.0.1 --port 8000
noyalib-mcp --transport sse --host 127.0.0.1 --port 8001
```

The server negotiates supported MCP revisions during `initialize`; clients
should not send raw tool calls before completing that handshake.

## The noyalib-mcp ecosystem

| Component | Role |
| :--- | :--- |
| `noyalib-mcp` | MCP server, transports, tools, prompts, and resources |
| [`noyalib`](https://github.com/sebastienrousseau/noyalib) | YAML parser, CST editor, and schema validator |
| [`noya-cli`](https://github.com/sebastienrousseau/noya-cli) | Human-facing formatting and validation commands |
| `rmcp` | Official Rust MCP SDK used for protocol handling |

## Capabilities at a glance

| Area | Capability | Status |
| :--- | :--- | :--- |
| Transport | stdio, streamable HTTP, and HTTP+SSE | Supported |
| Protocol | Revisions from 2024-11-05 through 2026-07-28 | Negotiated |
| Results | Text plus typed `structuredContent` | Supported |
| YAML | Parse, get, set, multidocument set, edit, and validate | Supported |
| Metadata | Tool schemas, annotations, examples, prompts, and resources | Published |

## Ecosystem comparison

| Delivery surface | Lossless YAML edits | Remote transport | Typed tool results |
| :--- | :---: | :---: | :---: |
| **noyalib-mcp** | Yes | HTTP and SSE | Yes |
| `noya-cli` | Yes | No | Not applicable |
| `noyalib` | Yes | No | Native Rust types |

See [`docs/COMPARISON.md`](docs/COMPARISON.md) for scope and evidence.

## Benchmarks

The checked-in `mcp_tools` Criterion harness measures tool dispatch and YAML
operations without treating noisy timing values as correctness gates.

```bash
cargo bench --bench mcp_tools
```

See [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) for methodology.

## Features

- Six typed tools for YAML reads, edits, parsing, and validation.
- Declared `outputSchema`, `structuredContent`, annotations, and examples.
- stdio, streamable HTTP, and legacy HTTP+SSE transports.
- Prompts and resources for agent integration.
- Protocol revision negotiation and session-aware routing.

## Configuration

| Option | Effect |
| :--- | :--- |
| `--transport stdio` | Use process stdin and stdout; this is the default |
| `--transport streamable-http` | Serve MCP at `/mcp` |
| `--transport sse` | Serve `/sse` and `/messages/` |
| `--host ADDRESS` | Bind an HTTP transport to an address |
| `--port PORT` | Select the HTTP listening port |

Bind remote transports deliberately. The default loopback host avoids exposing
the server beyond the local machine.

## Examples

- [`mcp_session.rs`](examples/mcp_session.rs): SDK-backed in-memory session.
- [`handshake.sh`](examples/handshake.sh): initialize and list tools.
- [`set-then-get.sh`](examples/set-then-get.sh): lossless mutation and readback.

Client configuration recipes are in
[`docs/agent-integration.md`](docs/agent-integration.md).

## When not to use noyalib-mcp

- Use `noyalib` directly inside a Rust application that does not need MCP.
- Use `noya-cli` for shell pipelines and repository-wide validation.
- Do not expose an HTTP transport to an untrusted network without an
  authentication and authorization layer in front of it.
- The server does not fetch remote schemas; callers must supply trusted schema
  content.

The [detailed README reference](docs/README-REFERENCE.md) retains client setup,
tool detail, verification steps, and conformance discussion.

## Development

```bash
make
make test
make clippy
make fmt
cargo fuzz run fuzz_tool_call
```

CI checks Rust formatting, linting, tests, documentation, dependency policy,
fuzz regressions, protocol behaviour, and the shared YAML test suite. See
[`DEVELOPMENT.md`](DEVELOPMENT.md).

## Security

Report vulnerabilities through [`SECURITY.md`](SECURITY.md). The crate forbids
`unsafe` code, audits dependencies, uses bounded noyalib operations, and emits
release attestations and SBOMs. Treat file content, YAML, schemas, and MCP
metadata as untrusted input.

## Documentation

- [User Manual](https://sebastienrousseau.github.io/noyalib-mcp/manual/)
- [API reference](https://docs.rs/noyalib-mcp)
- [Developer documentation](DEVELOPMENT.md)
- [Agent integration](docs/agent-integration.md)
- [Tool reference](docs/tools-reference.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Engineering policies](docs/POLICIES.md)
- [Compliance grade](docs/COMPLIANCE-GRADE.md)
- [Detailed README reference](docs/README-REFERENCE.md)

## Stability guarantees

- During `0.0.x`, the patch component is the breaking-change axis.
- Tool names, argument schemas, output schemas, and transport routes form the
  public compatibility surface.
- Protocol revisions are negotiated rather than inferred.
- The MSRV may rise only on the breaking axis with a changelog explanation.

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), at your option.
