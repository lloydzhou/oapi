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
        if path in ("/petstore.json", "/upload.json"):
            try:
                with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), path.lstrip("/")), "rb") as f:
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
        elif path == "/api/headers":
            self._send(200, {"authorization": self.headers.get("Authorization", "")})
        elif path == "/api/blob":
            body = bytes([0x00, 0x01, 0xFF, 0xFE]) + "中文二进制".encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif path == "/api/fail":
            self._send(500, {"error": "boom"})
        else:
            self.send_error(404)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length)
        ctype = self.headers.get("Content-Type", "")
        if ctype.startswith("multipart/form-data;"):
            boundary = ctype.split("boundary=")[1].strip().encode()
            self._send(200, {"created": True, "multipart": parse_multipart(raw, boundary)})
            return
        try:
            data = json.loads(raw) if raw else {}
        except ValueError:
            data = {"_raw": raw.decode("utf-8", "replace")}
        self._send(200, {"created": True, "received": data})


def parse_multipart(raw, boundary):
    """Minimal multipart parser: returns [{name, filename, size, text}]."""
    parts = raw.split(b"--" + boundary)
    out = []
    for part in parts:
        if part in (b"", b"--", b"\r\n", b"--\r\n"):
            continue
        part = part.lstrip(b"\r\n")
        head, _, body = part.partition(b"\r\n\r\n")
        headers = {}
        for line in head.decode("utf-8", "replace").split("\r\n"):
            if ":" in line:
                k, v = line.split(":", 1)
                headers[k.strip().lower()] = v.strip()
        disp = headers.get("content-disposition", "")
        name = ""
        filename = None
        for token in disp.split(";"):
            token = token.strip()
            if token.startswith("name="):
                name = token[5:].strip('"')
            elif token.startswith("filename="):
                filename = token[9:].strip('"')
        if body.endswith(b"\r\n"):
            body = body[:-2]
        out.append({
            "name": name,
            "filename": filename,
            "size": len(body),
            "text": body.decode("utf-8", "replace") if filename is None else None,
        })
    return out

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
