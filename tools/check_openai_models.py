#!/usr/bin/env python3
"""
Verify that the configured API key can call specific OpenAI models via LiteLLM.

This script:
- loads .env locally (no values printed)
- makes a minimal chat completion call for each model
- prints success/failure, a short response snippet, and usage if available
"""

from __future__ import annotations

import argparse
import asyncio
import os
from dataclasses import dataclass
from typing import Any, Dict, Optional

import litellm

from tools.load_dotenv_local import load_dotenv


@dataclass
class CallResult:
    model: str
    ok: bool
    text_snippet: str = ""
    usage: Optional[Dict[str, Any]] = None
    error: str = ""


def _snippet(s: str, n: int = 160) -> str:
    s = (s or "").replace("\n", "\\n")
    return s[:n]


async def _one(model: str, timeout: int) -> CallResult:
    messages = [
        {"role": "system", "content": "You are a test harness. Reply with exactly: OK"},
        {"role": "user", "content": "Reply with exactly: OK"},
    ]
    try:
        resp = await litellm.acompletion(
            model=model,
            messages=messages,
            max_tokens=8,
            timeout=timeout,
        )
        # LiteLLM response generally has choices[0].message.content
        content = resp.choices[0].message.content or ""
        usage = getattr(resp, "usage", None)
        return CallResult(model=model, ok=True, text_snippet=_snippet(content), usage=usage)
    except Exception as e:
        return CallResult(model=model, ok=False, error=str(e))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", nargs="+", default=["gpt-5.2", "gpt-5.2-mini"])
    ap.add_argument("--timeout", type=int, default=60)
    ap.add_argument("--dotenv", default=".env")
    args = ap.parse_args()

    load_dotenv(args.dotenv)
    if not os.environ.get("OPENAI_API_KEY"):
        print("ERROR: OPENAI_API_KEY is not set (load .env or export it).")
        return 2

    results = asyncio.run(_run_all(args.models, args.timeout))
    ok_all = all(r.ok for r in results)

    for r in results:
        print(f"- model={r.model} ok={r.ok}")
        if r.ok:
            print(f"  text={r.text_snippet!r}")
            if r.usage is not None:
                print(f"  usage={dict(r.usage)}")
        else:
            print(f"  error={r.error}")

    if ok_all:
        print("ALL_OK")
        return 0
    print("SOME_FAILED")
    return 1


async def _run_all(models: list[str], timeout: int) -> list[CallResult]:
    # Run sequentially to keep logs ordered and avoid accidental concurrency costs.
    out: list[CallResult] = []
    for m in models:
        out.append(await _one(m, timeout))
    return out


if __name__ == "__main__":
    raise SystemExit(main())
