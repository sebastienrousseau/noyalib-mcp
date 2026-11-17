<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `noyalib-mcp` examples

Agent-driving demos and per-host client configuration snippets.

## Protocol-level scripts

The Bash entry points use Python 3 to hold a real stdio session. Each
request waits for its response before continuing. They print the server's
JSON responses, check their semantics and stop with an error on failure.
Set `NOYALIB_MCP_BIN` to select a built binary; otherwise it must be on PATH.
`PYTHON` can select the Python 3 interpreter.

| Script | Executed operation |
|---|---|
| [`handshake.sh`](handshake.sh) | Initialize and verify the complete six-tool catalogue. |
| [`set-then-get.sh`](set-then-get.sh) | Call `noyalib_set`, verify surrounding bytes, then read back with `noyalib_get`. |
| [`format-call.sh`](format-call.sh) | Retrieve `format_and_lint_yaml` via `prompts/get`. The historical filename is retained; there is no `format` tool. |

```sh
bash examples/handshake.sh
bash examples/set-then-get.sh
bash examples/format-call.sh
```

The clients use a private temporary root. The file demo edits only its own
scratch file and removes it afterward. They leave the caller's files
untouched. `mcp-session.py` is the shared implementation.

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

Once registered, the client can discover all six
`noyalib_*` tools (`tools/list` is the discovery handshake; the
agent decides when to invoke based on user intent).

If you wire up a different MCP host and the config is
non-obvious, PRs adding a `client-<name>.{md,json}` snippet are
welcome.

## License

Dual-licensed under Apache 2.0 or MIT, at your option.
