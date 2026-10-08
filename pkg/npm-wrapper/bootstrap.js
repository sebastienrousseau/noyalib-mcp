// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

// Download, verify and cache the `noyalib-mcp` binary for this
// platform. Fails closed: nothing is executed unless its SHA-256
// matches `digests.json`, which ships inside this npm package (and so
// is covered by its provenance attestation).
//
// digests.json, written by the release workflow:
//
//   {
//     "version": "0.0.56",
//     "targets": {
//       "x86_64-unknown-linux-musl": {
//         "archive": "noyalib-mcp-0.0.56-x86_64-unknown-linux-musl.tar.gz",
//         "archive_sha256": "<64 hex>",
//         "binary_sha256": "<64 hex>"
//       }
//     }
//   }
//
// The archive is checked before it is unpacked, the binary after, and
// the cached binary again before every run.

"use strict";

const crypto    = require("node:crypto");
const fs        = require("node:fs");
const fsp       = require("node:fs/promises");
const path      = require("node:path");
const os        = require("node:os");
const https     = require("node:https");
const { spawnSync } = require("node:child_process");

const PKG = require("./package.json");

const REPO = "sebastienrousseau/noyalib-mcp";

// Hosts a release download may be served from: the release URL itself
// and the hosts GitHub redirects release assets to.
const ALLOWED_HOSTS = new Set([
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
]);
const MAX_REDIRECTS = 5;
const MAX_ARCHIVE_BYTES = 256 * 1024 * 1024;

// Node's runtime identifiers to the Rust target triples of the
// release archives.
const TARGET_TABLE = {
    "linux-x64":     "x86_64-unknown-linux-musl",
    "linux-arm64":   "aarch64-unknown-linux-musl",
    "darwin-x64":    "x86_64-apple-darwin",
    "darwin-arm64":  "aarch64-apple-darwin",
    "win32-x64":     "x86_64-pc-windows-msvc",
};

function alternatives(version) {
    return "Install it another way instead:\n"
        + `  cargo install noyalib-mcp --version ${version} --locked\n`
        + `  docker run -i --rm ghcr.io/${REPO}:${version}`;
}

function targetTriple(platform, arch) {
    const key = `${platform}-${arch}`;
    const triple = TARGET_TABLE[key];
    if (!triple) {
        throw new Error(`no prebuilt binary for ${key}.\n${alternatives(PKG.version)}`);
    }
    return triple;
}

function cacheDir(version, root = path.join(os.homedir(), ".cache", "noyalib-mcp")) {
    return path.join(root, version);
}

function sha256(buffer) {
    return crypto.createHash("sha256").update(buffer).digest("hex");
}

// The digest entry for `triple`, or an error that says what to do.
function loadEntry(digests, version, triple) {
    const entry = digests && digests.version === version && digests.targets
        ? digests.targets[triple]
        : undefined;
    const hex = /^[0-9a-f]{64}$/;
    if (!entry || !hex.test(entry.archive_sha256 || "") || !hex.test(entry.binary_sha256 || "")
        || typeof entry.archive !== "string" || !/^[\w.-]+$/.test(entry.archive)) {
        throw new Error(
            `this package carries no verified digest for ${triple} at ${version}, `
            + `so it will not download or run a binary.\n${alternatives(version)}`,
        );
    }
    return entry;
}

function readDigests(file = path.join(__dirname, "digests.json")) {
    try {
        return JSON.parse(fs.readFileSync(file, "utf8"));
    } catch {
        return undefined;
    }
}

// Refuse any URL that is not https on a GitHub release host.
function checkUrl(url) {
    const u = new URL(url);
    if (u.protocol !== "https:" || !ALLOWED_HOSTS.has(u.hostname)) {
        throw new Error(`refusing to download from ${u.protocol}//${u.hostname}`);
    }
    return u;
}

