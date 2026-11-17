<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Benchmarks

```sh
cargo bench --bench mcp_tools
```

The existing `mcp_tools` harness measures core formatting, parsing,
value-path lookup and CST editing primitives. It does not invoke
`YamlServer`, the SDK tool router, file I/O or a protocol transport.
Its historical `tools/call format` label is a core formatter scenario;
there is no registered tool named `format`. Do not describe these numbers
as six-tool throughput or end-to-end MCP latency.

[SDK tests](../tests/serve.rs) and real-binary
[stdio](../tests/protocol.rs)/[HTTP](../tests/http.rs) tests establish
correctness separately. Cold-process protocol measurements must include
startup/handshake and identify that scope; warmed request measurements
need a persistent client and individual round trips.

Retain CPU/OS/toolchain, source, dependency and corpus identities, release
profile, repetitions, raw samples and run-to-run variance with results.
Shared-runner or busy local-host timings do not establish a dedicated
latency threshold. Report resource observations with their sampling method;
an end-of-run RSS sample is not an observed peak.
