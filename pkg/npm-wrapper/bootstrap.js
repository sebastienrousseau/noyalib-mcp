// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

// Download + cache resolver for the npm wrapper. Maps node's
// (process.platform, process.arch) tuple to the matching tarball
// name under the GitHub Release for this package version.

"use strict";

const fs        = require("node:fs");
const fsp       = require("node:fs/promises");
const path      = require("node:path");
const os        = require("node:os");
const https     = require("node:https");
const { pipeline } = require("node:stream/promises");
const zlib      = require("node:zlib");
const { spawn } = require("node:child_process");

const PKG = require("./package.json");

// Maps Node's runtime identifiers to Rust target triples used in
// the GitHub Release artefact filenames.
const TARGET_TABLE = {
    "linux-x64":     "x86_64-unknown-linux-musl",
    "linux-arm64":   "aarch64-unknown-linux-musl",
    "linux-arm":     "armv7-unknown-linux-gnueabihf",
    "darwin-x64":    "x86_64-apple-darwin",
    "darwin-arm64":  "aarch64-apple-darwin",
    "win32-x64":     "x86_64-pc-windows-msvc",
    "win32-arm64":   "aarch64-pc-windows-msvc",
};

function targetTriple(platform, arch) {
    const key = `${platform}-${arch}`;
    const triple = TARGET_TABLE[key];
    if (!triple) {
        throw new Error(`unsupported platform: ${key}`);
    }
    return triple;
}

function cacheDir(version) {
    return path.join(os.homedir(), ".cache", "noyalib-mcp", version);
}

async function exists(p) {
    try {
        await fsp.access(p, fs.constants.X_OK);
        return true;
    } catch {
        return false;
    }
}

function fetch(url) {
    return new Promise((resolve, reject) => {
        const req = https.get(url, { headers: { "user-agent": `noyalib-mcp-npm/${PKG.version}` } }, (res) => {
            if (res.statusCode === 302 || res.statusCode === 301) {
                resolve(fetch(res.headers.location));
                return;
            }
            if ((res.statusCode ?? 500) >= 400) {
                reject(new Error(`HTTP ${res.statusCode} for ${url}`));
                return;
            }
            resolve(res);
        });
        req.on("error", reject);
    });
}

async function downloadOrCached(platform, arch) {
    const triple = targetTriple(platform, arch);
    const ext    = platform === "win32" ? ".exe" : "";
    const dir    = cacheDir(PKG.version);
    const binary = path.join(dir, `noyalib-mcp${ext}`);

    if (await exists(binary)) {
        return binary;
    }

    await fsp.mkdir(dir, { recursive: true });

    const archiveExt = platform === "win32" ? "zip" : "tar.gz";
    const archive    = `noyalib-${PKG.version}-${triple}.${archiveExt}`;
    const url        = `https://github.com/sebastienrousseau/noyalib-mcp/releases/download/v${PKG.version}/${archive}`;

    process.stderr.write(`noyalib-mcp: fetching ${url} (first run only) …\n`);

    if (archiveExt === "tar.gz") {
        const res = await fetch(url);
        const tar = spawn("tar", ["-xzf", "-", "-C", dir, "--strip-components=1"], {
            stdio: ["pipe", "inherit", "inherit"],
        });
        await pipeline(res, tar.stdin);
        await new Promise((resolve, reject) => {
            tar.on("exit", (code) =>
                code === 0 ? resolve() : reject(new Error(`tar exited ${code}`)),
            );
        });
    } else {
        // Windows .zip path — defer to PowerShell's Expand-Archive
        // since Node's stdlib doesn't include a zip extractor and we
        // do not want to add a runtime dependency.
        const tmpZip = path.join(dir, archive);
        const res = await fetch(url);
        await pipeline(res, fs.createWriteStream(tmpZip));
        const ps = spawn("powershell.exe", [
            "-NoProfile", "-Command",
            `Expand-Archive -Path "${tmpZip}" -DestinationPath "${dir}" -Force`,
        ], { stdio: "inherit" });
        await new Promise((resolve, reject) => {
            ps.on("exit", (code) =>
                code === 0 ? resolve() : reject(new Error(`Expand-Archive exited ${code}`)),
            );
        });
        await fsp.unlink(tmpZip);
    }

    if (!(await exists(binary))) {
        throw new Error(`extracted archive does not contain expected binary: ${binary}`);
    }
    return binary;
}

module.exports = { downloadOrCached, targetTriple, cacheDir };
