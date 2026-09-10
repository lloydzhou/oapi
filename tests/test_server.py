#!/usr/bin/env python3
"""Test HTTP server for oapi e2e: serves the spec file and API endpoints.

Run in the foreground; the caller backgrounds it and waits for $ROOT/ready.
"""
import os
import sys

BOOTFILE = os.path.join(sys.argv[1], "boot")
try:
    with open(BOOTFILE, "w") as f:
        f.write("boot\n")
except OSError:
    pass

import json
from http.server import BaseHTTPRequestHandler, HTTPServer

ROOT = sys.argv[1]
PORT = int(sys.argv[2])
PIDFILE = os.path.join(ROOT, "test.pid")
READYFILE = os.path.join(ROOT, "ready")
ERRFILE = os.path.join(ROOT, "error.log")

class Handler(BaseHTTPRequestHandler):
    def _send(self, status, obj):
        body = (json.dumps(obj) + "\n").encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass

    def do_GET(self):
        path = self.path.split("?")[0]
        if path == "/petstore.json":
            try:
                with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "petstore.json"), "rb") as f:
                    body = f.read()
            except OSError:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/api/pets":
            self._send(200, [{"id": 1, "name": "fluffy"}, {"id": 2, "name": "brie"}])
        elif path.startswith("/api/pets/"):
            pid = path.rsplit("/", 1)[1]
            self._send(200, {"id": pid, "name": "pet-" + pid})
        elif path == "/api/fail":
            self._send(500, {"error": "boom"})
        else:
            self.send_error(404)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        try:
            data = json.loads(raw) if raw else {}
        except ValueError:
            data = {"_raw": raw.decode("utf-8", "replace")}
        self._send(200, {"created": True, "received": data})

try:
    server = HTTPServer(("127.0.0.1", PORT), Handler)
except Exception as e:
    with open(ERRFILE, "w") as f:
        f.write(repr(e) + "\n")
    sys.exit(1)

with open(PIDFILE, "w") as f:
    f.write(str(os.getpid()))
with open(READYFILE, "w") as f:
    f.write("ok\n")

server.serve_forever()
