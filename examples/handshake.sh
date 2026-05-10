#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Drive an `initialize` → `tools/list` round-trip against
# `noyalib-mcp`. Useful as a smoke test that the server binary
# is on $PATH and announces the expected tool set.
#
# Transport: stdio with newline-delimited JSON-RPC 2.0
# (per the MCP specification).

set -euo pipefail

{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke-test","version":"0.0.1"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
} | noyalib-mcp
