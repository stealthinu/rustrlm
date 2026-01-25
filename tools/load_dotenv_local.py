#!/usr/bin/env python3
"""
Minimal .env loader to avoid extra dependencies and to keep secrets out of logs.

Rules:
- Does not print values.
- Ignores blank lines and comments.
- Only supports KEY=VALUE (no export, no quotes parsing beyond stripping whitespace).
"""

from __future__ import annotations

from pathlib import Path
import os


def load_dotenv(path: str | os.PathLike = ".env", override: bool = False) -> None:
    p = Path(path)
    if not p.exists():
        return

    for raw in p.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        k, v = line.split("=", 1)
        k = k.strip()
        v = v.strip()
        if not k:
            continue
        if not override and k in os.environ:
            continue
        os.environ[k] = v

