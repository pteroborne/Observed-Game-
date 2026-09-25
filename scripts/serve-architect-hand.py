#!/usr/bin/env python3
"""Small gzip-aware static server for the Architect Hand Lab."""

from __future__ import annotations

import argparse
import functools
import gzip
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

COMPRESSIBLE = {".js", ".wasm"}


def refresh_gzip(root: Path) -> None:
    for source in root.iterdir():
        if not source.is_file() or source.suffix not in COMPRESSIBLE:
            continue
        compressed = source.with_name(f"{source.name}.gz")
        if compressed.exists() and compressed.stat().st_mtime_ns >= source.stat().st_mtime_ns:
            continue
        with source.open("rb") as reader, compressed.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=0) as writer:
                writer.write(reader.read())


class Handler(SimpleHTTPRequestHandler):
    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    def send_head(self):  # noqa: ANN201 - stdlib override
        source = Path(self.translate_path(self.path.split("?", 1)[0]))
        compressed = source.with_name(f"{source.name}.gz")
        accepts = "gzip" in self.headers.get("Accept-Encoding", "").lower()
        if accepts and source.is_file() and source.suffix in COMPRESSIBLE and compressed.is_file():
            reader = compressed.open("rb")
            self.send_response(200)
            self.send_header("Content-Type", self.guess_type(str(source)))
            self.send_header("Content-Encoding", "gzip")
            self.send_header("Vary", "Accept-Encoding")
            self.send_header("Content-Length", str(compressed.stat().st_size))
            self.end_headers()
            return reader
        return super().send_head()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8082)
    parser.add_argument("--bind", default="0.0.0.0")
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    root = args.directory.resolve(strict=True)
    refresh_gzip(root)
    handler = functools.partial(Handler, directory=str(root))
    ThreadingHTTPServer((args.bind, args.port), handler).serve_forever()


if __name__ == "__main__":
    main()
