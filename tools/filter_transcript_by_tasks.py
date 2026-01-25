#!/usr/bin/env python3
"""
Filter a transcript JSONL to only include events for a selected set of tasks.

This is used to create apples-to-apples diffs between two runs when only a subset
of tasks are re-executed (e.g. rerunning only import-failure tasks).
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, Set, Tuple


def _iter_events(path: Path) -> Iterable[dict]:
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            yield json.loads(line)


def _task_key(ev: dict) -> Tuple[str, str] | None:
    ds = ev.get("dataset")
    tid = ev.get("task_id")
    if isinstance(ds, str) and isinstance(tid, str):
        return (ds, tid)
    return None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in-transcript", required=True)
    ap.add_argument(
        "--tasks-from-transcript",
        required=True,
        help="Use task ids observed in this transcript (task_start events) as the allowlist.",
    )
    ap.add_argument("--out-transcript", required=True)
    args = ap.parse_args()

    allow: Set[Tuple[str, str]] = set()
    for ev in _iter_events(Path(args.tasks_from_transcript)):
        if ev.get("type") != "task_start":
            continue
        k = _task_key(ev)
        if k:
            allow.add(k)

    out_path = Path(args.out_transcript)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    n_in = 0
    n_out = 0
    with out_path.open("w", encoding="utf-8") as f:
        for ev in _iter_events(Path(args.in_transcript)):
            n_in += 1
            k = _task_key(ev)
            if k and k in allow:
                f.write(json.dumps(ev, ensure_ascii=True) + "\n")
                n_out += 1
    print(f"filtered {n_out}/{n_in} events into {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

