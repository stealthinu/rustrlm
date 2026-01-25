#!/usr/bin/env python3
"""
Extract text listings embedded in arXiv HTML as data:text/plain;base64,... links.

We use the listing <div id="..."> immediately preceding the data link as the output name.
This is intended for paper artifact extraction (e.g., prompts/trajectories in appendices).
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


@dataclass(frozen=True)
class Listing:
    listing_id: str
    text: str
    sha256: str


def _safe_name(s: str) -> str:
    s = s.strip()
    if not s:
        return "unknown"
    return re.sub(r"[^A-Za-z0-9._-]", "_", s)


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def _iter_listing_starts(html: str) -> list[tuple[int, str]]:
    # Capture the `id="..."` for arXiv listing blocks.
    # Example: <div class="ltx_listing ... " id="A2.SS1.p2.1" ...>
    starts: list[tuple[int, str]] = []
    pat = re.compile(r'<div class="ltx_listing[^>]*\bid="([^"]+)"', re.IGNORECASE)
    for m in pat.finditer(html):
        starts.append((m.start(), m.group(1)))
    return starts


def _iter_b64_payloads(html: str) -> Iterable[tuple[int, str]]:
    pat = re.compile(r'href="data:text/plain;base64,([^"]+)"', re.IGNORECASE)
    for m in pat.finditer(html):
        yield (m.start(), m.group(1))


def _nearest_listing_id(starts: list[tuple[int, str]], pos: int) -> str | None:
    # starts are sorted by position; find last start <= pos
    best: str | None = None
    for start_pos, listing_id in starts:
        if start_pos > pos:
            break
        best = listing_id
    return best


def _decode_payload(b64: str) -> str:
    raw = base64.b64decode(b64)
    return raw.decode("utf-8", errors="replace")


def extract_listings(html_path: Path) -> list[Listing]:
    html = _read_text(html_path)
    starts = _iter_listing_starts(html)
    listings: list[Listing] = []

    seq = 0
    for pos, b64 in _iter_b64_payloads(html):
        listing_id = _nearest_listing_id(starts, pos)
        if listing_id is None:
            listing_id = f"listing_{seq}"
            seq += 1

        text = _decode_payload(b64)
        sha = hashlib.sha256(text.encode("utf-8", errors="replace")).hexdigest()
        listings.append(Listing(listing_id=listing_id, text=text, sha256=sha))

    return listings


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--html", required=True, help="Path to arXiv HTML file")
    ap.add_argument(
        "--out-dir",
        required=True,
        help="Directory to write extracted .txt files and a manifest.json",
    )
    args = ap.parse_args()

    html_path = Path(args.html)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    listings = extract_listings(html_path)

    manifest = []
    for lst in listings:
        name = _safe_name(lst.listing_id) + ".txt"
        path = out_dir / name
        # Avoid accidental overwrite on collisions: append hash prefix if needed.
        if path.exists():
            name = _safe_name(lst.listing_id) + "." + lst.sha256[:12] + ".txt"
            path = out_dir / name

        path.write_text(lst.text, encoding="utf-8")
        manifest.append(
            {
                "id": lst.listing_id,
                "file": os.fspath(path.relative_to(out_dir)),
                "sha256": lst.sha256,
                "bytes": len(lst.text.encode("utf-8", errors="replace")),
            }
        )

    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=True, indent=2) + "\n", encoding="utf-8"
    )

    print(f"extracted {len(listings)} listing(s) into {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

