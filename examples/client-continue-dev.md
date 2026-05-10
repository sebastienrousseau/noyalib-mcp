<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# `noyalib-mcp` in Continue.dev

[Continue.dev](https://continue.dev) is an open-source AI coding
assistant for VS Code and JetBrains IDEs. It supports MCP
servers via its standard `config.json` (or `config.yaml`).

## Install

1. Install the Continue.dev extension:
   - **VS Code**: [Continue - Codestral, Claude, and more](https://marketplace.visualstudio.com/items?itemName=Continue.continue)
   - **JetBrains**: <https://plugins.jetbrains.com/plugin/22707-continue>

2. Open Continue's config file:
   - **VS Code**: `Cmd/Ctrl+Shift+P` → `Continue: Open config.json`.
   - **JetBrains**: Continue panel → gear icon → `Open Config File`.

3. Default location: `~/.continue/config.json`
   (`%USERPROFILE%\.continue\config.json` on Windows).

## Configure

Add the `mcpServers` block:

```json
{
  "models": [
    /* your existing model configs */
  ],
  "mcpServers": {
    "noyalib": {
      "command": "noyalib-mcp",
      "args": []
    }
  }
}
```

If `noyalib-mcp` isn't on `$PATH`, use the absolute Cargo
install path:

```json
{
  "mcpServers": {
    "noyalib": {
      "command": "/Users/<you>/.cargo/bin/noyalib-mcp",
      "args": []
    }
  }
}
```

## YAML config alternative

Continue.dev also supports `~/.continue/config.yaml`. The MCP
server block looks like:

```yaml
mcpServers:
  noyalib:
    command: noyalib-mcp
    args: []
```

(YAML is technically dog-fooding — Continue's YAML config is
itself parseable / writeable by `noyalib-mcp`.)

## Verifying the install

After saving config, reload Continue (Continue panel → 3-dot
menu → `Reload`). The two `noyalib_*` tools should appear in
the *Tools* drawer:

- `noyalib_get` — read a YAML path.
- `noyalib_set` — write a YAML path with comment-preserving
  surgical edit.

Smoke test: ask Continue *"Read port from /tmp/test.yaml"* —
the assistant should dispatch `noyalib_get` and surface the
value inline.

## Continue.dev tool-allow-list

For production teams, lock down which tools Continue can call
without explicit user confirmation via the
[`autoAllowedTools`](https://docs.continue.dev/customize/deep-dives/configuration#autoallowedtools)
field:

```json
{
  "autoAllowedTools": ["noyalib_get"],
  "_comment_on_set": "noyalib_set is intentionally NOT auto-allowed — file-mutation tools should always require confirmation in production agent loops."
}
```
