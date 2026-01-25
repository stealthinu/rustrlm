#!/usr/bin/env python3
"""
Replay a prior unofficial transcript's REPL inputs against the Rust CLI REPL,
and compare outputs/errors step-by-step.

This lets us validate parity without re-calling any LLMs (no API cost).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, Iterator, List, Optional, Tuple


@dataclass
class Task:
    dataset: str
    task_id: str
    query: str
    context: str


def _iter_jsonl(path: Path) -> Iterator[dict]:
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            yield json.loads(line)


def _load_s_niah_task(path: Path, tid: str) -> Optional[Task]:
    by_id: Dict[str, dict] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        o = json.loads(line)
        by_id[str(o["id"])] = o
    o = by_id.get(tid)
    if not o:
        return None

    full = str(o.get("context", ""))
    marker = "\nWhat is the special magic number for "
    q_idx = full.rfind(marker)
    if q_idx != -1:
        question = full[q_idx + 1 :].strip()
        before = full[:q_idx]
        first_nl = before.find("\n")
        ctx = before[first_nl + 1 :].strip() if first_nl != -1 else before.strip()
    else:
        question = f"What is the special magic number for {o.get('query','')} mentioned in the provided text?"
        ctx = full

    return Task(dataset="s_niah", task_id=tid, query=question, context=ctx)


def _load_oolong_synth_small_task(path: Path, tid: str) -> Optional[Task]:
    try:
        i = int(tid)
    except Exception:
        return None
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines()):
        if not line.strip():
            continue
        if n == i:
            o = json.loads(line)
            return Task(dataset="oolong_synth_small", task_id=tid, query="", context=o["prompt"])
    return None


def _load_longbench_codeqa_task(path: Path, tid: str) -> Optional[Task]:
    data = json.loads(path.read_text(encoding="utf-8"))
    for ex in data:
        if ex.get("sub_domain") != "Code repo QA":
            continue
        if str(ex.get("_id", "")) == tid:
            return Task(
                dataset="longbench_v2_codeqa",
                task_id=tid,
                query=ex["question"],
                context=ex["context"],
            )
    return None


def _load_browsecomp_task(path: Path, tid: str, max_context_chars: int) -> Optional[Task]:
    try:
        i = int(tid)
    except Exception:
        return None
    import pyarrow.parquet as pq  # type: ignore

    t = pq.read_table(path)
    if i < 0 or i >= t.num_rows:
        return None
    row = {name: t.column(name)[i].as_py() for name in t.column_names}
    query = row["query"]
    docs: List[str] = []
    for group in ["gold_docs", "evidence_docs", "negative_docs"]:
        for d in row.get(group, []) or []:
            docs.append(d.get("text", ""))
    context = "\n\n".join(docs)
    if len(context) > max_context_chars:
        context = context[:max_context_chars]
    return Task(dataset="browsecomp_plus", task_id=tid, query=query, context=context)


def _load_task(args: Any, dataset: str, task_id: str) -> Optional[Task]:
    if dataset == "s_niah":
        return _load_s_niah_task(Path(args.s_niah_path), task_id)
    if dataset == "oolong_synth_small":
        return _load_oolong_synth_small_task(Path(args.oolong_jsonl), task_id)
    if dataset == "longbench_v2_codeqa":
        return _load_longbench_codeqa_task(Path(args.longbench_json), task_id)
    if dataset == "browsecomp_plus":
        return _load_browsecomp_task(Path(args.browsecomp_parquet), task_id, args.max_context_chars)
    return None


def _rust_exec(bin_path: str, task: Task, code: str, state: Optional[dict], max_output_chars: int) -> Tuple[bool, str, Optional[str], Optional[dict]]:
    req = {
        "context": task.context,
        "query": task.query,
        "code": code,
        "max_output_chars": max_output_chars,
        "state": state,
    }
    p = subprocess.run([bin_path], input=json.dumps(req), text=True, capture_output=True)
    if p.returncode != 0:
        return False, "", f"rust repl nonzero exit: {p.returncode}: {(p.stderr or p.stdout).strip()}", None
    try:
        resp = json.loads(p.stdout)
    except Exception as e:
        return False, "", f"rust repl invalid json: {e}", None
    ok = bool(resp.get("ok"))
    out = str(resp.get("output", ""))
    err = resp.get("error", None)
    st = resp.get("state", None)
    return ok, out, err if isinstance(err, str) else None, st if isinstance(st, dict) else None


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--transcript-jsonl", required=True)
    ap.add_argument("--out-jsonl", required=True)
    ap.add_argument("--bin", default="target/debug/python_string_repl")
    ap.add_argument("--max-output-chars", type=int, default=2000)
    ap.add_argument("--max-context-chars", type=int, default=200_000)
    ap.add_argument("--s-niah-path", default="extracted/eval/s_niah.jsonl")
    ap.add_argument(
        "--browsecomp-parquet",
        default="upstream/bench_datasets/hf/Tevatron__browsecomp-plus/data/test-00000-of-00006.parquet",
    )
    ap.add_argument("--longbench-json", default="upstream/bench_datasets/hf/zai-org__LongBench-v2/data.json")
    ap.add_argument("--oolong-jsonl", default="upstream/bench_datasets/hf/tonychenxyz__oolong-synth-1k-16k/plain/test.jsonl")
    args = ap.parse_args()

    bin_path = os.path.abspath(args.bin)
    if not Path(bin_path).exists():
        raise SystemExit(f"rust repl bin not found: {bin_path} (build with `cargo build`)")

    out_path = Path(args.out_jsonl)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    # Group events by task, preserving order.
    events = list(_iter_jsonl(Path(args.transcript_jsonl)))

    current_task: Optional[Task] = None
    current_key: Optional[Tuple[str, str]] = None
    rust_state: Optional[dict] = None
    pending_code: Optional[str] = None
    step_i = 0

    ok_steps = 0
    mismatch = 0

    with out_path.open("w", encoding="utf-8") as f:
        for ev in events:
            t = ev.get("type")
            if t == "task_start":
                ds = str(ev.get("dataset", ""))
                tid = str(ev.get("task_id", ""))
                current_key = (ds, tid)
                task = _load_task(args, ds, tid)
                if task is None:
                    raise SystemExit(f"failed to load task payload: {ds}:{tid}")
                current_task = task
                rust_state = None
                pending_code = None
                step_i = 0
                continue
            if t == "task_end":
                current_task = None
                current_key = None
                rust_state = None
                pending_code = None
                continue

            if current_task is None:
                continue

            if t == "repl_input":
                pending_code = str(ev.get("code", ""))
                continue
            if t in ("repl_output", "repl_error"):
                if pending_code is None:
                    continue

                expected_kind = t
                expected = str(ev.get("output") if t == "repl_output" else ev.get("error", ""))

                ok, out, err, st = _rust_exec(bin_path, current_task, pending_code, rust_state, args.max_output_chars)
                rust_state = st if st is not None else rust_state

                actual_kind = "repl_output" if ok else "repl_error"
                actual = out if ok else (err or "")

                same = False
                if expected_kind == "repl_output" and actual_kind == "repl_output":
                    same = (expected == actual)
                elif expected_kind == "repl_error" and actual_kind == "repl_error":
                    # Errors differ across engines; require only that it's an error.
                    same = True

                rec = {
                    "dataset": current_key[0],
                    "task_id": current_key[1],
                    "step": step_i,
                    "code": pending_code,
                    "expected_kind": expected_kind,
                    "expected": expected,
                    "actual_kind": actual_kind,
                    "actual": actual,
                    "match": same,
                }
                f.write(json.dumps(rec, ensure_ascii=True) + "\n")
                f.flush()

                ok_steps += 1
                if not same:
                    mismatch += 1

                pending_code = None
                step_i += 1

    print(f"wrote {out_path} (steps={ok_steps}, mismatches={mismatch})")
    return 0 if mismatch == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
