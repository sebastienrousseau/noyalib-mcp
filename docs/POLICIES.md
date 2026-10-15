<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Engineering policies

## Version and dependency policy

`noyalib-mcp` releases in strict lockstep with `noyalib`. Version 0.0.58 pins
the core at exactly `=0.0.58`. Release work uses `feat/v0.0.58`; each subsequent
iteration increments exactly 0.0.1.

## Minimum Rust version

The minimum supported Rust version is 1.89.0, the floor shared across the noyalib
family; the MCP SDK stack alone requires 1.88.0. The manifest declares it and CI verifies it. A future increase is a
breaking-axis change and requires a changelog entry.

## Protocol and transport stability

Tool names, input and output schemas, prompts, resources, transport routes,
header behaviour, and advertised protocol revisions are public compatibility
surfaces. Bind network transports to loopback by default. Authentication and
authorization belong at the deployment boundary.

During `0.0.x`, the patch component is the breaking axis. Family-wide policies
live in the core
[`POLICIES.md`](https://github.com/sebastienrousseau/noyalib/blob/main/docs/POLICIES.md).
