#!/usr/bin/env python3
"""
Extract ```repl fenced code blocks from text files.

This mirrors the official `find_code_blocks` regex used by alexzhang13/rlm so we
can build a paper-derived corpus of REPL snippets.
"""

from __future__ import annotations

import argparse
import json
import os
import re
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class CodeBlock:
    source_file: str
    index: int
    code: str


_PATTERN = re.compile(r"```repl\s*\n(.*?)\n```", re.DOTALL)


def extract_from_text(text: str) -> list[str]:
    blocks: list[str] = []
    for m in _PATTERN.finditer(text):
        blocks.append(m.group(1).strip())
    return blocks


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in-dir", required=True, help="Directory containing .txt inputs")
    ap.add_argument("--out-dir", required=True, help="Directory to write extracted blocks")
    args = ap.parse_args()

    in_dir = Path(args.in_dir)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    manifest = []
    total = 0

    for path in sorted(in_dir.glob("*.txt")):
        text = path.read_text(encoding="utf-8", errors="replace")
        blocks = extract_from_text(text)
        for i, code in enumerate(blocks):
            name = f"{path.stem}__repl_{i:03d}.py"
            out_path = out_dir / name
            out_path.write_text(code + "\n", encoding="utf-8")
            manifest.append(
                {
                    "source_file": os.fspath(path.relative_to(in_dir)),
                    "block_index": i,
                    "file": os.fspath(out_path.relative_to(out_dir)),
                    "lines": code.count("\n") + 1,
                }
            )
            total += 1

    (out_dir / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=True, indent=2) + "\n", encoding="utf-8"
    )

    print(f"extracted {total} repl block(s) into {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

