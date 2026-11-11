#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Run a checked stdio example against the installed server in a scratch root."""

import argparse
import json
import os
from pathlib import Path
from queue import Empty, Queue
import subprocess
import sys
import tempfile
import threading
import time


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(mode):
    binary = os.environ.get('NOYALIB_MCP_BIN', 'noyalib-mcp')
    original = '# example\nserver:\n  host: api.example.com # keep\n  port: 8080\n'
    with tempfile.TemporaryDirectory(prefix='noyalib-mcp-example-') as scratch:
        file = Path(scratch) / 'config.yaml'
        file.write_text(original, encoding='utf-8')
        child = subprocess.Popen([binary, '--root', scratch], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, text=True, encoding='utf-8')
        replies = Queue()

        def read():
            try:
                for line in child.stdout:
                    replies.put(line)
            finally:
                replies.put(None)

        reader = threading.Thread(target=read, daemon=True)
        reader.start()

        def send(method, params=None, request_id=None):
            message = {'jsonrpc': '2.0', 'method': method}
            if params is not None:
                message['params'] = params
            if request_id is not None:
                message['id'] = request_id
            child.stdin.write(json.dumps(message) + '\n')
            child.stdin.flush()

        def receive(request_id):
            deadline = time.monotonic() + 10
            for _ in range(16):
                try:
                    line = replies.get(timeout=max(0, deadline - time.monotonic()))
                except Empty as error:
                    raise TimeoutError(f'no response to request {request_id}') from error
                if line is None:
                    raise ValueError('server closed before answering')
                response = json.loads(line)
                if response.get('id') == request_id:
                    require('error' not in response, f'protocol failure: {response}')
                    print(json.dumps(response), flush=True)
                    return response['result']
            raise ValueError('too many unrelated responses')

        def tool(name, arguments, request_id):
            send('tools/call', {'name': name, 'arguments': arguments}, request_id)
            result = receive(request_id)
            require(not result.get('isError'), f'tool failed: {result}')
            return result['structuredContent']

        try:
            send('initialize', {'protocolVersion': '2025-11-25', 'capabilities': {},
                                'clientInfo': {'name': 'noyalib-example', 'version': '1'}}, 1)
            info = receive(1)
            require(info['serverInfo']['name'] == 'noyalib-mcp', 'unexpected server')
            send('notifications/initialized')
            if mode == 'handshake':
                send('tools/list', {}, 2)
                names = {item['name'] for item in receive(2)['tools']}
                require(names == {'noyalib_get', 'noyalib_set', 'noyalib_set_multidoc',
                                  'noyalib_parse', 'noyalib_edit', 'noyalib_validate'},
                        f'unexpected catalogue: {names}')
            elif mode == 'edit-and-get':
                result = tool('noyalib_set', {'file': 'config.yaml', 'path': 'server.port',
                                             'value': '9090'}, 2)
                require(result['value'] == '9090', 'set returned an unexpected value')
                require(file.read_text(encoding='utf-8') == original.replace('8080', '9090'),
                        'edit changed surrounding source')
                result = tool('noyalib_get', {'file': 'config.yaml', 'path': 'server.port'}, 3)
                require(result == {'value': '9090'}, 'readback disagrees with the edit')
            else:
                send('prompts/get', {'name': 'format_and_lint_yaml',
                                     'arguments': {'file': 'config.yaml'}}, 2)
                require('noyalib_set' in json.dumps(receive(2)['messages']),
                        'prompt omitted the documented workflow')
        finally:
            try:
                child.stdin.close()
            except OSError:
                pass
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
            reader.join(timeout=1)
            child.stdout.close()
        require(child.returncode == 0, f'server exited with status {child.returncode}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['handshake', 'edit-and-get', 'prompt'])
    args = parser.parse_args()
    try:
        run(args.mode)
    except Exception as error:
        print(f'noyalib-mcp example: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
