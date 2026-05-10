# MCP tools reference

The complete list of tools `noyalib-mcp` advertises via
`tools/list`, with their JSON Schema input shape and example
invocations. This file is the user-facing reference; the
authoritative source is
[`crates/noyalib-mcp/src/tools.rs::descriptors()`](../src/tools.rs).

The server speaks **MCP 2024-11-05**. Tool calls go through
`tools/call`; see the [MCP spec](https://spec.modelcontextprotocol.io/)
for the JSON-RPC envelope.

## Tool list

The server advertises **2 tools**. Every tool delegates to
`noyalib::cst::Document` so edits round-trip with comments,
indentation, and sibling entries preserved byte-for-byte.

### `noyalib_get`

Read the YAML value at a dotted/indexed path in a file. Returns
the source slice **exactly** — no re-quoting, no canonicalisation.
Preserves comments and formatting for any later `noyalib_set`
round-trip.

**Input schema:**

```json
{
  "type": "object",
  "properties": {
    "file": {
      "type": "string",
      "description": "Path to the YAML file on disk."
    },
    "path": {
      "type": "string",
      "description": "Dotted/indexed path into the YAML, e.g. `server.host` or `items[0].name`."
    }
  },
  "required": ["file", "path"]
}
```

**Example invocation:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "noyalib_get",
    "arguments": {
      "file": "/etc/myapp/config.yaml",
      "path": "server.host"
    }
  }
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      { "type": "text", "text": "api.example.com" }
    ]
  }
}
```

**Error codes:**

| Code | Meaning |
|---|---|
| `-32000` | I/O error reading the file |
| `-32001` | Parse error in the YAML |
| `-32002` | Path not found in the document |
| `-32602` | Missing or non-string argument |

### `noyalib_set`

Set the YAML value at a dotted/indexed path in a file. Only the
touched span is rewritten — every comment, blank line, and
sibling entry is preserved byte-for-byte. Useful for
Renovate-style version bumps and config patches by AI agents.

**Input schema:**

```json
{
  "type": "object",
  "properties": {
    "file": {
      "type": "string",
      "description": "Path to the YAML file on disk."
    },
    "path": {
      "type": "string",
      "description": "Dotted/indexed path into the YAML."
    },
    "value": {
      "type": "string",
      "description": "Replacement value as a YAML fragment (e.g. `0.0.2`, `\"hello\"`, `[1, 2, 3]`). Must parse in the target position; the document is left unchanged on parse error."
    }
  },
  "required": ["file", "path", "value"]
}
```

**Example invocation:**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "noyalib_set",
    "arguments": {
      "file": "/etc/myapp/config.yaml",
      "path": "server.port",
      "value": "9090"
    }
  }
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      { "type": "text", "text": "set server.port = 9090 in /etc/myapp/config.yaml (lossless: comments and formatting preserved)" }
    ]
  }
}
```

**Error codes:**

| Code | Meaning |
|---|---|
| `-32000` | I/O error reading or writing the file |
| `-32001` | Parse error in the source YAML |
| `-32003` | The new value cannot be applied at the path (parse error in the fragment, or path doesn't exist) |
| `-32602` | Missing or non-string argument |

## Lossless edit guarantee

Both tools route through `noyalib::cst::Document`, so the on-disk
file after a `noyalib_set`:

- Has the same comments at the same positions
- Has the same blank lines and indentation
- Has the same anchor / alias relationships
- Has the same key order
- Has the new value at the requested path

The only bytes that change are the bytes for the value itself.
Empirically: `git diff` after a single `noyalib_set` shows a 1-line
change. This is the contract.

## Path syntax

The `path` argument follows the same syntax as
`noyalib::Value::get_path` and `noyalib::cst::Document::get`:

| Path | Selects |
|---|---|
| `name` | Top-level key `name` |
| `server.host` | `host` inside the mapping at `server` |
| `items[0]` | First element of the sequence at `items` |
| `items[0].name` | `name` inside the first element |
| `services.api.replicas` | Deeply nested |

## Path-not-found semantics

`noyalib_get` returns error `-32002` if the path does not exist.
`noyalib_set` will *create* missing intermediate keys at the same
indent level when possible; if the parent path does not exist or
points to an incompatible shape (e.g. `server.host[0]` when
`server.host` is a string), it returns `-32003`.

## Concurrency

The server processes one request at a time per stdio session.
Concurrent edits to the same file from multiple sessions are not
serialised by the server — clients should coordinate externally
(e.g. via flock) if multiple agents may edit the same file.

## Capabilities advertised at `initialize`

```json
{
  "protocolVersion": "2024-11-05",
  "serverInfo": {
    "name": "noyalib-mcp",
    "version": "0.0.1"
  },
  "capabilities": {
    "tools": {}
  }
}
```

The server does not advertise `prompts` or `resources` capabilities
— it's tool-only.

## Related

- [Agent integration](./agent-integration.md) — Claude Desktop,
  Cursor, and other client setup
- [Crate README](../README.md) — install + crate-level overview
