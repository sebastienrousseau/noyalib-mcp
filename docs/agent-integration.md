# Agent integration

Wire `noyalib-mcp` into the AI agent platforms that speak MCP.
Each section is self-contained — find your agent, paste the
config, restart, and the `noyalib_get` / `noyalib_set` tools
appear in the agent's tool palette.

For the full tool surface and JSON Schema, see
[`tools-reference.md`](./tools-reference.md).

## Install the binary

```sh
cargo install noyalib-mcp
```

The binary lands in `~/.cargo/bin/noyalib-mcp`. Throughout this
document we assume that path is on `PATH`; if not, substitute the
absolute path in each agent's config.

## Claude Desktop

`~/Library/Application Support/Claude/claude_desktop_config.json`
(macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```jsonc
{
  "mcpServers": {
    "noyalib": {
      "command": "noyalib-mcp",
      "args": []
    }
  }
}
```

Restart Claude Desktop. The `noyalib_get` and `noyalib_set` tools
appear under the tool icon in any conversation.

To restrict the tools to a specific working directory:

```jsonc
{
  "mcpServers": {
    "noyalib": {
      "command": "noyalib-mcp",
      "args": ["--root", "/Users/me/Projects/configs"]
    }
  }
}
```

The server accepts `--root`; it does not read a `NOYALIB_MCP_ROOT`
environment variable. File tools refuse paths outside the configured root
with a readable `isError` tool result. See the
[tools reference](tools-reference.md#paths-and-failures).

## Cursor

`~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "noyalib": {
      "command": "noyalib-mcp"
    }
  }
}
```

Cursor picks up the change without restart on most workspaces.
Open the AI sidebar; the noyalib tools are listed.

## Continue.dev

`~/.continue/config.json`:

```jsonc
{
  "mcpServers": [
    {
      "name": "noyalib",
      "command": "noyalib-mcp"
    }
  ]
}
```

Restart Continue. Tools appear in the agent's tool palette.

## Generic MCP client

Any client that speaks MCP over stdio can use (the server is dual-era: 2026-07-28 per-request `_meta` / `server/discover`, or the 2025-06-18 `initialize` handshake shown here):

```sh
noyalib-mcp
```

The server reads JSON-RPC requests from stdin and writes responses
to stdout, line-delimited. No flags, no environment variables
required for the basic case.

## Verifying the server is wired up

In any MCP client, ask the agent: "List your available tools." The
agent should mention `noyalib_get` and `noyalib_set` (or describe
them as "read/write YAML values at paths").

For a CLI smoke test independent of any agent:

```sh
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}' \
  | noyalib-mcp
```

Should respond with the server capabilities. Then:

```sh
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
} | noyalib-mcp
```

Should list both `noyalib_get` and `noyalib_set` with their full
input schemas. The full handshake script lives in
`crates/noyalib-mcp/examples/handshake.sh`.

## Common agent prompts

Agents wielding the noyalib MCP tools handle prompts like:

> "Update the version field in `Cargo.toml` to 0.0.2."
>
> Agent: calls `noyalib_get` with `file=Cargo.toml, path=version`,
> sees the current value, then calls `noyalib_set` with the new
> value. The diff in `git diff` is one line.

> "Read the database connection string from `config/prod.yaml`."
>
> Agent: `noyalib_get` with `path=database.connection_string`.

> "Increment the `replicas` count in every `*.yaml` under `k8s/`."
>
> Agent: `Glob` for the files, then per-file `noyalib_get` to read
> the current count, then `noyalib_set` to write the incremented
> value. Each file's comments and indentation survive intact.

## Editing and file access

`noyalib_set` and `noyalib_set_multidoc` replace a selected YAML fragment
while preserving untouched source. Inline `noyalib_edit` returns source
without writing a file. Command-line formatting is provided by `noyafmt
--write` in the separate CLI crate.

## Security considerations

- File tools use the configured `--root`, defaulting to the working
  directory. Run with the smallest root and OS permissions required.
- Unix directory-relative opens constrain symlinks and path traversal.
  The canonical-path fallback on other platforms has different concurrent
  path-swap guarantees; see [`src/fsio.rs`](../src/fsio.rs).
- Stdio relies on the host process boundary. HTTP Host/Origin guards are
  not client authentication. Expose a network service only within an
  appropriate access-controlled deployment.
- A call timeout cannot interrupt a running blocking operation. Verify
  completion before retrying an uncertain mutation; coordinate writers
  to the same file externally.

See the [security policy](../SECURITY.md) and
[tool limits](tools-reference.md#root-resources-and-work-limits).

## Troubleshooting

### Tools don't appear in the agent

1. Verify the binary works: `noyalib-mcp --version`
2. Verify the agent's config syntax (most agents log MCP server
   startup; check the agent's logs)
3. Restart the agent — most clients only re-read the MCP config
   on launch

### Edit fails with "path not found"

`noyalib_set` requires the parent path to exist. To set
`server.host` when `server` doesn't exist, first set `server` to
`{}`, then set `server.host`. Or pre-populate the file with the
parent structure.

### Agent says "no such tool"

Some agents serve a curated tool list and require explicit
allow-listing. Check the agent's tool-permissions UI; toggle
`noyalib_get` and `noyalib_set` on.

### Working directory differs from expectation

The server inherits the working directory of its parent process
(the agent). Relative file arguments resolve under the configured root; its default
is the inherited working directory. Set `--root` explicitly for a client
whose working directory is uncertain.

## Related

- [Tools reference](./tools-reference.md) — full input schemas
  and error codes
- [Crate README](../README.md) — install + crate-level overview
- [`examples/`](../examples/) — handshake.sh
  and other diagnostic scripts
