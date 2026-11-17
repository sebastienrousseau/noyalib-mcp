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

# 2. Smoke-test the JSON-RPC handshake (waits for initialize and the tool catalogue)
bash examples/handshake.sh

# 3. Set a value and read it back
bash examples/set-then-get.sh
```

## File replacement

File tools replace a synced temporary sibling over the selected file.
This prevents partial content, while cross-client edit isolation,
power-loss durability and complete metadata preservation require separate
coordination. Unix confinement and the other platforms' path fallback
have different guarantees; see the
[tools reference](../docs/tools-reference.md#root-resources-and-work-limits).
