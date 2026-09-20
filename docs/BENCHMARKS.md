<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Benchmarks

The `mcp_tools` Criterion harness measures representative tool operations.

```bash
cargo bench --bench mcp_tools
```

Transport tests cover protocol correctness separately from the microbenchmark.
Record the CPU, operating system, Rust version, commit, input size, transport,
and command with any result. CI does not use shared-runner latency as a pass/fail
threshold.
