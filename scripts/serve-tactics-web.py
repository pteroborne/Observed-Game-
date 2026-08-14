"""Small development-only static server with precompressed WASM delivery."""

from __future__ import annotations

import argparse
import gzip
import http.server
import os
from pathlib import Path


COMPRESSIBLE_SUFFIXES = {".js", ".wasm"}


def refresh_gzip_assets(root: Path) -> None:
    """Create deterministic gzip neighbors for large browser artifacts."""
    for source in root.iterdir():
        if not source.is_file() or source.suffix not in COMPRESSIBLE_SUFFIXES:
            continue
        compressed = source.with_name(f"{source.name}.gz")
        if compressed.exists() and compressed.stat().st_mtime_ns >= source.stat().st_mtime_ns:
            continue
        with source.open("rb") as reader, compressed.open("wb") as raw_writer:
            with gzip.GzipFile(
                filename="",
                mode="wb",
                fileobj=raw_writer,
                compresslevel=9,
                mtime=0,
            ) as writer:
                writer.write(reader.read())


class GzipStaticHandler(http.server.SimpleHTTPRequestHandler):
    """Serve generated gzip files while preserving the source content type."""

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    def send_head(self):  # noqa: ANN201 - stdlib override intentionally mirrors its API
        source = Path(self.translate_path(self.path))
        accepts_gzip = "gzip" in self.headers.get("Accept-Encoding", "").lower()
        compressed = source.with_name(f"{source.name}.gz")
        if (
            accepts_gzip
            and source.is_file()
            and source.suffix in COMPRESSIBLE_SUFFIXES
            and compressed.is_file()
        ):
            reader = compressed.open("rb")
            self.send_response(200)
            self.send_header("Content-Type", self.guess_type(str(source)))
            self.send_header("Content-Encoding", "gzip")
            self.send_header("Vary", "Accept-Encoding")
            self.send_header("Content-Length", str(compressed.stat().st_size))
            self.send_header("Last-Modified", self.date_time_string(source.stat().st_mtime))
            self.end_headers()
            return reader
        return super().send_head()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--bind", default="0.0.0.0")
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()

    root = args.directory.resolve(strict=True)
    refresh_gzip_assets(root)
    os.chdir(root)
    server = http.server.ThreadingHTTPServer((args.bind, args.port), GzipStaticHandler)
    server.serve_forever()


if __name__ == "__main__":
    main()
