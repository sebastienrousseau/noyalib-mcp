// SPDX-FileCopyrightText: 2026 Noyalib
// SPDX-License-Identifier: MIT OR Apache-2.0

// Rehearse recovery with an independently verified, installed npm package and
// its matching native release archive. All changes occur in a disposable home.
"use strict";

const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

async function run() {
    const [installed, archive] = process.argv.slice(2);
    assert(installed && archive, "usage: node scripts/check-wrapper-recovery.js <installed package directory> <verified archive>");
    const pkg = path.resolve(installed);
    const bootstrap = require(path.join(pkg, "bootstrap.js"));
    const manifest = require(path.join(pkg, "package.json"));
    const digests = require(path.join(pkg, "digests.json"));
    const { version } = manifest;
    const { platform, arch } = process;
    assert.equal(manifest.name, "@sebastienrousseau/noyalib-mcp");
    const target = bootstrap.targetTriple(platform, arch);
    const entry = bootstrap.loadEntry(digests, version, target);
    assert.equal(path.basename(archive), entry.archive);
    const bytes = fs.readFileSync(archive);
    assert.equal(bootstrap.sha256(bytes), entry.archive_sha256);

    const home = fs.mkdtempSync(path.join(os.tmpdir(), "noyalib-wrapper-recovery-"));
    try {
        const cacheRoot = path.join(home, ".cache", "noyalib-mcp");
        const name = platform === "win32" ? "noyalib-mcp.exe" : "noyalib-mcp";
        const binary = path.join(cacheRoot, version, name);
        const offline = path.join(home, "offline.cjs");
        fs.writeFileSync(offline, 'require("node:https").get = () => { throw new Error("release download unavailable (offline drill)"); };\n');
        const env = { ...process.env, HOME: home, USERPROFILE: home };
        delete env.NODE_OPTIONS;
        const invoke = (input, args) => {
            const result = spawnSync(process.execPath, ["--require", offline, path.join(pkg, "index.js"), ...args], {
                cwd: home, env, input, encoding: "utf8", timeout: 10000,
            });
            if (result.error) throw result.error;
            return result;
        };

        const missing = invoke(undefined, ["--version"]);
        assert.equal(missing.status, 1);
        assert.match(missing.stderr, /release download unavailable \(offline drill\)/);
        assert(!fs.existsSync(binary));
        await assert.rejects(bootstrap.downloadOrCached(platform, arch, {
            cacheRoot, fetch: async () => Buffer.concat([bytes, Buffer.from([0])]),
        }), /does not match its published SHA-256/);
        assert(!fs.existsSync(binary));

        const restore = async () => {
            assert.equal(await bootstrap.downloadOrCached(platform, arch, {
                cacheRoot, fetch: async () => bytes,
            }), binary);
            assert.equal(bootstrap.sha256(fs.readFileSync(binary)), entry.binary_sha256);
            const warm = invoke(undefined, ["--version"]);
            assert.equal(warm.status, 0, warm.stderr);
            assert(warm.stdout.includes(version), warm.stdout);
        };
        await restore();

        fs.appendFileSync(binary, Buffer.from([0]));
        const corrupted = invoke(undefined, ["--version"]);
        assert.equal(corrupted.status, 1);
        assert.match(corrupted.stderr, /does not match its published digest/);
        assert.equal(corrupted.stdout, "");
        fs.unlinkSync(binary);
        await restore();

        const messages = [
            { jsonrpc: "2.0", id: 1, method: "initialize", params: {
                protocolVersion: "2025-11-25", capabilities: {},
                clientInfo: { name: "offline-recovery", version: "1" },
            } },
            { jsonrpc: "2.0", method: "notifications/initialized" },
            { jsonrpc: "2.0", id: 2, method: "tools/call", params: {
                name: "noyalib_parse", arguments: { yaml: "port: 8080\n" },
            } },
        ];
        const input = messages.map(message => JSON.stringify(message)).join("\n") + "\n";
        const parse = invoke(input, ["--root", home]);
        assert.equal(parse.status, 0, parse.stderr);
        const replies = parse.stdout.trim().split("\n").map(line => JSON.parse(line));
        const result = replies.find(reply => reply.id === 2)?.result;
        assert(result && !result.isError, parse.stdout);
        assert.match(JSON.stringify(result), /8080/);

        console.log(JSON.stringify({
            status: "passed", version, target, node: process.version,
            archive_sha256: entry.archive_sha256, binary_sha256: entry.binary_sha256,
            checks: ["missing cache offline", "corrupt archive refusal", "warm cache offline",
                "corrupt binary refusal", "restore verified cache", "stdio parse offline"],
        }, null, 2));
    } finally {
        fs.rmSync(home, { recursive: true, force: true });
    }
}

run().catch(error => {
    console.error(error);
    process.exitCode = 1;
});
