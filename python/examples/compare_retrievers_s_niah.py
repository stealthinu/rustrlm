import argparse
import json
import os
import re
import sys
from typing import Iterable, List, Tuple

# Avoid writing under /home/stealth/.local and disable telemetry
os.environ.setdefault("CONTINUOUS_EVAL_DO_NOT_TRACK", "true")
os.environ.setdefault("XDG_DATA_HOME", "/tmp/continuous_eval")

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
os.chdir(REPO_ROOT)

from rustrlm_client import RustRLMClient  # noqa: E402

try:
    from langchain_core.documents import Document as LCDocument  # noqa: E402
    from langchain_community.retrievers import BM25Retriever  # noqa: E402
except Exception as exc:  # pragma: no cover - optional dependency
    raise SystemExit("langchain-community is required for this example.") from exc

try:
    from llama_index.core.indices.keyword_table.simple_base import (  # noqa: E402
        SimpleKeywordTableIndex,
    )
    from llama_index.core.schema import Document as LIDocument  # noqa: E402
    from llama_index.core.settings import Settings  # noqa: E402
    from llama_index.core.llms.mock import MockLLM  # noqa: E402
except Exception as exc:  # pragma: no cover - optional dependency
    raise SystemExit("llama-index is required for this example.") from exc


def normalize_text(text: str) -> str:
    return " ".join(text.lower().split())


def contains_answer(texts: Iterable[str], answer: str) -> bool:
    answer_norm = normalize_text(answer)
    for t in texts:
        if answer_norm and answer_norm in normalize_text(t):
            return True
    return False


def chunk_text(text: str, chunk_chars: int, overlap: int) -> List[str]:
    if chunk_chars <= 0:
        return [text]
    if overlap >= chunk_chars:
        overlap = max(0, chunk_chars // 4)
    chunks = []
    i = 0
    while i < len(text):
        chunks.append(text[i : i + chunk_chars])
        if i + chunk_chars >= len(text):
            break
        i = i + chunk_chars - overlap
    return chunks


def build_docs(task_id: int, context: str, chunk_chars: int, overlap: int):
    docs_rustrlm = []
    docs_langchain = []
    docs_llama = []
    for idx, chunk in enumerate(chunk_text(context, chunk_chars, overlap)):
        doc_id = f"{task_id}:{idx}"
        meta = {"task_id": task_id}
        docs_rustrlm.append({"id": doc_id, "text": chunk, "metadata": meta})
        docs_langchain.append(LCDocument(page_content=chunk, metadata=meta))
        docs_llama.append(LIDocument(text=chunk, metadata=meta, id_=doc_id))
    return docs_rustrlm, docs_langchain, docs_llama


def retrieve_rustrlm(client: RustRLMClient, query: str, docs, top_k: int) -> List[str]:
    res = client.retrieve(query, docs, {"top_k": top_k, "include_spans": False})
    return [r.get("text", "") for r in res.get("results", [])]


def retrieve_langchain(retriever: BM25Retriever, query: str) -> List[str]:
    if hasattr(retriever, "get_relevant_documents"):
        docs = retriever.get_relevant_documents(query)
    else:
        docs = retriever.invoke(query)
    return [d.page_content for d in docs]


def retrieve_llamaindex(retriever, query: str) -> List[str]:
    nodes = retriever.retrieve(query)
    out: List[str] = []
    for n in nodes:
        node = getattr(n, "node", n)
        if hasattr(node, "get_content"):
            out.append(node.get_content())
        else:
            out.append(getattr(node, "text", ""))
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--top-k", type=int, default=3)
    parser.add_argument("--show", type=int, default=3)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--chunk-chars", type=int, default=800)
    parser.add_argument("--overlap", type=int, default=120)
    parser.add_argument("--truncate", type=int, default=0, help="truncate context to N chars (0 = full)")
    parser.add_argument("--base-url", default=os.environ.get("RUSTRLM_BASE_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--input", default=os.path.join("extracted", "eval", "s_niah.jsonl"))
    args = parser.parse_args()

    client = RustRLMClient(base_url=args.base_url)
    Settings.llm = MockLLM()

    total = 0
    hits_rustrlm = 0
    hits_langchain = 0
    hits_llama = 0
    rows = []

    with open(args.input, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            task = json.loads(line)
            total += 1
            if args.limit and total > args.limit:
                break
            context = task.get("context", "")
            if args.truncate and len(context) > args.truncate:
                context = context[: args.truncate]
            query = task.get("query", "")
            answer = str(task.get("answer", ""))

            docs_r, docs_lc, docs_li = build_docs(task.get("id", total), context, args.chunk_chars, args.overlap)

            rustrlm_texts = retrieve_rustrlm(client, query, docs_r, args.top_k)
            lc_retriever = BM25Retriever.from_documents(docs_lc)
            lc_texts = retrieve_langchain(lc_retriever, query)
            li_index = SimpleKeywordTableIndex.from_documents(docs_li, show_progress=False)
            li_retriever = li_index.as_retriever(retriever_mode="simple")
            li_texts = retrieve_llamaindex(li_retriever, query)

            hit_r = contains_answer(rustrlm_texts, answer)
            hit_lc = contains_answer(lc_texts[: args.top_k], answer)
            hit_li = contains_answer(li_texts[: args.top_k], answer)

            hits_rustrlm += 1 if hit_r else 0
            hits_langchain += 1 if hit_lc else 0
            hits_llama += 1 if hit_li else 0

            rows.append(
                {
                    "query": query,
                    "answer": answer,
                    "hit": {"rustrlm": hit_r, "langchain": hit_lc, "llamaindex": hit_li},
                    "rustrlm": rustrlm_texts[: args.top_k],
                    "langchain": lc_texts[: args.top_k],
                    "llamaindex": li_texts[: args.top_k],
                }
            )

    print("# S-NIAH comparison (RLM paper task)")
    print(f"Total queries: {total}")
    if args.truncate:
        print(f"Context truncate: {args.truncate} chars")
    print(
        "Hit@{k}: RustRLM={r:.2%}, LangChain-BM25={lc:.2%}, LlamaIndex-Simple={li:.2%}".format(
            k=args.top_k,
            r=(hits_rustrlm / total) if total else 0.0,
            lc=(hits_langchain / total) if total else 0.0,
            li=(hits_llama / total) if total else 0.0,
        )
    )

    print("\n# Sample queries")
    for i, row in enumerate(rows[: args.show], start=1):
        print(f"\n[{i}] Q: {row['query']}")
        print(f"Answer: {row['answer']}")
        print("RustRLM hit:", row["hit"]["rustrlm"])
        print("LangChain-BM25 hit:", row["hit"]["langchain"])
        print("LlamaIndex-Simple hit:", row["hit"]["llamaindex"])
        print("- RustRLM top1:", row["rustrlm"][0] if row["rustrlm"] else "")
        print("- LangChain top1:", row["langchain"][0] if row["langchain"] else "")
        print("- LlamaIndex top1:", row["llamaindex"][0] if row["llamaindex"] else "")


if __name__ == "__main__":
    main()
