#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Call the `format` tool on a poorly-formatted YAML document.
# Demonstrates the byte-faithful CST formatter from an AI-agent
# perspective: comments + indentation preserved through the
# round-trip.

set -euo pipefail

YAML_DOC='# production config\nserver:\n  host:    api.example.com   # endpoint\n  port:8080\n'

{
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"format-demo","version":"0.0.1"}}}'
    echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"format","arguments":{"yaml":"%s"}}}\n' "$YAML_DOC"
} | noyalib-mcp
