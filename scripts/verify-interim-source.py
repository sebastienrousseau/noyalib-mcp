#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify the reviewed Git source used exclusively by interim MCP builds."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent
REVISION = "399ec598b509a728eb13f4e82be0a2526137e81f"
REPOSITORY = "https://github.com/RustCrypto/formats.git"
SOURCE = f"git+{REPOSITORY}?rev={REVISION}#{REVISION}"


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def read_toml(path):
    return tomllib.loads(path.read_text(encoding="utf-8"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-only", action="store_true", help="check identity without rerunning regressions")
    args = parser.parse_args()
    expected = {"git": REPOSITORY, "rev": REVISION}
    for relative in ["Cargo.toml", "fuzz/Cargo.toml"]:
        manifest = read_toml(ROOT / relative)
        require(manifest.get("patch", {}).get("crates-io") == {"const-oid": expected}, f"{relative}: unexpected patch")
    harness = ROOT / "tests/interim-oid/Cargo.toml"
    require(read_toml(harness)["dependencies"]["const-oid"] == expected, "regression harness must use the same fixed source")
    for relative in ["Cargo.lock", "fuzz/Cargo.lock", "tests/interim-oid/Cargo.lock"]:
        packages = read_toml(ROOT / relative)["package"]
        oids = [p for p in packages if p["name"] == "const-oid"]
        require(len(oids) == 1 and oids[0]["source"] == SOURCE, f"{relative}: const-oid does not resolve to the reviewed revision")
        require(all(p.get("source") == SOURCE and p["name"] == "const-oid" for p in packages if p.get("source", "").startswith("git+")), f"{relative}: unreviewed Git dependency")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"], cwd=ROOT))
    packages = [p for p in metadata["packages"] if p["name"] == "const-oid"]
    require(len(packages) == 1 and packages[0]["source"] == SOURCE, "resolved source differs from the reviewed revision")
    package = packages[0]
    crate = Path(package["manifest_path"]).parent
    review = json.loads((ROOT / "supply-chain/reviews/const-oid-interim.json").read_text())
    require(review["revision"] == REVISION and review["repository"] == REPOSITORY, "review identity mismatch")
    files = [crate / "Cargo.toml", crate / "README.md", *sorted((crate / "src").rglob("*"))]
    actual = {str(p.relative_to(crate)).replace(os.sep, "/"): hashlib.sha256(p.read_bytes()).hexdigest() for p in files if p.is_file()}
    require(actual == review["reviewed_files"], "resolved const-oid source differs from the reviewed file hashes")
    require(not (crate / "build.rs").exists(), "unexpected const-oid build script")
    print(f"Verified exact const-oid Git source and reviewed file hashes: {REVISION}", flush=True)
    if not args.check_only:
        subprocess.run(["cargo", "test", "--manifest-path", str(harness), "--locked"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
