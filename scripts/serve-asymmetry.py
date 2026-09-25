"""Static server plus a turn relay for the two-seat asymmetry lab.

The entire shared state of a match is its **order log**. `observed_mechanics`
replays a match from a `ModeSpec`, a seed and a log, and a test pins that, so
two clients do not need lockstep, rollback or quantised intents — they need to
agree on a growing string. This relay is that agreement and nothing more.

It is deliberately not the game's transport. `observed_net` is UDP lockstep
built for continuous 60Hz movement; browsers cannot open a UDP socket at all,
and a turn-based match with no clock has no latency requirement to spend that
complexity on. Development only: no auth, no persistence, in-memory.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import threading
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

COMPRESSIBLE_SUFFIXES = {".js", ".wasm"}
SEATS = ("architect", "operator")


class Match:
    """One match: the settled log, and this turn's half-filled orders."""

    def __init__(self, seed: int) -> None:
        self.seed = seed
        self.turns: list[str] = []
        self.pending: dict[str, str] = {}

    def submit(self, seat: str, orders: str) -> None:
        self.pending[seat] = orders.strip()
        # A turn settles only when every seat has spoken. Neither player learns
        # what the other did before committing, which is what keeps the
        # resolution genuinely simultaneous across two machines.
        if all(s in self.pending for s in SEATS):
            joined = " ".join(self.pending[s] for s in SEATS if self.pending[s])
            self.turns.append(joined)
            self.pending = {}

    def view(self) -> dict:
        return {
            "seed": self.seed,
            "turn": len(self.turns),
            "log": " / ".join(self.turns),
            "waiting_on": [s for s in SEATS if s not in self.pending],
        }


class Relay:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.matches: dict[str, Match] = {}

    def get(self, name: str, seed: int) -> Match:
        with self.lock:
            if name not in self.matches:
                self.matches[name] = Match(seed)
            return self.matches[name]

    def reset(self, name: str, seed: int) -> Match:
        with self.lock:
            self.matches[name] = Match(seed)
            return self.matches[name]


RELAY = Relay()


def refresh_gzip_assets(root: Path) -> None:
    for source in root.iterdir():
        if not source.is_file() or source.suffix not in COMPRESSIBLE_SUFFIXES:
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
        # Two seats may be served from different origins during development.
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "content-type")
        super().end_headers()

    def _json(self, payload: dict, status: int = HTTPStatus.OK) -> None:
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self) -> None:  # noqa: N802 - stdlib naming
        self.send_response(HTTPStatus.NO_CONTENT)
        self.end_headers()

    def do_POST(self) -> None:  # noqa: N802 - stdlib naming
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        if len(parts) != 2 or parts[0] != "match":
            self._json({"error": "POST /match/<name>?seat=…&seed=…"}, HTTPStatus.NOT_FOUND)
            return
        query = parse_qs(parsed.query)
        seat = (query.get("seat") or [""])[0]
        seed = int((query.get("seed") or ["0"])[0])
        if seat not in SEATS:
            self._json({"error": f"seat must be one of {SEATS}"}, HTTPStatus.BAD_REQUEST)
            return
        length = int(self.headers.get("Content-Length") or 0)
        orders = self.rfile.read(length).decode("utf-8", "replace")
        if (query.get("reset") or ["0"])[0] == "1":
            RELAY.reset(parts[1], seed)
        match = RELAY.get(parts[1], seed)
        match.submit(seat, orders)
        self._json(match.view())

    def do_GET(self) -> None:  # noqa: N802 - stdlib naming
        parsed = urlparse(self.path)
        parts = [p for p in parsed.path.split("/") if p]
        if parts and parts[0] == "match":
            if len(parts) != 2:
                self._json({"error": "GET /match/<name>"}, HTTPStatus.NOT_FOUND)
                return
            seed = int((parse_qs(parsed.query).get("seed") or ["0"])[0])
            self._json(RELAY.get(parts[1], seed).view())
            return
        super().do_GET()

    def send_head(self):  # noqa: ANN201 - stdlib override
        source = Path(self.translate_path(urlparse(self.path).path))
        accepts_gzip = "gzip" in self.headers.get("Accept-Encoding", "").lower()
        compressed = source.with_name(f"{source.name}.gz")
        if accepts_gzip and source.is_file() and source.suffix in COMPRESSIBLE_SUFFIXES and compressed.is_file():
            reader = compressed.open("rb")
            self.send_response(HTTPStatus.OK)
            self.send_header("Content-Type", self.guess_type(str(source)))
            self.send_header("Content-Encoding", "gzip")
            self.send_header("Vary", "Accept-Encoding")
            self.send_header("Content-Length", str(compressed.stat().st_size))
            self.end_headers()
            return reader
        return super().send_head()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8081)
    parser.add_argument("--bind", default="0.0.0.0")
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()

    root = args.directory.resolve(strict=True)
    refresh_gzip_assets(root)
    os.chdir(root)
    ThreadingHTTPServer((args.bind, args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
