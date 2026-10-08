// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

// The release step that writes digests.json, run against real .tar.gz
// and .zip archives for every target, so it cannot first fail at
// release time. Run: node --test test/

"use strict";

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { execFileSync } = require("node:child_process");
const { computeDigests, targetTriples } = require("../scripts/write-digests.js");

const bootstrap = fs.readFileSync(path.join(__dirname, "..", "bootstrap.js"), "utf8");
const sha256 = (b) => crypto.createHash("sha256").update(b).digest("hex");

function archives(version) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "archives-"));
  for (const triple of targetTriples(bootstrap)) {
    const isZip = triple.includes("windows");
    const name = `noyalib-mcp-${version}-${triple}`;
    const stage = path.join(dir, "stage", name);
    fs.mkdirSync(stage, { recursive: true });
    fs.writeFileSync(path.join(stage, isZip ? "noyalib-mcp.exe" : "noyalib-mcp"), `binary for ${triple}`);
    const cwd = path.join(dir, "stage");
    if (isZip) execFileSync("zip", ["-q", "-r", path.join(dir, `${name}.zip`), name], { cwd });
    else execFileSync("tar", ["-czf", path.join(dir, `${name}.tar.gz`), name], { cwd });
  }
  return dir;
}

test("every target, the Windows zip included, gets both digests", () => {
  const triples = targetTriples(bootstrap);
  assert.ok(triples.some((t) => t.includes("windows")), "a zip target exists");
  const dir = archives("9.9.9");
  const digests = computeDigests("9.9.9", dir, bootstrap);
  assert.strictEqual(digests.version, "9.9.9");
  assert.deepStrictEqual(Object.keys(digests.targets).sort(), [...triples].sort());
  for (const [triple, entry] of Object.entries(digests.targets)) {
    const isZip = triple.includes("windows");
    assert.strictEqual(entry.archive_sha256, sha256(fs.readFileSync(path.join(dir, entry.archive))));
    assert.strictEqual(entry.binary_sha256, sha256(Buffer.from(`binary for ${triple}`)), isZip ? "zip" : "tar.gz");
  }
});

test("a missing archive fails the step instead of a user's install", () => {
  const dir = archives("9.9.9");
  const missing = fs.readdirSync(dir).find((f) => f.endsWith(".zip"));
  fs.rmSync(path.join(dir, missing));
  assert.throws(() => computeDigests("9.9.9", dir, bootstrap), /no release archive/);
});
