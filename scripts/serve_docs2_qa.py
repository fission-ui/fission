#!/usr/bin/env python3
"""Serve generated docs2 output locally with the Web SQLite prerequisites.

QA helper only: these headers do not configure the production documentation
host. No builder, content rewrite, SPA fallback or alternative storage provider
is involved. By default this serves docs2/dist/site on loopback port 6467.
"""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class DocsQaHandler(SimpleHTTPRequestHandler):
    extensions_map = {
        **SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".mjs": "text/javascript",
    }

    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=6467)
    parser.add_argument(
        "--directory",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "docs2/dist/site",
    )
    arguments = parser.parse_args()
    if not 0 <= arguments.port <= 65535:
        parser.error("port must be between 0 and 65535 (0 selects an available port)")
    directory = arguments.directory.resolve()
    if not directory.is_dir():
        parser.error(f"site directory does not exist: {directory}; build docs2 first")
    handler = partial(DocsQaHandler, directory=str(directory))
    with ThreadingHTTPServer(("127.0.0.1", arguments.port), handler) as server:
        print(f"Docs2 QA: http://127.0.0.1:{server.server_port}/ ({directory})", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
