<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# MCP delivery comparison

| Delivery surface | Lossless YAML edits | Remote transport | Typed tool results |
| :--- | :---: | :---: | :---: |
| `noyalib-mcp` | Yes | Streamable HTTP and SSE | Yes |
| `noya-cli` | Yes | No | Not applicable |
| Native `noyalib` | Yes | No | Rust types |

`noyalib-mcp` is the protocol adapter when an MCP-aware agent must reach the
same parser and lossless editor used by the native tools. It is not a general
MCP gateway or an authentication service. Compare MCP servers by tool contract,
transport, trust boundary, protocol revision, and deployment model rather than
by tool count alone.
