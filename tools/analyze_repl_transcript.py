#!/usr/bin/env python3
"""
Analyze a REPL transcript JSONL produced by tools/run_unofficial_rlm_logged_eval.py.

Outputs:
- JSON summary with counts + feature histograms
- optional Markdown summary for docs
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Tuple


@dataclass
class SnippetStat:
    code: str
    count: int = 0
    ok: int = 0
    datasets: set[str] = None  # type: ignore[assignment]

    def __post_init__(self):
        if self.datasets is None:
            self.datasets = set()


class FeatureVisitor(ast.NodeVisitor):
    def __init__(self):
        self.node_types = Counter()
        self.call_names = Counter()
        self.attr_calls = Counter()
        self.imports = Counter()

    def generic_visit(self, node):
        self.node_types[type(node).__name__] += 1
        return super().generic_visit(node)

    def visit_Import(self, node: ast.Import):
        self.node_types["Import"] += 1
        for alias in node.names:
            self.imports[alias.name] += 1

    def visit_ImportFrom(self, node: ast.ImportFrom):
        self.node_types["ImportFrom"] += 1
        mod = node.module or ""
        self.imports[mod] += 1

    def visit_Call(self, node: ast.Call):
        # record call target name
        fn = node.func
        if isinstance(fn, ast.Name):
            self.call_names[fn.id] += 1
        elif isinstance(fn, ast.Attribute):
            # attempt to get base.name.attr
            base = fn.value
            if isinstance(base, ast.Name):
                self.attr_calls[f"{base.id}.{fn.attr}"] += 1
            else:
                self.attr_calls[fn.attr] += 1
        self.node_types["Call"] += 1
        self.generic_visit(node)


def _hash_code(s: str) -> str:
    return hashlib.sha256(s.encode("utf-8")).hexdigest()[:16]


def _iter_events(path: Path) -> Iterable[dict]:
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        yield json.loads(line)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--transcript-jsonl", required=True)
    ap.add_argument("--out-json", required=True)
    ap.add_argument("--out-md", default="")
    ap.add_argument("--max-snippets", type=int, default=200)
    args = ap.parse_args()

    events = list(_iter_events(Path(args.transcript_jsonl)))

    # Pair repl_input with following repl_output/repl_error.
    snippets: Dict[str, SnippetStat] = {}
    ok_counts = Counter()
    fail_counts = Counter()

    feature = FeatureVisitor()
    parse_fail = Counter()

    pending_code: Optional[Tuple[str, str]] = None  # (dataset, code)
    pending_ds: Optional[str] = None

    for ev in events:
        t = ev.get("type")
        if t == "repl_input":
            pending_code = (ev.get("dataset") or "", ev.get("code") or "")
            pending_ds = ev.get("dataset") or ""
        elif t in ("repl_output", "repl_error"):
            if pending_code is None:
                continue
            ds, code = pending_code
            h = _hash_code(code)
            st = snippets.get(h)
            if st is None:
                st = SnippetStat(code=code)
                snippets[h] = st
            st.count += 1
            if ds:
                st.datasets.add(ds)
            if t == "repl_output":
                st.ok += 1
                ok_counts[ds] += 1
                # Only analyze successful snippets; failures often include prose/invalid syntax.
                try:
                    tree = ast.parse(code)
                    feature.visit(tree)
                except Exception as e:
                    parse_fail[type(e).__name__] += 1
            else:
                fail_counts[ds] += 1
            pending_code = None
            pending_ds = None

    # Sort snippets by frequency
    top = sorted(snippets.items(), key=lambda kv: kv[1].count, reverse=True)
    top = top[: args.max_snippets]

    out = {
        "transcript": str(args.transcript_jsonl),
        "repl": {
            "unique_snippets": len(snippets),
            "top_snippets": [
                {
                    "id": hid,
                    "count": st.count,
                    "ok": st.ok,
                    "datasets": sorted(st.datasets),
                    "code": st.code,
                }
                for hid, st in top
            ],
            "ok_by_dataset": dict(ok_counts),
            "fail_by_dataset": dict(fail_counts),
        },
        "ast_features": {
            "node_types": dict(feature.node_types),
            "call_names": dict(feature.call_names),
            "attr_calls": dict(feature.attr_calls),
            "imports": dict(feature.imports),
            "parse_fail": dict(parse_fail),
        },
    }

    out_path = Path(args.out_json)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")

    if args.out_md:
        md_path = Path(args.out_md)
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(_render_md(out), encoding="utf-8")

    print(f"wrote {out_path}")
    if args.out_md:
        print(f"wrote {args.out_md}")
    return 0


def _render_md(out: dict) -> str:
    astf = out["ast_features"]
    repl = out["repl"]

    def top_items(d: dict, n: int = 20) -> List[Tuple[str, int]]:
        return sorted(((k, int(v)) for k, v in d.items()), key=lambda kv: kv[1], reverse=True)[:n]

    lines: List[str] = []
    lines.append("# 非公式ベースライン実測: REPL使用機能サマリ（30タスク）\n")
    lines.append(f"- transcript: `{out['transcript']}`\n")
    lines.append(f"- unique REPL snippets: {repl['unique_snippets']}\n")
    lines.append(f"- ok_by_dataset: {repl['ok_by_dataset']}\n")
    lines.append(f"- fail_by_dataset: {repl['fail_by_dataset']}\n")
    lines.append("\n## AST特徴（成功したREPLスニペットのみ）\n")

    lines.append("\n### node_types (top)\n")
    for k, v in top_items(astf["node_types"]):
        lines.append(f"- {k}: {v}\n")

    lines.append("\n### call_names (top)\n")
    for k, v in top_items(astf["call_names"]):
        lines.append(f"- {k}: {v}\n")

    lines.append("\n### attr_calls (top)\n")
    for k, v in top_items(astf["attr_calls"]):
        lines.append(f"- {k}: {v}\n")

    if astf.get("imports"):
        lines.append("\n### imports (observed)\n")
        for k, v in top_items(astf["imports"], n=50):
            lines.append(f"- {k}: {v}\n")

    lines.append("\n## 上位REPLスニペット（頻出; 生コード）\n")
    for s in repl["top_snippets"][:20]:
        lines.append(f"\n### {s['id']} (count={s['count']} ok={s['ok']} datasets={s['datasets']})\n")
        lines.append("```python\n")
        lines.append(s["code"].rstrip() + "\n")
        lines.append("```\n")

    return "".join(lines)


if __name__ == "__main__":
    raise SystemExit(main())

