#!/usr/bin/env python3
"""
Static analysis of extracted REPL blocks using Python's `ast`.

This helps us identify which Python constructs are actually used in the paper's
prompted examples, so we can decide what to implement in a Rust subset.
"""

from __future__ import annotations

import argparse
import ast
import json
from collections import Counter, defaultdict
from pathlib import Path


class FeatureVisitor(ast.NodeVisitor):
    def __init__(self) -> None:
        self.node_types: Counter[str] = Counter()
        self.call_names: Counter[str] = Counter()
        self.attr_calls: Counter[str] = Counter()
        self.imports: Counter[str] = Counter()
        self.binops: Counter[str] = Counter()
        self.cmpops: Counter[str] = Counter()

    def generic_visit(self, node):
        self.node_types[type(node).__name__] += 1
        super().generic_visit(node)

    def visit_Import(self, node: ast.Import) -> None:
        for alias in node.names:
            self.imports[alias.name] += 1
        self.generic_visit(node)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        mod = node.module or ""
        self.imports[mod] += 1
        self.generic_visit(node)

    def visit_Call(self, node: ast.Call) -> None:
        # func(...)
        if isinstance(node.func, ast.Name):
            self.call_names[node.func.id] += 1
        # obj.method(...)
        elif isinstance(node.func, ast.Attribute):
            parts = []
            cur = node.func
            # best-effort build "a.b.c" string
            while isinstance(cur, ast.Attribute):
                parts.append(cur.attr)
                cur = cur.value
            if isinstance(cur, ast.Name):
                parts.append(cur.id)
            full = ".".join(reversed(parts))
            if full:
                self.attr_calls[full] += 1
        self.generic_visit(node)

    def visit_BinOp(self, node: ast.BinOp) -> None:
        self.binops[type(node.op).__name__] += 1
        self.generic_visit(node)

    def visit_Compare(self, node: ast.Compare) -> None:
        for op in node.ops:
            self.cmpops[type(op).__name__] += 1
        self.generic_visit(node)


def analyze_file(path: Path) -> dict:
    code = path.read_text(encoding="utf-8", errors="replace")
    try:
        tree = ast.parse(code, filename=str(path))
    except SyntaxError as e:
        # Paper/prompt snippets can contain illustrative code that is not
        # syntactically valid (e.g. unescaped quotes). Record and continue.
        return {
            "file": str(path),
            "lines": code.count("\n") + 1,
            "parse_error": {
                "msg": str(e.msg),
                "lineno": e.lineno,
                "offset": e.offset,
                "text": (e.text or "").strip(),
            },
        }

    v = FeatureVisitor()
    v.visit(tree)
    return {
        "file": str(path),
        "lines": code.count("\n") + 1,
        "node_types": dict(v.node_types),
        "call_names": dict(v.call_names),
        "attr_calls": dict(v.attr_calls),
        "imports": dict(v.imports),
        "binops": dict(v.binops),
        "cmpops": dict(v.cmpops),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in-dir", required=True, help="Directory containing extracted .py blocks")
    ap.add_argument("--out-json", required=True, help="Path to write JSON summary")
    args = ap.parse_args()

    in_dir = Path(args.in_dir)
    out_json = Path(args.out_json)
    out_json.parent.mkdir(parents=True, exist_ok=True)

    per_file = []
    totals = defaultdict(Counter)

    for path in sorted(in_dir.glob("*.py")):
        info = analyze_file(path)
        per_file.append(info)
        if "parse_error" in info:
            continue
        for k in ["node_types", "call_names", "attr_calls", "imports", "binops", "cmpops"]:
            totals[k].update(info[k])

    out = {
        "files": per_file,
        "totals": {k: dict(v) for k, v in totals.items()},
        "file_count": len(per_file),
    }
    out_json.write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    print(f"analyzed {len(per_file)} file(s) -> {out_json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
