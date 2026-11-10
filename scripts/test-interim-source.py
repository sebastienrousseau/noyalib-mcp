#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Exercise fail-closed source verification against isolated real-source fixtures."""

import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("interim_source", ROOT / "scripts/verify-interim-source.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class SourceVerification(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"], cwd=ROOT))
        cls.package = next(p for p in metadata["packages"] if p["name"] == "const-oid")

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for name in ["Cargo.toml", "fuzz/Cargo.toml", "tests/interim-oid/Cargo.toml", "Cargo.lock", "fuzz/Cargo.lock", "tests/interim-oid/Cargo.lock", "supply-chain/reviews/const-oid-interim.json"]:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, target)
        original = Path(self.package["manifest_path"]).parent
        inventory = json.loads((ROOT / "supply-chain/reviews/const-oid-interim.json").read_text())
        for name in inventory["reviewed_files"]:
            target = self.root / "dependency" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original / name, target)
        package = {**self.package, "manifest_path": str(self.root / "dependency/Cargo.toml")}
        self.metadata = json.dumps({"packages": [package]}).encode()

    def verify(self):
        with patch.object(gate, "ROOT", self.root), patch.object(sys, "argv", ["verify-interim-source.py", "--check-only"]), patch.object(gate.subprocess, "check_output", return_value=self.metadata):
            gate.main()

    def test_reviewed_source_passes(self):
        self.verify()

    def test_different_resolved_revision_fails(self):
        path = self.root / "fuzz/Cargo.lock"
        path.write_text(path.read_text().replace(gate.REVISION, "0" * 40))
        with self.assertRaisesRegex(SystemExit, "reviewed revision"):
            self.verify()

    def test_additional_git_dependency_fails(self):
        path = self.root / "Cargo.lock"
        path.write_text(path.read_text() + '\n[[package]]\nname = "unexpected"\nversion = "1.0.0"\nsource = "git+https://example.invalid/dependency#0000000000000000000000000000000000000000"\n')
        with self.assertRaisesRegex(SystemExit, "unreviewed Git dependency"):
            self.verify()

    def test_modified_dependency_source_fails(self):
        path = self.root / "dependency/src/arcs.rs"
        path.write_text(path.read_text() + "\n// Unreviewed change\n")
        with self.assertRaisesRegex(SystemExit, "reviewed file hashes"):
            self.verify()

    def test_unexpected_source_file_fails(self):
        (self.root / "dependency/src/unreviewed.rs").write_text("// New source\n")
        with self.assertRaisesRegex(SystemExit, "reviewed file hashes"):
            self.verify()

    def test_unreviewed_override_fails(self):
        path = self.root / "Cargo.toml"
        path.write_text(path.read_text().replace(gate.REVISION, "0" * 40))
        with self.assertRaisesRegex(SystemExit, "unexpected patch"):
            self.verify()


if __name__ == "__main__":
    unittest.main()
