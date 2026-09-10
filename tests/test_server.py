#!/usr/bin/env python3
"""Test HTTP server for oapi e2e: serves the spec file and API endpoints."""
import json
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

ROOT = sys.argv[1]
PIDFILE = os.path.join(ROOT, "test.pid")

def daemonize():
    pid = os.fork()
    if pid > 0:
        for _ in range(50):
            if os.path.exists(PIDFILE):
                break
            time.sleep(0.05)
        sys.exit(0)
    os.setsid()
    pid = os.fork()
    if pid > 0:
        sys.exit(0)
    os.chdir("/")
    os.umask(0)
    for fd in range(0, 3):
        try:
            os.close(fd)
        except OSError:
            pass
    sys.stdin = open(os.devnull, "r")
    sys.stdout = open(os.devnull, "w")
    sys.stderr = open(os.devnull, "w")

if len(sys.argv) > 3 and sys.argv[3] == "--daemon":
    daemonize()

class Handler(BaseHTTPRequestHandler):
    def _send(self, status, obj):
        body = (json.dumps(obj) + "\n").encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

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

with open(PIDFILE, "w") as f:
    f.write(str(os.getpid()))

port = int(sys.argv[2])
server = HTTPServer(("127.0.0.1", port), Handler)
server.serve_forever()