// GET `url` into memory, following at most MAX_REDIRECTS redirects,
// each to an allowed host.
function fetchBuffer(url, redirects = 0) {
    return new Promise((resolve, reject) => {
        let u;
        try {
            u = checkUrl(url);
        } catch (e) {
            reject(e);
            return;
        }
        const headers = { "user-agent": `noyalib-mcp-npm/${PKG.version}` };
        const req = https.get(u, { headers }, (res) => {
            const status = res.statusCode ?? 500;
            if (status >= 300 && status < 400 && res.headers.location) {
                res.resume();
                if (redirects >= MAX_REDIRECTS) {
                    reject(new Error(`too many redirects for ${url}`));
                    return;
                }
                const next = new URL(res.headers.location, u).toString();
                resolve(fetchBuffer(next, redirects + 1));
                return;
            }
            if (status >= 400) {
                res.resume();
                reject(new Error(`HTTP ${status} for ${url}`));
                return;
            }
            const chunks = [];
            let size = 0;
            res.on("data", (chunk) => {
                size += chunk.length;
                if (size > MAX_ARCHIVE_BYTES) {
                    req.destroy(new Error(`download larger than ${MAX_ARCHIVE_BYTES} bytes`));
                    return;
                }
                chunks.push(chunk);
            });
            res.on("end", () => resolve(Buffer.concat(chunks)));
            res.on("error", reject);
        });
        req.on("error", reject);
    });
}

function releaseUrl(version, archive) {
    return `https://github.com/${REPO}/releases/download/v${version}/${archive}`;
}

// The cached binary, if present: verified, or an error. `undefined`
// when there is nothing cached.
async function verifiedCached(binary, entry) {
    let bytes;
    try {
        bytes = await fsp.readFile(binary);
    } catch {
        return undefined;
    }
    if (sha256(bytes) !== entry.binary_sha256) {
        throw new Error(
            `the cached binary ${binary} does not match its published digest; `
            + "it will not be run. Delete it to download a fresh copy.",
        );
    }
    return binary;
}

// Unpack a verified archive into a fresh staging directory beside the
// cache, check the binary, and move it into place.
async function install(archiveBytes, entry, dir, binaryName) {
    const staging = await fsp.mkdtemp(path.join(dir, ".staging-"));
    try {
        const archivePath = path.join(staging, entry.archive);
        await fsp.writeFile(archivePath, archiveBytes, { mode: 0o600 });
        // bsdtar (macOS, Windows 10+) and GNU tar both unpack .tar.gz;
        // bsdtar also unpacks .zip.
        const tar = spawnSync("tar", ["-xf", archivePath, "-C", staging], { stdio: "inherit" });
        if (tar.status !== 0) {
            throw new Error(`tar exited ${tar.status}`);
        }
        const found = await findFile(staging, binaryName);
        if (!found) {
            throw new Error(`the archive does not contain ${binaryName}`);
        }
        if (sha256(await fsp.readFile(found)) !== entry.binary_sha256) {
            throw new Error("the unpacked binary does not match its published digest");
        }
        await fsp.chmod(found, 0o755);
        const binary = path.join(dir, binaryName);
        await fsp.rename(found, binary);
        return binary;
    } finally {
        await fsp.rm(staging, { recursive: true, force: true });
    }
}

async function findFile(dir, name) {
    for (const item of await fsp.readdir(dir, { withFileTypes: true })) {
        const full = path.join(dir, item.name);
        if (item.isFile() && item.name === name) {
            return full;
        }
        if (item.isDirectory()) {
            const inner = await findFile(full, name);
            if (inner) {
                return inner;
            }
        }
    }
    return undefined;
}

// The path of a verified binary for this platform, downloading it on
// first use. `options` exists for tests: { digests, cacheRoot, fetch }.
async function downloadOrCached(platform, arch, options = {}) {
    const version = PKG.version;
    const triple = targetTriple(platform, arch);
    const digests = "digests" in options ? options.digests : readDigests();
    const entry = loadEntry(digests, version, triple);
    const dir = cacheDir(version, options.cacheRoot);
    const binaryName = platform === "win32" ? "noyalib-mcp.exe" : "noyalib-mcp";
    const binary = path.join(dir, binaryName);

    const cached = await verifiedCached(binary, entry);
    if (cached) {
        return cached;
    }

    await fsp.mkdir(dir, { recursive: true, mode: 0o700 });
    const url = releaseUrl(version, entry.archive);
    process.stderr.write(`noyalib-mcp: fetching ${url} (first run only)\n`);
    const archiveBytes = await (options.fetch || fetchBuffer)(url);
    if (sha256(archiveBytes) !== entry.archive_sha256) {
        throw new Error(`${entry.archive} does not match its published SHA-256; refusing to unpack it`);
    }
    return install(archiveBytes, entry, dir, binaryName);
}

module.exports = {
    downloadOrCached,
    targetTriple,
    cacheDir,
    checkUrl,
    loadEntry,
    verifiedCached,
    sha256,
};
