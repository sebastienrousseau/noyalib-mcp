#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
# Initialize and discover the six registered tools.
# Requires Python 3 and noyalib-mcp on PATH, or NOYALIB_MCP_BIN.

set -euo pipefail
example_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "${PYTHON:-python3}" "$example_dir/mcp-session.py" handshake
