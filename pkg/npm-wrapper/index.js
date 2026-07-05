#!/usr/bin/env node
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

// `noyalib-mcp` — npm-installable wrapper that downloads the
// platform-appropriate `noyalib-mcp` binary from the matching
// GitHub Release on first run, caches it under
// `~/.cache/noyalib-mcp/<version>/`, and exec's into it.
//
// Lets AI agents (Claude Code, GitHub Copilot, …) call the MCP
// server with `npx noyalib-mcp` — no Rust toolchain required.

"use strict";

const { spawn } = require("node:child_process");
const { downloadOrCached } = require("./bootstrap");

(async () => {
    try {
        const binary = await downloadOrCached(process.platform, process.arch);
        const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });
        child.on("exit", (code, signal) => {
            if (signal) {
                process.kill(process.pid, signal);
            } else {
                process.exit(code ?? 1);
            }
        });
    } catch (err) {
        // eslint-disable-next-line no-console
        console.error(`noyalib-mcp wrapper: ${err.message}`);
        process.exit(1);
    }
})();
