#!/usr/bin/env python3
"""
Run the unofficial RLM baseline on a small task set, while recording a full REPL transcript.

This addresses the core goal of this repo: extract the *actually used* REPL surface (code executed, outputs, errors)
from real benchmark-derived inputs.

Outputs:
- per-task results: --out-jsonl
- step-by-step transcript (LLM call + REPL exec + FINAL): --transcript-jsonl

Security:
- never logs OPENAI_API_KEY
- does not persist full benchmark contexts (only length); REPL outputs are already truncated by REPLExecutor
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import random
import re
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Tuple

import litellm

from tools.load_dotenv_local import load_dotenv


@dataclass
class Task:
    dataset: str
    task_id: str
    query: str
    context: str


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


def _iter_s_niah(path: Path) -> Iterable[Task]:
    with path.open("r", encoding="utf-8") as f:
        for line in f:
            o = json.loads(line)
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
            yield Task(dataset="s_niah", task_id=str(o["id"]), query=question, context=ctx)


def _iter_oolong_synth_small(path: Path) -> Iterable[Task]:
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
        yield Task(
            dataset="longbench_v2_codeqa",
            task_id=task_id,
            query=ex["question"],
            context=ex["context"],
        )


def _iter_browsecomp_plus(path: Path, max_context_chars: int) -> Iterable[Task]:
    import pyarrow.parquet as pq

    t = pq.read_table(path)
    for i in range(t.num_rows):
        row = {name: t.column(name)[i].as_py() for name in t.column_names}
        query = row["query"]
        docs: list[str] = []
        for group in ["gold_docs", "evidence_docs", "negative_docs"]:
            for d in row.get(group, []) or []:
                docs.append(d.get("text", ""))
        context = "\n\n".join(docs)
        if len(context) > max_context_chars:
            context = context[:max_context_chars]
        yield Task(dataset="browsecomp_plus", task_id=str(i), query=query, context=context)


def _load_tasks_from_import_errors(args: Any) -> List[Task]:
    """
    Load only tasks that previously produced `Execution error: __import__ not found`,
    based on a prior transcript JSONL.
    """
    transcript = Path(args.only_import_errors_from_transcript)
    if not transcript.exists():
        raise FileNotFoundError(str(transcript))

    wanted: set[tuple[str, str]] = set()
    for line in transcript.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        ev = json.loads(line)
        if ev.get("type") != "repl_error":
            continue
        if "__import__ not found" not in str(ev.get("error", "")):
            continue
        ds = ev.get("dataset")
        tid = ev.get("task_id")
        if isinstance(ds, str) and isinstance(tid, str):
            wanted.add((ds, tid))

    # Resolve exact task payloads from local datasets.
    out: List[Task] = []

    # S-NIAH
    if any(ds == "s_niah" for ds, _ in wanted):
        by_id: dict[str, dict] = {}
        with Path(args.s_niah_path).open("r", encoding="utf-8") as f:
            for line in f:
                o = json.loads(line)
                by_id[str(o["id"])] = o
        for ds, tid in sorted(wanted):
            if ds != "s_niah":
                continue
            o = by_id.get(tid)
            if not o:
                continue
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
            out.append(Task(dataset="s_niah", task_id=tid, query=question, context=ctx))

    # BrowseComp+
    if any(ds == "browsecomp_plus" for ds, _ in wanted):
        import pyarrow.parquet as pq

        browse_path = Path(args.browsecomp_parquet)
        t = pq.read_table(browse_path)
        for ds, tid in sorted(wanted):
            if ds != "browsecomp_plus":
                continue
            try:
                i = int(tid)
            except Exception:
                continue
            if i < 0 or i >= t.num_rows:
                continue
            row = {name: t.column(name)[i].as_py() for name in t.column_names}
            query = row["query"]
            docs: list[str] = []
            for group in ["gold_docs", "evidence_docs", "negative_docs"]:
                for d in row.get(group, []) or []:
                    docs.append(d.get("text", ""))
            context = "\n\n".join(docs)
            if len(context) > args.max_context_chars:
                context = context[: args.max_context_chars]
            out.append(Task(dataset="browsecomp_plus", task_id=tid, query=query, context=context))

    # LongBench-v2 CodeQA
    if any(ds == "longbench_v2_codeqa" for ds, _ in wanted):
        codeqa_path = Path(args.longbench_json)
        data = json.loads(codeqa_path.read_text(encoding="utf-8"))
        by_id: dict[str, dict] = {}
        for ex in data:
            if ex.get("sub_domain") != "Code repo QA":
                continue
            _id = str(ex.get("_id", ""))
            by_id[_id] = ex
        for ds, tid in sorted(wanted):
            if ds != "longbench_v2_codeqa":
                continue
            ex = by_id.get(tid)
            if not ex:
                continue
            out.append(Task(dataset="longbench_v2_codeqa", task_id=tid, query=ex["question"], context=ex["context"]))

    out.sort(key=lambda t: (t.dataset, t.task_id))
    return out


def _load_tasks(args: Any, rng: random.Random) -> List[Task]:
    if args.only_import_errors_from_transcript:
        return _load_tasks_from_import_errors(args)

    tasks: List[Task] = []

    s_niah_path = Path(args.s_niah_path)
    if s_niah_path.exists():
        tasks.extend(_reservoir_sample(_iter_s_niah(s_niah_path), args.n_s_niah, rng))

    browse_path = Path(args.browsecomp_parquet)
    if browse_path.exists():
        tasks.extend(
            _reservoir_sample(
                _iter_browsecomp_plus(browse_path, args.max_context_chars),
                args.n_browsecomp,
                rng,
            )
        )

    codeqa_path = Path(args.longbench_json)
    if codeqa_path.exists():
        tasks.extend(_reservoir_sample(_iter_longbench_v2_codeqa(codeqa_path), args.n_codeqa, rng))

    oolong_path = Path(args.oolong_jsonl)
    if oolong_path.exists():
        tasks.extend(_reservoir_sample(_iter_oolong_synth_small(oolong_path), args.n_oolong_synth_small, rng))

    tasks.sort(key=lambda t: (t.dataset, t.task_id))
    return tasks


class TranscriptWriter:
    def __init__(self, path: Path):
        self.path = path
        self._lock = threading.Lock()
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def write(self, rec: dict) -> None:
        line = json.dumps(rec, ensure_ascii=True)
        with self._lock:
            with self.path.open("a", encoding="utf-8") as f:
                f.write(line + "\n")
                f.flush()


def _truncate(s: str, n: int) -> str:
    if s is None:
        return ""
    if len(s) <= n:
        return s
    return s[:n] + f"\n\n[truncated {len(s)} chars -> {n}]"


def _install_instrumentation(
    rlm_core: Any,
    rlm_repl: Any,
    rlm_parser: Any,
    writer: TranscriptWriter,
    *,
    max_event_chars: int,
    strict_code: bool,
    inject_b64zlib: bool,
    zlib_max_output_bytes: int,
) -> None:
    """
    Patch the unofficial implementation in-process to emit transcript events.
    """

    # Ensure LiteLLM doesn't crash on unsupported params (e.g. gpt-5-mini temperature).
    litellm.drop_params = True

    if strict_code:
        orig_build_system_prompt = rlm_core.build_system_prompt

        def patched_build_system_prompt(context_size: int, depth: int = 0) -> str:
            s = orig_build_system_prompt(context_size, depth)
            # Enforce code-only + ASCII to reduce non-code chatter that breaks the REPL.
            if inject_b64zlib:
                s += (
                    "\n\nNOTE:\n"
                    "- The REPL already provides base64, binascii, and zlib (do not import them).\n"
                    "- zlib only supports zlib.decompress(data[, wbits]) with an output-size cap.\n"
                )
            s += (
                "\n\nIMPORTANT:\n"
                "- Reply with ONLY valid Python code (no prose).\n"
                "- Do NOT use markdown fences.\n"
                "- Use ASCII characters only.\n"
                "- If you need to explain, use Python comments starting with # and ASCII only.\n"
                "- Do NOT use import statements (modules are already available in globals).\n"
                "- When done, output FINAL(\"...\") as plain text on its own line.\n"
                "- If you computed a variable, use FINAL_VAR(var_name) (do not write FINAL(var_name)).\n"
            )
            return s

        rlm_core.build_system_prompt = patched_build_system_prompt

    if inject_b64zlib:
        # Patch REPL globals (allowlist) to include base64/binascii and a size-capped zlib.decompress.
        import base64
        import binascii
        import zlib
        import operator
        from RestrictedPython.Guards import full_write_guard

        def _inplacevar_(op: str, x: Any, y: Any) -> Any:
            ops = {
                "+=": operator.add,
                "-=": operator.sub,
                "*=": operator.mul,
                "/=": operator.truediv,
                "//=": operator.floordiv,
                "%=": operator.mod,
                "**=": operator.pow,
                "<<=": operator.lshift,
                ">>=": operator.rshift,
                "&=": operator.and_,
                "^=": operator.xor,
                "|=": operator.or_,
            }
            fn = ops.get(str(op))
            if fn is None:
                raise ValueError(f"unsupported inplace op: {op!r}")
            return fn(x, y)

        class SafeZlib:
            def __init__(self, max_output_bytes: int):
                self._max_output_bytes = int(max_output_bytes)

            def decompress(self, data: bytes, wbits: int = 15) -> bytes:
                # Stream-decompress with a hard output cap to avoid zlib bombs.
                max_out = self._max_output_bytes
                if max_out <= 0:
                    raise ValueError("invalid zlib max_output_bytes")
                d = zlib.decompressobj(wbits)
                out_parts: list[bytes] = []
                produced = 0

                # First chunk
                chunk = d.decompress(data, max_length=max_out + 1)
                out_parts.append(chunk)
                produced += len(chunk)
                if produced > max_out:
                    raise ValueError("zlib output exceeds limit")

                # Continue consuming any remaining compressed tail
                while d.unconsumed_tail and produced < max_out:
                    chunk = d.decompress(d.unconsumed_tail, max_length=(max_out + 1 - produced))
                    out_parts.append(chunk)
                    produced += len(chunk)
                    if produced > max_out:
                        raise ValueError("zlib output exceeds limit")

                # Flush any buffered output in bounded chunks
                while produced < max_out:
                    chunk = d.flush(min(16384, max_out + 1 - produced))
                    if not chunk:
                        break
                    out_parts.append(chunk)
                    produced += len(chunk)
                    if produced > max_out:
                        raise ValueError("zlib output exceeds limit")

                # If there is still pending output/tail after hitting cap, fail fast.
                if d.unconsumed_tail or produced > max_out:
                    raise ValueError("zlib output exceeds limit")

                return b"".join(out_parts)

            def __getattr__(self, name: str) -> Any:
                # Only `decompress` is intentionally exposed.
                raise AttributeError(name)

        # When running with the Rust backend (`RLM_REPL_BACKEND=rust`), REPLExecutor is
        # RustREPLExecutor and does not have `_build_globals`. In that mode, base64/zlib
        # are already provided by the Rust subset; we only need the prompt tweak.
        if hasattr(rlm_repl.REPLExecutor, "_build_globals"):
            orig_build_globals = rlm_repl.REPLExecutor._build_globals

            def wrapped_build_globals(self, env: Dict[str, Any]) -> Dict[str, Any]:  # type: ignore
                g = orig_build_globals(self, env)
                # Guards required by RestrictedPython transformations.
                g.setdefault("_write_", full_write_guard)
                g.setdefault("_inplacevar_", _inplacevar_)
                # Extra stdlib modules (no import needed).
                g.setdefault("base64", base64)
                g.setdefault("binascii", binascii)
                g.setdefault("zlib", SafeZlib(zlib_max_output_bytes))
                return g

            rlm_repl.REPLExecutor._build_globals = wrapped_build_globals  # type: ignore

    # Patch recursive wrapper cleanup to avoid asyncio SSL transport errors.
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
                    future = executor.submit(asyncio.run, recursive_llm_with_cleanup(sub_query, sub_context))
                    return future.result()
            except RuntimeError:
                return asyncio.run(recursive_llm_with_cleanup(sub_query, sub_context))

        return sync_recursive_llm

    rlm_core.RLM._make_recursive_fn = patched_make_recursive_fn

    # Patch _build_repl_env so REPL logs can include task metadata.
    orig_build_env = rlm_core.RLM._build_repl_env

    def wrapped_build_env(self, query: str, context: str):  # type: ignore
        env = orig_build_env(self, query, context)
        meta = getattr(self, "_task_meta", None)
        if isinstance(meta, dict):
            env["_task_meta"] = meta
        return env

    rlm_core.RLM._build_repl_env = wrapped_build_env  # type: ignore

    # Patch LLM call to log raw assistant messages + model + usage/cost if available.
    orig_call_llm = rlm_core.RLM._call_llm

    async def wrapped_call_llm(self, messages: list, **kwargs):  # type: ignore
        t0 = time.time()
        try:
            resp_text = await orig_call_llm(self, messages, **kwargs)
            dt_ms = int((time.time() - t0) * 1000)
            meta = getattr(self, "_task_meta", None) or {}
            writer.write(
                {
                    "type": "llm_response",
                    "dataset": meta.get("dataset"),
                    "task_id": meta.get("task_id"),
                    "depth": getattr(self, "_current_depth", None),
                    "iteration": getattr(self, "_iterations", None),
                    "model_selected": kwargs.get("model") or (self.model if getattr(self, "_current_depth", 0) == 0 else self.recursive_model),
                    "elapsed_ms": dt_ms,
                    "content": _truncate(resp_text or "", max_event_chars),
                }
            )
            return resp_text
        except Exception as e:
            dt_ms = int((time.time() - t0) * 1000)
            meta = getattr(self, "_task_meta", None) or {}
            writer.write(
                {
                    "type": "llm_error",
                    "dataset": meta.get("dataset"),
                    "task_id": meta.get("task_id"),
                    "depth": getattr(self, "_current_depth", None),
                    "iteration": getattr(self, "_iterations", None),
                    "elapsed_ms": dt_ms,
                    "error": str(e),
                }
            )
            raise

    rlm_core.RLM._call_llm = wrapped_call_llm  # type: ignore

    # Patch REPL execution to log code + output/errors.
    orig_execute = rlm_repl.REPLExecutor.execute

    def wrapped_execute(self, code: str, env: dict):  # type: ignore
        meta = env.get("_task_meta") if isinstance(env, dict) else None
        writer.write(
            {
                "type": "repl_input",
                "dataset": (meta or {}).get("dataset") if isinstance(meta, dict) else None,
                "task_id": (meta or {}).get("task_id") if isinstance(meta, dict) else None,
                "code": _truncate(code or "", max_event_chars),
            }
        )
        try:
            out = orig_execute(self, code, env)
            writer.write(
                {
                    "type": "repl_output",
                    "dataset": (meta or {}).get("dataset") if isinstance(meta, dict) else None,
                    "task_id": (meta or {}).get("task_id") if isinstance(meta, dict) else None,
                    "output": _truncate(out or "", max_event_chars),
                }
            )
            return out
        except Exception as e:
            writer.write(
                {
                    "type": "repl_error",
                    "dataset": (meta or {}).get("dataset") if isinstance(meta, dict) else None,
                    "task_id": (meta or {}).get("task_id") if isinstance(meta, dict) else None,
                    "error": str(e),
                }
            )
            raise

    rlm_repl.REPLExecutor.execute = wrapped_execute  # type: ignore

    # Patch parse_response to log final answers (truncated).
    # NOTE: core.py imports `parse_response` into its module namespace:
    # `from .parser import parse_response, is_final`
    # so patching rlm_parser.parse_response alone does not affect execution.
    orig_parse = rlm_core.parse_response

    def wrapped_parse(response: str, env: dict):  # type: ignore
        ans = orig_parse(response, env)
        meta = env.get("_task_meta") if isinstance(env, dict) else None
        writer.write(
            {
                "type": "final_parsed",
                "dataset": (meta or {}).get("dataset") if isinstance(meta, dict) else None,
                "task_id": (meta or {}).get("task_id") if isinstance(meta, dict) else None,
                "answer": _truncate(str(ans or ""), max_event_chars),
            }
        )
        return ans

    rlm_core.parse_response = wrapped_parse  # type: ignore


def _load_unofficial_modules() -> Tuple[Any, Any, Any, Any]:
    sys.path.insert(0, os.fspath(Path("upstream/recursive-llm/src")))
    import rlm.core as rlm_core  # type: ignore
    import rlm.repl as rlm_repl  # type: ignore
    import rlm.parser as rlm_parser  # type: ignore

    return rlm_core, rlm_repl, rlm_parser, rlm_core.RLM


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dotenv", default=".env")
    ap.add_argument("--seed", type=int, default=42)

    ap.add_argument("--model", default="gpt-5.2")
    ap.add_argument("--recursive-model", default="gpt-5-mini")
    ap.add_argument("--max-depth", type=int, default=5)
    ap.add_argument("--max-iterations", type=int, default=20)
    ap.add_argument("--llm-timeout", type=int, default=90)
    ap.add_argument("--llm-max-tokens", type=int, default=900)
    ap.add_argument("--temperature", type=float, default=0.0)
    ap.add_argument("--retries", type=int, default=5)
    ap.add_argument("--retry-backoff-seconds", type=float, default=2.0)
    ap.add_argument("--max-context-chars", type=int, default=200_000)

    # Task selection (paper-ish 1/10 sizing)
    ap.add_argument("--n-browsecomp", type=int, default=15)
    ap.add_argument("--n-codeqa", type=int, default=5)
    ap.add_argument("--n-oolong-synth-small", type=int, default=5)
    ap.add_argument("--n-s-niah", type=int, default=5)

    # Paths
    ap.add_argument(
        "--s-niah-path",
        default="extracted/eval/s_niah.jsonl",
        dest="s_niah_path",
    )
    ap.add_argument(
        "--browsecomp-parquet",
        default="upstream/bench_datasets/hf/Tevatron__browsecomp-plus/data/test-00000-of-00006.parquet",
        dest="browsecomp_parquet",
    )
    ap.add_argument(
        "--longbench-json",
        default="upstream/bench_datasets/hf/zai-org__LongBench-v2/data.json",
        dest="longbench_json",
    )
    ap.add_argument(
        "--oolong-jsonl",
        default="upstream/bench_datasets/hf/tonychenxyz__oolong-synth-1k-16k/plain/test.jsonl",
        dest="oolong_jsonl",
    )

    ap.add_argument("--out-jsonl", default="extracted/runs/unofficial_tasks30_logged.jsonl")
    ap.add_argument("--transcript-jsonl", default="extracted/runs/unofficial_tasks30_transcript.jsonl")
    ap.add_argument("--max-event-chars", type=int, default=20000)
    ap.add_argument("--append", action="store_true")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--strict-code", action="store_true", help="Patch the system prompt to enforce code-only + ASCII")
    ap.add_argument(
        "--only-import-errors-from-transcript",
        default="",
        help="Rerun only tasks that had `__import__ not found` in a prior transcript JSONL.",
    )
    ap.add_argument(
        "--inject-b64zlib",
        action="store_true",
        help="Inject base64/binascii and a size-capped zlib.decompress into restricted globals (no imports).",
    )
    ap.add_argument("--zlib-max-output-bytes", type=int, default=1_000_000)

    args = ap.parse_args()

    load_dotenv(args.dotenv)
    if not os.environ.get("OPENAI_API_KEY"):
        print("ERROR: OPENAI_API_KEY is not set (load .env or export it).")
        return 2

    rng = random.Random(args.seed)
    tasks = _load_tasks(args, rng)
    if not tasks:
        print("ERROR: no tasks found (datasets missing?).")
        return 2

    if args.dry_run:
        for t in tasks:
            print(f"- {t.dataset}:{t.task_id} query_len={len(t.query)} context_len={len(t.context)}")
        return 0

    done: set[tuple[str, str]] = set()
    out_path = Path(args.out_jsonl)
    if args.append and args.resume and out_path.exists():
        for line in out_path.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            try:
                o = json.loads(line)
            except Exception:
                continue
            ds = o.get("dataset")
            tid = o.get("task_id")
            if isinstance(ds, str) and isinstance(tid, str):
                done.add((ds, tid))

    writer = TranscriptWriter(Path(args.transcript_jsonl))
    rlm_core, rlm_repl, rlm_parser, RLM = _load_unofficial_modules()
    _install_instrumentation(
        rlm_core,
        rlm_repl,
        rlm_parser,
        writer,
        max_event_chars=args.max_event_chars,
        strict_code=args.strict_code,
        inject_b64zlib=args.inject_b64zlib,
        zlib_max_output_bytes=args.zlib_max_output_bytes,
    )

    asyncio.run(_run_all(tasks, done, args, RLM, writer))

    print(f"wrote {out_path}")
    print(f"wrote {args.transcript_jsonl}")
    return 0


async def _run_all(tasks: List[Task], done: set[tuple[str, str]], args: Any, RLM: Any, writer: TranscriptWriter) -> None:
    out_path = Path(args.out_jsonl)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    mode = "a" if args.append else "w"

    with out_path.open(mode, encoding="utf-8") as f:
        for t in tasks:
            if done and (t.dataset, t.task_id) in done:
                continue

            writer.write(
                {
                    "type": "task_start",
                    "dataset": t.dataset,
                    "task_id": t.task_id,
                    "query_len": len(t.query),
                    "query": _truncate(t.query, min(2000, args.max_event_chars)),
                    "context_len": len(t.context),
                    "model": args.model,
                    "recursive_model": args.recursive_model,
                }
            )

            last_err: Optional[str] = None
            ok = False
            answer: Optional[str] = None

            for attempt in range(args.retries + 1):
                try:
                    rlm = RLM(
                        model=args.model,
                        recursive_model=args.recursive_model,
                        max_depth=args.max_depth,
                        max_iterations=args.max_iterations,
                        temperature=args.temperature,
                        timeout=args.llm_timeout,
                        max_tokens=args.llm_max_tokens,
                    )
                    rlm._task_meta = {"dataset": t.dataset, "task_id": t.task_id}  # type: ignore[attr-defined]
                    answer = await rlm.acompletion(query=t.query, context=t.context)
                    ok = True
                    last_err = None
                    break
                except Exception as e:
                    last_err = str(e)
                    writer.write(
                        {
                            "type": "task_error",
                            "dataset": t.dataset,
                            "task_id": t.task_id,
                            "attempt": attempt,
                            "error": last_err,
                        }
                    )
                    if attempt >= args.retries:
                        break
                    await asyncio.sleep(args.retry_backoff_seconds * (2**attempt))

            rec = {
                "dataset": t.dataset,
                "task_id": t.task_id,
                "ok": ok,
                "error": last_err,
                "answer_snippet": _truncate(answer or "", 400),
                "model": args.model,
                "recursive_model": args.recursive_model,
                "max_depth": args.max_depth,
                "max_iterations": args.max_iterations,
                "llm_timeout": args.llm_timeout,
                "llm_max_tokens": args.llm_max_tokens,
                "temperature": args.temperature,
                "retries": args.retries,
                "retry_backoff_seconds": args.retry_backoff_seconds,
            }
            f.write(json.dumps(rec, ensure_ascii=True) + "\n")
            f.flush()

            writer.write(
                {
                    "type": "task_end",
                    "dataset": t.dataset,
                    "task_id": t.task_id,
                    "ok": ok,
                    "error": last_err,
                }
            )

    try:
        await litellm.close_litellm_async_clients()
    except Exception:
        pass


if __name__ == "__main__":
    raise SystemExit(main())
