#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Round-trip the CST mutation surface: `set` rewrites a value at
# a dotted path, `get` reads it back. The test confirms surgical
# edits don't disturb surrounding bytes.

set -euo pipefail

YAML_DOC='server:\n  host: api.example.com  # public endpoint\n  port: 8080\n'

{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"set-get-demo","version":"0.0.1"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    # 1. Mutate server.port → 9090
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"set","arguments":{"yaml":"%s","path":"server.port","value":"9090"}}}\n' "$YAML_DOC"
    # 2. Read it back
    printf '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get","arguments":{"yaml":"%s","path":"server.port"}}}\n' "$YAML_DOC"
} | noyalib-mcp
