<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `noyalib-mcp` in Cursor / VS Code with Cline

Cursor and VS Code (with the
[Cline extension](https://marketplace.visualstudio.com/items?itemName=saoudrizwan.claude-dev))
both consume MCP servers via JSON config.

## Cursor

Cursor's `Settings → MCP` pane writes to
`~/.cursor/mcp.json` (macOS / Linux) or
`%USERPROFILE%\.cursor\mcp.json` (Windows). Drop in:

```json
{
  "mcpServers": {
    "noyalib": {
      "command": "noyalib-mcp",
      "args": []
    }
  }
}
```

If `noyalib-mcp` isn't on `$PATH`, use the absolute Cargo
install location:

```json
{
  "mcpServers": {
    "noyalib": {
      "command": "/Users/<you>/.cargo/bin/noyalib-mcp"
    }
  }
}
```

After saving, click *Refresh* in the MCP pane — the two
`noyalib_*` tools should appear under `noyalib`.

## VS Code with Cline

Cline reads MCP server configs from VS Code settings. Open the
*MCP* sidebar and click the *Configure MCP Servers* gear, or
edit `settings.json` directly:

```json
{
  "cline.mcpServers": {
    "noyalib": {
      "command": "noyalib-mcp",
      "args": []
    }
  }
}
```

The `cline.mcpServers` key is a Cline convention; if you're
using a different MCP host extension, check that extension's
settings for the equivalent key (most use `mcpServers`).

## Tools advertised

```jsonc
// what tools/list returns
{
  "tools": [
    {
      "name": "noyalib_get",
      "description": "Read a value at a YAML path from a file.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "file": { "type": "string" },
          "path": { "type": "string" }
        },
        "required": ["file", "path"]
      }
    },
    {
      "name": "noyalib_set",
      "description": "Set a value at a YAML path. Comments and formatting are preserved.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "file": { "type": "string" },
          "path": { "type": "string" },
          "value": { "type": "string" }
        },
        "required": ["file", "path", "value"]
      }
    }
  ]
}
```

## Verifying the install

```bash
# 1. Confirm the binary
which noyalib-mcp
noyalib-mcp --version

# 2. Smoke-test the JSON-RPC handshake (returns one reply)
crates/noyalib-mcp/examples/handshake.sh

# 3. Set a value and read it back
crates/noyalib-mcp/examples/set-then-get.sh
```

## Atomic writes — Windows note

`noyalib_set` writes via *atomic rename*: it writes the new
contents to a sibling temp file, fsync's, then renames over
the target. On POSIX this is naturally atomic; on Windows it
uses
`MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`
semantics so concurrent readers always see either the old or
the new contents — never a half-write or a stale page-cache
observation. This matters for editors that re-read the file
immediately after the LLM call lands.
