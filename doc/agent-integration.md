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
      "args": [],
      "env": {
        "NOYALIB_MCP_ROOT": "/Users/me/Projects/configs"
      }
    }
  }
}
```

(The `NOYALIB_MCP_ROOT` env var is honoured by the server when
present; tool calls that reference paths outside this root
return `-32000` "outside permitted root".)

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

Any client that speaks MCP 2024-11-05 over stdio can use:

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
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' \
  | noyalib-mcp
```

Should respond with the server capabilities. Then:

```sh
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}'
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

## Why MCP for YAML editing?

Existing approaches:

- **`sed` / `yq`** — text-or-AST replacement that often loses
  comments, normalises indentation, and breaks anchor references
- **`Edit` tool with full file rewrite** — agent regenerates the
  whole file from memory, which scrambles comments, comments
  drift, and is hard to review
- **MCP `noyalib_set`** — surgical edit through the lossless CST.
  One value changes; everything else is byte-identical

This is the same property that makes `noyafmt --fix` safe: the
green tree preserves trivia by construction.

## Security considerations

- **Server has full filesystem access** — it can read and write
  any path the user running the binary can. For untrusted
  environments, use `NOYALIB_MCP_ROOT` to confine to a directory.
- **No authentication on the JSON-RPC channel** — MCP runs over
  stdio; the security boundary is the OS process. Don't expose
  the binary over a network without an authenticating proxy.
- **Path traversal** — the server resolves paths as-given. If
  client-side path sanitisation matters, sanitise before calling.

For the full security policy see the workspace
[SECURITY.md](../../../SECURITY.md).

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
(the agent). Relative paths in tool arguments resolve from there.
Use absolute paths when in doubt.

## Related

- [Tools reference](./tools-reference.md) — full input schemas
  and error codes
- [Crate README](../README.md) — install + crate-level overview
- [`crates/noyalib-mcp/examples/`](../examples/) — handshake.sh
  and other diagnostic scripts
