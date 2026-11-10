#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify the reviewed registry source used by every MCP build graph."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent
VERSION = "0.10.3"
CHECKSUM = "be95d4a884358a21a199689323c29e9256b4a1b315e22f183d155422041d3a9a"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
SOURCE = REGISTRY


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def read_toml(path):
    return tomllib.loads(path.read_text(encoding="utf-8"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-only", action="store_true", help="check identity without rerunning regressions")
    args = parser.parse_args()
    for relative in ["Cargo.toml", "fuzz/Cargo.toml"]:
        manifest = read_toml(ROOT / relative)
        require(not manifest.get("patch"), f"{relative}: unexpected patch")
    harness = ROOT / "tests/const-oid-regression/Cargo.toml"
    require(read_toml(harness)["dependencies"]["const-oid"] == "=" + VERSION, "regression harness must use the same fixed registry version")
    for relative in ["Cargo.lock", "fuzz/Cargo.lock", "tests/const-oid-regression/Cargo.lock"]:
        packages = read_toml(ROOT / relative)["package"]
        oids = [p for p in packages if p["name"] == "const-oid"]
        require(len(oids) == 1 and oids[0]["source"] == SOURCE and oids[0]["version"] == VERSION and oids[0]["checksum"] == CHECKSUM, f"{relative}: const-oid does not resolve to the reviewed registry version")
        require(not any(p.get("source", "").startswith("git+") for p in packages), f"{relative}: unreviewed Git dependency")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"], cwd=ROOT))
    packages = [p for p in metadata["packages"] if p["name"] == "const-oid"]
    require(len(packages) == 1 and packages[0]["source"] == SOURCE and packages[0]["version"] == VERSION, "resolved source differs from the reviewed registry version")
    package = packages[0]
    crate = Path(package["manifest_path"]).parent
    review = json.loads((ROOT / "supply-chain/reviews/const-oid-0.10.3.json").read_text())
    require(review["version"] == VERSION and review["archive_sha256"] == CHECKSUM, "review identity mismatch")
    files = [crate / "Cargo.toml", crate / "README.md", *sorted((crate / "src").rglob("*"))]
    actual = {str(p.relative_to(crate)).replace(os.sep, "/"): hashlib.sha256(p.read_bytes()).hexdigest() for p in files if p.is_file()}
    require(actual == review["reviewed_files"], "resolved const-oid source differs from the reviewed file hashes")
    require(not (crate / "build.rs").exists(), "unexpected const-oid build script")
    print(f"Verified registry const-oid {VERSION} source and reviewed file hashes", flush=True)
    if not args.check_only:
        subprocess.run(["cargo", "test", "--manifest-path", str(harness), "--locked"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
