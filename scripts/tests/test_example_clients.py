#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Execute the documented clients against the selected real server binary."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ExampleClients(unittest.TestCase):
    def server_binary(self):
        target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
        local = target / 'debug' / ('noyalib-mcp.exe' if os.name == 'nt' else 'noyalib-mcp')
        binary = Path(os.environ.get('NOYALIB_MCP_BIN') or shutil.which('noyalib-mcp') or local)
        self.assertTrue(binary.is_file(), 'set NOYALIB_MCP_BIN to the built server')
        return binary.resolve()

    def run_example(self, name):
        binary = self.server_binary()
        env = {**os.environ, 'NOYALIB_MCP_BIN': str(binary), 'PYTHON': sys.executable,
               'PATH': str(binary.parent) + os.pathsep + os.environ['PATH']}
        with tempfile.TemporaryDirectory(prefix='noyalib-example-caller-') as scratch:
            sentinel = Path(scratch) / 'config.yaml'
            sentinel.write_text('port: 7000 # caller owns this\n')
            shell = shutil.which('bash', path=env['PATH'])
            self.assertIsNotNone(shell, 'Bash must be installed for the shell examples')
            shell = str(Path(shell).resolve())
            process = subprocess.run([shell, str(ROOT / 'examples' / name)], cwd=scratch,
                                     env=env, capture_output=True, text=True, timeout=20)
            self.assertEqual(sentinel.read_text(), 'port: 7000 # caller owns this\n')
        self.assertEqual(process.returncode, 0,
                         f'launcher={shell!r}\nstdout:\n{process.stdout}\nstderr:\n{process.stderr}')
        messages = [json.loads(line) for line in process.stdout.splitlines() if line.strip()]
        self.assertTrue(messages, 'example emitted no protocol responses')
        self.assertTrue(all('error' not in response for response in messages), messages)
        return {response['id']: response['result'] for response in messages if 'id' in response}

    def test_successful_responses_do_not_hide_a_failed_server_exit(self):
        binary = self.server_binary()
        with tempfile.TemporaryDirectory(prefix='noyalib-example-exit-') as scratch:
            source = Path(scratch) / 'wrapper.rs'
            wrapper = Path(scratch) / ('wrapper.exe' if os.name == 'nt' else 'wrapper')
            source.write_text('#![forbid(unsafe_code)]\nfn main() {\n    let _ = std::process::Command::new(std::env::var_os("NOYALIB_REAL_MCP_BIN").unwrap())\n        .args(std::env::args_os().skip(1)).status().unwrap();\n    std::process::exit(7);\n}\n')
            subprocess.run(['rustc', str(source), '-o', str(wrapper)], check=True,
                           capture_output=True, timeout=30)
            env = {**os.environ, 'NOYALIB_MCP_BIN': str(wrapper),
                   'NOYALIB_REAL_MCP_BIN': str(binary)}
            process = subprocess.run([sys.executable, str(ROOT / 'examples/mcp-session.py'),
                                      'handshake'], env=env, capture_output=True,
                                     text=True, timeout=20)
        self.assertNotEqual(process.returncode, 0, process.stdout)
        self.assertIn('exited with status 7', process.stderr)

    def test_handshake_waits_for_the_complete_catalogue(self):
        responses = self.run_example('handshake.sh')
        self.assertEqual(responses[1]['serverInfo']['name'], 'noyalib-mcp')
        self.assertEqual({tool['name'] for tool in responses[2]['tools']}, {
            'noyalib_get', 'noyalib_set', 'noyalib_set_multidoc',
            'noyalib_parse', 'noyalib_edit', 'noyalib_validate'})

    def test_file_edit_and_readback_use_the_registered_tools(self):
        responses = self.run_example('set-then-get.sh')
        self.assertFalse(responses[2].get('isError', False), responses)
        self.assertEqual(responses[2]['structuredContent']['value'], '9090')
        self.assertFalse(responses[3].get('isError', False), responses)
        self.assertEqual(responses[3]['structuredContent'], {'value': '9090'})

    def test_format_example_requests_the_existing_prompt(self):
        responses = self.run_example('format-call.sh')
        self.assertIn('noyalib_set', json.dumps(responses[2]['messages']))


if __name__ == '__main__':
    unittest.main()
