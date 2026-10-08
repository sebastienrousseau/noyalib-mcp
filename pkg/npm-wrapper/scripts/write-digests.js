// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 Noyalib. All rights reserved.

// Write digests.json, the file the wrapper trusts: the SHA-256 of each
// release archive and of the binary inside it, for every target
// bootstrap.js maps a platform to. Run by the release workflow after
// the per-target archives are downloaded, and tested by
// test/write-digests.test.js so a broken step is caught in CI, not at
// release time (v0.0.55's run failed here on the Windows .zip).
//
//   node scripts/write-digests.js <version> <archive-dir>

"use strict";

const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const crypto = require("node:crypto");
const { execFileSync } = require("node:child_process");

const sha256 = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

/** The Rust target triples bootstrap.js maps platforms to. */
function targetTriples(bootstrapSource) {
  return [...bootstrapSource.matchAll(/"[a-z0-9]+-[a-z0-9]+":\s+"([^"]+)"/g)].map((m) => m[1]);
}

function findFile(dir, name) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isFile() && entry.name === name) return full;
    if (entry.isDirectory()) {
      const found = findFile(full, name);
      if (found) return found;
    }
  }
  return null;
}

/** Unpack `file` into a fresh directory; GNU tar cannot read a .zip. */
function unpack(file, isZip) {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), "digest-"));
  if (isZip) execFileSync("unzip", ["-q", file, "-d", work]);
  else execFileSync("tar", ["-xf", file, "-C", work]);
  return work;
}

/** The digests object for `version`, reading archives from `dir`. */
function computeDigests(version, dir, bootstrapSource) {
  const targets = {};
  for (const triple of targetTriples(bootstrapSource)) {
    const isZip = triple.includes("windows");
    const archive = `noyalib-mcp-${version}-${triple}.${isZip ? "zip" : "tar.gz"}`;
    const file = path.join(dir, archive);
    if (!fs.existsSync(file)) throw new Error(`no release archive for ${triple}: ${archive}`);
    const work = unpack(file, isZip);
    try {
      const bin = findFile(work, isZip ? "noyalib-mcp.exe" : "noyalib-mcp");
      if (!bin) throw new Error(`${archive} holds no binary`);
      targets[triple] = {
        archive,
        archive_sha256: sha256(fs.readFileSync(file)),
        binary_sha256: sha256(fs.readFileSync(bin)),
      };
    } finally {
      fs.rmSync(work, { recursive: true, force: true });
    }
  }
  return { version, targets };
}

module.exports = { computeDigests, targetTriples };

if (require.main === module) {
  const [version, dir] = process.argv.slice(2);
  if (!version || !dir) {
    console.error("usage: node scripts/write-digests.js <version> <archive-dir>");
    process.exit(2);
  }
  const root = path.join(__dirname, "..");
  const digests = computeDigests(version, dir, fs.readFileSync(path.join(root, "bootstrap.js"), "utf8"));
  fs.writeFileSync(path.join(root, "digests.json"), JSON.stringify(digests, null, 2) + "\n");
  console.log(`digests.json: ${Object.keys(digests.targets).length} targets`);
}
