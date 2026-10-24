# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Local, controllable resource for real export cancellation acceptance."""
import argparse
import http.server
import json
from pathlib import Path
import time

parser = argparse.ArgumentParser()
parser.add_argument('output', type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
png = bytes.fromhex('89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d49444154789c63f8cfc0f01f00050001ff89993d1d0000000049454e44ae426082')


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with (args.output / 'requests.log').open('a') as log:
            log.write(str(time.time()) + '\n')
        until = time.monotonic() + 30
        while not (args.output / 'release').exists() and time.monotonic() < until:
            time.sleep(.02)
        if not (args.output / 'release').exists():
            return
        try:
            self.send_response(200)
            self.send_header('Content-Type', 'image/png')
            self.send_header('Content-Length', str(len(png)))
            self.end_headers()
            self.wfile.write(png)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *_):
        pass


server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
server.daemon_threads = True
(args.output / 'ready.json').write_text(json.dumps({'port': server.server_port}))
server.serve_forever()
