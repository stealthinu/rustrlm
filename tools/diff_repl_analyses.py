#!/usr/bin/env python3
"""
Diff two `tools/analyze_repl_transcript.py` JSON outputs (AST-focused).

We report:
- added keys (new-only)
- increased counts for existing keys

This is intended to answer: "what extra surface did we observe after enabling X?"
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Dict, Tuple


def _load(path: str) -> dict:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def _counter(d: Dict[str, int]) -> Counter:
    c = Counter()
    for k, v in (d or {}).items():
        try:
            c[str(k)] = int(v)
        except Exception:
            continue
    return c


def _diff(a: Counter, b: Counter) -> Tuple[Dict[str, int], Dict[str, int]]:
    """Return (added_in_b, increased_in_b)."""
    added: Dict[str, int] = {}
    inc: Dict[str, int] = {}
    for k, vb in b.items():
        va = a.get(k, 0)
        if va == 0 and vb != 0:
            if k not in a:
                added[k] = vb
            else:
                inc[k] = vb - va
        elif vb > va:
            inc[k] = vb - va
    return added, inc


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", required=True)
    ap.add_argument("--rerun", required=True)
    ap.add_argument("--out-json", required=True)
    args = ap.parse_args()

    base = _load(args.baseline)
    new = _load(args.rerun)

    out = {"baseline": args.baseline, "rerun": args.rerun, "diff": {}}

    for k in ("node_types", "call_names", "attr_calls", "imports", "parse_fail"):
        a = _counter(base["ast_features"].get(k, {}))
        b = _counter(new["ast_features"].get(k, {}))
        added, inc = _diff(a, b)
        out["diff"][k] = {
            "added": dict(sorted(added.items(), key=lambda kv: (-kv[1], kv[0]))),
            "increased": dict(sorted(inc.items(), key=lambda kv: (-kv[1], kv[0]))),
        }

    Path(args.out_json).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out_json).write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.out_json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

