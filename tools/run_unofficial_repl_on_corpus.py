#!/usr/bin/env python3
"""
Run the non-official `ysz/recursive-llm` REPL executor against our local corpus.

Goal: empirically observe what code patterns are accepted/rejected and what output
format looks like, without needing any real LLM calls.

We intentionally import modules by file-path to avoid pulling the whole package
dependency tree (e.g. `litellm`) during analysis.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, Tuple


def _load_module_from_path(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, os.fspath(path))
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module spec: {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _git_head(path: Path) -> str:
    try:
        out = subprocess.check_output(["git", "-C", os.fspath(path), "rev-parse", "HEAD"])
        return out.decode("utf-8", errors="replace").strip()
    except Exception:
        return "UNKNOWN"


def _pick_context_and_env(code: str) -> Tuple[str, Dict[str, Any]]:
    """
    Best-effort environment builder for paper snippets.

    Many paper blocks are illustrative and omit variables (`query`, `buffers`).
    We seed them to reduce spurious NameErrors.
    """
    env: Dict[str, Any] = {}

    # Seed common variables referenced in examples.
    env["query"] = "STUB_QUERY"
    env["buffers"] = "STUB_BUFFERS"

    # Minimal llm_query stub.
    def llm_query(prompt: str, model: str | None = None) -> str:
        _ = (prompt, model)
        return "STUB_LLM_RESPONSE"

    env["llm_query"] = llm_query
    env["recursive_llm"] = llm_query

    # Context type heuristics.
    if 'context["content"]' in code or "context['content']" in code:
        context_kind = "dict"
        env["context"] = {
            "content": "### H1\nalpha\n### H2\nmagic number is 12345\n### H3\nomega\n"
        }
        return context_kind, env

    # Many examples treat context as a list[str] (chunks) when enumerating/iterating.
    if ("enumerate(context)" in code) or re.search(r"for\s+.+\s+in\s+context\b", code, flags=re.S):
        context_kind = "list"
        env["context"] = [
            "first chunk",
            "this chunk mentions magic and number: 12345",
            "last chunk",
        ]
        return context_kind, env

    context_kind = "str"
    env["context"] = ("X" * 100) + " magic number is 12345 " + ("Y" * 100)
    return context_kind, env


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus-manifest", required=True, help="manifest.json for repl blocks")
    ap.add_argument("--out-jsonl", required=True, help="Write results as JSON Lines")
    ap.add_argument(
        "--upstream-dir",
        default="upstream/recursive-llm",
        help="Path to cloned non-official repo",
    )
    args = ap.parse_args()

    corpus_manifest = Path(args.corpus_manifest)
    out_jsonl = Path(args.out_jsonl)
    upstream_dir = Path(args.upstream_dir)

    repl_mod = _load_module_from_path(
        "recursive_llm_repl",
        upstream_dir / "src" / "rlm" / "repl.py",
    )
    parser_mod = _load_module_from_path(
        "recursive_llm_parser",
        upstream_dir / "src" / "rlm" / "parser.py",
    )

    REPLExecutor = repl_mod.REPLExecutor
    REPLError = repl_mod.REPLError

    executor = REPLExecutor()

    manifest = json.loads(corpus_manifest.read_text(encoding="utf-8"))

    out_jsonl.parent.mkdir(parents=True, exist_ok=True)
    with out_jsonl.open("w", encoding="utf-8") as f:
        for item in manifest:
            block_path = Path("extracted/paper/repl_blocks") / item["file"]
            code = block_path.read_text(encoding="utf-8", errors="replace")

            context_kind, env = _pick_context_and_env(code)

            record: Dict[str, Any] = {
                "baseline": "ysz/recursive-llm",
                "baseline_commit": _git_head(upstream_dir),
                "corpus_file": os.fspath(block_path),
                "source_file": item.get("source_file"),
                "block_index": item.get("block_index"),
                "context_kind": context_kind,
            }

            try:
                output = executor.execute(code, env)
                record["ok"] = True
                record["output"] = output
                record["final"] = parser_mod.parse_response(code, env)
                # Snapshot keys for later feature decisions (don’t dump large values).
                record["env_keys"] = sorted([k for k in env.keys() if not k.startswith("_")])
            except REPLError as e:
                record["ok"] = False
                record["error"] = str(e)
                record["final"] = parser_mod.parse_response(code, env)
                record["env_keys"] = sorted([k for k in env.keys() if not k.startswith("_")])

            f.write(json.dumps(record, ensure_ascii=True) + "\n")

    print(f"wrote: {out_jsonl}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
