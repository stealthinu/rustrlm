#!/usr/bin/env python3
"""
Run a small, budget-capped evaluation using the unofficial RLM implementation.

Goals:
- root model uses --model (e.g. gpt-5.2)
- recursive subcalls use --recursive-model (e.g. gpt-5.2-mini)
- record token usage + estimated cost (if LiteLLM knows pricing)
- stop when reaching --max-cost-usd (soft cap; stops after finishing the current task)

NOTE: This is not a full reproduction of the paper's eval harness.
It runs a small sample from locally available datasets to validate end-to-end wiring + costs.
"""

from __future__ import annotations

import argparse
import asyncio
import importlib.util
import json
import os
import random
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Tuple

import litellm

from tools.load_dotenv_local import load_dotenv


def _load_module_from_path(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, os.fspath(path))
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module spec: {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


@dataclass
class Task:
    dataset: str
    task_id: str
    query: str
    context: str


def _iter_s_niah(path: Path) -> Iterable[Task]:
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            o = json.loads(line)
            # The generator stores a full prompt-like blob in "context". For RLM wiring we want:
            # - context: the long haystack text
            # - query: the actual question
            full = str(o.get("context", ""))
            marker = "\nWhat is the special magic number for "
            q_idx = full.rfind(marker)
            if q_idx != -1:
                question = full[q_idx + 1 :].strip()
                before = full[:q_idx]
                # after the first newline is the actual haystack text
                first_nl = before.find("\n")
                ctx = before[first_nl + 1 :].strip() if first_nl != -1 else before.strip()
            else:
                question = f"What is the special magic number for {o.get('query','')} mentioned in the provided text?"
                ctx = full
            yield Task(dataset="s_niah", task_id=str(o["id"]), query=question, context=ctx)


def _iter_oolong_synth_small(path: Path) -> Iterable[Task]:
    # The prompt contains the question/instructions; keep query empty.
    with path.open("r", encoding="utf-8") as f:
        for i, line in enumerate(f):
            o = json.loads(line)
            yield Task(dataset="oolong_synth_small", task_id=str(i), query="", context=o["prompt"])


def _iter_longbench_v2_codeqa(path: Path) -> Iterable[Task]:
    data = json.loads(path.read_text(encoding="utf-8"))
    for ex in data:
        if ex.get("sub_domain") != "Code repo QA":
            continue
        task_id = str(ex.get("_id", ""))
        query = ex["question"]
        # Keep context bounded to reduce runaways; paper uses full corpuses, but this is a wiring check.
        context = ex["context"]
        yield Task(dataset="longbench_v2_codeqa", task_id=task_id, query=query, context=context)


def _iter_browsecomp_plus(path: Path, max_context_chars: int) -> Iterable[Task]:
    # Use pyarrow lazily.
    import pyarrow.parquet as pq

    t = pq.read_table(path)
    for i in range(t.num_rows):
        row = {name: t.column(name)[i].as_py() for name in t.column_names}
        query = row["query"]
        docs = []
        for group in ["gold_docs", "evidence_docs", "negative_docs"]:
            for d in row.get(group, []) or []:
                docs.append(d.get("text", ""))
        context = "\n\n".join(docs)
        if len(context) > max_context_chars:
            context = context[:max_context_chars]
        yield Task(dataset="browsecomp_plus", task_id=str(i), query=query, context=context)


def _sample(tasks: List[Task], n: int, rng: random.Random) -> List[Task]:
    if n <= 0:
        return []
    if n >= len(tasks):
        return tasks
    idx = list(range(len(tasks)))
    rng.shuffle(idx)
    return [tasks[i] for i in idx[:n]]


def _reservoir_sample(it: Iterable[Task], k: int, rng: random.Random) -> List[Task]:
    if k <= 0:
        return []
    out: List[Task] = []
    for n, item in enumerate(it):
        if n < k:
            out.append(item)
            continue
        j = rng.randrange(n + 1)
        if j < k:
            out[j] = item
    return out


def _wrap_litellm_for_cost() -> Tuple[dict, Any]:
    """
    Monkeypatch litellm.acompletion to record per-call usage + estimated cost.
    Returns (state, original_fn).
    """
    state: dict = {
        "calls": [],  # list of dicts
        "cost_total_usd": 0.0,
        "cost_known": True,
        "by_model_calls": Counter(),
        "by_model_cost_usd": defaultdict(float),
    }

    orig = litellm.acompletion

    async def wrapped(*args, **kwargs):
        resp = await orig(*args, **kwargs)
        model = kwargs.get("model") or (args[0] if args else None)
        state["by_model_calls"][model] += 1

        usage = getattr(resp, "usage", None)
        rec = {"model": model, "usage": dict(usage) if usage is not None else None}

        # Try to compute cost via LiteLLM built-in pricing tables.
        try:
            cost = litellm.completion_cost(completion_response=resp, model=model)
            rec["cost_usd"] = float(cost)
            state["cost_total_usd"] += float(cost)
            state["by_model_cost_usd"][model] += float(cost)
        except Exception as e:
            rec["cost_usd_error"] = str(e)
            state["cost_known"] = False

        state["calls"].append(rec)
        return resp

    litellm.acompletion = wrapped
    return state, orig


def _restore_litellm(orig) -> None:
    litellm.acompletion = orig


async def _run_one(
    rlm,
    task: Task,
    max_context_chars: int,
    llm_timeout: int,
    llm_max_tokens: int,
    temperature: float,
    retries: int,
    retry_backoff_s: float,
) -> dict:
    # Apply a local guardrail to avoid enormous REPL slicing runs.
    ctx = task.context
    if len(ctx) > max_context_chars:
        ctx = ctx[:max_context_chars]

    # These kwargs are forwarded to LiteLLM in the unofficial implementation.
    last_err: Exception | None = None
    for attempt in range(retries + 1):
        try:
            ans = await rlm.acompletion(
                query=task.query,
                context=ctx,
                timeout=llm_timeout,
                max_tokens=llm_max_tokens,
                temperature=temperature,
            )
            return {"ok": True, "answer": ans}
        except Exception as e:
            last_err = e
            if attempt >= retries:
                break
            # Best-effort retry for transient network/provider errors.
            await asyncio.sleep(retry_backoff_s * (2**attempt))

    return {"ok": False, "error": str(last_err) if last_err is not None else "unknown error"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="gpt-5.2")
    ap.add_argument("--recursive-model", default="gpt-5.2-mini")
    ap.add_argument("--max-depth", type=int, default=5)
    ap.add_argument("--max-iterations", type=int, default=20)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--max-cost-usd", type=float, default=22.0)
    ap.add_argument("--out-jsonl", default="extracted/runs/unofficial_budgeted_eval.jsonl")
    ap.add_argument("--dotenv", default=".env")
    ap.add_argument("--max-context-chars", type=int, default=200_000)
    ap.add_argument("--llm-timeout", type=int, default=60, help="Per-LLM-call timeout (seconds)")
    ap.add_argument("--llm-max-tokens", type=int, default=800, help="Per-LLM-call max_tokens")
    ap.add_argument(
        "--temperature",
        type=float,
        default=0.0,
        help="Sampling temperature (LiteLLM will drop unsupported params when needed).",
    )
    ap.add_argument("--retries", type=int, default=2, help="Retries per task on transient LLM errors")
    ap.add_argument("--retry-backoff-seconds", type=float, default=2.0, help="Base backoff seconds for retries")
    ap.add_argument(
        "--append",
        action="store_true",
        help="Append to --out-jsonl instead of overwriting (useful with timeouts).",
    )
    ap.add_argument(
        "--resume",
        action="store_true",
        help="When used with --append, skip tasks already present in --out-jsonl (by dataset/task_id).",
    )
    ap.add_argument("--dry-run", action="store_true", help="Print planned task list and exit")
    ap.add_argument(
        "--force-subcall",
        action="store_true",
        help="Patch the system prompt to require at least one recursive_llm(...) subcall before FINAL() (wiring/cost check).",
    )

    # Per-dataset sample sizes (defaults aim to be ~1/10-ish, but budget caps anyway)
    ap.add_argument("--n-s-niah", type=int, default=5)
    ap.add_argument("--n-browsecomp", type=int, default=15)
    ap.add_argument("--n-codeqa", type=int, default=5)
    ap.add_argument("--n-oolong-synth-small", type=int, default=5)
    args = ap.parse_args()

    load_dotenv(args.dotenv)
    if not os.environ.get("OPENAI_API_KEY"):
        print("ERROR: OPENAI_API_KEY is not set (load .env or export it).")
        return 2

    upstream_dir = Path("upstream/recursive-llm")
    # Import the unofficial package by adding its src/ to sys.path.
    # This keeps relative imports (from .types import ...) working without installing the package.
    sys.path.insert(0, os.fspath(upstream_dir / "src"))
    try:
        from rlm.core import RLM  # type: ignore
        import rlm.core as rlm_core  # type: ignore
    except Exception as e:
        print(f"ERROR: failed to import unofficial RLM package from {upstream_dir / 'src'}: {e}")
        return 2

    # Patch recursive_llm's sync wrapper to close LiteLLM async clients before its event loop closes.
    # The upstream implementation uses asyncio.run() inside a thread, which can leave SSL transports open
    # and trigger "Event loop is closed" errors on shutdown.
    orig_make_recursive_fn = rlm_core.RLM._make_recursive_fn

    def patched_make_recursive_fn(self):  # type: ignore
        async def recursive_llm_with_cleanup(sub_query: str, sub_context: str) -> str:
            try:
                if self._current_depth + 1 >= self.max_depth:
                    return f"Max recursion depth ({self.max_depth}) reached"

                sub_rlm = rlm_core.RLM(
                    model=self.recursive_model,
                    recursive_model=self.recursive_model,
                    api_base=self.api_base,
                    api_key=self.api_key,
                    max_depth=self.max_depth,
                    max_iterations=self.max_iterations,
                    _current_depth=self._current_depth + 1,
                    **self.llm_kwargs,
                )
                return await sub_rlm.acompletion(sub_query, sub_context)
            finally:
                try:
                    await litellm.close_litellm_async_clients()
                except Exception:
                    pass

        def sync_recursive_llm(sub_query: str, sub_context: str) -> str:
            try:
                _ = asyncio.get_running_loop()
                import concurrent.futures

                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(
                        asyncio.run, recursive_llm_with_cleanup(sub_query, sub_context)
                    )
                    return future.result()
            except RuntimeError:
                return asyncio.run(recursive_llm_with_cleanup(sub_query, sub_context))

        return sync_recursive_llm

    rlm_core.RLM._make_recursive_fn = patched_make_recursive_fn

    if args.force_subcall:
        orig_build_system_prompt = rlm_core.build_system_prompt

        def patched_build_system_prompt(context_size: int, depth: int = 0) -> str:
            s = orig_build_system_prompt(context_size, depth)
            if depth == 0:
                s += (
                    "\n\nMANDATORY (test harness): Before you produce FINAL(...), you must call "
                    "recursive_llm('ping', str(context)[:2000]) at least once. "
                    "If you have not done this yet, your next message MUST be Python code that calls recursive_llm and prints the result. "
                    "Only after that may you produce FINAL(...)."
                )
            return s

        # core.py imports build_system_prompt by name; patch the symbol it actually calls.
        rlm_core.build_system_prompt = patched_build_system_prompt

    rng = random.Random(args.seed)

    # Load tasks (local snapshots).
    tasks: List[Task] = []

    s_niah_path = Path("extracted/eval/s_niah.jsonl")
    if s_niah_path.exists():
        tasks.extend(_reservoir_sample(_iter_s_niah(s_niah_path), args.n_s_niah, rng))

    browse_path = Path("upstream/bench_datasets/hf/Tevatron__browsecomp-plus/data/test-00000-of-00006.parquet")
    if browse_path.exists():
        tasks.extend(_reservoir_sample(_iter_browsecomp_plus(browse_path, args.max_context_chars), args.n_browsecomp, rng))

    codeqa_path = Path("upstream/bench_datasets/hf/zai-org__LongBench-v2/data.json")
    if codeqa_path.exists():
        tasks.extend(_reservoir_sample(_iter_longbench_v2_codeqa(codeqa_path), args.n_codeqa, rng))

    oolong_small_path = Path("upstream/bench_datasets/hf/tonychenxyz__oolong-synth-1k-16k/plain/test.jsonl")
    if oolong_small_path.exists():
        tasks.extend(_reservoir_sample(_iter_oolong_synth_small(oolong_small_path), args.n_oolong_synth_small, rng))

    if not tasks:
        print("ERROR: no tasks found (datasets missing?).")
        return 2

    # Deterministic ordering to ease diffing across runs.
    tasks.sort(key=lambda t: (t.dataset, t.task_id))

    if args.dry_run:
        print("planned_tasks:")
        for t in tasks:
            print(f"  - {t.dataset}:{t.task_id} query_len={len(t.query)} context_len={len(t.context)}")
        print("DRY_RUN")
        return 0

    # Instrument cost + per-model call counts.
    state, orig_acompletion = _wrap_litellm_for_cost()
    # Make runs more robust across model families (e.g., gpt-5-mini rejecting some params).
    litellm.drop_params = True

    out_path = Path(args.out_jsonl)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    total_tasks = 0
    try:
        total_tasks = asyncio.run(_run_eval(tasks, args, RLM, state, out_path))
    finally:
        _restore_litellm(orig_acompletion)

    # Print a small summary for the operator.
    print(f"wrote {out_path} (tasks_run={total_tasks})")
    print(f"cost_known={state['cost_known']} cost_total_usd={state['cost_total_usd']:.4f}")
    print("calls_by_model:")
    for m, c in state["by_model_calls"].most_common():
        cost = state["by_model_cost_usd"].get(m, 0.0)
        print(f"  {m}: calls={c} cost_usd={cost:.4f}")

    # Sanity: ensure at least one recursive-model call happened (otherwise we didn't test the cheap path).
    if state["by_model_calls"].get(args.recursive_model, 0) == 0:
        print("WARNING: no recursive-model calls were observed. Cost-optimization path may not have been exercised.")

    return 0


async def _run_eval(tasks: List[Task], args: Any, RLM: Any, state: dict, out_path: Path) -> int:
    total_tasks = 0
    mode = "a" if args.append else "w"
    done: set[tuple[str, str]] = set()
    if args.append and args.resume and out_path.exists():
        try:
            with out_path.open("r", encoding="utf-8") as rf:
                for line in rf:
                    try:
                        o = json.loads(line)
                    except Exception:
                        continue
                    ds = o.get("dataset")
                    tid = o.get("task_id")
                    if isinstance(ds, str) and isinstance(tid, str):
                        done.add((ds, tid))
        except Exception:
            # If resume scan fails, fall back to not skipping.
            done = set()

    with out_path.open(mode, encoding="utf-8") as f:
        for t in tasks:
            if done and (t.dataset, t.task_id) in done:
                continue
            rlm = RLM(
                model=args.model,
                recursive_model=args.recursive_model,
                max_depth=args.max_depth,
                max_iterations=args.max_iterations,
            )

            res = await _run_one(
                rlm,
                t,
                args.max_context_chars,
                llm_timeout=args.llm_timeout,
                llm_max_tokens=args.llm_max_tokens,
                temperature=args.temperature,
                retries=args.retries,
                retry_backoff_s=args.retry_backoff_seconds,
            )
            rec = {
                "dataset": t.dataset,
                "task_id": t.task_id,
                "model": args.model,
                "recursive_model": args.recursive_model,
                "max_depth": args.max_depth,
                "max_iterations": args.max_iterations,
                "llm_timeout": args.llm_timeout,
                "llm_max_tokens": args.llm_max_tokens,
                "temperature": args.temperature,
                "retries": args.retries,
                "retry_backoff_seconds": args.retry_backoff_seconds,
                "ok": res["ok"],
                "error": res.get("error"),
                # Avoid dumping full answer; keep it small.
                "answer_snippet": (res.get("answer") or "")[:400],
                "stats": getattr(rlm, "stats", None),
                "cost_total_usd_so_far": state["cost_total_usd"],
                "cost_known": state["cost_known"],
            }
            f.write(json.dumps(rec, ensure_ascii=True) + "\n")
            f.flush()
            total_tasks += 1

            # Soft budget: stop after finishing this task.
            if state["cost_known"] and state["cost_total_usd"] >= args.max_cost_usd:
                break

    # Ensure async HTTP clients are closed before the event loop shuts down to avoid SSL transport errors.
    try:
        await litellm.close_litellm_async_clients()
    except Exception:
        pass
    return total_tasks


if __name__ == "__main__":
    raise SystemExit(main())
