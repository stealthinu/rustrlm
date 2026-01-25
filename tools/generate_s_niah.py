#!/usr/bin/env python3
"""
Generate a lightweight S-NIAH-like dataset (50 tasks) inspired by RULER NIAH.

This avoids tokenizer dependencies and focuses on the REPL-relevant property:
finding a specific needle (number) inside a large, mostly irrelevant haystack.

Output format (jsonl):
{
  "id": int,
  "context": str,
  "query": str,
  "answer": str
}
"""

from __future__ import annotations

import argparse
import json
import random
from pathlib import Path


def _load_paul_graham_text(path: Path) -> str:
    obj = json.loads(path.read_text(encoding="utf-8"))
    text = obj.get("text", "")
    return " ".join(text.split())


def _make_task(haystack_words: list[str], target_chars: int, rng: random.Random, task_id: int) -> dict:
    # Choose a slice of words that roughly matches the target size.
    # We do a simple grow-until approach for determinism + simplicity.
    start = rng.randrange(0, max(1, len(haystack_words) - 1000))
    words = []
    char_count = 0
    i = start
    while i < len(haystack_words) and char_count < target_chars:
        w = haystack_words[i]
        words.append(w)
        char_count += len(w) + 1
        i += 1
    if not words:
        words = haystack_words[: min(1000, len(haystack_words))]

    key = f"key-{task_id}"
    value = str(rng.randint(10**8, 10**9 - 1))
    needle_sentence = f"One of the special magic numbers for {key} is: {value}."

    insert_at = rng.randrange(0, max(1, len(words)))
    # Insert as its own "word" chunk so it stays searchable as a sentence.
    words.insert(insert_at, needle_sentence)
    context = " ".join(words)

    query = (
        "Some special magic numbers are hidden within the following text. "
        "Make sure to memorize it. I will quiz you about the numbers afterwards.\n"
        f"{context}\n"
        f"What is the special magic number for {key} mentioned in the provided text?"
    )

    return {"id": task_id, "context": query, "query": key, "answer": value}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pg-json", required=True, help="Path to PaulGrahamEssays.json")
    ap.add_argument("--out-jsonl", required=True, help="Output jsonl path")
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--num", type=int, default=50)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    text = _load_paul_graham_text(Path(args.pg_json))
    haystack_words = text.split(" ")

    # Use a spread of sizes to emulate scaling; keep within reasonable local limits.
    sizes = [8_000, 16_000, 32_000, 64_000, 128_000, 256_000]
    out_path = Path(args.out_jsonl)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    with out_path.open("w", encoding="utf-8") as f:
        for i in range(args.num):
            target = sizes[i % len(sizes)]
            rec = _make_task(haystack_words, target_chars=target, rng=rng, task_id=i)
            f.write(json.dumps(rec, ensure_ascii=True) + "\n")

    print(f"wrote {args.num} tasks -> {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

