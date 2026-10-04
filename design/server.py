#!/usr/bin/env python3
"""The design board's server (see README.md). Serves every round in design/, and keeps each
round's picks and comments in <round>/choices.json, with <round>/decisions.md beside it as the
spec agents build from.

    python3 design/server.py          # http://127.0.0.1:4477
"""

import html
import json
import os
import re
import sys
import tempfile
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.abspath(__file__))
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 4477
API = re.compile(r"^/api/([a-z0-9-]+)/choices$")


def rounds():
    """Each folder with a page is a round."""
    for name in sorted(os.listdir(ROOT)):
        page = os.path.join(ROOT, name, "index.html")
        if os.path.isfile(page) and name != "board":
            with open(page) as file:
                title = re.search(r'data-title="([^"]+)"', file.read())
            yield name, title.group(1) if title else name


def write_atomically(path, text):
    # Write beside the file and rename, so a crash never leaves half a file.
    handle, temporary = tempfile.mkstemp(dir=os.path.dirname(path), suffix=".tmp")
    with os.fdopen(handle, "w") as file:
        file.write(text)
    os.replace(temporary, path)


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def end_headers(self):
        # Designs change between rounds; a refresh should always show the latest.
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, format, *args):
        pass

    def send_body(self, status, body, content_type):
        data = body.encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def send_json(self, status, body):
        self.send_body(status, json.dumps(body), "application/json")

    def round_directory(self):
        match = API.match(self.path.split("?")[0])
        if not match:
            return None
        directory = os.path.join(ROOT, match.group(1))
        return directory if os.path.isfile(os.path.join(directory, "index.html")) else None

    def do_GET(self):
        if self.path in ("/", "/index.html"):
            items = "".join(
                f'<li><a href="/{name}/">{html.escape(title)}</a></li>' for name, title in rounds()
            )
            page = (
                "<!doctype html><meta charset=utf-8><title>agentZ Design Boards</title>"
                '<link rel="stylesheet" href="/board/board.css">'
                '<main class="page" style="margin:0 auto;max-width:720px">'
                "<h2>agentZ design boards</h2><ul style=\"font-size:16px;line-height:2\">"
                f"{items}</ul></main>"
            )
            self.send_body(200, page, "text/html; charset=utf-8")
            return
        if self.path.startswith("/api/"):
            directory = self.round_directory()
            if directory is None:
                self.send_json(404, {"error": "no such round"})
                return
            try:
                with open(os.path.join(directory, "choices.json")) as file:
                    self.send_json(200, json.load(file))
            except FileNotFoundError:
                self.send_json(200, {})
            return
        super().do_GET()

    def do_PUT(self):
        directory = self.round_directory()
        if directory is None:
            self.send_json(404, {"error": "no such round"})
            return
        length = int(self.headers.get("Content-Length", 0))
        try:
            body = json.loads(self.rfile.read(length))
        except ValueError:
            self.send_json(400, {"error": "not JSON"})
            return
        write_atomically(
            os.path.join(directory, "choices.json"),
            json.dumps(body.get("state", {}), indent=2, ensure_ascii=False) + "\n",
        )
        if isinstance(body.get("decisions"), str):
            write_atomically(os.path.join(directory, "decisions.md"), body["decisions"])
        self.send_json(200, {"ok": True})


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    print(f"Design boards: http://127.0.0.1:{PORT}", flush=True)
    server.serve_forever()
