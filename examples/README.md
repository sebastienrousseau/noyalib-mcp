<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `noyalib-mcp` examples

Agent-driving demos and per-host client configuration snippets.

## Protocol-level scripts

Each script pipes a sequence of newline-delimited JSON-RPC 2.0
frames into `noyalib-mcp` over stdio and prints the response
stream.

| Script | What it shows |
|---|---|
| [`handshake.sh`](handshake.sh) | `initialize` → `tools/list` smoke test. Confirms the binary speaks the protocol and announces the expected tools. |
| [`format-call.sh`](format-call.sh) | Call the `format` tool on a poorly-spaced document. Demonstrates that comments + indentation pass through the CST formatter unchanged. |
| [`set-then-get.sh`](set-then-get.sh) | Round-trip the mutation surface: `set` rewrites `server.port`, `get` reads it back. Surgical edit; surrounding bytes untouched. |

```bash
chmod +x crates/noyalib-mcp/examples/*.sh
crates/noyalib-mcp/examples/handshake.sh
```

The scripts use `printf` / `echo` only (no `jq`, no `node`), so
they work on any POSIX shell. Pipe through `jq -c .` if you
want pretty-printed JSON responses for human reading.

## Client configurations

Drop-in config snippets for the most common MCP-host
applications:

| File | Host | What it sets up |
|---|---|---|
| [`client-claude-desktop.json`](client-claude-desktop.json) | Claude Desktop (macOS / Linux / Windows) | `mcpServers.noyalib` block for `claude_desktop_config.json`, with platform-specific install paths |
| [`client-cursor.md`](client-cursor.md) | Cursor + VS Code with Cline | `mcpServers` JSON for Cursor's MCP pane and `cline.mcpServers` for the Cline extension |
| [`client-zed.json`](client-zed.json) | Zed | `context_servers.noyalib` block for Zed `settings.json` |
| [`client-continue-dev.md`](client-continue-dev.md) | Continue.dev (VS Code + JetBrains) | `mcpServers.noyalib` block for `~/.continue/config.{json,yaml}`, plus `autoAllowedTools` hardening note |
| [`hosted-mcp-run.md`](hosted-mcp-run.md) | Remote / hosted gateways (mcp.run, mcp-proxy, Docker / Kubernetes) | Sidecar deployment, stdio→SSE / HTTP brokers, container Dockerfile, hardening notes |

Once registered, the LLM agent in each host can call the two
`noyalib_*` tools (`tools/list` is the discovery handshake; the
agent decides when to invoke based on user intent).

If you wire up a different MCP host and the config is
non-obvious, PRs adding a `client-<name>.{md,json}` snippet are
welcome.

## License

Dual-licensed under Apache 2.0 or MIT, at your option.
