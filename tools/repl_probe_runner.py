#!/usr/bin/env python3
"""
Probe what the non-official REPL executor can/can't run on real benchmark inputs.

This is NOT solving the benchmarks; it is a compatibility harness to empirically
discover which Python constructs/builtins/modules are needed and what breaks.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import random
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Tuple


def _load_module_from_path(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, os.fspath(path))
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module spec: {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _build_env(context: Any, query: str) -> Dict[str, Any]:
    env: Dict[str, Any] = {"context": context, "query": query}

    def llm_query(prompt: str, model: str | None = None) -> str:
        _ = (prompt, model)
        return "STUB_LLM_RESPONSE"

    env["llm_query"] = llm_query
    env["recursive_llm"] = llm_query
    return env


def _load_browsecomp_plus_sample() -> Tuple[str, str]:
    # Use pyarrow lazily to avoid hard dependency unless needed.
    import pyarrow.parquet as pq

    p = Path("upstream/bench_datasets/hf/Tevatron__browsecomp-plus/data/test-00000-of-00006.parquet")
    t = pq.read_table(p)
    row0 = {name: t.column(name)[0].as_py() for name in t.column_names}

    query = row0["query"]
    # Join docs into a single context string; this approximates "1000 docs in context".
    docs = []
    for group in ["gold_docs", "evidence_docs", "negative_docs"]:
        for d in row0.get(group, []) or []:
            docs.append(d.get("text", ""))
    # Keep size bounded for probes.
    context = "\n\n".join(docs)[:200_000]
    return context, query


def _load_longbench_v2_codeqa_sample() -> Tuple[str, str]:
    p = Path("upstream/bench_datasets/hf/zai-org__LongBench-v2/data.json")
    data = json.loads(p.read_text(encoding="utf-8"))
    ex = next(x for x in data if x.get("sub_domain") == "Code repo QA")
    query = ex["question"]
    context = ex["context"][:200_000]
    return context, query


def _load_oolong_synth_small_sample() -> Tuple[str, str]:
    p = Path("upstream/bench_datasets/hf/tonychenxyz__oolong-synth-1k-16k/plain/test.jsonl")
    with p.open("r", encoding="utf-8") as f:
        ex = json.loads(f.readline())
    # Treat the whole prompt as context; query is empty (the prompt contains instructions/questions).
    return ex["prompt"], ""


def _load_s_niah_sample() -> Tuple[str, str]:
    p = Path("extracted/eval/s_niah.jsonl")
    with p.open("r", encoding="utf-8") as f:
        ex = json.loads(f.readline())
    return ex["context"], ex["query"]


def _probes() -> List[dict]:
    # Keep probes small and focused; each is a multi-line code snippet.
    return [
        {
            "name": "slice_head_len",
            "code": "chunk = context[:10000]\nprint(len(chunk))",
        },
        {
            "name": "splitlines_count",
            "code": "lines = str(context).splitlines()\nprint(len(lines))",
        },
        {
            "name": "regex_findall_digits_no_import",
            "code": "matches = re.findall(r\"\\d+\", str(context))\nprint(matches[:10])",
        },
        {
            "name": "import_re_should_fail",
            "code": "import re\nprint('imported')",
        },
        {
            "name": "listcomp_context_in_expr_slice",
            "code": "chunks = [context[i:i+10] for i in range(0, min(100, len(context)), 10)]\nprint(len(chunks))",
        },
        {
            "name": "listcomp_context_in_iter_clause",
            "code": "chars = [c for c in str(context) if c.isupper()]\nprint(chars[:10])",
        },
        {
            "name": "for_loop_append",
            "code": "buf = []\nfor c in str(context)[:1000]:\n    if c.isupper():\n        buf.append(c)\nprint(buf[:10])",
        },
        {
            "name": "regex_split_whitespace_no_import",
            "code": "parts = re.split(r\"\\s+\", str(context)[:10000])\nprint(parts[:10])",
        },
        {
            "name": "fstring_len",
            "code": "n = len(str(context))\nprint(f\"len={n}\")",
        },
        {
            "name": "any_generatorexp_digit",
            "code": "print(any(c.isdigit() for c in str(context)[:1000]))",
        },
        {
            "name": "json_loads_no_import",
            "code": "obj = json.loads('{\"a\": 1, \"b\": [2, 3]}')\nprint(obj['a'], obj['b'][0])",
        },
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--out-jsonl",
        required=True,
        help="Write probe results as JSONL (one record per probe per dataset)",
    )
    ap.add_argument(
        "--upstream-dir",
        default="upstream/recursive-llm",
        help="Path to cloned non-official repo",
    )
    args = ap.parse_args()

    upstream_dir = Path(args.upstream_dir)
    repl_mod = _load_module_from_path("recursive_llm_repl", upstream_dir / "src" / "rlm" / "repl.py")
    REPLExecutor = repl_mod.REPLExecutor
    REPLError = repl_mod.REPLError

    executor = REPLExecutor()

    datasets = [
        ("browsecomp_plus", _load_browsecomp_plus_sample),
        ("longbench_v2_codeqa", _load_longbench_v2_codeqa_sample),
        ("oolong_synth_small", _load_oolong_synth_small_sample),
        ("s_niah", _load_s_niah_sample),
    ]

    out_path = Path(args.out_jsonl)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    with out_path.open("w", encoding="utf-8") as f:
        for ds_name, loader in datasets:
            context, query = loader()
            env = _build_env(context=context, query=query)

            for probe in _probes():
                rec: Dict[str, Any] = {
                    "dataset": ds_name,
                    "probe": probe["name"],
                    "ok": False,
                }
                try:
                    out = executor.execute(probe["code"], env)
                    rec["ok"] = True
                    rec["output"] = out
                except REPLError as e:
                    rec["error"] = str(e)
                f.write(json.dumps(rec, ensure_ascii=True) + "\n")

    print(f"wrote {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
