#!/usr/bin/env python3
"""
Extract evaluation artifacts embedded directly in the RLM paper HTML.

Currently supported:
- OOLONG-Pairs task prompts listed in Appendix E.1 (A5.SS1).

This is useful because some parts of the "evaluation suite" are specified
directly in the paper, independent of external dataset releases.
"""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from pathlib import Path


def _strip_tags(s: str) -> str:
    # Cheap HTML tag stripper, good enough for ltx_* markup.
    s = re.sub(r"<br\\s*/?>", "\n", s)
    s = re.sub(r"<[^>]+>", "", s)
    s = re.sub(r"\s+\n", "\n", s)
    s = re.sub(r"\n\s+", "\n", s)
    return s.strip()


def extract_oolong_pairs_tasks(html: str) -> list[dict]:
    """
    Extract Task N blocks from Appendix E.1 section (A5.SS1).
    """
    # Find the subsection block.
    m = re.search(
        r'<section class="ltx_subsection" id="A5\.SS1">(.*?)</section>',
        html,
        re.DOTALL,
    )
    if not m:
        return []
    sec = m.group(1)

    tasks = []
    # Each task paragraph has id="A5.SS1.p{N}.1" and begins with "Task X".
    # We'll parse the visible text after stripping tags.
    for pm in re.finditer(
        r'<p class="ltx_p" id="A5\.SS1\.p(\d+)\.1">(.*?)</p>',
        sec,
        re.DOTALL,
    ):
        pid = int(pm.group(1))
        raw = pm.group(2)
        txt = _strip_tags(raw)
        # Skip the intro paragraphs (p1.1, p2.1) which are not "Task ..." entries.
        if not txt.startswith("Task "):
            continue
        # Extract the task number from "Task X"
        mnum = re.match(r"Task\s+(\d+)\s*(.*)$", txt, re.DOTALL)
        if not mnum:
            continue
        task_num = int(mnum.group(1))
        body = mnum.group(2).strip()
        tasks.append(
            {
                "task_num": task_num,
                "paper_pid": f"A5.SS1.p{pid}.1",
                "prompt": body,
            }
        )
    return tasks


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--html", required=True, help="Path to RLM arXiv HTML (2512.24601v1)")
    ap.add_argument("--out-json", required=True, help="Output JSON path")
    args = ap.parse_args()

    html_path = Path(args.html)
    out_json = Path(args.out_json)
    out_json.parent.mkdir(parents=True, exist_ok=True)

    html = html_path.read_text(encoding="utf-8", errors="replace")
    tasks = extract_oolong_pairs_tasks(html)
    out = {
        "source": {"paper": "2512.24601v1", "section": "Appendix E.1 (A5.SS1)"},
        "oolong_pairs_tasks": tasks,
        "count": len(tasks),
    }
    out_json.write_text(json.dumps(out, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    print(f"extracted {len(tasks)} OOLONG-Pairs task(s) -> {out_json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
